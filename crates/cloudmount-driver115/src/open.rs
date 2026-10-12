//! 115 官方 OpenAPI 驱动（主通道）。
//!
//! 端点清单（来自官方文档 <https://www.yuque.com/115yun/open>）：
//!
//! | 功能 | 方法 | 路径 |
//! |---|---|---|
//! | 文件列表 | GET | `/open/ufile/files` |
//! | 搜索 | GET | `/open/ufile/search` |
//! | 详情 | GET | `/open/folder/get_info` |
//! | 新建目录 | POST | `/open/folder/add` |
//! | 重命名 | POST | `/open/ufile/update` |
//! | 移动 | POST | `/open/ufile/move` |
//! | 复制 | POST | `/open/ufile/copy` |
//! | 删除 | POST | `/open/ufile/delete` |
//! | 下载直链 | POST | `/open/ufile/downurl` |
//! | 上传初始化 | POST | `/open/upload/init` |
//! | 上传凭证 | GET | `/open/upload/get_token` |
//! | 断点续传 | POST | `/open/upload/resume` |
//! | 配额/用户 | GET | `/open/user/info` |
//!
//! 上传流程：`init`（fileid=SHA1、preid=前128K SHA1）→ 秒传命中(status=2)即完成；
//! 未命中(status=1) → `get_token` 拿阿里云 OSS STS → 直传 OSS（带 callback）。
//! 下载流程：`downurl`(pick_code) → 直链支持 HTTP Range 并发分块。

use crate::ratelimit::RateLimiter;
use cloudmount_auth::open::OpenApiAuth;
use cloudmount_core::{
    EntryKind, Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result, UploadResult,
};
use std::io::Read;

/// 官方 OpenAPI 驱动。
#[allow(dead_code)] // 骨架：字段将在 M0 实现请求逻辑时启用
pub struct Open115Provider {
    /// 认证器（持 token，负责刷新）。
    auth: OpenApiAuth,
    /// 全局限速器（保守 1 r/s）。
    limiter: RateLimiter,
    /// HTTP 客户端（同步 ureq + rustls，依赖最浅）。
    agent: ureq::Agent,
}

impl Open115Provider {
    /// 用 AppID 创建驱动。
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            auth: OpenApiAuth::new(app_id),
            limiter: RateLimiter::conservative(),
            agent: ureq::Agent::config_builder()
                .user_agent(cloudmount_core::DEFAULT_USER_AGENT)
                .build()
                .into(),
        }
    }

    /// 统一的受控请求入口：先限速，失败按退避重试。
    ///
    /// TODO(M0)：实现通用 `call<T: DeserializeOwned>(method, path, params)`，
    /// 处理 Bearer 头、业务码（state:false → Error::Remote）、401 → 刷新重试、
    /// 429/701 等频控码 → RateLimited。
    fn call(&self, _path: &str) -> Result<serde_json::Value> {
        self.limiter.try_acquire()?;
        Err(Error::Unimplemented("Open115Provider::call".into()))
    }
}

impl Provider for Open115Provider {
    fn name(&self) -> &'static str {
        "115-open"
    }

    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        let _ = self.call("/open/ufile/files")?;
        // TODO(M0): 分页拉取（limit/offset），映射 cid/pid/fc/name/s/sha1/ptime/usertime。
        Err(Error::Unimplemented(format!("list({dir})")))
    }

    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
        let _ = self.call("/open/folder/get_info")?;
        Err(Error::Unimplemented(format!("stat({id})")))
    }

    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        // TODO(M0)：downurl 取直链（缓存，带过期）→ ureq GET with Range 头。
        let _ = (self.call("/open/ufile/downurl")?, range.to_header());
        Err(Error::Unimplemented(format!("open_range({id})")))
    }

    fn create_dir(&self, _parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
        let _ = (self.call("/open/folder/add")?, name);
        Err(Error::Unimplemented("create_dir".into()))
    }

    fn upload(
        &self,
        _parent: &RemoteId,
        name: &str,
        source: cloudmount_core::provider::UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        // TODO(M0/M1)：
        //   1. 计算 SHA1 与前 128K SHA1（流式，不整读内存）
        //   2. init → status=2 秒传命中，直接返回
        //   3. status=1 → get_token 拿 OSS STS → 分片直传（并发 3–4）
        //   4. 701/700 → sign_check 二次校验（读指定区间再算 SHA1）
        match source {
            cloudmount_core::provider::UploadSource::Bytes(_) => {}
            cloudmount_core::provider::UploadSource::Path(_) => {}
        }
        Err(Error::Unimplemented(format!("upload({name}, {size}B)")))
    }

    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
        let _ = (self.call("/open/ufile/update")?, id, new_name);
        Err(Error::Unimplemented("rename".into()))
    }

    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
        let _ = (self.call("/open/ufile/move")?, id, new_parent);
        Err(Error::Unimplemented("move_entry".into()))
    }

    fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId> {
        let _ = (self.call("/open/ufile/copy")?, id, new_parent);
        Err(Error::Unimplemented("copy".into()))
    }

    fn delete(&self, id: &RemoteId) -> Result<()> {
        let _ = self.call("/open/ufile/delete")?;
        Err(Error::Unimplemented(format!("delete({id})")))
    }

    fn quota(&self) -> Result<Quota> {
        let _ = self.call("/open/user/info")?;
        Err(Error::Unimplemented("quota".into()))
    }
}

/// 目录/文件类型映射辅助（fc 字段：1=文件 0=目录）。
#[allow(dead_code)]
pub(crate) fn kind_from_fc(fc: i64) -> EntryKind {
    if fc == 0 {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}
