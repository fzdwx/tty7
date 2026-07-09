use std::path::Path;

use gpui::{AnyElement, ClipboardItem, Context, MouseButton, MouseDownEvent, div, prelude::*, px};
use gpui_component::input::Input;
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _, Size};

use crate::ui::app::Tty7App;
use crate::ui::workspace::WORKSPACE_RAIL_WIDTH;

impl Tty7App {
    pub(crate) fn render_workspace_switcher(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self.workspace_count();
        let mut rows = Vec::with_capacity(count);
        for index in 0..count {
            rows.push(self.render_workspace_item(index, cx));
        }

        div()
            .w(px(WORKSPACE_RAIL_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(div().flex_1().min_h_0().py_2().children(rows))
            .child(
                div()
                    .p_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(self.render_new_workspace_button(cx)),
            )
            .into_any_element()
    }

    fn render_workspace_item(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let active = index == self.active_workspace;
        let label = self.workspace_label(index);
        let branch = self.workspace_git_branch(index);
        let root = if active {
            self.file_tree_root.clone()
        } else {
            self.workspace_snapshots
                .get(index)
                .map(|workspace| workspace.root.clone())
                .unwrap_or_else(|| self.file_tree_root.clone())
        };
        let icon = if active {
            IconName::FolderOpen
        } else {
            IconName::Folder
        };
        let rename_input = self
            .workspace_renaming
            .as_ref()
            .filter(|renaming| renaming.index == index)
            .map(|renaming| renaming.input.clone());
        let label_region: AnyElement = match rename_input {
            Some(input) => div()
                .w(px(52.))
                .h(px(22.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(Input::new(&input).small().w_full().h_full())
                .into_any_element(),
            None => div()
                .max_w(px(52.))
                .truncate()
                .text_xs()
                .child(label)
                .into_any_element(),
        };
        let app = cx.weak_entity();

        div()
            .id(("workspace-switch", index))
            .mx_2()
            .mb_1()
            .h(px(58.))
            .rounded_lg()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_0p5()
            .cursor_pointer()
            .text_color(if active {
                cx.theme().foreground
            } else {
                cx.theme().muted_foreground
            })
            .bg(if active {
                cx.theme().secondary
            } else {
                cx.theme().transparent
            })
            .hover(|item| item.bg(cx.theme().muted))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    if this
                        .workspace_renaming
                        .as_ref()
                        .is_some_and(|renaming| renaming.index == index)
                    {
                        cx.stop_propagation();
                        return;
                    }
                    this.switch_workspace(index, window, cx);
                }),
            )
            .child(Icon::new(icon).size(px(16.)))
            .child(label_region)
            .when_some(branch, |item, branch| {
                item.child(
                    div()
                        .max_w(px(52.))
                        .truncate()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(branch),
                )
            })
            .context_menu(move |menu, _window, _cx| {
                let rename_app = app.clone();
                let delete_app = app.clone();
                let choose_root_app = app.clone();
                let copy_root_app = app.clone();
                let copy_root = root.clone();
                let reveal_root_app = app.clone();
                let reveal_root = root.clone();

                menu.with_size(Size::Small)
                    .min_w(px(220.))
                    .item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
                        if let Some(app) = rename_app.upgrade() {
                            app.update(cx, |this, cx| {
                                this.start_workspace_rename(index, window, cx);
                            });
                        }
                    }))
                    .item(
                        PopupMenuItem::new("Change Root...").on_click(move |_, _window, cx| {
                            if let Some(app) = choose_root_app.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.choose_workspace_root(index, cx);
                                });
                            }
                        }),
                    )
                    .separator()
                    .item(PopupMenuItem::new("Delete").on_click(move |_, window, cx| {
                        if let Some(app) = delete_app.upgrade() {
                            app.update(cx, |this, cx| {
                                this.delete_workspace(index, window, cx);
                            });
                        }
                    }))
                    .separator()
                    .item(
                        PopupMenuItem::new("Copy Root Path").on_click(move |_, _window, cx| {
                            if let Some(app) = copy_root_app.upgrade() {
                                app.update(cx, |_this, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        copy_root.display().to_string(),
                                    ));
                                });
                            }
                        }),
                    )
                    .item(
                        PopupMenuItem::new("Reveal Root").on_click(move |_, _window, cx| {
                            if let Some(app) = reveal_root_app.upgrade() {
                                app.update(cx, |_this, _cx| {
                                    reveal_in_file_manager(&reveal_root);
                                });
                            }
                        }),
                    )
            })
            .into_any_element()
    }

    fn render_new_workspace_button(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("workspace-add")
            .h(px(36.))
            .rounded_lg()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(cx.theme().muted_foreground)
            .hover(|item| item.bg(cx.theme().muted))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    this.new_workspace(window, cx);
                }),
            )
            .child(Icon::new(IconName::Plus).size(px(16.)))
            .into_any_element()
    }
}

fn reveal_in_file_manager(path: &Path) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(err) = std::process::Command::new(opener).arg(path).spawn() {
        log::warn!("failed to reveal workspace root {}: {err}", path.display());
    }
}
