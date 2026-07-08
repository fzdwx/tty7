use std::path::{Path, PathBuf};

use gpui::{
    App, Context, FocusHandle, Focusable, FontWeight, MouseButton, MouseDownEvent, Render, Window,
    div, img, prelude::*, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};

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
        FilePreviewBody::Markdown { source, truncated } => {
            render_markdown_preview(source, *truncated, cx)
        }
        FilePreviewBody::Image { .. } => div()
            .id("file-preview-image")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .overflow_x_scroll()
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                img(document.path.clone())
                    .max_w_full()
                    .max_h_full()
                    .with_fallback(|| {
                        div()
                            .p_4()
                            .text_sm()
                            .child("Failed to load image")
                            .into_any_element()
                    }),
            )
            .into_any_element(),
        FilePreviewBody::Binary => status_body("Binary file", cx),
        FilePreviewBody::Error(error) => status_body(error, cx),
    }
}

fn render_markdown_preview(
    source: &str,
    truncated: bool,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    let mut children = markdown_blocks(source, cx);
    if truncated {
        children.push(truncated_notice(cx));
    }

    div()
        .id("file-preview-markdown")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .overflow_x_scroll()
        .p_6()
        .child(
            v_flex()
                .gap_3()
                .w_full()
                .max_w(px(920.))
                .mx_auto()
                .children(children),
        )
        .into_any_element()
}

fn markdown_blocks(source: &str, cx: &mut Context<FilePreview>) -> Vec<gpui::AnyElement> {
    let lines: Vec<&str> = source.lines().collect();
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        if trimmed.starts_with("```") {
            let start = index + 1;
            index = start;
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                index += 1;
            }
            blocks.push(markdown_code_block(lines[start..index].join("\n"), cx));
            if index < lines.len() {
                index += 1;
            }
            continue;
        }

        if let Some((depth, text)) = markdown_heading(trimmed) {
            blocks.push(markdown_heading_block(depth, text, cx));
            index += 1;
            continue;
        }

        if is_markdown_table_at(&lines, index) {
            let header = split_table_cells(lines[index]);
            index += 2;
            let mut rows = Vec::new();
            while index < lines.len() && split_table_cells(lines[index]).len() > 1 {
                rows.push(split_table_cells(lines[index]));
                index += 1;
            }
            blocks.push(markdown_table(header, rows, cx));
            continue;
        }

        if let Some(text) = markdown_list_item(trimmed) {
            blocks.push(markdown_list_block(text, cx));
            index += 1;
            continue;
        }

        if let Some(text) = trimmed.strip_prefix("> ") {
            blocks.push(markdown_quote_block(text, cx));
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < lines.len() && !starts_markdown_block(lines[index], &lines, index) {
            index += 1;
        }
        blocks.push(markdown_paragraph(lines[start..index].join(" "), cx));
    }

    blocks
}

fn starts_markdown_block(line: &str, lines: &[&str], index: usize) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || trimmed.starts_with("```")
        || markdown_heading(trimmed).is_some()
        || markdown_list_item(trimmed).is_some()
        || trimmed.starts_with("> ")
        || is_markdown_table_at(lines, index)
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let depth = line.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&depth) {
        return None;
    }
    let text = line.get(depth..)?.strip_prefix(' ')?;
    Some((depth, text.trim()))
}

fn markdown_list_item(line: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(text) = line.strip_prefix(marker) {
            return Some(text.trim());
        }
    }

    let dot = line.find(". ")?;
    if dot > 0 && line[..dot].chars().all(|ch| ch.is_ascii_digit()) {
        return Some(line[dot + 2..].trim());
    }
    None
}

fn is_markdown_table_at(lines: &[&str], index: usize) -> bool {
    index + 1 < lines.len()
        && split_table_cells(lines[index]).len() > 1
        && is_markdown_table_separator(lines[index + 1])
}

fn is_markdown_table_separator(line: &str) -> bool {
    let cells = split_table_cells(line);
    cells.len() > 1
        && cells.iter().all(|cell| {
            let stripped = cell.trim_matches(':').trim();
            !stripped.is_empty() && stripped.chars().all(|ch| ch == '-')
        })
}

fn split_table_cells(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = line
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect();
    if cells.first().is_some_and(|cell| cell.is_empty()) {
        cells.remove(0);
    }
    if cells.last().is_some_and(|cell| cell.is_empty()) {
        cells.pop();
    }
    cells
}

fn markdown_heading_block(
    depth: usize,
    text: &str,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    let size = match depth {
        1 => px(28.),
        2 => px(22.),
        3 => px(18.),
        _ => px(15.),
    };
    div()
        .mt(if depth == 1 { px(2.) } else { px(8.) })
        .text_size(size)
        .line_height(px(size.as_f32() + 8.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(text.to_string())
        .into_any_element()
}

fn markdown_paragraph(text: String, cx: &mut Context<FilePreview>) -> gpui::AnyElement {
    div()
        .text_sm()
        .line_height(px(22.))
        .text_color(cx.theme().foreground)
        .child(text.trim().to_string())
        .into_any_element()
}

fn markdown_list_block(text: &str, cx: &mut Context<FilePreview>) -> gpui::AnyElement {
    h_flex()
        .items_start()
        .gap_2()
        .text_sm()
        .line_height(px(22.))
        .child(
            div()
                .w(px(14.))
                .text_color(cx.theme().muted_foreground)
                .child("-"),
        )
        .child(div().flex_1().child(text.to_string()))
        .into_any_element()
}

fn markdown_quote_block(text: &str, cx: &mut Context<FilePreview>) -> gpui::AnyElement {
    div()
        .border_l_2()
        .border_color(cx.theme().border)
        .pl_3()
        .py_1()
        .text_sm()
        .line_height(px(22.))
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
        .into_any_element()
}

fn markdown_code_block(code: String, cx: &mut Context<FilePreview>) -> gpui::AnyElement {
    let font_family = cx.global::<Config>().font_family.clone();
    div()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted.opacity(0.45))
        .p_3()
        .font_family(font_family)
        .text_sm()
        .line_height(px(20.))
        .whitespace_nowrap()
        .child(code)
        .into_any_element()
}

fn markdown_table(
    header: Vec<String>,
    rows: Vec<Vec<String>>,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    let mut table = v_flex()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border);

    table = table.child(markdown_table_row(header, true, cx));
    for row in rows {
        table = table.child(markdown_table_row(row, false, cx));
    }

    table.into_any_element()
}

fn markdown_table_row(
    cells: Vec<String>,
    header: bool,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    h_flex()
        .items_stretch()
        .bg(if header {
            cx.theme().muted
        } else {
            cx.theme().transparent
        })
        .border_b_1()
        .border_color(cx.theme().border)
        .children(cells.into_iter().map(|cell| {
            div()
                .w(px(180.))
                .flex_shrink_0()
                .px_3()
                .py_2()
                .text_sm()
                .line_height(px(20.))
                .when(header, |cell| cell.font_weight(FontWeight::MEDIUM))
                .child(cell)
        }))
        .into_any_element()
}

fn truncated_notice(cx: &mut Context<FilePreview>) -> gpui::AnyElement {
    div()
        .mt_4()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child("Preview truncated")
        .into_any_element()
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
