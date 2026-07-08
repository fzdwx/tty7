use gpui::{AnyElement, Context, MouseButton, MouseDownEvent, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, Icon, IconName};

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
        let icon = if active {
            IconName::FolderOpen
        } else {
            IconName::Folder
        };

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
                    this.switch_workspace(index, window, cx);
                }),
            )
            .child(Icon::new(icon).size(px(16.)))
            .child(div().max_w(px(52.)).truncate().text_xs().child(label))
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
