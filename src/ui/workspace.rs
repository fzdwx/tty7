use std::path::{Path, PathBuf};

use gpui::{Context, Entity, Window};

use crate::core::session::{SessionFileTreeState, SessionPane, SessionTab, SessionWorkspace};
use crate::terminal::view::TerminalView;
use crate::ui::app::{Tab, Tty7App, alive_panes, new_terminal, session_to_tab};
use crate::ui::pane::Pane;

mod render;

pub(crate) const WORKSPACE_RAIL_WIDTH: f32 = 72.0;
const WORKSPACE_LABEL_MAX: usize = 8;

impl Tty7App {
    pub(crate) fn switch_workspace(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut workspaces = self.materialized_workspaces(cx);
        if index >= workspaces.len() || index == self.active_workspace {
            return;
        }

        let current = self.active_workspace.min(workspaces.len() - 1);
        let snapshot = self.snapshot_workspace(current, workspaces.get(current), cx);
        workspaces[current] = snapshot;
        let workspace = workspaces[index].clone();
        self.restore_workspace(index, workspace, workspaces, window, cx);
    }

    pub(crate) fn new_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut workspaces = self.materialized_workspaces(cx);
        let current = self.active_workspace.min(workspaces.len() - 1);
        let snapshot = self.snapshot_workspace(current, workspaces.get(current), cx);
        workspaces[current] = snapshot;

        let cwd = self.seed_cwd(window, cx);
        let id = workspace_id(workspaces.len());
        let seed_tab = SessionTab::terminal(
            None,
            SessionPane::Leaf {
                cwd: Some(cwd.clone()),
                pane_id: None,
            },
        );
        let workspace = SessionWorkspace::from_tabs(id, 0, vec![seed_tab]);
        let terminal = new_terminal(self.font_size, Some(cwd), None, window, cx);
        let index = workspaces.len();
        workspaces.push(workspace.clone());

        self.tabs = vec![Tab::new(Pane::leaf(terminal))];
        self.active = 0;
        self.active_workspace = index;
        self.workspace_snapshots = workspaces;
        self.file_tree_root = workspace.root;
        self.file_tree_state = workspace.file_tree;
        self.maximized = None;
        self.renaming = None;
        self.focus_active(window, cx);
        self.save_session(cx);
        cx.notify();
    }

    pub(crate) fn on_terminal_cwd_changed(
        &mut self,
        view: &Entity<TerminalView>,
        cwd: &Path,
        _window: &Window,
        cx: &mut Context<Self>,
    ) {
        let root_changed = self.workspace_identity_terminal_matches(view)
            && self.update_workspace_root_from_identity_cwd(cwd);
        self.save_session(cx);
        if root_changed {
            cx.notify();
        }
    }

    fn materialized_workspaces(&self, cx: &gpui::App) -> Vec<SessionWorkspace> {
        if self.workspace_snapshots.is_empty() {
            vec![self.snapshot_workspace(self.active_workspace, None, cx)]
        } else {
            self.workspace_snapshots.clone()
        }
    }

    fn snapshot_workspace(
        &self,
        index: usize,
        existing: Option<&SessionWorkspace>,
        cx: &gpui::App,
    ) -> SessionWorkspace {
        let (active_tab, tabs) = self.session_tabs(cx);
        let mut workspace = existing
            .cloned()
            .unwrap_or_else(|| empty_workspace(index, self.file_tree_root.clone()));
        workspace = workspace.with_tabs(active_tab, tabs);
        workspace.with_file_tree_snapshot(&self.file_tree_root, &self.file_tree_state)
    }

    fn restore_workspace(
        &mut self,
        index: usize,
        workspace: SessionWorkspace,
        workspaces: Vec<SessionWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let alive = alive_panes();
        let mut tabs = Vec::with_capacity(workspace.tabs.len());
        for st in &workspace.tabs {
            tabs.push(session_to_tab(st, &alive, self.font_size, window, cx));
        }

        self.tabs = tabs;
        self.active = if self.tabs.is_empty() {
            0
        } else {
            workspace.active_tab.min(self.tabs.len() - 1)
        };
        self.active_workspace = index;
        self.workspace_snapshots = workspaces;
        self.file_tree_root = workspace_root(&workspace);
        self.file_tree_state = workspace.file_tree;
        self.maximized = None;
        self.renaming = None;
        self.focus_active(window, cx);
        self.save_session(cx);
        cx.notify();
    }

    fn workspace_count(&self) -> usize {
        self.workspace_snapshots.len().max(1)
    }

    fn workspace_label(&self, index: usize) -> String {
        let label = if index == self.active_workspace {
            self.workspace_snapshots
                .get(index)
                .and_then(|workspace| workspace.name.as_deref())
                .map(str::to_string)
                .unwrap_or_else(|| path_label(&self.file_tree_root))
        } else {
            self.workspace_snapshots
                .get(index)
                .map(|workspace| {
                    workspace
                        .name
                        .clone()
                        .unwrap_or_else(|| path_label(&workspace.root))
                })
                .unwrap_or_else(|| format!("{}", index + 1))
        };
        clamp_label(&label)
    }

    fn seed_cwd(&self, window: &Window, cx: &gpui::App) -> PathBuf {
        self.tabs
            .get(self.active)
            .and_then(|tab| {
                if let Some(preview) = tab.preview.as_ref() {
                    return preview
                        .read(cx)
                        .path
                        .parent()
                        .map(std::path::Path::to_path_buf);
                }
                tab.pane
                    .focused_or_first(window, cx)
                    .and_then(|leaf| leaf.read(cx).cwd())
            })
            .unwrap_or_else(|| self.file_tree_root.clone())
    }

    fn workspace_identity_terminal_matches(&self, view: &Entity<TerminalView>) -> bool {
        self.tabs
            .first()
            .and_then(|tab| tab.pane.first_leaf())
            .is_some_and(|terminal| terminal.entity_id() == view.entity_id())
    }

    fn update_workspace_root_from_identity_cwd(&mut self, cwd: &Path) -> bool {
        if !cwd.is_absolute() || !cwd.is_dir() {
            return false;
        }
        let root = crate::core::session::discover_workspace_root(cwd);
        if root == self.file_tree_root {
            return false;
        }

        self.file_tree_root = root;
        self.file_tree_state.expanded_dirs.clear();
        self.file_tree_state.selected_path = None;
        true
    }
}

fn empty_workspace(index: usize, root: PathBuf) -> SessionWorkspace {
    SessionWorkspace {
        id: workspace_id(index),
        name: None,
        identity_cwd: root.clone(),
        root,
        active_tab: 0,
        tabs: Vec::new(),
        file_tree: SessionFileTreeState::default(),
    }
}

fn workspace_root(workspace: &SessionWorkspace) -> PathBuf {
    if workspace.root.as_os_str().is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    } else {
        workspace.root.clone()
    }
}

fn workspace_id(index: usize) -> String {
    format!("w{}", index + 1)
}

fn path_label(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

fn clamp_label(label: &str) -> String {
    if label.chars().count() > WORKSPACE_LABEL_MAX {
        format!(
            "{}...",
            label
                .chars()
                .take(WORKSPACE_LABEL_MAX.saturating_sub(3))
                .collect::<String>()
        )
    } else {
        label.to_string()
    }
}

#[cfg(test)]
mod tests;
