use gpui::{Context, Render, SharedString, Window, div, prelude::*};
use gpui_component::ActiveTheme as _;

#[derive(Clone)]
pub(super) struct DragTab {
    pub(super) index: usize,
    pub(super) label: SharedString,
}

impl Render for DragTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .rounded_lg()
            .bg(cx.theme().secondary)
            .border_1()
            .border_color(cx.theme().border)
            .text_sm()
            .text_color(cx.theme().foreground)
            .child(self.label.clone())
    }
}
