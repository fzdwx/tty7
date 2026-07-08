use std::path::{Path, PathBuf};

use super::*;

#[test]
fn list_children_keeps_hidden_files_and_sorts_directories_first_naturally() {
    let root = temp_tree("sort");
    mkdir(&root, "src10");
    mkdir(&root, "src2");
    touch(&root, ".env");
    touch(&root, "file10.txt");
    touch(&root, "file2.txt");

    let tree = FileTree::new(&root).unwrap();
    let entries = tree.list_children(&root).unwrap();
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();

    assert_eq!(names, ["src2", "src10", ".env", "file2.txt", "file10.txt"]);
    assert!(entries[0].is_dir());
    assert!(!entries[2].is_dir());

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn list_children_is_lazy_and_only_reads_the_requested_directory() {
    let root = temp_tree("lazy");
    mkdir(&root, "big");
    touch(&root.join("big"), "nested.txt");
    touch(&root, "top.txt");

    let tree = FileTree::new(&root).unwrap();
    let root_entries = tree.list_children(&root).unwrap();

    assert_eq!(root_entries.len(), 2);
    assert!(
        root_entries
            .iter()
            .any(|entry| entry.name == "big" && entry.is_dir())
    );
    assert!(root_entries.iter().all(|entry| entry.name != "nested.txt"));

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn list_children_omits_default_ignored_directories() {
    let root = temp_tree("default-ignore");
    mkdir(&root, ".git");
    mkdir(&root, "node_modules");
    mkdir(&root, "target");
    mkdir(&root, "dist");
    mkdir(&root, "src");
    touch(&root, ".env");

    let tree = FileTree::new(&root).unwrap();
    let entries = tree.list_children(&root).unwrap();
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();

    assert_eq!(names, ["src", ".env"]);

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn list_children_rejects_paths_outside_the_workspace_root() {
    let root = temp_tree("root");
    let outside = temp_tree("outside");

    let tree = FileTree::new(&root).unwrap();
    let err = tree.list_children(&outside).unwrap_err();

    assert!(matches!(err, FileTreeError::OutsideRoot { .. }));

    std::fs::remove_dir_all(root).ok();
    std::fs::remove_dir_all(outside).ok();
}

#[test]
fn list_children_rejects_files_as_expand_targets() {
    let root = temp_tree("file-target");
    touch(&root, "main.rs");

    let tree = FileTree::new(&root).unwrap();
    let err = tree.list_children(root.join("main.rs")).unwrap_err();

    assert!(matches!(err, FileTreeError::NotDirectory { .. }));

    std::fs::remove_dir_all(root).ok();
}

fn temp_tree(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tty7-file-tree-{label}-{}-{}",
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

fn mkdir(root: &Path, name: &str) {
    std::fs::create_dir_all(root.join(name)).unwrap();
}

fn touch(root: &Path, name: &str) {
    std::fs::write(root.join(name), "").unwrap();
}
