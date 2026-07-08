use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use crate::core::file_tree::{FileTree, FileTreeEntry};

#[derive(Default)]
pub(crate) struct FileTreeCache {
    root: Option<PathBuf>,
    children: HashMap<PathBuf, CachedChildren>,
}

type CachedChildren = Result<Rc<Vec<FileTreeEntry>>, String>;

impl FileTreeCache {
    pub(crate) fn reset_to_root(&mut self, root: &Path) {
        if self.root.as_deref() == Some(root) {
            return;
        }
        log::debug!(
            target: "tty7::file_tree",
            "reset file tree cache root={} previous_root={} cached_dirs={}",
            root.display(),
            self.root
                .as_deref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            self.children.len()
        );
        self.root = Some(root.to_path_buf());
        self.children.clear();
    }

    pub(crate) fn children(&mut self, tree: &FileTree, dir: &Path) -> CachedChildren {
        let key = dir.to_path_buf();
        if let Some(cached) = self.children.get(&key) {
            log::trace!(
                target: "tty7::file_tree",
                "file tree cache hit dir={} result={} entries={}",
                dir.display(),
                if cached.is_ok() { "ok" } else { "err" },
                cached.as_ref().map(|entries| entries.len()).unwrap_or(0)
            );
            return cached.clone();
        }

        let started = Instant::now();
        let loaded = tree
            .list_children(dir)
            .map(Rc::new)
            .map_err(|err| err.to_string());
        log::debug!(
            target: "tty7::file_tree",
            "file tree cache miss dir={} result={} entries={} elapsed_ms={:.2}",
            dir.display(),
            if loaded.is_ok() { "ok" } else { "err" },
            loaded.as_ref().map(|entries| entries.len()).unwrap_or(0),
            started.elapsed().as_secs_f64() * 1000.0
        );
        self.children.insert(key, loaded.clone());
        loaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::file_tree::FileTreeEntryKind;

    #[test]
    fn reset_to_root_keeps_cache_for_same_root() {
        let root = PathBuf::from("/workspace");
        let mut cache = FileTreeCache::default();

        cache.root = Some(root.clone());
        cache.children.insert(
            root.clone(),
            Ok(Rc::new(vec![FileTreeEntry {
                path: root.join("src"),
                name: "src".to_string(),
                kind: FileTreeEntryKind::Directory,
            }])),
        );

        cache.reset_to_root(&root);

        assert_eq!(cache.children.len(), 1);
    }

    #[test]
    fn reset_to_root_clears_cache_for_new_root() {
        let root = PathBuf::from("/workspace");
        let next_root = PathBuf::from("/other");
        let mut cache = FileTreeCache::default();

        cache.root = Some(root.clone());
        cache.children.insert(
            root.clone(),
            Ok(Rc::new(vec![FileTreeEntry {
                path: root.join("src"),
                name: "src".to_string(),
                kind: FileTreeEntryKind::Directory,
            }])),
        );

        cache.reset_to_root(&next_root);

        assert!(cache.children.is_empty());
        assert_eq!(cache.root, Some(next_root));
    }

    #[test]
    fn children_reuses_cached_directory_entries() {
        let root = temp_tree("cache-reuse");
        std::fs::write(root.join("one.txt"), "").unwrap();
        let tree = FileTree::new(&root).unwrap();
        let mut cache = FileTreeCache::default();

        let first = cache.children(&tree, &root).unwrap();
        std::fs::write(root.join("two.txt"), "").unwrap();
        let second = cache.children(&tree, &root).unwrap();

        assert_eq!(names(&first), vec!["one.txt"]);
        assert_eq!(names(&second), vec!["one.txt"]);

        std::fs::remove_dir_all(root).ok();
    }

    fn names(entries: &[FileTreeEntry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.name.as_str()).collect()
    }

    fn temp_tree(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tty7-file-tree-cache-{label}-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
