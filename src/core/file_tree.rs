use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileTree {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: FileTreeEntryKind,
}

impl FileTreeEntry {
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, FileTreeEntryKind::Directory)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeEntryKind {
    Directory,
    File,
    Symlink,
}

#[derive(Debug)]
pub enum FileTreeError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    NotDirectory {
        path: PathBuf,
    },
    OutsideRoot {
        root: PathBuf,
        path: PathBuf,
    },
}

impl FileTree {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, FileTreeError> {
        let root = canonicalize(root.as_ref())?;
        if !root.is_dir() {
            return Err(FileTreeError::NotDirectory { path: root });
        }
        Ok(Self { root })
    }

    pub fn list_children(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<Vec<FileTreeEntry>, FileTreeError> {
        let dir = canonicalize(dir.as_ref())?;
        if !dir.starts_with(&self.root) {
            return Err(FileTreeError::OutsideRoot {
                root: self.root.clone(),
                path: dir,
            });
        }
        if !dir.is_dir() {
            return Err(FileTreeError::NotDirectory { path: dir });
        }

        let mut entries = Vec::new();
        let read_dir = std::fs::read_dir(&dir).map_err(|source| FileTreeError::Io {
            path: dir.clone(),
            source,
        })?;
        for entry in read_dir {
            let entry = entry.map_err(|source| FileTreeError::Io {
                path: dir.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| FileTreeError::Io {
                path: path.clone(),
                source,
            })?;
            if file_type.is_dir() && is_default_ignored_dir(&name) {
                continue;
            }
            let kind = if file_type.is_dir() {
                FileTreeEntryKind::Directory
            } else if file_type.is_symlink() {
                FileTreeEntryKind::Symlink
            } else {
                FileTreeEntryKind::File
            };
            entries.push(FileTreeEntry { name, path, kind });
        }
        entries.sort_by(compare_entries);
        Ok(entries)
    }
}

impl fmt::Display for FileTreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NotDirectory { path } => write!(f, "{} is not a directory", path.display()),
            Self::OutsideRoot { root, path } => {
                write!(f, "{} is outside {}", path.display(), root.display())
            }
        }
    }
}

impl std::error::Error for FileTreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::NotDirectory { .. } | Self::OutsideRoot { .. } => None,
        }
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, FileTreeError> {
    path.canonicalize().map_err(|source| FileTreeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn compare_entries(a: &FileTreeEntry, b: &FileTreeEntry) -> Ordering {
    match (a.is_dir(), b.is_dir()) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (true, true) | (false, false) => natural_cmp(&a.name, &b.name),
    }
}

fn is_default_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | ".next"
            | ".nuxt"
            | ".turbo"
            | ".cache"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "coverage"
            | "vendor"
    )
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let (mut ai, mut bi) = (0, 0);
    while ai < ab.len() && bi < bb.len() {
        if ab[ai].is_ascii_digit() && bb[bi].is_ascii_digit() {
            let ordering = cmp_number(ab, &mut ai, bb, &mut bi);
            if ordering != Ordering::Equal {
                return ordering;
            }
            continue;
        }

        let ac = ab[ai].to_ascii_lowercase();
        let bc = bb[bi].to_ascii_lowercase();
        if ac != bc {
            return ac.cmp(&bc);
        }
        ai += 1;
        bi += 1;
    }
    ab.len().cmp(&bb.len())
}

fn cmp_number(a: &[u8], ai: &mut usize, b: &[u8], bi: &mut usize) -> Ordering {
    let a_start = *ai;
    let b_start = *bi;
    while *ai < a.len() && a[*ai].is_ascii_digit() {
        *ai += 1;
    }
    while *bi < b.len() && b[*bi].is_ascii_digit() {
        *bi += 1;
    }

    let a_digits = trim_leading_zeroes(&a[a_start..*ai]);
    let b_digits = trim_leading_zeroes(&b[b_start..*bi]);
    a_digits
        .len()
        .cmp(&b_digits.len())
        .then_with(|| a_digits.cmp(b_digits))
        .then_with(|| (*ai - a_start).cmp(&(*bi - b_start)))
}

fn trim_leading_zeroes(digits: &[u8]) -> &[u8] {
    let trimmed = digits
        .iter()
        .position(|digit| *digit != b'0')
        .unwrap_or(digits.len());
    &digits[trimmed..]
}

#[cfg(test)]
mod tests;
