use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui::{AnyElement, Context, MouseButton, MouseDownEvent, div, img, prelude::*, px};
use gpui_component::{ActiveTheme as _, Icon, IconName};

use crate::core::file_tree::{FileTree, FileTreeEntry, FileTreeEntryKind};
use crate::ui::app::Tty7App;
use crate::ui::file_icons::{file_icon_path, file_symlink_icon_path, folder_icon_path};

mod cache;
mod rows;

use rows as file_tree_rows;

pub(crate) use cache::FileTreeCache;

fn expand_dirs_to_reveal_path(root: &Path, path: &Path, expanded_dirs: &mut Vec<PathBuf>) {
    if !path.starts_with(root) {
        return;
    }
    let Some(mut dir) = path.parent() else {
        return;
    };
    let mut dirs = Vec::new();
    while dir != root {
        dirs.push(dir.to_path_buf());
        let Some(parent) = dir.parent() else {
            break;
        };
        dir = parent;
    }
    dirs.reverse();
    for dir in dirs {
        if !expanded_dirs.iter().any(|expanded| expanded == &dir) {
            expanded_dirs.push(dir);
        }
    }
}

fn path_is_within_dir(path: &Path, dir: &Path) -> bool {
    path == dir || path.starts_with(dir)
}

impl Tty7App {
    pub(crate) fn refresh_file_tree(&mut self, cx: &mut Context<Self>) {
        log::debug!(
            target: "tty7::file_tree",
            "refresh file tree root={}",
            self.file_tree_root.display()
        );
        self.file_tree_cache.clear();
        self.file_search_index = None;
        cx.notify();
    }

    pub(crate) fn reveal_file_tree_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.starts_with(&self.file_tree_root) {
            return;
        }
        self.file_tree_state.selected_path = Some(path.clone());
        expand_dirs_to_reveal_path(
            &self.file_tree_root,
            &path,
            &mut self.file_tree_state.expanded_dirs,
        );
        self.pending_file_tree_reveal = Some(path.clone());
        log::debug!(
            target: "tty7::file_tree",
            "reveal file tree path={} expanded_dirs={}",
            path.display(),
            self.file_tree_state.expanded_dirs.len()
        );
        self.save_session(cx);
        cx.notify();
    }

    pub(crate) fn render_file_tree(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let started = Instant::now();
        let root = self.file_tree_root.clone();
        self.file_tree_cache.reset_to_root(&root);
        let mut rows = Vec::new();
        let mut selected_row = None;
        match FileTree::new(&root) {
            Ok(tree) => {
                self.collect_file_tree_rows(&tree, &root, 0, &mut rows, &mut selected_row, cx)
            }
            Err(err) => rows.push(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(err.to_string())
                    .into_any_element(),
            ),
        }
        if self.pending_file_tree_reveal.is_some() {
            if let Some(row) = selected_row {
                self.file_tree_scroll_handle.scroll_to_item(row);
                log::debug!(
                    target: "tty7::file_tree",
                    "scroll file tree to selected row row={row}"
                );
            }
            self.pending_file_tree_reveal = None;
        }
        log::debug!(
            target: "tty7::file_tree",
            "render file tree root={} rows={} expanded_dirs={} elapsed_ms={:.2}",
            root.display(),
            rows.len(),
            self.file_tree_state.expanded_dirs.len(),
            started.elapsed().as_secs_f64() * 1000.0
        );

        div()
            .w(px(self.file_tree_state.width))
            .h_full()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                div()
                    .id("file-tree-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.file_tree_scroll_handle)
                    .py_1()
                    .children(rows),
            )
            .into_any_element()
    }

    fn collect_file_tree_rows(
        &mut self,
        tree: &FileTree,
        dir: &Path,
        depth: usize,
        rows: &mut Vec<AnyElement>,
        selected_row: &mut Option<usize>,
        cx: &mut Context<Self>,
    ) {
        match self.file_tree_cache.children(tree, dir) {
            Ok(entries) => {
                self.collect_file_tree_entries(
                    tree,
                    entries.as_ref(),
                    depth,
                    rows,
                    selected_row,
                    cx,
                );
            }
            Err(err) => rows.push(file_tree_rows::error_row(err, depth, cx)),
        }
    }

    fn collect_file_tree_entries(
        &mut self,
        tree: &FileTree,
        entries: &[FileTreeEntry],
        depth: usize,
        rows: &mut Vec<AnyElement>,
        selected_row: &mut Option<usize>,
        cx: &mut Context<Self>,
    ) {
        for entry in entries {
            let expanded = self.is_file_tree_expanded(&entry.path);
            if self
                .file_tree_state
                .selected_path
                .as_ref()
                .is_some_and(|selected| selected == &entry.path)
            {
                *selected_row = Some(rows.len());
            }
            rows.push(self.render_file_tree_entry(entry, depth, expanded, cx));
            if entry.is_dir() && expanded {
                self.collect_expanded_file_tree_dir(
                    tree,
                    &entry.path,
                    depth + 1,
                    rows,
                    selected_row,
                    cx,
                );
            }
        }
    }

    fn collect_expanded_file_tree_dir(
        &mut self,
        tree: &FileTree,
        dir: &Path,
        depth: usize,
        rows: &mut Vec<AnyElement>,
        selected_row: &mut Option<usize>,
        cx: &mut Context<Self>,
    ) {
        match self.file_tree_cache.children(tree, dir) {
            Ok(entries) if file_tree_rows::dir_children_need_inner_scroll(entries.len()) => {
                let mut inner_rows = Vec::new();
                let mut inner_selected_row = None;
                self.collect_file_tree_entries(
                    tree,
                    entries.as_ref(),
                    depth,
                    &mut inner_rows,
                    &mut inner_selected_row,
                    cx,
                );
                if self
                    .file_tree_state
                    .selected_path
                    .as_ref()
                    .is_some_and(|selected| path_is_within_dir(selected, dir))
                {
                    *selected_row = Some(rows.len());
                }
                log::debug!(
                    target: "tty7::file_tree",
                    "render long file tree dir as inner scroll dir={} entries={} height={}",
                    dir.display(),
                    entries.len(),
                    file_tree_rows::long_dir_inner_scroll_height()
                );
                rows.push(file_tree_rows::long_dir_inner_scroll(dir, inner_rows, cx));
            }
            Ok(entries) => {
                self.collect_file_tree_entries(
                    tree,
                    entries.as_ref(),
                    depth,
                    rows,
                    selected_row,
                    cx,
                );
            }
            Err(err) => rows.push(file_tree_rows::error_row(err, depth, cx)),
        }
    }

    fn render_file_tree_entry(
        &self,
        entry: &FileTreeEntry,
        depth: usize,
        expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = entry.path.clone();
        let entry_is_dir = entry.is_dir();
        let selected = self
            .file_tree_state
            .selected_path
            .as_ref()
            .is_some_and(|selected| selected == &entry.path);
        let entry_icon: AnyElement = match entry.kind {
            FileTreeEntryKind::Directory => img(folder_icon_path(expanded))
                .size(px(14.))
                .flex_none()
                .into_any_element(),
            FileTreeEntryKind::File => img(file_icon_path(&entry.path))
                .size(px(14.))
                .flex_none()
                .into_any_element(),
            FileTreeEntryKind::Symlink => img(file_symlink_icon_path())
                .size(px(14.))
                .flex_none()
                .into_any_element(),
        };
        let chevron = if entry.is_dir() {
            Some(if expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
        } else {
            None
        };
        let has_chevron = chevron.is_some();

        div()
            .h(px(file_tree_rows::ROW_HEIGHT))
            .px_2()
            .flex()
            .items_center()
            .gap_1p5()
            .cursor_pointer()
            .text_sm()
            .text_color(if selected {
                cx.theme().foreground
            } else {
                cx.theme().muted_foreground
            })
            .bg(if selected {
                cx.theme().muted
            } else {
                cx.theme().transparent
            })
            .hover(|row| row.bg(cx.theme().muted))
            .pl(px(8.0 + depth as f32 * file_tree_rows::INDENT))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    if entry_is_dir {
                        this.toggle_file_tree_dir(path.clone(), cx);
                    } else {
                        this.open_file_preview(path.clone(), window, cx);
                    }
                }),
            )
            .children(chevron.map(|name| {
                Icon::new(name)
                    .size(px(13.))
                    .text_color(cx.theme().muted_foreground)
            }))
            .when(!has_chevron, |row| row.child(div().size(px(13.))))
            .child(entry_icon)
            .child(div().min_w_0().truncate().child(entry.name.clone()))
            .into_any_element()
    }

    fn toggle_file_tree_dir(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.file_tree_state.selected_path = Some(path.clone());
        let expanded = if let Some(index) = self
            .file_tree_state
            .expanded_dirs
            .iter()
            .position(|expanded| expanded == &path)
        {
            self.file_tree_state.expanded_dirs.remove(index);
            false
        } else {
            self.file_tree_state.expanded_dirs.push(path.clone());
            true
        };
        log::debug!(
            target: "tty7::file_tree",
            "toggle file tree dir path={} expanded={} expanded_dirs={}",
            path.display(),
            expanded,
            self.file_tree_state.expanded_dirs.len()
        );
        self.save_session(cx);
        cx.notify();
    }

    fn is_file_tree_expanded(&self, path: &Path) -> bool {
        self.file_tree_state
            .expanded_dirs
            .iter()
            .any(|expanded| expanded == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_dirs_to_reveal_path_expands_ancestors_inside_root() {
        let root = PathBuf::from("/workspace");
        let path = root.join("src/ui/preview.rs");
        let mut expanded = vec![root.join("src")];

        expand_dirs_to_reveal_path(&root, &path, &mut expanded);

        assert_eq!(expanded, vec![root.join("src"), root.join("src/ui")]);
    }

    #[test]
    fn expand_dirs_to_reveal_path_ignores_paths_outside_root() {
        let root = PathBuf::from("/workspace");
        let mut expanded = Vec::new();

        expand_dirs_to_reveal_path(&root, Path::new("/other/src/main.rs"), &mut expanded);

        assert!(expanded.is_empty());
    }
}
