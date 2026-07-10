use std::path::PathBuf;

use super::test_support::{lock_session_file, pin_config_dir};
use super::*;

#[test]
fn session_json_round_trips_nested_tree() {
    let session = Session::from_tabs(
        1,
        vec![
            SessionTab::terminal(
                Some("build".into()),
                SessionPane::Leaf {
                    cwd: Some(PathBuf::from("/work")),
                    pane_id: Some(7),
                },
            ),
            SessionTab::terminal(
                None,
                SessionPane::Split {
                    axis: SessionAxis::Vertical,
                    ratio: 0.3,
                    a: Box::new(SessionPane::Leaf {
                        cwd: None,
                        pane_id: None,
                    }),
                    b: Box::new(SessionPane::Leaf {
                        cwd: Some(PathBuf::from("/tmp")),
                        pane_id: Some(9),
                    }),
                },
            ),
        ],
    );
    let json = serde_json::to_string(&session).unwrap();
    let back: Session = serde_json::from_str(&json).unwrap();
    let workspace = &back.workspaces[back.active_workspace];
    assert_eq!(workspace.active_tab, 1);
    assert_eq!(workspace.tabs.len(), 2);
    assert!(matches!(
        workspace.tabs[0].terminal_pane(),
        Some(SessionPane::Leaf {
            pane_id: Some(7),
            ..
        })
    ));
    match workspace.tabs[1].terminal_pane() {
        Some(SessionPane::Split { ratio, .. }) => assert!((ratio - 0.3).abs() < 1e-6),
        _ => panic!("expected a split"),
    }
}

#[test]
fn session_json_round_trips_preview_tabs() {
    let session = Session::from_tabs(
        1,
        vec![
            SessionTab::terminal(
                None,
                SessionPane::Leaf {
                    cwd: Some(PathBuf::from("/work")),
                    pane_id: Some(7),
                },
            ),
            SessionTab::preview(Some("main.rs".into()), PathBuf::from("/work/src/main.rs")),
        ],
    );

    let json = serde_json::to_string(&session).unwrap();
    let back: Session = serde_json::from_str(&json).unwrap();
    let workspace = &back.workspaces[0];

    assert_eq!(workspace.active_tab, 1);
    match &workspace.tabs[1].kind {
        SessionTabKind::Preview { path } => {
            assert_eq!(path, &PathBuf::from("/work/src/main.rs"));
        }
        SessionTabKind::Terminal { .. } => panic!("expected preview tab"),
    }
}

#[test]
fn preview_tab_serializes_as_tab_kind_without_pane_tree() {
    let tab = SessionTab::preview(None, PathBuf::from("/work/src/lib.rs"));

    let json = serde_json::to_value(tab).unwrap();

    assert_eq!(
        json.get("kind").and_then(serde_json::Value::as_str),
        Some("Preview")
    );
    assert_eq!(
        json.get("path").and_then(serde_json::Value::as_str),
        Some("/work/src/lib.rs")
    );
    assert!(json.get("pane").is_none());
}

#[test]
fn workspace_seed_cwd_prefers_terminal_tabs_over_preview_tabs() {
    let session = Session::from_tabs(
        0,
        vec![
            SessionTab::preview(
                Some("preview first".into()),
                PathBuf::from("/work/src/main.rs"),
            ),
            SessionTab::terminal(
                None,
                SessionPane::Leaf {
                    cwd: Some(PathBuf::from("/work")),
                    pane_id: None,
                },
            ),
        ],
    );

    assert_eq!(session.workspaces[0].identity_cwd, PathBuf::from("/work"));
}

#[test]
fn session_defaults_fill_missing_fields() {
    let s: Session = serde_json::from_str("{}").unwrap();
    assert_eq!(s.active_workspace, 0);
    assert!(s.workspaces.is_empty());

    let pane: SessionPane =
        serde_json::from_str(r#"{"Split":{"axis":"Horizontal","a":{"Leaf":{}},"b":{"Leaf":{}}}}"#)
            .unwrap();
    match pane {
        SessionPane::Split { ratio, .. } => assert_eq!(ratio, 0.5),
        _ => panic!("expected split"),
    }
}

#[test]
fn save_then_load_recovers_the_session() {
    let _file = lock_session_file();
    pin_config_dir();
    let session = Session::from_tabs(
        0,
        vec![SessionTab::terminal(
            Some("main".into()),
            SessionPane::Leaf {
                cwd: Some(PathBuf::from("/home/u")),
                pane_id: Some(1),
            },
        )],
    );
    session.save();
    let loaded = Session::load().expect("a saved session should load back");
    let workspace = &loaded.workspaces[loaded.active_workspace];
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(workspace.tabs[0].name.as_deref(), Some("main"));
}

#[test]
fn legacy_session_json_migrates_to_single_workspace() {
    let root = std::env::temp_dir().join(format!("tty7-session-migration-{}", std::process::id()));
    let nested = root.join("src").join("ui");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

    let legacy = serde_json::json!({
        "active": 2,
        "tabs": [{
            "name": "main",
            "pane": {
                "Leaf": {
                    "cwd": nested.to_string_lossy(),
                    "pane_id": 42
                }
            }
        }]
    });

    let session: Session = serde_json::from_value(legacy).unwrap();

    assert_eq!(session.active_workspace, 0);
    assert_eq!(session.workspaces.len(), 1);
    let workspace = &session.workspaces[0];
    assert_eq!(workspace.active_tab, 0);
    assert_eq!(workspace.identity_cwd, nested);
    assert_eq!(workspace.root, root);
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(workspace.tabs[0].name.as_deref(), Some("main"));

    std::fs::remove_dir_all(workspace.root.clone()).ok();
}

#[test]
fn session_serializes_workspace_schema_without_legacy_top_level_tabs() {
    let session = Session::from_tabs(
        0,
        vec![SessionTab::terminal(
            None,
            SessionPane::Leaf {
                cwd: Some(PathBuf::from("/tmp/project")),
                pane_id: Some(9),
            },
        )],
    );

    let json = serde_json::to_value(&session).unwrap();

    assert!(json.get("workspaces").is_some());
    assert!(json.get("active_workspace").is_some());
    assert!(json.get("tabs").is_none());
    assert!(json.get("active").is_none());
}

#[test]
fn session_from_workspaces_preserves_inactive_workspace_tabs() {
    let workspace_a = SessionWorkspace::from_tabs("w1".into(), 0, vec![tab("alpha", "/a")]);
    let workspace_b = SessionWorkspace::from_tabs("w2".into(), 0, vec![tab("beta", "/b")]);

    let session = Session::from_workspaces(1, vec![workspace_a, workspace_b]);

    assert_eq!(session.active_workspace, 1);
    assert_eq!(session.workspaces.len(), 2);
    assert_eq!(session.workspaces[0].tabs[0].name.as_deref(), Some("alpha"));
    assert_eq!(session.workspaces[1].tabs[0].name.as_deref(), Some("beta"));
}

#[test]
fn session_normalize_recomputes_stale_workspace_root_from_identity_cwd() {
    let stale_root = std::env::temp_dir().join(format!(
        "tty7-session-stale-root-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let identity_cwd = stale_root.join("github_project");
    std::fs::create_dir_all(&identity_cwd).unwrap();

    let session_json = serde_json::json!({
        "active_workspace": 0,
        "workspaces": [{
            "id": "w1",
            "name": null,
            "identity_cwd": identity_cwd.display().to_string(),
            "root": stale_root.display().to_string(),
            "active_tab": 0,
            "tabs": [{
                "name": null,
                "kind": "Terminal",
                "pane": {
                    "Leaf": {
                        "cwd": identity_cwd.display().to_string(),
                        "pane_id": 24
                    }
                }
            }],
            "file_tree": {
                "visible": true,
                "width": 300.0,
                "expanded_dirs": [stale_root.join(".config").display().to_string()],
                "selected_path": stale_root.join(".shell.pre-oh-my-zsh").display().to_string()
            }
        }]
    });

    let session: Session = serde_json::from_value(session_json).unwrap();
    let workspace = &session.workspaces[0];

    assert_eq!(workspace.identity_cwd, identity_cwd);
    assert_eq!(workspace.root, identity_cwd);
    assert!(workspace.file_tree.visible);
    assert_eq!(workspace.file_tree.width, 300.0);
    assert!(workspace.file_tree.expanded_dirs.is_empty());
    assert!(workspace.file_tree.selected_path.is_none());

    std::fs::remove_dir_all(stale_root).ok();
}

#[test]
fn workspace_with_tabs_preserves_root_and_file_tree_state() {
    let root = std::env::temp_dir().join(format!("tty7-workspace-update-{}", std::process::id()));
    let old_nested = root.join("old");
    let new_nested = root.join("crates").join("app");
    std::fs::create_dir_all(&old_nested).unwrap();
    std::fs::create_dir_all(&new_nested).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

    let mut workspace = SessionWorkspace::from_tabs(
        "w1".into(),
        0,
        vec![SessionTab::terminal(
            Some("old".into()),
            SessionPane::Leaf {
                cwd: Some(old_nested),
                pane_id: None,
            },
        )],
    );
    workspace.file_tree.expanded_dirs = vec![root.join("src")];
    workspace.file_tree.selected_path = Some(root.join("src/main.rs"));

    let workspace = workspace.with_tabs(
        9,
        vec![SessionTab::terminal(
            Some("new".into()),
            SessionPane::Leaf {
                cwd: Some(new_nested.clone()),
                pane_id: None,
            },
        )],
    );

    assert_eq!(workspace.active_tab, 0);
    assert_eq!(workspace.identity_cwd, new_nested);
    assert_eq!(workspace.root, root);
    assert_eq!(workspace.file_tree.expanded_dirs.len(), 1);
    assert!(workspace.file_tree.selected_path.is_some());

    std::fs::remove_dir_all(workspace.root.clone()).ok();
}

fn tab(name: &str, cwd: &str) -> SessionTab {
    SessionTab::terminal(
        Some(name.into()),
        SessionPane::Leaf {
            cwd: Some(PathBuf::from(cwd)),
            pane_id: None,
        },
    )
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
