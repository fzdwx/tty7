//! Session persistence: remember the tab / split-pane layout and each
//! terminal's working directory across restarts, plus a stack of recently
//! closed tabs for "Reopen Closed Tab".
//!
//! The on-disk model mirrors the live `Pane` tree but stays purely
//! serializable (no GPUI entities, no `gpui::Axis` which isn't `Serialize`).
//! It lives at `~/.config/tty7/session.json`, alongside `config.json`.
//!
//! All IO and parsing is best-effort: a missing/corrupt file just means "no
//! session to restore", and write failures are logged rather than fatal — the
//! app must never crash or stall over session bookkeeping.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod discovery;
mod wire;

pub fn discover_workspace_root(seed: impl AsRef<Path>) -> PathBuf {
    discovery::discover_workspace_root(seed.as_ref())
}

/// Split orientation, mirroring `gpui::Axis` (which isn't `Serialize`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SessionAxis {
    Horizontal,
    Vertical,
}

/// A serializable mirror of one tab's `Pane` tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionPane {
    /// A single terminal, restored in `cwd` (or the default dir if `None`).
    Leaf {
        #[serde(default)]
        cwd: Option<PathBuf>,
        /// Daemon pane id this leaf was mirroring. On restore we re-`attach` to
        /// it when the daemon still has it alive (process + scrollback intact),
        /// else fall back to spawning a fresh shell in `cwd`. `None` for sessions
        /// written by an older build (they just spawn fresh).
        #[serde(default)]
        pane_id: Option<u64>,
    },
    /// A split of two subtrees along `axis`, with `a` taking `ratio` of space.
    Split {
        axis: SessionAxis,
        #[serde(default = "default_ratio")]
        ratio: f32,
        a: Box<SessionPane>,
        b: Box<SessionPane>,
    },
}

fn default_ratio() -> f32 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SessionTabKind {
    Terminal { pane: SessionPane },
    Preview { path: PathBuf },
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionTab {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub kind: SessionTabKind,
}

impl SessionTab {
    pub fn terminal(name: Option<String>, pane: SessionPane) -> Self {
        Self {
            name,
            kind: SessionTabKind::Terminal { pane },
        }
    }

    pub fn preview(name: Option<String>, path: PathBuf) -> Self {
        Self {
            name,
            kind: SessionTabKind::Preview { path },
        }
    }

    pub fn terminal_pane(&self) -> Option<&SessionPane> {
        match &self.kind {
            SessionTabKind::Terminal { pane } => Some(pane),
            SessionTabKind::Preview { .. } => None,
        }
    }
}

fn default_file_tree_visible() -> bool {
    true
}

fn default_file_tree_width() -> f32 {
    280.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionFileTreeState {
    pub visible: bool,
    pub width: f32,
    pub expanded_dirs: Vec<PathBuf>,
    pub selected_path: Option<PathBuf>,
}

impl Default for SessionFileTreeState {
    fn default() -> Self {
        Self {
            visible: default_file_tree_visible(),
            width: default_file_tree_width(),
            expanded_dirs: Vec::new(),
            selected_path: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionWorkspace {
    pub id: String,
    pub name: Option<String>,
    pub identity_cwd: PathBuf,
    pub root: PathBuf,
    pub active_tab: usize,
    pub tabs: Vec<SessionTab>,
    pub file_tree: SessionFileTreeState,
}

impl SessionWorkspace {
    pub fn from_tabs(id: String, active_tab: usize, tabs: Vec<SessionTab>) -> Self {
        let active_tab = clamp_active_tab(active_tab, &tabs);
        let identity_cwd = workspace_seed_cwd(active_tab, &tabs).unwrap_or_else(default_cwd);
        let root = discover_workspace_root(&identity_cwd);
        Self {
            id,
            name: None,
            identity_cwd,
            root,
            active_tab,
            tabs,
            file_tree: SessionFileTreeState::default(),
        }
    }

    pub fn with_tabs(mut self, active_tab: usize, tabs: Vec<SessionTab>) -> Self {
        self.active_tab = clamp_active_tab(active_tab, &tabs);
        if let Some(identity_cwd) = workspace_seed_cwd(self.active_tab, &tabs) {
            let root = discover_workspace_root(&identity_cwd);
            if self.root != root {
                self.root = root;
                self.file_tree.expanded_dirs.clear();
                self.file_tree.selected_path = None;
            }
            self.identity_cwd = identity_cwd;
        }
        self.tabs = tabs;
        self
    }

    pub fn with_file_tree_snapshot(
        mut self,
        file_tree_root: &Path,
        file_tree: &SessionFileTreeState,
    ) -> Self {
        self.file_tree.visible = file_tree.visible;
        self.file_tree.width = file_tree.width;
        if self.root == file_tree_root {
            self.file_tree.expanded_dirs = file_tree.expanded_dirs.clone();
            self.file_tree.selected_path = file_tree.selected_path.clone();
        } else {
            self.file_tree.expanded_dirs.clear();
            self.file_tree.selected_path = None;
        }
        self
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(default)]
pub struct Session {
    pub active_workspace: usize,
    pub workspaces: Vec<SessionWorkspace>,
}

impl Session {
    pub fn from_tabs(active_tab: usize, tabs: Vec<SessionTab>) -> Self {
        if tabs.is_empty() {
            return Self::default();
        }

        Self {
            active_workspace: 0,
            workspaces: vec![SessionWorkspace::from_tabs("w1".into(), active_tab, tabs)],
        }
    }

    pub fn from_workspaces(active_workspace: usize, workspaces: Vec<SessionWorkspace>) -> Self {
        let mut session = Self {
            active_workspace,
            workspaces,
        };
        session.normalize();
        session
    }

    /// Load the saved session. Returns `None` when the file is absent or
    /// unreadable, and `None` (with a warning) when it fails to parse — never
    /// panics.
    pub fn load() -> Option<Session> {
        let path = Self::path()?;
        // Absent/unreadable file is the normal first-run case: silently None.
        let text = std::fs::read_to_string(&path).ok()?;
        match serde_json::from_str::<Session>(&text) {
            Ok(session) => Some(session),
            Err(e) => {
                log::warn!(
                    "failed to parse session at {}: {e}; ignoring",
                    path.display()
                );
                None
            }
        }
    }

    /// Persist the session as JSON, creating the parent directory if needed.
    /// Any IO/serialization error is logged and swallowed.
    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            log::warn!("failed to create session dir {}: {e}", parent.display());
            return;
        }
        let json = match serde_json::to_string_pretty(self) {
            Ok(j) => j,
            Err(e) => {
                log::warn!("failed to serialize session: {e}");
                return;
            }
        };
        if let Err(e) = crate::core::config::write_atomic(&path, json.as_bytes()) {
            log::warn!("failed to write session to {}: {e}", path.display());
        }
    }

    /// `~/.config/tty7/session.json`, alongside `config.json`.
    fn path() -> Option<PathBuf> {
        crate::core::config::config_path("session.json")
    }
}

impl Session {
    fn normalize(&mut self) {
        if self.workspaces.is_empty() {
            self.active_workspace = 0;
            return;
        }

        self.active_workspace = self.active_workspace.min(self.workspaces.len() - 1);
        for (index, workspace) in self.workspaces.iter_mut().enumerate() {
            if workspace.id.trim().is_empty() {
                workspace.id = format!("w{}", index + 1);
            }
            if workspace.tabs.is_empty() {
                workspace.active_tab = 0;
            } else {
                workspace.active_tab = workspace.active_tab.min(workspace.tabs.len() - 1);
            }

            let identity_cwd = workspace_seed_cwd(workspace.active_tab, &workspace.tabs)
                .unwrap_or_else(|| {
                    if workspace.identity_cwd.as_os_str().is_empty() {
                        default_cwd()
                    } else {
                        workspace.identity_cwd.clone()
                    }
                });
            let root = discover_workspace_root(&identity_cwd);
            if workspace.root != root {
                workspace.root = root;
                workspace.file_tree.expanded_dirs.clear();
                workspace.file_tree.selected_path = None;
            }
            workspace.identity_cwd = identity_cwd;
        }
    }
}

fn default_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
}

fn workspace_seed_cwd(active_tab: usize, tabs: &[SessionTab]) -> Option<PathBuf> {
    tabs.iter()
        .find_map(|tab| terminal_tab_cwd(tab).cloned())
        .or_else(|| tabs.get(active_tab).and_then(tab_cwd))
        .or_else(|| tabs.iter().find_map(tab_cwd))
}

fn clamp_active_tab(active_tab: usize, tabs: &[SessionTab]) -> usize {
    if tabs.is_empty() {
        0
    } else {
        active_tab.min(tabs.len() - 1)
    }
}

fn first_leaf_cwd(pane: &SessionPane) -> Option<&PathBuf> {
    match pane {
        SessionPane::Leaf { cwd, .. } => cwd.as_ref(),
        SessionPane::Split { a, b, .. } => first_leaf_cwd(a).or_else(|| first_leaf_cwd(b)),
    }
}

fn terminal_tab_cwd(tab: &SessionTab) -> Option<&PathBuf> {
    tab.terminal_pane().and_then(first_leaf_cwd)
}

fn tab_cwd(tab: &SessionTab) -> Option<PathBuf> {
    match &tab.kind {
        SessionTabKind::Terminal { pane } => first_leaf_cwd(pane).cloned(),
        SessionTabKind::Preview { path } => path.parent().map(std::path::Path::to_path_buf),
    }
}

#[cfg(test)]
mod tests;
