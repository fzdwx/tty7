use std::path::Path;

use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::ActiveTheme as _;

use crate::ui::app::Tty7App;

pub(super) const INDENT: f32 = 16.0;
pub(super) const ROW_HEIGHT: f32 = 26.0;

const LONG_DIR_CHILD_THRESHOLD: usize = 30;
const LONG_DIR_VISIBLE_ROWS: usize = 12;

pub(super) fn dir_children_need_inner_scroll(child_count: usize) -> bool {
    child_count > LONG_DIR_CHILD_THRESHOLD
}

pub(super) fn long_dir_inner_scroll_height() -> f32 {
    ROW_HEIGHT * LONG_DIR_VISIBLE_ROWS as f32
}

pub(super) fn long_dir_inner_scroll(
    dir: &Path,
    rows: Vec<AnyElement>,
    cx: &mut Context<Tty7App>,
) -> AnyElement {
    div()
        .id(format!("file-tree-long-dir:{}", dir.display()))
        .occlude()
        .h(px(long_dir_inner_scroll_height()))
        .min_h_0()
        .overflow_y_scroll()
        .on_scroll_wheel(|_, _, cx| {
            cx.stop_propagation();
        })
        .border_l_1()
        .border_color(cx.theme().border)
        .children(rows)
        .into_any_element()
}

pub(super) fn error_row(err: String, depth: usize, cx: &mut Context<Tty7App>) -> AnyElement {
    div()
        .px_3()
        .py_1()
        .pl(px(8.0 + depth as f32 * INDENT))
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(err)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_directory_children_use_inner_scroll_only_after_threshold() {
        assert!(!dir_children_need_inner_scroll(30));
        assert!(dir_children_need_inner_scroll(31));
    }

    #[test]
    fn long_directory_inner_scroll_height_matches_visible_row_budget() {
        assert_eq!(long_dir_inner_scroll_height(), 312.0);
    }
}
