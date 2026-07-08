use std::path::{Path, PathBuf};

use gpui::{
    App, Context, FocusHandle, Focusable, MouseButton, MouseDownEvent, Render, Window, div,
    prelude::*, px,
};
use gpui_component::ActiveTheme as _;

use crate::core::config::Config;
use crate::core::file_preview::{self, FilePreviewBody, FilePreviewDocument};

pub struct FilePreview {
    pub path: PathBuf,
    pub focus_handle: FocusHandle,
    document: FilePreviewDocument,
}

impl FilePreview {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let document = file_preview::load(&path);
        Self {
            path: document.path.clone(),
            focus_handle: cx.focus_handle(),
            document,
        }
    }

    pub fn title(&self) -> String {
        file_label(&self.path)
    }
}

impl Focusable for FilePreview {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FilePreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = render_body(&self.document, cx);

        div()
            .id("file-preview")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle, cx);
                }),
            )
            .child(body)
    }
}

fn render_body(document: &FilePreviewDocument, cx: &mut Context<FilePreview>) -> gpui::AnyElement {
    match &document.body {
        FilePreviewBody::Text { text, truncated } => {
            let font_family = cx.global::<Config>().font_family.clone();
            div()
                .id("file-preview-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .overflow_x_scroll()
                .p_4()
                .font_family(font_family)
                .text_sm()
                .line_height(px(20.))
                .whitespace_nowrap()
                .child(text.clone())
                .when(*truncated, |body| {
                    body.child(
                        div()
                            .mt_4()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Preview truncated"),
                    )
                })
                .into_any_element()
        }
        FilePreviewBody::Binary => status_body("Binary file", cx),
        FilePreviewBody::Error(error) => status_body(error, cx),
    }
}

fn status_body(message: impl Into<String>, cx: &mut Context<FilePreview>) -> gpui::AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(message.into())
        .into_any_element()
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
