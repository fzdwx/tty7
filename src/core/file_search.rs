use std::fmt;
use std::path::{Path, PathBuf};

use fff_search::{
    FFFMode, FilePicker, FilePickerOptions, FuzzySearchOptions, PaginationArgs, QueryParser,
};

const DEFAULT_LIMIT: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSearchResult {
    pub path: PathBuf,
    pub relative_path: String,
    pub file_name: String,
    pub score: i32,
}

#[derive(Debug)]
pub struct FileSearchIndex {
    root: PathBuf,
    picker: FilePicker,
}

impl FileSearchIndex {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, FileSearchError> {
        let root = root.as_ref();
        if !root.exists() {
            return Err(FileSearchError::RootDoesNotExist {
                path: root.to_path_buf(),
            });
        }
        if !root.is_dir() {
            return Err(FileSearchError::RootNotDirectory {
                path: root.to_path_buf(),
            });
        }

        let root = root.canonicalize().map_err(|source| FileSearchError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let mut picker = FilePicker::new(FilePickerOptions {
            base_path: root.to_string_lossy().into_owned(),
            mode: FFFMode::Ai,
            watch: false,
            ..Default::default()
        })
        .map_err(FileSearchError::Index)?;
        picker.collect_files().map_err(FileSearchError::Index)?;

        Ok(Self { root, picker })
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<FileSearchResult> {
        let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
        let parser = QueryParser::default();
        let query = parser.parse(query);
        let results = self.picker.fuzzy_search(
            &query,
            None,
            FuzzySearchOptions {
                project_path: Some(&self.root),
                pagination: PaginationArgs { offset: 0, limit },
                ..Default::default()
            },
        );

        results
            .items
            .into_iter()
            .zip(results.scores)
            .map(|(item, score)| FileSearchResult {
                path: item.absolute_path(&self.picker, &self.root),
                relative_path: item.relative_path(&self.picker),
                file_name: item.file_name(&self.picker),
                score: score.total,
            })
            .collect()
    }
}

#[derive(Debug)]
pub enum FileSearchError {
    RootDoesNotExist {
        path: PathBuf,
    },
    RootNotDirectory {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Index(fff_search::Error),
}

impl fmt::Display for FileSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootDoesNotExist { path } => write!(f, "{} does not exist", path.display()),
            Self::RootNotDirectory { path } => write!(f, "{} is not a directory", path.display()),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Index(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for FileSearchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Index(source) => Some(source),
            Self::RootDoesNotExist { .. } | Self::RootNotDirectory { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn fuzzy_search_finds_file_under_root() {
        let temp = TestDir::new("finds-file");
        temp.write("src/main.rs", "fn main() {}\n");
        temp.write("src/lib.rs", "pub fn lib() {}\n");
        temp.write("README.md", "# test\n");

        let search = FileSearchIndex::new(temp.path()).expect("index root");
        let results = search.search("main", 20);

        assert!(
            results
                .iter()
                .any(|result| result.relative_path == "src/main.rs"),
            "expected src/main.rs in {results:#?}"
        );
    }

    #[test]
    fn empty_query_returns_limited_results() {
        let temp = TestDir::new("empty-query");
        temp.write("a.rs", "");
        temp.write("b.rs", "");
        temp.write("c.rs", "");

        let search = FileSearchIndex::new(temp.path()).expect("index root");
        let results = search.search("", 2);

        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .all(|result| result.path.starts_with(temp.path()))
        );
    }

    #[test]
    fn missing_root_returns_error() {
        let temp = TestDir::new("missing-root");
        let missing = temp.path().join("missing");

        let err = FileSearchIndex::new(&missing).expect_err("missing root should fail");

        assert!(err.to_string().contains("does not exist"));
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("tty7-file-search-{name}-{suffix}"));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(path, contents).expect("write file");
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
