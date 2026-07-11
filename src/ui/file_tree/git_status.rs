use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GitTreeStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Untracked,
}

impl GitTreeStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Modified => "M",
            Self::Renamed => "R",
            Self::Untracked => "?",
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct FileTreeGitStatus {
    statuses: Arc<HashMap<PathBuf, GitTreeStatus>>,
}

impl FileTreeGitStatus {
    pub(super) fn load(root: &Path) -> Self {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(root)
            .args(["status", "--porcelain=v1", "-z"]);
        configure_background_command(&mut command);

        let Ok(output) = command.output() else {
            return Self::default();
        };
        if !output.status.success() {
            return Self::default();
        }
        Self {
            statuses: Arc::new(parse_git_status_z(root, &output.stdout)),
        }
    }

    pub(super) fn status_for(&self, path: &Path, is_dir: bool) -> Option<GitTreeStatus> {
        if let Some(status) = self.statuses.get(path) {
            return Some(*status);
        }
        is_dir.then(|| {
            self.statuses
                .iter()
                .find_map(|(changed, status)| changed.starts_with(path).then_some(*status))
        })?
    }
}

#[derive(Default)]
pub(crate) struct FileTreeGitStatusCache {
    root: Option<PathBuf>,
    loading: Option<(PathBuf, u64)>,
    generation: u64,
    status: FileTreeGitStatus,
}

impl FileTreeGitStatusCache {
    pub(super) fn begin_refresh(&mut self, root: &Path, force: bool) -> Option<u64> {
        let already_loaded = self.root.as_deref() == Some(root);
        let already_loading = self
            .loading
            .as_ref()
            .is_some_and(|(loading_root, _)| loading_root == root);
        if !force && (already_loaded || already_loading) {
            return None;
        }

        self.generation = self.generation.wrapping_add(1);
        if !already_loaded {
            self.root = None;
            self.status = FileTreeGitStatus::default();
        }
        self.loading = Some((root.to_path_buf(), self.generation));
        Some(self.generation)
    }

    pub(super) fn finish_refresh(
        &mut self,
        root: &Path,
        generation: u64,
        status: FileTreeGitStatus,
    ) -> bool {
        let is_current = self
            .loading
            .as_ref()
            .is_some_and(|(loading_root, loading_generation)| {
                loading_root == root && *loading_generation == generation
            });
        if !is_current {
            return false;
        }
        self.root = Some(root.to_path_buf());
        self.loading = None;
        self.status = status;
        true
    }

    pub(super) fn snapshot(&self) -> FileTreeGitStatus {
        self.status.clone()
    }
}

#[cfg(windows)]
fn configure_background_command(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_background_command(_command: &mut Command) {}

fn parse_git_status_z(root: &Path, output: &[u8]) -> HashMap<PathBuf, GitTreeStatus> {
    let mut statuses = HashMap::new();
    let mut fields = output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let x = field[0] as char;
        let y = field[1] as char;
        let path = String::from_utf8_lossy(&field[3..]).into_owned();
        let status = git_status_kind(x, y);
        statuses.insert(root.join(path), status);
        if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
            let _ = fields.next();
        }
    }
    statuses
}

fn git_status_kind(x: char, y: char) -> GitTreeStatus {
    if x == '?' || y == '?' {
        GitTreeStatus::Untracked
    } else if x == 'A' || y == 'A' {
        GitTreeStatus::Added
    } else if x == 'D' || y == 'D' {
        GitTreeStatus::Deleted
    } else if x == 'R' || y == 'R' {
        GitTreeStatus::Renamed
    } else {
        GitTreeStatus::Modified
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{FileTreeGitStatusCache, GitTreeStatus, parse_git_status_z};

    #[test]
    fn file_tree_git_status_cache_schedules_one_load_per_root() {
        let root = Path::new("/repo");
        let mut cache = FileTreeGitStatusCache::default();

        let generation = cache
            .begin_refresh(root, false)
            .expect("a new root needs loading");
        assert!(
            cache.begin_refresh(root, false).is_none(),
            "rendering again while loading must not start another git process"
        );

        assert!(cache.finish_refresh(root, generation, Default::default()));
        assert!(
            cache.begin_refresh(root, false).is_none(),
            "rendering a loaded root must reuse the cached result"
        );
        assert!(
            cache.begin_refresh(root, true).is_some(),
            "an explicit refresh must be able to reload the current root"
        );
    }

    #[test]
    fn file_tree_git_status_cache_rejects_stale_workspace_results() {
        let first_root = Path::new("/first");
        let second_root = Path::new("/second");
        let mut cache = FileTreeGitStatusCache::default();

        let first_generation = cache.begin_refresh(first_root, false).unwrap();
        let second_generation = cache.begin_refresh(second_root, false).unwrap();

        assert!(!cache.finish_refresh(first_root, first_generation, Default::default()));
        assert!(cache.finish_refresh(second_root, second_generation, Default::default()));
    }

    #[test]
    fn parse_git_status_z_maps_statuses_to_absolute_paths() {
        let root = PathBuf::from("/repo");
        let statuses = parse_git_status_z(
            &root,
            b" M src/main.rs\0A  src/lib.rs\0?? notes.md\0R  new.rs\0old.rs\0",
        );

        assert_eq!(
            statuses.get(&root.join("src/main.rs")),
            Some(&GitTreeStatus::Modified)
        );
        assert_eq!(
            statuses.get(&root.join("src/lib.rs")),
            Some(&GitTreeStatus::Added)
        );
        assert_eq!(
            statuses.get(&root.join("notes.md")),
            Some(&GitTreeStatus::Untracked)
        );
        assert_eq!(
            statuses.get(&root.join("new.rs")),
            Some(&GitTreeStatus::Renamed)
        );
        assert!(!statuses.contains_key(&root.join("old.rs")));
    }
}
