use std::path::{Path, PathBuf};

use gpui::{
    App, Context, FocusHandle, Focusable, FontWeight, MouseButton, MouseDownEvent, Render, Window,
    div, img, prelude::*, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};

use crate::core::config::Config;
use crate::core::file_preview::{self, FilePreviewBody, FilePreviewDocument};

const MAX_HIGHLIGHT_BYTES: usize = 96 * 1024;

pub struct FilePreview {
    pub path: PathBuf,
    pub focus_handle: FocusHandle,
    document: FilePreviewDocument,
    markdown_mode: MarkdownPreviewMode,
    image_fit: ImagePreviewFit,
}

impl FilePreview {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let document = file_preview::load(&path);
        Self {
            path: document.path.clone(),
            focus_handle: cx.focus_handle(),
            document,
            markdown_mode: MarkdownPreviewMode::Rendered,
            image_fit: ImagePreviewFit::Contain,
        }
    }

    pub fn title(&self) -> String {
        file_label(&self.path)
    }

    fn render_body(&self, cx: &mut Context<FilePreview>) -> gpui::AnyElement {
        match &self.document.body {
            FilePreviewBody::Text { text, truncated } => {
                render_text_preview(&self.document.path, text, *truncated, cx)
            }
            FilePreviewBody::Markdown { source, truncated } => {
                render_markdown_preview(source, *truncated, self.markdown_mode, cx)
            }
            FilePreviewBody::Image { .. } => {
                render_image_preview(&self.document.path, self.image_fit, cx)
            }
            FilePreviewBody::Binary => status_body("Binary file cannot be previewed", cx),
            FilePreviewBody::Error(error) => status_body(error, cx),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkdownPreviewMode {
    Rendered,
    Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImagePreviewFit {
    Contain,
    ActualSize,
}

impl Focusable for FilePreview {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FilePreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = self.render_body(cx);

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

fn render_text_preview(
    path: &Path,
    text: &str,
    truncated: bool,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    let font_family = cx.global::<Config>().font_family.clone();
    if !should_highlight_text(text, truncated) {
        return render_plain_text_preview(text, truncated, font_family, cx);
    }
    let language = preview_language(path);
    let mut lines: Vec<_> = text
        .lines()
        .map(|line| render_code_line(line, language, cx))
        .collect();
    if text.ends_with('\n') {
        lines.push(render_code_line("", language, cx));
    }

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
        .children(lines)
        .when(truncated, |body| body.child(truncated_notice(cx)))
        .into_any_element()
}

fn render_plain_text_preview(
    text: &str,
    truncated: bool,
    font_family: String,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
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
        .child(text.to_string())
        .when(truncated, |body| body.child(truncated_notice(cx)))
        .into_any_element()
}

fn should_highlight_text(text: &str, truncated: bool) -> bool {
    !truncated && text.len() <= MAX_HIGHLIGHT_BYTES
}

fn render_markdown_preview(
    source: &str,
    truncated: bool,
    mode: MarkdownPreviewMode,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    let toolbar = preview_mode_toolbar(
        "Rendered",
        "Source",
        mode == MarkdownPreviewMode::Rendered,
        cx.listener(|this, _: &MouseDownEvent, _window, cx| {
            this.markdown_mode = MarkdownPreviewMode::Rendered;
            cx.notify();
        }),
        cx.listener(|this, _: &MouseDownEvent, _window, cx| {
            this.markdown_mode = MarkdownPreviewMode::Source;
            cx.notify();
        }),
        cx,
    );

    let body = if mode == MarkdownPreviewMode::Source {
        render_text_preview(Path::new("preview.md"), source, truncated, cx)
    } else {
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
    };

    v_flex()
        .size_full()
        .child(toolbar)
        .child(body)
        .into_any_element()
}

fn render_image_preview(
    path: &Path,
    fit: ImagePreviewFit,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    let toolbar = preview_mode_toolbar(
        "Fit",
        "1:1",
        fit == ImagePreviewFit::Contain,
        cx.listener(|this, _: &MouseDownEvent, _window, cx| {
            this.image_fit = ImagePreviewFit::Contain;
            cx.notify();
        }),
        cx.listener(|this, _: &MouseDownEvent, _window, cx| {
            this.image_fit = ImagePreviewFit::ActualSize;
            cx.notify();
        }),
        cx,
    );

    let image = img(path.to_path_buf()).with_fallback(|| {
        div()
            .p_4()
            .text_sm()
            .child("Failed to load image")
            .into_any_element()
    });
    let image = match fit {
        ImagePreviewFit::Contain => image.max_w_full().max_h_full(),
        ImagePreviewFit::ActualSize => image,
    };

    v_flex()
        .size_full()
        .child(toolbar)
        .child(
            div()
                .id("file-preview-image")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .overflow_x_scroll()
                .flex()
                .items_center()
                .justify_center()
                .p_4()
                .child(image),
        )
        .into_any_element()
}

fn preview_mode_toolbar(
    first: &'static str,
    second: &'static str,
    first_active: bool,
    on_first: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_second: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    h_flex()
        .h(px(36.))
        .flex_shrink_0()
        .items_center()
        .justify_end()
        .gap_1()
        .border_b_1()
        .border_color(cx.theme().border)
        .px_3()
        .child(preview_mode_button(first, first_active, on_first, cx))
        .child(preview_mode_button(second, !first_active, on_second, cx))
        .into_any_element()
}

fn preview_mode_button(
    label: &'static str,
    active: bool,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    div()
        .h(px(24.))
        .min_w(px(48.))
        .px_2()
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
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
        .child(label)
        .on_mouse_down(MouseButton::Left, on_click)
        .into_any_element()
}

fn render_code_line(
    line: &str,
    language: Option<&'static str>,
    cx: &mut Context<FilePreview>,
) -> gpui::AnyElement {
    let segments = syntax_segments(line, language);
    div()
        .min_w(px(1.))
        .whitespace_nowrap()
        .flex()
        .children(segments.into_iter().map(|segment| {
            div()
                .whitespace_nowrap()
                .text_color(syntax_color(segment.kind, cx))
                .child(segment.text)
        }))
        .into_any_element()
}

fn syntax_color(kind: SyntaxKind, cx: &mut Context<FilePreview>) -> gpui::Hsla {
    match kind {
        SyntaxKind::Plain => cx.theme().foreground,
        SyntaxKind::Keyword => cx.theme().blue,
        SyntaxKind::String => cx.theme().success,
        SyntaxKind::Comment => cx.theme().muted_foreground,
    }
}

fn preview_language(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "rs" => Some("rust"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "tsx" => Some("typescript"),
        "json" | "jsonc" => Some("json"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "sh" | "bash" | "zsh" | "fish" => Some("shell"),
        "py" => Some("python"),
        "md" | "markdown" | "mdown" | "mkd" | "mdx" => Some("markdown"),
        "html" | "htm" => Some("html"),
        "css" | "scss" | "sass" => Some("css"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntaxKind {
    Plain,
    Keyword,
    String,
    Comment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SyntaxSegment {
    text: String,
    kind: SyntaxKind,
}

fn syntax_segments(line: &str, language: Option<&'static str>) -> Vec<SyntaxSegment> {
    let comment_marker = comment_marker(language);
    let mut segments = Vec::new();
    let mut plain_start = 0;
    let mut iter = line.char_indices().peekable();

    while let Some((index, ch)) = iter.next() {
        if let Some(marker) = comment_marker
            && line[index..].starts_with(marker)
        {
            push_keyword_segments(&line[plain_start..index], language, &mut segments);
            segments.push(SyntaxSegment {
                text: line[index..].to_string(),
                kind: SyntaxKind::Comment,
            });
            return segments;
        }

        if matches!(ch, '"' | '\'' | '`') {
            push_keyword_segments(&line[plain_start..index], language, &mut segments);
            let quote = ch;
            let mut end = line.len();
            let mut escaped = false;
            for (next_index, next_ch) in iter.by_ref() {
                if escaped {
                    escaped = false;
                    continue;
                }
                if next_ch == '\\' {
                    escaped = true;
                    continue;
                }
                if next_ch == quote {
                    end = next_index + next_ch.len_utf8();
                    break;
                }
            }
            segments.push(SyntaxSegment {
                text: line[index..end].to_string(),
                kind: SyntaxKind::String,
            });
            plain_start = end;
        }
    }

    push_keyword_segments(&line[plain_start..], language, &mut segments);
    if segments.is_empty() {
        segments.push(SyntaxSegment {
            text: String::new(),
            kind: SyntaxKind::Plain,
        });
    }
    segments
}

fn push_keyword_segments(
    text: &str,
    language: Option<&'static str>,
    segments: &mut Vec<SyntaxSegment>,
) {
    let mut word_start = None;
    let mut plain_start = None;
    for (index, ch) in text.char_indices() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            if let Some(start) = plain_start.take() {
                push_plain(&text[start..index], segments);
            }
            word_start.get_or_insert(index);
            continue;
        }

        if let Some(start) = word_start.take() {
            push_word(&text[start..index], language, segments);
        }
        plain_start.get_or_insert(index);
    }
    if let Some(start) = word_start {
        push_word(&text[start..], language, segments);
    }
    if let Some(start) = plain_start {
        push_plain(&text[start..], segments);
    }
}

fn push_plain(text: &str, segments: &mut Vec<SyntaxSegment>) {
    if text.is_empty() {
        return;
    }
    segments.push(SyntaxSegment {
        text: text.to_string(),
        kind: SyntaxKind::Plain,
    });
}

fn push_word(word: &str, language: Option<&'static str>, segments: &mut Vec<SyntaxSegment>) {
    segments.push(SyntaxSegment {
        text: word.to_string(),
        kind: if is_keyword(word, language) {
            SyntaxKind::Keyword
        } else {
            SyntaxKind::Plain
        },
    });
}

fn comment_marker(language: Option<&'static str>) -> Option<&'static str> {
    match language {
        Some("python" | "shell" | "toml" | "yaml") => Some("#"),
        Some("html") => Some("<!--"),
        Some(_) => Some("//"),
        None => None,
    }
}

fn is_keyword(word: &str, language: Option<&'static str>) -> bool {
    match language {
        Some("rust") => matches!(
            word,
            "as" | "async"
                | "await"
                | "const"
                | "crate"
                | "else"
                | "enum"
                | "fn"
                | "for"
                | "if"
                | "impl"
                | "let"
                | "match"
                | "mod"
                | "mut"
                | "pub"
                | "return"
                | "self"
                | "struct"
                | "trait"
                | "type"
                | "use"
                | "where"
        ),
        Some("javascript" | "typescript") => matches!(
            word,
            "async"
                | "await"
                | "class"
                | "const"
                | "else"
                | "export"
                | "function"
                | "if"
                | "import"
                | "interface"
                | "let"
                | "return"
                | "type"
                | "var"
        ),
        Some("python") => matches!(
            word,
            "class"
                | "def"
                | "elif"
                | "else"
                | "for"
                | "from"
                | "if"
                | "import"
                | "in"
                | "return"
                | "self"
                | "with"
        ),
        Some("shell") => matches!(
            word,
            "case" | "do" | "done" | "elif" | "else" | "fi" | "for" | "function" | "if" | "then"
        ),
        _ => false,
    }
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
        .child(format!(
            "Preview truncated at {} KiB",
            file_preview::MAX_PREVIEW_BYTES / 1024
        ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_language_uses_common_extensions() {
        assert_eq!(preview_language(Path::new("src/main.rs")), Some("rust"));
        assert_eq!(preview_language(Path::new("package.json")), Some("json"));
        assert_eq!(preview_language(Path::new("app.tsx")), Some("typescript"));
        assert_eq!(preview_language(Path::new("README")), None);
    }

    #[test]
    fn syntax_segments_classify_comments_strings_and_keywords() {
        let segments = syntax_segments("fn main() { println!(\"hi\"); // ok }", Some("rust"));

        assert!(
            segments
                .iter()
                .any(|segment| segment.kind == SyntaxKind::Keyword)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.kind == SyntaxKind::String)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.kind == SyntaxKind::Comment)
        );
    }

    #[test]
    fn syntax_segments_groups_plain_runs() {
        let segments = syntax_segments("let value = call(arg);", Some("rust"));

        assert!(segments.len() < "let value = call(arg);".len() / 2);
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == " = " && segment.kind == SyntaxKind::Plain)
        );
    }

    #[test]
    fn large_or_truncated_text_is_not_highlighted() {
        assert!(should_highlight_text("fn main() {}", false));
        assert!(!should_highlight_text(
            &"a".repeat(MAX_HIGHLIGHT_BYTES + 1),
            false
        ));
        assert!(!should_highlight_text("fn main() {}", true));
    }
}
