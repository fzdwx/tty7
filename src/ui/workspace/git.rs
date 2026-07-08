use std::path::{Path, PathBuf};

pub(super) fn branch_label(root: &Path) -> Option<String> {
    let git_dir = find_git_dir(root)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(branch) = head.strip_prefix("ref: refs/heads/") {
        return (!branch.is_empty()).then(|| branch.to_string());
    }
    if let Some(reference) = head.strip_prefix("ref: ") {
        return reference
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_string);
    }
    let short_hash: String = head.chars().take(7).collect();
    (short_hash.len() == 7).then_some(short_hash)
}

fn find_git_dir(seed: &Path) -> Option<PathBuf> {
    let mut current = if seed.is_dir() {
        seed.to_path_buf()
    } else {
        seed.parent()?.to_path_buf()
    };

    loop {
        let marker = current.join(".git");
        if marker.is_dir() {
            return Some(marker);
        }
        if marker.is_file()
            && let Some(path) = git_dir_from_file(&marker, &current)
        {
            return Some(path);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn git_dir_from_file(marker: &Path, repo_root: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(marker).ok()?;
    let path = text.trim().strip_prefix("gitdir:")?.trim();
    if path.is_empty() {
        return None;
    }
    let git_dir = PathBuf::from(path);
    Some(if git_dir.is_absolute() {
        git_dir
    } else {
        repo_root.join(git_dir)
    })
}
