use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui::{
    AnyElement, ClipboardItem, Context, MouseButton, MouseDownEvent, PromptLevel, div, img,
    prelude::*, px,
};
use gpui_component::input::Input;
use gpui_component::input::{InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::{ActiveTheme as _, Icon, IconName, Size};

use crate::core::file_tree::{FileTree, FileTreeEntry, FileTreeEntryKind};
use crate::ui::app::{FileTreeRenaming, Tty7App};
use crate::ui::file_icons::{file_icon_path, file_symlink_icon_path, folder_icon_path};

mod cache;
mod git_status;
mod rows;

use rows as file_tree_rows;

pub(crate) use cache::FileTreeCache;
pub(crate) use git_status::FileTreeGitStatusCache;
use git_status::{FileTreeGitStatus, GitTreeStatus};

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

fn unique_child_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }

    let (stem, extension) = name
        .rsplit_once('.')
        .map(|(stem, extension)| (stem, Some(extension)))
        .unwrap_or((name, None));
    for index in 2.. {
        let name = match extension {
            Some(extension) => format!("{stem} {index}.{extension}"),
            None => format!("{stem} {index}"),
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("unbounded unique filename search must return")
}

fn reveal_in_file_manager(path: &Path) {
    let target = path.parent().filter(|_| path.is_file()).unwrap_or(path);
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(err) = std::process::Command::new(opener).arg(target).spawn() {
        log::warn!("failed to reveal {}: {err}", path.display());
    }
}

impl Tty7App {
    fn refresh_file_tree_git_status(&mut self, force: bool, cx: &mut Context<Self>) {
        let root = self.file_tree_root.clone();
        let Some(generation) = self.file_tree_git_status.begin_refresh(&root, force) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let load_root = root.clone();
            let status = cx
                .background_spawn(async move { FileTreeGitStatus::load(&load_root) })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app
                    .file_tree_git_status
                    .finish_refresh(&root, generation, status)
                {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn refresh_file_tree(&mut self, cx: &mut Context<Self>) {
        log::debug!(
            target: "tty7::file_tree",
            "refresh file tree root={}",
            self.file_tree_root.display()
        );
        self.file_tree_cache.clear();
        self.file_search_index = None;
        self.refresh_file_tree_git_status(true, cx);
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
        self.refresh_file_tree_git_status(false, cx);
        let git_status = self.file_tree_git_status.snapshot();
        let mut rows = Vec::new();
        let mut selected_row = None;
        match FileTree::new(&root) {
            Ok(tree) => self.collect_file_tree_rows(
                &tree,
                &root,
                &git_status,
                0,
                &mut rows,
                &mut selected_row,
                cx,
            ),
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
        git_status: &FileTreeGitStatus,
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
                    git_status,
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
        git_status: &FileTreeGitStatus,
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
            rows.push(self.render_file_tree_entry(entry, git_status, depth, expanded, cx));
            if entry.is_dir() && expanded {
                self.collect_expanded_file_tree_dir(
                    tree,
                    &entry.path,
                    git_status,
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
        git_status: &FileTreeGitStatus,
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
                    git_status,
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
                    git_status,
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
        git_status: &FileTreeGitStatus,
        depth: usize,
        expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = entry.path.clone();
        let entry_is_dir = entry.is_dir();
        let menu_path = path.clone();
        let menu_is_dir = entry_is_dir;
        let app = cx.entity().downgrade();
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
        let git_badge = git_status.status_for(&entry.path, entry_is_dir);
        let renaming_input = self
            .file_tree_renaming
            .as_ref()
            .filter(|renaming| renaming.path == entry.path)
            .map(|renaming| renaming.input.clone());
        let name_region = if let Some(input) = renaming_input {
            div()
                .min_w_0()
                .flex_1()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(Input::new(&input).appearance(false))
                .into_any_element()
        } else {
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .child(entry.name.clone())
                .into_any_element()
        };
        let status_region = match git_badge {
            Some(status) => div()
                .w(px(14.))
                .flex_none()
                .text_xs()
                .text_color(match status {
                    GitTreeStatus::Added => cx.theme().success,
                    GitTreeStatus::Deleted => cx.theme().danger,
                    GitTreeStatus::Modified | GitTreeStatus::Renamed => cx.theme().warning,
                    GitTreeStatus::Untracked => cx.theme().muted_foreground,
                })
                .child(status.label())
                .into_any_element(),
            None => div().w(px(14.)).flex_none().into_any_element(),
        };

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
            .child(name_region)
            .child(status_region)
            .context_menu(move |menu, _window, _cx| {
                let target_dir = if menu_is_dir {
                    menu_path.clone()
                } else {
                    menu_path
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| menu_path.clone())
                };
                let new_file_app = app.clone();
                let new_file_dir = target_dir.clone();
                let new_folder_app = app.clone();
                let new_folder_dir = target_dir.clone();
                let rename_app = app.clone();
                let rename_path = menu_path.clone();
                let delete_app = app.clone();
                let delete_path = menu_path.clone();
                let copy_app = app.clone();
                let copy_path = menu_path.clone();
                let reveal_app = app.clone();
                let reveal_path = menu_path.clone();

                menu.with_size(Size::Small)
                    .min_w(px(220.))
                    .item(
                        PopupMenuItem::new("New File").on_click(move |_, window, cx| {
                            if let Some(app) = new_file_app.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.create_file_tree_entry(
                                        new_file_dir.clone(),
                                        false,
                                        window,
                                        cx,
                                    );
                                });
                            }
                        }),
                    )
                    .item(
                        PopupMenuItem::new("New Folder").on_click(move |_, window, cx| {
                            if let Some(app) = new_folder_app.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.create_file_tree_entry(
                                        new_folder_dir.clone(),
                                        true,
                                        window,
                                        cx,
                                    );
                                });
                            }
                        }),
                    )
                    .separator()
                    .item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
                        if let Some(app) = rename_app.upgrade() {
                            app.update(cx, |this, cx| {
                                this.start_file_tree_rename(rename_path.clone(), window, cx);
                            });
                        }
                    }))
                    .item(PopupMenuItem::new("Delete").on_click(move |_, window, cx| {
                        if let Some(app) = delete_app.upgrade() {
                            app.update(cx, |this, cx| {
                                this.delete_file_tree_path(delete_path.clone(), window, cx);
                            });
                        }
                    }))
                    .separator()
                    .item(
                        PopupMenuItem::new("Copy Path").on_click(move |_, _window, cx| {
                            if let Some(app) = copy_app.upgrade() {
                                app.update(cx, |_this, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        copy_path.display().to_string(),
                                    ));
                                });
                            }
                        }),
                    )
                    .item(PopupMenuItem::new("Reveal in File Manager").on_click(
                        move |_, _window, cx| {
                            if let Some(app) = reveal_app.upgrade() {
                                app.update(cx, |_this, _cx| {
                                    reveal_in_file_manager(&reveal_path);
                                });
                            }
                        },
                    ))
            })
            .into_any_element()
    }

    pub(crate) fn start_file_tree_rename(
        &mut self,
        path: PathBuf,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        input.update(cx, |state, cx| state.focus(window, cx));
        let subs = vec![cx.subscribe_in(
            &input,
            window,
            |this, _input, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    this.commit_file_tree_rename(window, cx)
                }
                _ => {}
            },
        )];
        self.file_tree_renaming = Some(FileTreeRenaming {
            path,
            input,
            _subs: subs,
        });
        cx.notify();
    }

    fn commit_file_tree_rename(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.file_tree_renaming.take() else {
            return;
        };
        let value = renaming.input.read(cx).value().trim().to_string();
        if value.is_empty() {
            self.focus_active(window, cx);
            cx.notify();
            return;
        }
        let Some(parent) = renaming.path.parent() else {
            return;
        };
        let target = parent.join(value);
        if target == renaming.path {
            self.focus_active(window, cx);
            cx.notify();
            return;
        }
        if target.exists() {
            self.warn_file_tree_error(
                "Rename Failed",
                format!("{} already exists", target.display()),
                window,
                cx,
            );
            self.focus_active(window, cx);
            cx.notify();
            return;
        }
        match std::fs::rename(&renaming.path, &target) {
            Ok(()) => {
                self.update_file_tree_paths_after_rename(&renaming.path, &target);
                self.file_tree_cache.clear();
                self.file_search_index = None;
                self.save_session(cx);
            }
            Err(err) => self.warn_file_tree_error("Rename Failed", err.to_string(), window, cx),
        }
        self.focus_active(window, cx);
        cx.notify();
    }

    pub(crate) fn create_file_tree_entry(
        &mut self,
        dir: PathBuf,
        directory: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let path = unique_child_path(
            &dir,
            if directory {
                "untitled"
            } else {
                "untitled.txt"
            },
        );
        let result = if directory {
            std::fs::create_dir(&path)
        } else {
            std::fs::File::create(&path).map(|_| ())
        };
        match result {
            Ok(()) => {
                if !self
                    .file_tree_state
                    .expanded_dirs
                    .iter()
                    .any(|expanded| expanded == &dir)
                {
                    self.file_tree_state.expanded_dirs.push(dir);
                }
                self.file_tree_state.selected_path = Some(path.clone());
                self.file_tree_cache.clear();
                self.file_search_index = None;
                self.start_file_tree_rename(path, window, cx);
                self.save_session(cx);
            }
            Err(err) => self.warn_file_tree_error("Create Failed", err.to_string(), window, cx),
        }
        cx.notify();
    }

    pub(crate) fn delete_file_tree_path(
        &mut self,
        path: PathBuf,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let message = format!("Delete {}?", path.display());
        let answer = window.prompt(
            PromptLevel::Warning,
            "Delete File Tree Entry?",
            Some(&message),
            &["Cancel", "Delete"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if !matches!(answer.await, Ok(1)) {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                let result = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                match result {
                    Ok(()) => {
                        this.file_tree_state.selected_path = None;
                        this.file_tree_state
                            .expanded_dirs
                            .retain(|expanded| !path_is_within_dir(expanded, &path));
                        this.file_tree_cache.clear();
                        this.file_search_index = None;
                        this.save_session(cx);
                    }
                    Err(err) => {
                        log::warn!("delete file tree entry failed {}: {err}", path.display())
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn update_file_tree_paths_after_rename(&mut self, from: &Path, to: &Path) {
        if self
            .file_tree_state
            .selected_path
            .as_ref()
            .is_some_and(|selected| selected == from)
        {
            self.file_tree_state.selected_path = Some(to.to_path_buf());
        }
        for expanded in &mut self.file_tree_state.expanded_dirs {
            if expanded == from {
                *expanded = to.to_path_buf();
            } else if let Ok(rest) = expanded.strip_prefix(from) {
                *expanded = to.join(rest);
            }
        }
    }

    fn warn_file_tree_error(
        &self,
        title: &'static str,
        message: String,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let prompt = window.prompt(PromptLevel::Warning, title, Some(&message), &["OK"], cx);
        cx.spawn(async move |_this, _cx| {
            let _ = prompt.await;
        })
        .detach();
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

    #[test]
    fn unique_child_path_adds_suffix_before_extension() {
        let root = temp_dir("unique-child");
        std::fs::write(root.join("untitled.txt"), "").unwrap();
        std::fs::write(root.join("untitled 2.txt"), "").unwrap();

        let path = unique_child_path(&root, "untitled.txt");

        assert_eq!(path, root.join("untitled 3.txt"));
        std::fs::remove_dir_all(root).ok();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tty7-file-tree-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
