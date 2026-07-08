use std::io::Read as _;
use std::path::{Path, PathBuf};

pub const MAX_PREVIEW_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePreviewDocument {
    pub path: PathBuf,
    pub body: FilePreviewBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePreviewBody {
    Text { text: String, truncated: bool },
    Binary,
    Error(String),
}

pub fn load(path: impl AsRef<Path>) -> FilePreviewDocument {
    let requested_path = path.as_ref();
    let body = load_body(requested_path);
    let path = preview_document_path(requested_path, &body);
    FilePreviewDocument { path, body }
}

fn preview_document_path(requested_path: &Path, body: &FilePreviewBody) -> PathBuf {
    match body {
        FilePreviewBody::Error(_) => requested_path.to_path_buf(),
        FilePreviewBody::Text { .. } | FilePreviewBody::Binary => {
            std::fs::canonicalize(requested_path).unwrap_or_else(|_| requested_path.to_path_buf())
        }
    }
}

fn load_body(path: &Path) -> FilePreviewBody {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(err) => return FilePreviewBody::Error(err.to_string()),
    };

    let mut bytes = Vec::with_capacity(MAX_PREVIEW_BYTES.saturating_add(1));
    let limit = MAX_PREVIEW_BYTES as u64 + 1;
    if let Err(err) = file.take(limit).read_to_end(&mut bytes) {
        return FilePreviewBody::Error(err.to_string());
    }

    let truncated = bytes.len() > MAX_PREVIEW_BYTES;
    if truncated {
        bytes.truncate(MAX_PREVIEW_BYTES);
    }
    decode_bytes(bytes, truncated)
}

fn decode_bytes(bytes: Vec<u8>, truncated: bool) -> FilePreviewBody {
    if bytes.contains(&0) {
        return FilePreviewBody::Binary;
    }

    match String::from_utf8(bytes) {
        Ok(text) => FilePreviewBody::Text { text, truncated },
        Err(_) => FilePreviewBody::Binary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_reads_utf8_text_files() {
        let path = temp_file("text", "hello\nworld\n".as_bytes());

        let preview = load(&path);

        assert_eq!(
            preview.body,
            FilePreviewBody::Text {
                text: "hello\nworld\n".into(),
                truncated: false,
            }
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn load_marks_nul_containing_files_as_binary() {
        let path = temp_file("binary", b"abc\0def");

        let preview = load(&path);

        assert_eq!(preview.body, FilePreviewBody::Binary);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn load_truncates_large_text_files() {
        let bytes = vec![b'a'; MAX_PREVIEW_BYTES + 10];
        let path = temp_file("large", &bytes);

        let preview = load(&path);

        assert_eq!(
            preview.body,
            FilePreviewBody::Text {
                text: "a".repeat(MAX_PREVIEW_BYTES),
                truncated: true,
            }
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    #[cfg(unix)]
    fn load_canonicalizes_successful_file_paths() {
        let path = temp_file("canonical", b"hello");
        let symlink_path = path.with_extension("link");
        std::os::unix::fs::symlink(&path, &symlink_path).unwrap();

        let preview = load(&symlink_path);

        assert_eq!(preview.path, path.canonicalize().unwrap());
        std::fs::remove_file(symlink_path).ok();
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn load_preserves_requested_path_when_file_is_missing() {
        let path = std::env::temp_dir().join(format!(
            "tty7-file-preview-missing-{}-{}",
            std::process::id(),
            unique_suffix()
        ));

        let preview = load(&path);

        assert_eq!(preview.path, path);
        assert!(matches!(preview.body, FilePreviewBody::Error(_)));
    }

    fn temp_file(label: &str, bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tty7-file-preview-{label}-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
