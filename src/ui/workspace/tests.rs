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

#[test]
fn git_branch_label_reads_branch_from_git_head() {
    let root = temp_workspace("git-branch");
    let git_dir = root.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature/workspace\n").unwrap();

    assert_eq!(
        git_branch_label(&root),
        Some("feature/workspace".to_string())
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn git_branch_label_follows_gitdir_file() {
    let root = temp_workspace("gitfile");
    let storage = root.with_extension("gitdir");
    std::fs::create_dir_all(&storage).unwrap();
    std::fs::write(
        root.join(".git"),
        format!("gitdir: {}\n", storage.display()),
    )
    .unwrap();
    std::fs::write(storage.join("HEAD"), "ref: refs/heads/main\n").unwrap();

    assert_eq!(git_branch_label(&root), Some("main".to_string()));
    std::fs::remove_dir_all(root).ok();
    std::fs::remove_dir_all(storage).ok();
}

#[test]
fn git_branch_label_is_none_outside_git_repository() {
    let root = temp_workspace("no-git");

    assert_eq!(git_branch_label(&root), None);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn workspace_index_after_delete_keeps_a_valid_neighbor_active() {
    assert_eq!(workspace_index_after_delete(0, 0, 1), None);
    assert_eq!(workspace_index_after_delete(1, 0, 3), Some(0));
    assert_eq!(workspace_index_after_delete(1, 1, 3), Some(1));
    assert_eq!(workspace_index_after_delete(2, 1, 3), Some(1));
    assert_eq!(workspace_index_after_delete(2, 2, 3), Some(1));
}

#[test]
fn apply_workspace_root_preserves_width_and_clears_path_state() {
    let old_root = temp_workspace("old-root");
    let new_root = temp_workspace("new-root");
    let mut workspace = SessionWorkspace {
        id: "w1".into(),
        name: Some("Work".into()),
        root_override: None,
        identity_cwd: old_root.clone(),
        root: old_root.clone(),
        active_tab: 0,
        tabs: Vec::new(),
        file_tree: SessionFileTreeState {
            visible: false,
            width: 360.0,
            expanded_dirs: vec![old_root.join("src")],
            selected_path: Some(old_root.join("src/main.rs")),
        },
    };

    apply_workspace_root(&mut workspace, new_root.clone());

    assert_eq!(workspace.identity_cwd, new_root);
    assert_eq!(workspace.root, workspace.identity_cwd);
    assert!(!workspace.file_tree.visible);
    assert_eq!(workspace.file_tree.width, 360.0);
    assert!(workspace.file_tree.expanded_dirs.is_empty());
    assert!(workspace.file_tree.selected_path.is_none());
    std::fs::remove_dir_all(old_root).ok();
    std::fs::remove_dir_all(workspace.root).ok();
}

#[test]
fn root_override_survives_identity_cwd_changes() {
    let manual_root = temp_workspace("manual-root");
    let inferred_root = temp_project("inferred-root");
    let inferred_cwd = inferred_root.join("src");
    std::fs::create_dir_all(&inferred_cwd).unwrap();
    let mut workspace = SessionWorkspace::from_tabs(
        "w1".into(),
        0,
        vec![SessionTab::terminal(
            None,
            SessionPane::Leaf {
                cwd: Some(manual_root.clone()),
                pane_id: None,
            },
        )],
    );
    apply_workspace_root(&mut workspace, manual_root.clone());

    let workspace = workspace.with_tabs(
        0,
        vec![SessionTab::terminal(
            None,
            SessionPane::Leaf {
                cwd: Some(inferred_cwd.clone()),
                pane_id: None,
            },
        )],
    );

    assert_eq!(workspace.identity_cwd, inferred_cwd);
    assert_eq!(workspace.root, manual_root);
    assert_eq!(workspace.root_override, Some(workspace.root.clone()));
    std::fs::remove_dir_all(workspace.root).ok();
    std::fs::remove_dir_all(inferred_root).ok();
}

#[test]
fn session_pane_ids_collects_nested_terminal_panes_only() {
    let tabs = vec![
        SessionTab::preview(Some("preview".into()), PathBuf::from("/work/README.md")),
        SessionTab::terminal(
            Some("term".into()),
            SessionPane::Split {
                axis: crate::core::session::SessionAxis::Horizontal,
                ratio: 0.5,
                a: Box::new(SessionPane::Leaf {
                    cwd: None,
                    pane_id: Some(10),
                }),
                b: Box::new(SessionPane::Split {
                    axis: crate::core::session::SessionAxis::Vertical,
                    ratio: 0.5,
                    a: Box::new(SessionPane::Leaf {
                        cwd: None,
                        pane_id: None,
                    }),
                    b: Box::new(SessionPane::Leaf {
                        cwd: None,
                        pane_id: Some(12),
                    }),
                }),
            },
        ),
    ];

    assert_eq!(session_pane_ids(&tabs), vec![10, 12]);
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

fn temp_workspace(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tty7-workspace-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
