use gpui::{Context, MouseButton, MouseDownEvent, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _};

use crate::ui::app::Tty7App;

pub(super) fn file_tree_toggle_icon(visible: bool) -> IconName {
    if visible {
        IconName::PanelRightClose
    } else {
        IconName::PanelRightOpen
    }
}

impl Tty7App {
    pub(crate) fn file_tree_toggle_tile(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id("file-tree-toggle")
            .occlude()
            .h_full()
            .w(px(34.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .text_color(cx.theme().muted_foreground)
            .hover(|s| s.bg(cx.theme().muted))
            .child(Icon::new(file_tree_toggle_icon(self.file_tree_state.visible)).small())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    cx.stop_propagation();
                    this.toggle_file_tree_visibility(cx);
                }),
            )
    }

    pub(super) fn toggle_file_tree_visibility(&mut self, cx: &mut Context<Self>) {
        self.file_tree_state.visible = !self.file_tree_state.visible;
        self.save_session(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_tree_toggle_icon_reflects_visibility() {
        assert!(matches!(
            file_tree_toggle_icon(true),
            IconName::PanelRightClose
        ));
        assert!(matches!(
            file_tree_toggle_icon(false),
            IconName::PanelRightOpen
        ));
    }
}
