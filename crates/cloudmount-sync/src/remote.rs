//! 远端变更发现：廉价探测 + 全量树拉取。
//!
//! 115 官方 API **没有**变更事件/增量接口，因此策略是：
//! 1. 先调 [`cloudmount_core::Provider::change_probe`]（一次请求拿"条目数 + 最新修改时间"）；
//! 2. 与上次记录的水位一致 ⇒ **短路**，本轮不拉取子树（省调用、避免频控）；
//! 3. 不一致或后端不支持探测 ⇒ 全量拉取并更新水位。
//!
//! 全量拉取注意：目录数 × 每目录一次 `list`，对 115 的 1 r/s 限速是硬约束，
//! 因此探测短路在真实使用中非常关键。

use cloudmount_core::{ChangeProbe, Provider, RemoteEntry, RemoteId, Result};
use std::collections::HashMap;

/// 单次全量扫描允许遍历的最大目录数（防误配根目录导致天量请求）。
pub const MAX_DIRS: usize = 20_000;

/// 远端目录树快照：相对路径 → 条目。
#[derive(Debug, Clone, Default)]
pub struct RemoteTree {
    /// 相对根目录的路径（`/` 分隔）。
    pub entries: HashMap<String, RemoteEntry>,
    /// 本次遍历的目录数（诊断用）。
    pub dirs_visited: usize,
}

impl RemoteTree {
    /// 取某路径的条目。
    pub fn get(&self, rel_path: &str) -> Option<&RemoteEntry> {
        self.entries.get(rel_path)
    }

    /// 某路径下是否有远端目录。
    pub fn dir_id(&self, rel_path: &str) -> Option<RemoteId> {
        self.entries
            .get(rel_path)
            .filter(|e| e.kind == cloudmount_core::EntryKind::Directory)
            .map(|e| e.id.clone())
    }
}

/// 廉价探测远端是否变化。
///
/// 返回 `Ok(None)` 表示后端不支持探测（调用方应做全量扫描）。
pub fn probe(provider: &dyn Provider, root: &RemoteId) -> Result<Option<ChangeProbe>> {
    provider.change_probe(root)
}

/// 把探测结果编码为水位字符串（存 `meta` 表）。
pub fn encode_probe(p: ChangeProbe) -> String {
    format!("{}|{}", p.count, p.max_mtime)
}

/// 解析水位字符串。
pub fn decode_probe(s: &str) -> Option<ChangeProbe> {
    let (count, mtime) = s.split_once('|')?;
    Some(ChangeProbe {
        count: count.trim().parse().ok()?,
        max_mtime: mtime.trim().parse().ok()?,
    })
}

/// 全量拉取根目录下的整棵子树（广度优先）。
///
/// 不跟随任何跨目录引用；遇到超过 [`MAX_DIRS`] 个目录即报错，
/// 避免把用户配置错的根（如整个账号根）变成天量请求。
pub fn fetch_tree(provider: &dyn Provider, root: &RemoteId) -> Result<RemoteTree> {
    let mut tree = RemoteTree::default();
    // (远端 ID, 相对路径前缀)
    let mut queue: Vec<(RemoteId, String)> = vec![(root.clone(), String::new())];
    let mut dirs_visited = 0usize;

    while let Some((dir_id, prefix)) = queue.pop() {
        dirs_visited += 1;
        if dirs_visited > MAX_DIRS {
            return Err(cloudmount_core::Error::Other(format!(
                "远端目录数超过上限 {MAX_DIRS}，疑似根目录配置错误"
            )));
        }
        for child in provider.list(&dir_id)? {
            let rel = if prefix.is_empty() {
                child.name.clone()
            } else {
                format!("{prefix}/{}", child.name)
            };
            let is_dir = child.kind == cloudmount_core::EntryKind::Directory;
            if is_dir {
                queue.push((child.id.clone(), rel.clone()));
            }
            tree.entries.insert(rel, child);
        }
    }

    tree.dirs_visited = dirs_visited;
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudmount_core::{
        EntryKind, Error, Quota, RangeSpec, RemoteEntry, UploadResult, UploadSource,
    };
    use std::sync::Mutex;

    /// 极简内存 Provider：只实现 list/stat，其余 `unimplemented`。
    struct TreeProvider {
        children: Mutex<HashMap<String, Vec<RemoteEntry>>>,
        probe: Option<ChangeProbe>,
    }

    impl TreeProvider {
        fn entry(id: &str, name: &str, is_dir: bool) -> RemoteEntry {
            RemoteEntry {
                id: RemoteId(id.into()),
                parent_id: RemoteId("0".into()),
                name: name.into(),
                kind: if is_dir {
                    EntryKind::Directory
                } else {
                    EntryKind::File
                },
                size: 0,
                mtime: 100,
                sha1: None,
                pick_code: None,
                deleted: false,
            }
        }
    }

    impl Provider for TreeProvider {
        fn name(&self) -> &'static str {
            "test-tree"
        }
        fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
            Ok(self
                .children
                .lock()
                .unwrap()
                .get(&dir.0)
                .cloned()
                .unwrap_or_default())
        }
        fn stat(&self, _id: &RemoteId) -> Result<RemoteEntry> {
            Err(Error::Unimplemented("stat".into()))
        }
        fn open_range(
            &self,
            _id: &RemoteId,
            _r: RangeSpec,
        ) -> Result<Box<dyn std::io::Read + Send>> {
            Err(Error::Unimplemented("open_range".into()))
        }
        fn create_dir(&self, _p: &RemoteId, _n: &str) -> Result<RemoteEntry> {
            Err(Error::Unimplemented("create_dir".into()))
        }
        fn upload(
            &self,
            _p: &RemoteId,
            _n: &str,
            _s: UploadSource<'_>,
            _z: u64,
        ) -> Result<UploadResult> {
            Err(Error::Unimplemented("upload".into()))
        }
        fn rename(&self, _i: &RemoteId, _n: &str) -> Result<()> {
            Err(Error::Unimplemented("rename".into()))
        }
        fn move_entry(&self, _i: &RemoteId, _p: &RemoteId) -> Result<()> {
            Err(Error::Unimplemented("move".into()))
        }
        fn copy(&self, _i: &RemoteId, _p: &RemoteId) -> Result<RemoteId> {
            Err(Error::Unimplemented("copy".into()))
        }
        fn delete(&self, _i: &RemoteId) -> Result<()> {
            Err(Error::Unimplemented("delete".into()))
        }
        fn quota(&self) -> Result<Quota> {
            Err(Error::Unimplemented("quota".into()))
        }
        fn change_probe(&self, _dir: &RemoteId) -> Result<Option<ChangeProbe>> {
            Ok(self.probe)
        }
    }

    #[test]
    fn fetch_tree_walks_nested_dirs() {
        let mut children = HashMap::new();
        children.insert(
            "0".to_string(),
            vec![
                TreeProvider::entry("d1", "docs", true),
                TreeProvider::entry("f1", "readme.md", false),
            ],
        );
        children.insert(
            "d1".to_string(),
            vec![TreeProvider::entry("f2", "a.txt", false)],
        );
        let p = TreeProvider {
            children: Mutex::new(children),
            probe: None,
        };

        let tree = fetch_tree(&p, &RemoteId("0".into())).unwrap();
        assert_eq!(tree.entries.len(), 3);
        assert!(tree.get("docs").unwrap().kind == EntryKind::Directory);
        assert_eq!(tree.get("docs/a.txt").unwrap().id.0, "f2");
        assert_eq!(tree.get("readme.md").unwrap().id.0, "f1");
        assert_eq!(tree.dir_id("docs").unwrap().0, "d1");
        assert!(tree.dir_id("readme.md").is_none(), "文件不应被当作目录");
        assert_eq!(tree.dirs_visited, 2);
    }

    #[test]
    fn probe_encoding_round_trip() {
        let p = ChangeProbe {
            count: 17,
            max_mtime: 1_700_000_000,
        };
        let s = encode_probe(p);
        assert_eq!(s, "17|1700000000");
        assert_eq!(decode_probe(&s), Some(p));
        assert_eq!(decode_probe("bad"), None);
        assert_eq!(decode_probe("1|"), None);
    }

    #[test]
    fn unsupported_probe_returns_none() {
        let p = TreeProvider {
            children: Mutex::new(HashMap::new()),
            probe: None,
        };
        assert!(probe(&p, &RemoteId("0".into())).unwrap().is_none());
    }
}
