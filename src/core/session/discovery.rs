use std::path::{Path, PathBuf};

const PROJECT_MARKERS: &[&str] = &[
    ".git",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
];

pub(super) fn discover_workspace_root(seed: &Path) -> PathBuf {
    discover_workspace_root_with_home(seed, home_dir().as_deref())
}

pub(super) fn discover_workspace_root_with_home(seed: &Path, home: Option<&Path>) -> PathBuf {
    let fallback = if seed.is_dir() {
        seed.to_path_buf()
    } else {
        seed.parent().unwrap_or(seed).to_path_buf()
    };
    let mut current = fallback.clone();

    loop {
        if home.is_some_and(|home| current == home && current != fallback) {
            return fallback;
        }
        if PROJECT_MARKERS
            .iter()
            .any(|marker| current.join(marker).exists())
        {
            return current;
        }
        if !current.pop() {
            return fallback;
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_promote_child_directory_to_home_marker() {
        let home = std::env::temp_dir().join(format!(
            "tty7-home-marker-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let workspace = home.join("github_project");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(home.join("package.json"), "{}\n").unwrap();

        let root = discover_workspace_root_with_home(&workspace, Some(&home));

        assert_eq!(root, workspace);
        std::fs::remove_dir_all(home).ok();
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
