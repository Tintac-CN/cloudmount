//! 测试用内存远端（Mock Provider）。
//!
//! 实现完整的 [`cloudmount_core::Provider`]，并暴露测试侧的操作句柄
//! （造文件、改内容、删文件、读内容、查请求次数）。
//!
//! 设计要点：
//! - 每次变更都推进 `mtime` 计数器（保证变更一定可被 (size, mtime) 检测到，
//!   规避真实文件系统"同秒同大小"的粒度问题）；
//! - 统计 `change_probe` / `list` 等调用次数，用于断言"短路确实省掉了请求"。

use cloudmount_core::{
    ChangeProbe, EntryKind, Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result,
    UploadResult, UploadSource,
};
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct Node {
    id: String,
    parent: String,
    name: String,
    is_dir: bool,
    data: Vec<u8>,
    sha1: Option<String>,
    mtime: i64,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    /// id → 节点
    nodes: HashMap<String, Node>,
}

/// 请求计数（用于验证优化生效）。
#[derive(Default)]
pub struct OpCounts {
    pub change_probe: AtomicUsize,
    pub list: AtomicUsize,
    pub upload: AtomicUsize,
    pub download: AtomicUsize,
    pub stat: AtomicUsize,
    pub create_dir: AtomicUsize,
    pub delete: AtomicUsize,
}

/// 内存远端。
#[derive(Clone)]
pub struct MockRemote {
    inner: Arc<Mutex<Inner>>,
    ops: Arc<OpCounts>,
    /// mtime 计数器（每次变更 +1，确保可检测）。
    clock: Arc<AtomicUsize>,
}

impl Default for MockRemote {
    fn default() -> Self {
        Self::new()
    }
}

impl MockRemote {
    /// 新建空远端（根目录 id = "0"）。
    pub fn new() -> Self {
        let mut inner = Inner {
            next_id: 100,
            nodes: HashMap::new(),
        };
        inner.nodes.insert(
            "0".to_string(),
            Node {
                id: "0".into(),
                parent: String::new(),
                name: String::new(),
                is_dir: true,
                data: Vec::new(),
                sha1: None,
                mtime: 1_000_000,
            },
        );
        Self {
            inner: Arc::new(Mutex::new(inner)),
            ops: Arc::new(OpCounts::default()),
            clock: Arc::new(AtomicUsize::new(1_000_000)),
        }
    }

    /// 请求计数（测试断言用）。
    pub fn ops(&self) -> &OpCounts {
        &self.ops
    }

    fn tick(&self) -> i64 {
        self.clock.fetch_add(1, Ordering::SeqCst) as i64
    }

    fn id_of_path(&self, inner: &Inner, path: &str) -> Option<String> {
        if path.is_empty() {
            return Some("0".to_string());
        }
        let mut current = "0".to_string();
        for comp in path.split('/').filter(|c| !c.is_empty()) {
            let child = inner
                .nodes
                .values()
                .find(|n| n.parent == current && n.name == comp)?;
            current = child.id.clone();
        }
        Some(current)
    }

    fn alloc_id(&self, inner: &mut Inner) -> String {
        inner.next_id += 1;
        format!("id{}", inner.next_id)
    }

    /// 创建（或覆盖）一个文件，返回其 id。
    pub fn put_file(&self, path: &str, data: &[u8]) -> String {
        let (parent_path, name) = split(path);
        let mtime = self.tick();
        let mut inner = self.inner.lock().unwrap();
        let parent = self
            .id_of_path(&inner, &parent_path)
            .unwrap_or_else(|| panic!("父目录不存在: {parent_path}"));
        // 同名覆盖
        if let Some(existing) = inner
            .nodes
            .values()
            .find(|n| n.parent == parent && n.name == name && !n.is_dir)
            .map(|n| n.id.clone())
        {
            let node = inner.nodes.get_mut(&existing).unwrap();
            node.data = data.to_vec();
            node.sha1 = Some(sha1_hex(data));
            node.mtime = mtime;
            return existing;
        }
        let id = self.alloc_id(&mut inner);
        inner.nodes.insert(
            id.clone(),
            Node {
                id: id.clone(),
                parent,
                name,
                is_dir: false,
                data: data.to_vec(),
                sha1: Some(sha1_hex(data)),
                mtime,
            },
        );
        id
    }

    /// 创建目录（含中间层级）。
    pub fn put_dir(&self, path: &str) -> String {
        let mut current = "0".to_string();
        let mut walked = String::new();
        let mtime = self.tick();
        let mut inner = self.inner.lock().unwrap();
        for comp in path.split('/').filter(|c| !c.is_empty()) {
            walked = if walked.is_empty() {
                comp.to_string()
            } else {
                format!("{walked}/{comp}")
            };
            let found = inner
                .nodes
                .values()
                .find(|n| n.parent == current && n.name == comp && n.is_dir)
                .map(|n| n.id.clone());
            current = match found {
                Some(id) => id,
                None => {
                    let id = self.alloc_id(&mut inner);
                    inner.nodes.insert(
                        id.clone(),
                        Node {
                            id: id.clone(),
                            parent: current.clone(),
                            name: comp.to_string(),
                            is_dir: true,
                            data: Vec::new(),
                            sha1: None,
                            mtime,
                        },
                    );
                    id
                }
            };
        }
        current
    }

    /// 远端删除（目录连带子项）。
    pub fn remove(&self, path: &str) {
        let mut inner = self.inner.lock().unwrap();
        let Some(id) = self.id_of_path(&inner, path) else {
            return;
        };
        let mut to_remove = vec![id.clone()];
        loop {
            let mut more: Vec<String> = Vec::new();
            for n in inner.nodes.values() {
                if to_remove.contains(&n.parent) && !to_remove.contains(&n.id) {
                    more.push(n.id.clone());
                }
            }
            if more.is_empty() {
                break;
            }
            to_remove.extend(more);
        }
        for id in to_remove {
            inner.nodes.remove(&id);
        }
    }

    /// 读取远端内容。
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        let id = self.id_of_path(&inner, path)?;
        inner.nodes.get(&id).map(|n| n.data.clone())
    }

    /// 远端是否存在该路径。
    pub fn exists(&self, path: &str) -> bool {
        let inner = self.inner.lock().unwrap();
        self.id_of_path(&inner, path).is_some()
    }

    /// 列出全部路径（排序），用于快照式断言。
    pub fn all_paths(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let mut out = Vec::new();
        for n in inner.nodes.values() {
            if n.id == "0" {
                continue;
            }
            let mut parts = vec![n.name.clone()];
            let mut cur = n.parent.clone();
            while cur != "0" && !cur.is_empty() {
                let Some(node) = inner.nodes.get(&cur) else {
                    break;
                };
                parts.push(node.name.clone());
                cur = node.parent.clone();
            }
            parts.reverse();
            out.push(parts.join("/"));
        }
        out.sort();
        out
    }
}

impl Provider for MockRemote {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        self.ops.list.fetch_add(1, Ordering::SeqCst);
        let inner = self.inner.lock().unwrap();
        let mut out: Vec<RemoteEntry> = inner
            .nodes
            .values()
            .filter(|n| n.parent == dir.0)
            .map(|n| node_to_entry(n, dir))
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
        self.ops.stat.fetch_add(1, Ordering::SeqCst);
        let inner = self.inner.lock().unwrap();
        let n = inner
            .nodes
            .get(&id.0)
            .ok_or_else(|| Error::NotFound(id.0.clone()))?;
        let parent = RemoteId(n.parent.clone());
        Ok(node_to_entry(n, &parent))
    }

    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        self.ops.download.fetch_add(1, Ordering::SeqCst);
        let inner = self.inner.lock().unwrap();
        let n = inner
            .nodes
            .get(&id.0)
            .ok_or_else(|| Error::NotFound(id.0.clone()))?;
        // 钳制到文件长度（调用方可能传 u64::MAX 表示"读到尾"）
        let end = range
            .end
            .map(|e| (e.min(n.data.len() as u64) as usize + 1).min(n.data.len()))
            .unwrap_or(n.data.len());
        let start = (range.start as usize).min(end);
        Ok(Box::new(std::io::Cursor::new(n.data[start..end].to_vec())))
    }

    fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
        self.ops.create_dir.fetch_add(1, Ordering::SeqCst);
        let mtime = self.tick();
        let mut inner = self.inner.lock().unwrap();
        let id = self.alloc_id(&mut inner);
        let node = Node {
            id: id.clone(),
            parent: parent.0.clone(),
            name: name.to_string(),
            is_dir: true,
            data: Vec::new(),
            sha1: None,
            mtime,
        };
        inner.nodes.insert(id, node.clone());
        Ok(node_to_entry(&node, parent))
    }

    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        _size: u64,
    ) -> Result<UploadResult> {
        self.ops.upload.fetch_add(1, Ordering::SeqCst);
        let data = match source {
            UploadSource::Bytes(b) => b.to_vec(),
            UploadSource::Path(p) => std::fs::read(p)?,
        };
        let mtime = self.tick();
        let mut inner = self.inner.lock().unwrap();
        // 同名覆盖（模拟服务端同名替换语义）
        let existing = inner
            .nodes
            .values()
            .find(|n| n.parent == parent.0 && n.name == name && !n.is_dir)
            .map(|n| n.id.clone());
        let id = match existing {
            Some(id) => {
                let node = inner.nodes.get_mut(&id).unwrap();
                node.data = data.clone();
                node.sha1 = Some(sha1_hex(&data));
                node.mtime = mtime;
                id
            }
            None => {
                let id = self.alloc_id(&mut inner);
                inner.nodes.insert(
                    id.clone(),
                    Node {
                        id: id.clone(),
                        parent: parent.0.clone(),
                        name: name.to_string(),
                        is_dir: false,
                        data: data.clone(),
                        sha1: Some(sha1_hex(&data)),
                        mtime,
                    },
                );
                id
            }
        };
        Ok(UploadResult {
            id: RemoteId(id),
            rapid_upload: false,
        })
    }

    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
        let mtime = self.tick();
        let mut inner = self.inner.lock().unwrap();
        let node = inner
            .nodes
            .get_mut(&id.0)
            .ok_or_else(|| Error::NotFound(id.0.clone()))?;
        node.name = new_name.to_string();
        node.mtime = mtime;
        Ok(())
    }

    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
        let mtime = self.tick();
        let mut inner = self.inner.lock().unwrap();
        let node = inner
            .nodes
            .get_mut(&id.0)
            .ok_or_else(|| Error::NotFound(id.0.clone()))?;
        node.parent = new_parent.0.clone();
        node.mtime = mtime;
        Ok(())
    }

    fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId> {
        let mtime = self.tick();
        let mut inner = self.inner.lock().unwrap();
        let src = inner
            .nodes
            .get(&id.0)
            .cloned()
            .ok_or_else(|| Error::NotFound(id.0.clone()))?;
        let new_id = self.alloc_id(&mut inner);
        inner.nodes.insert(
            new_id.clone(),
            Node {
                id: new_id.clone(),
                parent: new_parent.0.clone(),
                mtime,
                ..src
            },
        );
        Ok(RemoteId(new_id))
    }

    fn delete(&self, id: &RemoteId) -> Result<()> {
        self.ops.delete.fetch_add(1, Ordering::SeqCst);
        let mut inner = self.inner.lock().unwrap();
        inner.nodes.remove(&id.0);
        Ok(())
    }

    fn quota(&self) -> Result<Quota> {
        Ok(Quota {
            total: 1_000_000,
            used: 0,
            remaining: 1_000_000,
        })
    }

    fn change_probe(&self, dir: &RemoteId) -> Result<Option<ChangeProbe>> {
        self.ops.change_probe.fetch_add(1, Ordering::SeqCst);
        let inner = self.inner.lock().unwrap();
        let children: Vec<&Node> = inner.nodes.values().filter(|n| n.parent == dir.0).collect();
        Ok(Some(ChangeProbe {
            count: children.len() as u64,
            max_mtime: children.iter().map(|n| n.mtime).max().unwrap_or(0),
        }))
    }
}

fn node_to_entry(n: &Node, parent: &RemoteId) -> RemoteEntry {
    RemoteEntry {
        id: RemoteId(n.id.clone()),
        parent_id: parent.clone(),
        name: n.name.clone(),
        kind: if n.is_dir {
            EntryKind::Directory
        } else {
            EntryKind::File
        },
        size: n.data.len() as u64,
        mtime: n.mtime,
        sha1: n.sha1.clone(),
        pick_code: None,
        deleted: false,
    }
}

fn split(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((p, n)) => (p.to_string(), n.to_string()),
        None => (String::new(), path.to_string()),
    }
}

fn sha1_hex(data: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(data);
    hex::encode_upper(h.finalize())
}
