use std::path::PathBuf;

use super::*;

#[test]
fn identity_cwd_change_updates_workspace_file_tree_root() {
    let old_root = temp_project("stable");
    let new_root = temp_project("other");
    let new_cwd = new_root.join("crates").join("app");
    std::fs::create_dir_all(&new_cwd).unwrap();
    let file_tree = SessionFileTreeState {
        visible: true,
        width: 320.0,
        expanded_dirs: vec![old_root.join("src")],
        selected_path: Some(old_root.join("src/main.rs")),
    };
    let mut workspace = SessionWorkspace::from_tabs(
        "w1".into(),
        0,
        vec![SessionTab::terminal(
            None,
            SessionPane::Leaf {
                cwd: Some(old_root.clone()),
                pane_id: None,
            },
        )],
    );
    workspace.file_tree = file_tree.clone();

    let workspace = workspace.with_tabs(
        0,
        vec![SessionTab::terminal(
            None,
            SessionPane::Leaf {
                cwd: Some(new_cwd.clone()),
                pane_id: None,
            },
        )],
    );

    assert_eq!(workspace.identity_cwd, new_cwd);
    assert_eq!(workspace.root, new_root);
    assert!(workspace.file_tree.visible);
    assert_eq!(workspace.file_tree.width, 320.0);
    assert!(workspace.file_tree.expanded_dirs.is_empty());
    assert!(workspace.file_tree.selected_path.is_none());
    std::fs::remove_dir_all(old_root).ok();
    std::fs::remove_dir_all(new_root).ok();
}

#[test]
fn active_non_first_terminal_does_not_steal_workspace_root() {
    let root = temp_project("first");
    let other_root = temp_project("active");
    let other_cwd = other_root.join("nested");
    std::fs::create_dir_all(&other_cwd).unwrap();

    let workspace = SessionWorkspace::from_tabs(
        "w1".into(),
        1,
        vec![
            SessionTab::terminal(
                Some("first".into()),
                SessionPane::Leaf {
                    cwd: Some(root.clone()),
                    pane_id: None,
                },
            ),
            SessionTab::terminal(
                Some("active".into()),
                SessionPane::Leaf {
                    cwd: Some(other_cwd),
                    pane_id: None,
                },
            ),
        ],
    );

    assert_eq!(workspace.active_tab, 1);
    assert_eq!(workspace.identity_cwd, root);
    assert_eq!(workspace.root, root);
    std::fs::remove_dir_all(workspace.root.clone()).ok();
    std::fs::remove_dir_all(other_root).ok();
}

fn temp_project(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tty7-workspace-cwd-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    root
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
