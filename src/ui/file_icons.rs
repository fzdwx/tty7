use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use serde::Deserialize;

const ICON_THEME_JSON: &str = include_str!("../../assets/file-icons/theme-complete.json");
const ICON_PREFIX: &str = "file-icons/";
const FALLBACK_FILE_ICON: &str = "file-icons/file-duo.svg";
const FILE_SYMLINK_ICON: &str = "file-icons/file-symlink-duo.svg";
const FOLDER_ICON: &str = "file-icons/folder-duo.svg";
const FOLDER_OPEN_ICON: &str = "file-icons/folder-open-duo.svg";

static ICON_THEME: LazyLock<IconTheme> = LazyLock::new(IconTheme::load);

pub(crate) fn file_icon_path(path: &Path) -> &'static str {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return FALLBACK_FILE_ICON;
    };

    ICON_THEME.icon_for_name(name).unwrap_or(FALLBACK_FILE_ICON)
}

pub(crate) const fn file_symlink_icon_path() -> &'static str {
    FILE_SYMLINK_ICON
}

pub(crate) const fn folder_icon_path(expanded: bool) -> &'static str {
    if expanded {
        FOLDER_OPEN_ICON
    } else {
        FOLDER_ICON
    }
}

#[derive(Default)]
struct IconTheme {
    file_extensions: HashMap<String, String>,
    file_names: HashMap<String, String>,
}

impl IconTheme {
    fn load() -> Self {
        let raw = match serde_json::from_str::<RawIconTheme>(ICON_THEME_JSON) {
            Ok(raw) => raw,
            Err(err) => {
                log::error!("failed to parse bundled file icon theme: {err}");
                return Self::default();
            }
        };

        let icon_definitions = raw.icon_definitions;
        Self {
            file_extensions: resolve_icon_map(&icon_definitions, raw.file_extensions),
            file_names: resolve_icon_map(&icon_definitions, raw.file_names),
        }
    }

    fn icon_for_name(&'static self, name: &str) -> Option<&'static str> {
        let name = name.to_ascii_lowercase();
        self.file_names
            .get(&name)
            .map(String::as_str)
            .or_else(|| self.icon_for_compound_extension(&name))
    }

    fn icon_for_compound_extension(&'static self, name: &str) -> Option<&'static str> {
        name.match_indices('.').find_map(|(index, _)| {
            name.get(index + 1..)
                .and_then(|extension| self.file_extensions.get(extension))
                .map(String::as_str)
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawIconTheme {
    file_extensions: HashMap<String, String>,
    file_names: HashMap<String, String>,
    icon_definitions: HashMap<String, RawIconDefinition>,
}

#[derive(Deserialize)]
struct RawIconDefinition {
    #[serde(rename = "iconPath")]
    icon_path: String,
}

fn resolve_icon_map(
    icon_definitions: &HashMap<String, RawIconDefinition>,
    icons: HashMap<String, String>,
) -> HashMap<String, String> {
    icons
        .into_iter()
        .filter_map(|(key, icon)| {
            icon_definitions
                .get(&icon)
                .and_then(|definition| asset_path(&definition.icon_path))
                .map(|path| (key.to_ascii_lowercase(), path))
        })
        .collect()
}

fn asset_path(icon_path: &str) -> Option<String> {
    let path = icon_path.strip_prefix("./").unwrap_or(icon_path);
    (!path.is_empty()).then(|| format!("{ICON_PREFIX}{path}"))
}

#[cfg(test)]
mod tests {
    use super::{file_icon_path, file_symlink_icon_path, folder_icon_path};
    use std::path::Path;

    #[test]
    fn exact_file_names_take_priority_over_extension_mapping() {
        assert_eq!(
            file_icon_path(Path::new("package.json")),
            "file-icons/npm-color.svg"
        );
        assert_eq!(
            file_icon_path(Path::new(".gitignore")),
            "file-icons/git-color.svg"
        );
        assert_eq!(
            file_icon_path(Path::new("Dockerfile")),
            "file-icons/docker-color.svg"
        );
    }

    #[test]
    fn extension_mapping_is_case_insensitive() {
        assert_eq!(
            file_icon_path(Path::new("main.RS")),
            "file-icons/lang-rust-color.svg"
        );
        assert_eq!(
            file_icon_path(Path::new("component.TSX")),
            "file-icons/react-color.svg"
        );
        assert_eq!(
            file_icon_path(Path::new("diagram.SVG")),
            "file-icons/svg-2-color.svg"
        );
    }

    #[test]
    fn compound_extensions_use_the_longest_theme_match() {
        assert_eq!(
            file_icon_path(Path::new("site.env.local")),
            "file-icons/file-text-duo.svg"
        );
    }

    #[test]
    fn unknown_files_use_the_generic_file_icon() {
        assert_eq!(
            file_icon_path(Path::new("notes.unknown")),
            "file-icons/file-duo.svg"
        );
        assert_eq!(file_icon_path(Path::new("")), "file-icons/file-duo.svg");
    }

    #[test]
    fn tree_structural_icons_come_from_the_theme_assets() {
        assert_eq!(file_symlink_icon_path(), "file-icons/file-symlink-duo.svg");
        assert_eq!(folder_icon_path(false), "file-icons/folder-duo.svg");
        assert_eq!(folder_icon_path(true), "file-icons/folder-open-duo.svg");
    }
}
