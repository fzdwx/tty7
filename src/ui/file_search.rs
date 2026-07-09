use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, Context, Entity, EventEmitter, MouseButton, MouseDownEvent, Subscription, Task, Window,
    div, img, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, IndexPath, StyledExt as _, h_flex,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    v_flex,
};

use crate::core::file_search::{FileSearchIndex, FileSearchResult};
use crate::ui::file_icons::file_icon_path;

const FILE_SEARCH_LIMIT: usize = 80;

pub enum FileSearchEvent {
    Open(PathBuf),
    Dismiss,
}

pub struct FileSearchDelegate {
    index: Rc<FileSearchIndex>,
    matched: Vec<FileSearchResult>,
    selected: Option<IndexPath>,
}

impl FileSearchDelegate {
    fn new(index: Rc<FileSearchIndex>) -> Self {
        let matched = index.search("", FILE_SEARCH_LIMIT);
        let selected = (!matched.is_empty()).then(IndexPath::default);
        Self {
            index,
            matched,
            selected,
        }
    }

    fn path_at(&self, ix: IndexPath) -> Option<PathBuf> {
        self.matched.get(ix.row).map(|result| result.path.clone())
    }
}

impl ListDelegate for FileSearchDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.matched.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.matched = self.index.search(query, FILE_SEARCH_LIMIT);
        self.selected = (!self.matched.is_empty()).then(IndexPath::default);
        Task::ready(())
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let result = self.matched.get(ix.row)?;
        let parent = result
            .relative_path
            .rsplit_once(std::path::MAIN_SEPARATOR)
            .map(|(parent, _)| parent)
            .filter(|parent| !parent.is_empty());
        let selected = Some(ix) == self.selected;
        let theme = cx.theme();

        let details = v_flex()
            .min_w_0()
            .gap_0p5()
            .child(div().truncate().child(result.file_name.clone()))
            .when_some(parent, |view, parent| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(parent.to_string()),
                )
            });

        Some(
            ListItem::new(ix.row).selected(selected).py_1().child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(img(file_icon_path(&result.path)).size(px(16.)).flex_none())
                    .child(details),
            ),
        )
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_1()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("No matching files")
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground.opacity(0.75))
                    .child("Try a different path or filename"),
            )
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
        cx.notify();
    }
}

pub struct FileSearchView {
    root: PathBuf,
    list: Option<Entity<ListState<FileSearchDelegate>>>,
    error: Option<String>,
    _sub: Option<Subscription>,
}

impl FileSearchView {
    pub fn loading(root: PathBuf) -> Self {
        Self {
            root,
            list: None,
            error: None,
            _sub: None,
        }
    }

    pub fn ready(
        root: PathBuf,
        index: Rc<FileSearchIndex>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::loading(root);
        view.set_index(index, window, cx);
        view
    }

    pub fn set_index(
        &mut self,
        index: Rc<FileSearchIndex>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delegate = FileSearchDelegate::new(index);
        let list = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));
        list.update(cx, |state, cx| state.focus(window, cx));
        let _sub = cx.subscribe_in(&list, window, Self::on_list_event);
        self.list = Some(list);
        self.error = None;
        self._sub = Some(_sub);
        cx.notify();
    }

    pub fn set_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.list = None;
        self.error = Some(message);
        self._sub = None;
        cx.notify();
    }

    fn on_list_event(
        &mut self,
        list: &Entity<ListState<FileSearchDelegate>>,
        ev: &ListEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ev {
            ListEvent::Confirm(ix) => match list.read(cx).delegate().path_at(*ix) {
                Some(path) => cx.emit(FileSearchEvent::Open(path)),
                None => cx.emit(FileSearchEvent::Dismiss),
            },
            ListEvent::Cancel => cx.emit(FileSearchEvent::Dismiss),
            ListEvent::Select(_) => {}
        }
    }
}

impl EventEmitter<FileSearchEvent> for FileSearchView {}

impl Render for FileSearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let content = if let Some(list) = self.list.as_ref() {
            List::new(list).p_1().max_h(px(520.)).into_any_element()
        } else if let Some(error) = self.error.as_ref() {
            v_flex()
                .gap_2()
                .p_4()
                .text_sm()
                .child(div().font_bold().child("Open File Failed"))
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(error.clone()),
                )
                .into_any_element()
        } else {
            v_flex()
                .gap_2()
                .p_4()
                .text_sm()
                .child(div().font_bold().child("Indexing Files"))
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(self.root.display().to_string()),
                )
                .into_any_element()
        };

        let card = v_flex()
            .w(px(680.))
            .max_h(px(520.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .shadow_lg()
            .overflow_hidden()
            .child(content);

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(96.))
            .bg(theme.background.opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                    cx.emit(FileSearchEvent::Dismiss);
                }),
            )
            .child(div().occlude().child(card))
    }
}
