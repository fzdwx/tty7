use std::path::{Path, PathBuf};

use gpui::{
    AppContext as _, Context, Entity, PathPromptOptions, PromptLevel, SharedString, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::core::session::{SessionFileTreeState, SessionPane, SessionTab, SessionWorkspace};
use crate::terminal::view::TerminalView;
use crate::ui::app::{Tab, Tty7App, WorkspaceRenaming, alive_panes, new_terminal, session_to_tab};
use crate::ui::pane::Pane;

mod git;
mod render;

use git::branch_label as git_branch_label;

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
        let terminal = new_terminal(self.font_size, Some(cwd), None, None, window, cx);
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
        self.file_tree_renaming = None;
        self.workspace_renaming = None;
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
        self.file_tree_renaming = None;
        self.workspace_renaming = None;
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

    fn workspace_git_branch(&self, index: usize) -> Option<String> {
        let root = if index == self.active_workspace {
            self.file_tree_root.as_path()
        } else {
            self.workspace_snapshots.get(index)?.root.as_path()
        };
        git_branch_label(root).map(|branch| clamp_label(&branch))
    }

    fn workspace_full_label(&self, index: usize) -> String {
        if index == self.active_workspace {
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
                .unwrap_or_else(|| format!("Workspace {}", index + 1))
        }
    }

    pub(crate) fn start_workspace_rename(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.workspace_count() {
            return;
        }
        let current = self.workspace_full_label(index);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(current));
        input.update(cx, |state, cx| state.focus(window, cx));
        let subs = vec![cx.subscribe_in(
            &input,
            window,
            |this, _input, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    this.commit_workspace_rename(window, cx)
                }
                _ => {}
            },
        )];
        self.renaming = None;
        self.file_tree_renaming = None;
        self.workspace_renaming = Some(WorkspaceRenaming {
            index,
            input,
            _subs: subs,
        });
        cx.notify();
    }

    fn commit_workspace_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.workspace_renaming.take() else {
            return;
        };
        let value = renaming.input.read(cx).value().trim().to_string();
        let mut workspaces = self.materialized_workspaces(cx);
        if workspaces.is_empty() || renaming.index >= workspaces.len() {
            self.focus_active(window, cx);
            cx.notify();
            return;
        }
        let current = self.active_workspace.min(workspaces.len() - 1);
        workspaces[current] = self.snapshot_workspace(current, workspaces.get(current), cx);
        workspaces[renaming.index].name = if value.is_empty() { None } else { Some(value) };
        self.workspace_snapshots = workspaces;
        self.save_session(cx);
        self.focus_active(window, cx);
        cx.notify();
    }

    pub(crate) fn delete_workspace(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.workspace_count();
        if index >= count {
            return;
        }
        if count <= 1 {
            let answer = window.prompt(
                PromptLevel::Info,
                "Keep One Workspace",
                Some("tty7 keeps one workspace available."),
                &["OK"],
                cx,
            );
            cx.spawn(async move |_this, _cx| {
                let _ = answer.await;
            })
            .detach();
            return;
        }

        let label = self.workspace_full_label(index);
        let answer = window.prompt(
            PromptLevel::Warning,
            "Delete Workspace?",
            Some(&format!(
                "Close workspace {label}? Its tabs in this workspace will be closed."
            )),
            &["Cancel", "Delete"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if !matches!(answer.await, Ok(1)) {
                return;
            }
            let _ = this.update_in(cx, move |this, window, cx| {
                this.delete_workspace_now(index, window, cx);
            });
        })
        .detach();
    }

    fn delete_workspace_now(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut workspaces = self.materialized_workspaces(cx);
        if workspaces.len() <= 1 || index >= workspaces.len() {
            return;
        }
        let current = self.active_workspace.min(workspaces.len() - 1);
        workspaces[current] = self.snapshot_workspace(current, workspaces.get(current), cx);
        let Some(next_index) = workspace_index_after_delete(current, index, workspaces.len())
        else {
            return;
        };
        let removed = workspaces.remove(index);
        self.workspace_renaming = None;
        self.renaming = None;
        self.file_tree_renaming = None;
        self.kill_workspace_panes(&removed, cx);

        if index == current {
            let workspace = workspaces[next_index].clone();
            self.restore_workspace(next_index, workspace, workspaces, window, cx);
        } else {
            self.active_workspace = next_index;
            self.workspace_snapshots = workspaces;
            self.save_session(cx);
            cx.notify();
        }
    }

    pub(crate) fn choose_workspace_root(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.workspace_count() {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from("Select Workspace Root")),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let root = path.canonicalize().unwrap_or(path);
            let _ = this.update_in(cx, move |this, window, cx| {
                this.set_workspace_root(index, root, window, cx);
            });
        })
        .detach();
    }

    fn set_workspace_root(
        &mut self,
        index: usize,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !root.is_dir() {
            return;
        }
        let mut workspaces = self.materialized_workspaces(cx);
        if index >= workspaces.len() {
            return;
        }
        let current = self.active_workspace.min(workspaces.len() - 1);
        workspaces[current] = self.snapshot_workspace(current, workspaces.get(current), cx);
        apply_workspace_root(&mut workspaces[index], root.clone());

        self.file_tree_cache.clear();
        self.file_search_index = None;
        self.pending_file_tree_reveal = None;
        self.close_file_search(window, cx);
        if index == current {
            self.file_tree_root = root;
            self.file_tree_state = workspaces[index].file_tree.clone();
            self.focus_active(window, cx);
        }
        self.workspace_snapshots = workspaces;
        self.save_session(cx);
        cx.notify();
    }

    fn kill_workspace_panes(&self, workspace: &SessionWorkspace, cx: &gpui::App) {
        if workspace.active_tab >= self.tabs.len() {
            kill_session_tabs(&workspace.tabs);
            return;
        }
        if workspace.id
            == self
                .workspace_snapshots
                .get(self.active_workspace)
                .map(|workspace| workspace.id.as_str())
                .unwrap_or_default()
        {
            for tab in &self.tabs {
                for leaf in tab.pane.terminal_leaves() {
                    crate::terminal::RemoteTerminal::kill_pane(leaf.read(cx).pane_id);
                }
            }
        } else {
            kill_session_tabs(&workspace.tabs);
        }
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
        if self
            .workspace_snapshots
            .get(self.active_workspace)
            .and_then(|workspace| workspace.root_override.as_ref())
            .is_some()
        {
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

fn workspace_index_after_delete(
    active_index: usize,
    deleted_index: usize,
    workspace_count: usize,
) -> Option<usize> {
    if workspace_count <= 1 || deleted_index >= workspace_count {
        return None;
    }
    if deleted_index < active_index {
        Some(active_index - 1)
    } else if deleted_index == active_index {
        Some(active_index.min(workspace_count - 2))
    } else {
        Some(active_index)
    }
}

fn apply_workspace_root(workspace: &mut SessionWorkspace, root: PathBuf) {
    workspace.root_override = Some(root.clone());
    workspace.identity_cwd = root.clone();
    workspace.root = root;
    workspace.file_tree.expanded_dirs.clear();
    workspace.file_tree.selected_path = None;
}

fn kill_session_tabs(tabs: &[SessionTab]) {
    for pane_id in session_pane_ids(tabs) {
        crate::terminal::RemoteTerminal::kill_pane(pane_id);
    }
}

fn session_pane_ids(tabs: &[SessionTab]) -> Vec<u64> {
    let mut pane_ids = Vec::new();
    for tab in tabs {
        if let Some(pane) = tab.terminal_pane() {
            collect_session_pane_ids(pane, &mut pane_ids);
        }
    }
    pane_ids
}

fn collect_session_pane_ids(pane: &SessionPane, pane_ids: &mut Vec<u64>) {
    match pane {
        SessionPane::Leaf {
            pane_id: Some(pane_id),
            ..
        } => pane_ids.push(*pane_id),
        SessionPane::Leaf { pane_id: None, .. } => {}
        SessionPane::Split { a, b, .. } => {
            collect_session_pane_ids(a, pane_ids);
            collect_session_pane_ids(b, pane_ids);
        }
    }
}

fn empty_workspace(index: usize, root: PathBuf) -> SessionWorkspace {
    SessionWorkspace {
        id: workspace_id(index),
        name: None,
        root_override: None,
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
