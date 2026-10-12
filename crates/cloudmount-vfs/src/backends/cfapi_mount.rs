//! CfAPI 后端的高层封装：`CfapiBackend` 实现 [`crate::MountBackend`]。
//!
//! 组合低层原语（[`super::cfapi`]）为完整能力：
//!
//! ```text
//! mount(point, remote_view)
//!   1. 注册同步根（幂等）
//!   2. 投影：远端树 → 占位符（create_placeholders_ordered）
//!   3. 连接：水合回调 = rel_path → 远端 ID → provider.open_range（Range 拉取）
//!      ↑ 这正是 115 的 downurl + Range 模式；测试用 MockRemote 验证同一链路
//! project(remote)  增量投影新增条目（内容更新的重投影见 TODO）
//! ```
//!
//! ## 与同步引擎的关系（M2 后续接线）
//!
//! 同步引擎每轮产出远端视图（快照合成或全量拉取）→ `project()` 增量投影；
//! 用户读文件 → 过滤器回调 → Provider 直接按需拉取（不经过同步引擎的下载
//! 队列 —— 按需读取天然绕过全量下载）。

use super::cfapi::{self, PlaceholderSpec};
use crate::{Mount, MountBackend};
use cloudmount_core::{EntryKind, Error, RangeSpec, RemoteEntry, RemoteId, Result};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// CfAPI 挂载后端。
pub struct CfapiBackend {
    provider: Arc<dyn Provider>,
}

/// 挂载句柄内部共享状态（回调与控制面共享）。
struct MountState {
    /// 相对路径 → 远端条目（投影与水合的寻址表）。
    remote: RwLock<HashMap<String, RemoteEntry>>,
}

/// CfAPI 挂载句柄。
pub struct CfapiMount {
    root: PathBuf,
    connection: cfapi::CloudConnection,
    state: Arc<MountState>,
    /// 已投影的相对路径（增量投影用）。
    projected: Mutex<HashSet<String>>,
}

impl CfapiBackend {
    /// 创建后端（`provider` 提供水合数据源）。
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self { provider }
    }

    /// 挂载并返回具体句柄（暴露 [`CfapiMount::project`] 增量投影）。
    pub fn mount_concrete(
        &self,
        point: &Path,
        remote: &HashMap<String, RemoteEntry>,
    ) -> Result<CfapiMount> {
        // 1. 注册（幂等）
        cfapi::register(point, "CloudMount", cloudmount_core::VERSION)?;

        // 2. 共享状态（水合回调按 rel_path 寻址）
        let state = Arc::new(MountState {
            remote: RwLock::new(remote.clone()),
        });

        // 3. 投影
        let specs: Vec<PlaceholderSpec> = remote
            .iter()
            .map(|(rel, e)| {
                let mut spec = if e.kind == EntryKind::Directory {
                    PlaceholderSpec::dir(rel, e.mtime)
                } else {
                    PlaceholderSpec::file(rel, e.size, e.mtime)
                };
                spec.identity = rel.as_bytes().to_vec();
                spec
            })
            .collect();
        cfapi::create_placeholders_ordered(point, &specs)?;

        // 4. 连接：水合 = rel_path → ID → Provider Range 拉取
        let provider = self.provider.clone();
        let state_for_cb = state.clone();
        let hydrate: cfapi::HydrateFn = Box::new(move |rel, offset, len| {
            let entry = state_for_cb
                .remote
                .read()
                .map_err(|_| Error::Other("远端视图锁中毒".into()))?
                .get(rel)
                .cloned()
                .ok_or_else(|| Error::NotFound(rel.to_string()))?;
            // 请求范围按远端大小钳制（过滤器可能按块请求超出 EOF 的范围）
            let start = offset.min(entry.size);
            let end = start
                .saturating_add(len.saturating_sub(1))
                .min(entry.size.saturating_sub(1));
            if start > end {
                return Ok(Vec::new());
            }
            let mut reader = provider.open_range(
                &entry.id,
                RangeSpec {
                    start,
                    end: Some(end),
                },
            )?;
            let mut buf = Vec::with_capacity((end - start + 1) as usize);
            reader.read_to_end(&mut buf)?;
            Ok(buf)
        });
        let connection = cfapi::connect(point, hydrate)?;

        let projected = remote.keys().cloned().collect();
        Ok(CfapiMount {
            root: point.to_path_buf(),
            connection,
            state,
            projected: Mutex::new(projected),
        })
    }
}

impl MountBackend for CfapiBackend {
    fn name(&self) -> &'static str {
        "cfapi"
    }

    fn supports_placeholders(&self) -> bool {
        true
    }

    fn supports_status_icons(&self) -> bool {
        true
    }

    /// 注册 + 投影 + 连接。
    ///
    /// 注意：同步根注册失败（如 CI 虚拟机不支持）返回 Err，调用方可据此降级。
    fn mount(&self, point: &Path, remote: &HashMap<String, RemoteEntry>) -> Result<Box<dyn Mount>> {
        Ok(Box::new(self.mount_concrete(point, remote)?))
    }

    /// 直接按 ID 从 Provider 水合指定范围。
    fn hydrate(&self, id: &RemoteId, range: RangeSpec) -> Result<Vec<u8>> {
        let mut reader = self.provider.open_range(id, range)?;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// 释放空间（转为纯占位符）—— M2.5 实现（需 CfDehydratePlaceholder 语义）。
    fn dehydrate(&self, _id: &RemoteId) -> Result<()> {
        Err(Error::Unimplemented("dehydrate（M2.5）".into()))
    }
}

impl CfapiMount {
    /// 同步根路径。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 增量投影：远端视图中新增的路径创建为占位符（已有路径跳过）。
    ///
    /// 同时刷新水合寻址表（新增文件的 ID 对水合回调可见）。
    /// **已知边界**：已有占位符的**内容更新**不会重投影——那需要
    /// `CfUpdatePlaceholders` + 失效通知，属 M2.5。
    pub fn project(&self, remote: &HashMap<String, RemoteEntry>) -> Result<u32> {
        {
            let mut view = self
                .state
                .remote
                .write()
                .map_err(|_| Error::Other("远端视图锁中毒".into()))?;
            for (rel, e) in remote {
                view.insert(rel.clone(), e.clone());
            }
        }

        let mut specs = Vec::new();
        {
            let mut projected = self.projected.lock().map_err(lock_poisoned)?;
            for (rel, e) in remote {
                if projected.contains(rel) {
                    continue;
                }
                let mut spec = if e.kind == EntryKind::Directory {
                    PlaceholderSpec::dir(rel, e.mtime)
                } else {
                    PlaceholderSpec::file(rel, e.size, e.mtime)
                };
                spec.identity = rel.as_bytes().to_vec();
                specs.push(spec);
                projected.insert(rel.clone());
            }
        }
        if specs.is_empty() {
            return Ok(0);
        }
        cfapi::create_placeholders_ordered(&self.root, &specs)
    }
}

impl Mount for CfapiMount {
    fn mount_point(&self) -> &Path {
        &self.root
    }

    /// 卸载：断开连接（停止回调）+ 注销同步根（占位符还原/移除）。
    fn unmount(self: Box<Self>) -> Result<()> {
        // connection 先 Drop（断开回调），再注销
        drop(self.connection);
        cfapi::unregister(&self.root)
    }
}

fn lock_poisoned<T>(_: T) -> Error {
    Error::Other("投影集合锁中毒".into())
}

// 引用 Provider trait（供 open_range 方法解析）
use cloudmount_core::Provider;
