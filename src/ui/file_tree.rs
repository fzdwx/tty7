use std::path::{Path, PathBuf};

use gpui::{AnyElement, Context, MouseButton, MouseDownEvent, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, Icon, IconName};

use crate::core::file_tree::{FileTree, FileTreeEntry, FileTreeEntryKind};
use crate::ui::app::Tty7App;

const FILE_TREE_INDENT: f32 = 16.0;

impl Tty7App {
    pub(crate) fn render_file_tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut rows = Vec::new();
        match FileTree::new(&self.file_tree_root) {
            Ok(tree) => self.collect_file_tree_rows(&tree, &self.file_tree_root, 0, &mut rows, cx),
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
                    .py_1()
                    .children(rows),
            )
            .into_any_element()
    }

    fn collect_file_tree_rows(
        &self,
        tree: &FileTree,
        dir: &Path,
        depth: usize,
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        match tree.list_children(dir) {
            Ok(entries) => {
                for entry in entries {
                    let expanded = self.is_file_tree_expanded(&entry.path);
                    rows.push(self.render_file_tree_entry(&entry, depth, expanded, cx));
                    if entry.is_dir() && expanded {
                        self.collect_file_tree_rows(tree, &entry.path, depth + 1, rows, cx);
                    }
                }
            }
            Err(err) => rows.push(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(err.to_string())
                    .into_any_element(),
            ),
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
        let icon = match entry.kind {
            FileTreeEntryKind::Directory if expanded => IconName::FolderOpen,
            FileTreeEntryKind::Directory => IconName::Folder,
            FileTreeEntryKind::File | FileTreeEntryKind::Symlink => IconName::File,
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
            .h(px(26.))
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
            .pl(px(8.0 + depth as f32 * FILE_TREE_INDENT))
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
            .child(
                Icon::new(icon)
                    .size(px(14.))
                    .text_color(cx.theme().muted_foreground),
            )
            .child(div().min_w_0().truncate().child(entry.name.clone()))
            .into_any_element()
    }

    fn toggle_file_tree_dir(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.file_tree_state.selected_path = Some(path.clone());
        if let Some(index) = self
            .file_tree_state
            .expanded_dirs
            .iter()
            .position(|expanded| expanded == &path)
        {
            self.file_tree_state.expanded_dirs.remove(index);
        } else {
            self.file_tree_state.expanded_dirs.push(path);
        }
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
