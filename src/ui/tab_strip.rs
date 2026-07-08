//! The tab strip rendered into the title bar: one chip per tab (context icon,
//! label, close affordance), inline rename, drag-to-reorder, and the "+"
//! new-tab button. Split out of `app.rs` as an `impl Tty7App` block (the same
//! pattern `settings` uses) so the window-shell file stays focused on tab/pane
//! orchestration rather than chrome rendering.

use gpui::{
    AnyElement, App, Context, FontWeight, MouseButton, MouseDownEvent, SharedString, div, img,
    prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _, Size, h_flex};

use crate::core::config::Config;
use crate::daemon::protocol::ShellSpec;
use crate::ui::app::{Tab, Tty7App};
use crate::ui::file_icons::file_icon_path;
use crate::ui::hints::tab_badge_label;

use label::{icon_for_title, short_title};

mod chrome;
mod drag;
mod label;

use drag::DragTab;

impl Tty7App {
    /// The display label for a tab: the user-set name if present, otherwise the
    /// focused terminal's title (shortened), falling back to
    /// "Session N" when there's no title yet.
    pub(crate) fn tab_label(&self, tab: &Tab, index: usize, cx: &App) -> String {
        if let Some(name) = tab.name.as_ref() {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        let raw = tab.leaf_title(cx);
        let label = short_title(&raw);
        if label.trim().is_empty() {
            format!("Session {}", index + 1)
        } else {
            label
        }
    }

    fn tab_icon(&self, tab: &Tab, is_active: bool, cx: &mut Context<Self>) -> AnyElement {
        let icon_color = if is_active {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };
        if tab.is_settings() {
            return Icon::new(IconName::Settings)
                .size(px(15.))
                .text_color(icon_color)
                .into_any_element();
        }
        if let Some(preview) = tab.preview.as_ref() {
            return img(file_icon_path(preview.read(cx).path.as_path()))
                .size(px(15.))
                .flex_none()
                .into_any_element();
        }
        Icon::new(icon_for_title(&tab.leaf_title(cx)))
            .size(px(15.))
            .text_color(icon_color)
            .into_any_element()
    }

    pub(crate) fn tab_strip(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let active = self.active;
        // While the bare ⌘/Ctrl hold is armed (see `ui::hints`), each of the
        // first nine chips swaps its close affordance for a ⌘N badge — same
        // slot, so nothing shifts when the hints appear.
        let show_badges = self.mod_hint_badges;
        let mut strip = h_flex()
            .id("tab-strip")
            .items_center()
            .gap_1p5()
            .ml_2()
            .mt(px(2.))
            .flex_1()
            .flex_basis(px(0.))
            .min_w_0()
            .overflow_x_scroll()
            .track_scroll(&self.tab_scroll_handle);

        for (i, tab) in self.tabs.iter().enumerate() {
            let is_active = i == active;
            let label = self.tab_label(tab, i, cx);
            // A small leading glyph hinting the tab's context (dir / tool / settings).
            let icon = self.tab_icon(tab, is_active, cx);

            // Inline rename input for this tab, if it's the one being renamed.
            let rename_input = self
                .renaming
                .as_ref()
                .filter(|r| r.index == i)
                .map(|r| r.input.clone());
            // Clean label (no pane-count suffix) for the rename prefill / drag preview.
            let drag_label: SharedString = label.clone().into();

            // Either the editable input (while renaming) or the clickable,
            // draggable label.
            let label_region = match rename_input {
                Some(input) => div()
                    .id(("tab-rename", i))
                    .flex_1()
                    .min_w_0()
                    // Swallow mouse-downs (incl. double-click word-select inside
                    // the input) so they never reach the enclosing TitleBar and
                    // zoom/maximize the window.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(Input::new(&input).appearance(false))
                    .into_any_element(),
                None => div()
                    .id(("tab-label", i))
                    .flex_1()
                    .min_w_0()
                    // Ellipsis-truncate the label so a shrunken chip degrades
                    // gracefully instead of hard-clipping mid-glyph.
                    .truncate()
                    .text_sm()
                    // Active tab carries a hair more weight so hierarchy reads
                    // from the type, not from colour alone.
                    .when(is_active, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(label)
                    // Single click activates; double click starts a rename.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                            // Swallow the event so it never reaches the enclosing
                            // TitleBar, whose double-click handler would otherwise
                            // zoom/maximize the window on a rename double-click.
                            cx.stop_propagation();
                            if ev.click_count >= 2 {
                                this.start_rename(i, window, cx);
                            } else {
                                this.activate(i, window, cx);
                            }
                        }),
                    )
                    // Drag the tab by its label to reorder it.
                    .on_drag(
                        DragTab {
                            index: i,
                            label: drag_label.clone(),
                        },
                        |drag, _, _, cx| {
                            cx.stop_propagation();
                            cx.new(|_| drag.clone())
                        },
                    )
                    .into_any_element(),
            };

            let chip = h_flex()
                .id(("tab-chip", i))
                // The strip lives inside gpui-component's `TitleBar`, which marks
                // its whole area as `WindowControlArea::Drag`. On Windows that maps
                // to `HTCAPTION`, so unless an element on top registers a
                // mouse-blocking hitbox, the OS swallows clicks as window-drags and
                // our `on_mouse_down` never fires. `occlude()` makes the chip a
                // `BlockMouse` hitbox so hit-testing stops here (its label/close
                // children paint above it, so they still click through). No-op on
                // macOS, where titlebar dragging doesn't gate child hit-testing.
                .occlude()
                // A group so this chip's close affordance can reveal on hover
                // (progressive disclosure) without affecting sibling tabs.
                .group(SharedString::from(format!("tab-chip-{i}")))
                .items_center()
                .justify_between()
                .gap_1p5()
                .h(px(30.))
                // Size to content instead of a fixed width so a short label
                // ("~") doesn't claim as much room as a long one — but cap the
                // width and keep a generous floor, and let it shrink when the
                // strip gets crowded so chips stay inside the titlebar. The
                // floor is deliberately roomy (Safari-ish) so a chip reads as a
                // substantial tab rather than a cramped pill.
                .min_w(px(150.))
                .max_w(px(260.))
                .flex_shrink(1.)
                .pl_3()
                .pr_1p5()
                .rounded_lg()
                // Active tab: a soft lifted fill, no border — reads native
                // (Safari/Arc) rather than as a hard-edged box. Inactive: quiet
                // muted text with a barely-there fill on hover for feedback.
                .when(is_active, |s| {
                    s.bg(cx.theme().secondary).text_color(cx.theme().foreground)
                })
                .when(!is_active, |s| {
                    s.text_color(cx.theme().muted_foreground)
                        .hover(|s| s.bg(cx.theme().muted))
                })
                // Drop target: dropping a dragged tab here moves it to this slot.
                .drag_over::<DragTab>(|s, _, _, cx| s.bg(cx.theme().drag_border.opacity(0.2)))
                .on_drop(cx.listener(move |this, drag: &DragTab, _window, cx| {
                    this.move_tab(drag.index, i, cx);
                }))
                // A click anywhere on the chip activates the tab. Clicks on the
                // label or close button are handled by those children (which stop
                // propagation), so this fires for the rest — icon, padding, the
                // bare chip — making the whole tab a switch target, not just text.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.activate(i, window, cx);
                    }),
                )
                .child(div().flex_shrink_0().flex().child(icon))
                // Clickable / editable label region.
                .child(label_region)
                // Trailing slot: normally the close affordance — always shown on
                // the active tab; on the others it stays out of the way
                // (opacity 0) and fades in on chip hover, so a row of tabs reads
                // clean instead of three-icons-per-chip busy. Space is reserved
                // either way, so nothing shifts on hover. While the shortcut
                // hints are armed, the same slot shows the tab's ⌘N badge instead.
                .child(if show_badges && i < 9 {
                    // Bare digit, no keycap box — the hint blends into the chip
                    // rather than reading as another button. Sized to the exact
                    // 20px square of the close button it stands in for, so the
                    // swap can never change the chip's width (an ellipsized
                    // label would otherwise reflow and the strip would jitter).
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if is_active {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(tab_badge_label(i))
                        .into_any_element()
                } else {
                    div()
                        .flex_shrink_0()
                        .when(!is_active, |s| {
                            s.opacity(0.)
                                .group_hover(SharedString::from(format!("tab-chip-{i}")), |s| {
                                    s.opacity(1.)
                                })
                        })
                        .child(
                            Button::new(("tab-close", i))
                                .icon(IconName::Close)
                                .ghost()
                                .xsmall()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.close_tab(i, window, cx);
                                })),
                        )
                        .into_any_element()
                });

            strip = strip.child(chip);
        }

        // "+" new-tab button — click opens the shell picker. The default shell
        // leads the menu (so the common case is two quick clicks on the same
        // spot; ⌘T still opens a default tab in one), followed by every shell
        // discovered on this machine (`detected_shells`, probed at startup).
        // Built on gpui-component's `DropdownMenu`, which is only implemented
        // for `Button` — hence a ghost Button restyled to the title bar's 30px
        // tile rhythm (30px box, 15px glyph, soft corners) rather than the
        // hand-rolled tile the "+" used to be.
        let shells = self.detected_shells.clone();
        let default_name = crate::core::shells::default_shell_name(
            cx.global::<Config>()
                .shell
                .as_ref()
                .map(|s| s.program.as_str()),
        );
        let app = cx.entity().downgrade();
        strip = strip.child(
            // Same Windows titlebar note as the chips above: `occlude()` gives
            // the trigger a BlockMouse hitbox so the TitleBar's HTCAPTION drag
            // area doesn't swallow the click.
            div().occlude().flex_shrink_0().child(
                Button::new("tab-add")
                    .icon(Icon::new(IconName::Plus).size(px(15.)))
                    .ghost()
                    .xsmall()
                    .w(px(30.))
                    .h(px(30.))
                    .rounded_lg()
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut menu = menu.with_size(Size::Small).min_w(px(220.));
                        // Default first — what a bare "new tab" means today,
                        // named so the fallback is legible ("New Tab (zsh)").
                        let open_default = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new(format!("New Tab ({default_name})")).on_click(
                                move |_, window, cx| {
                                    if let Some(app) = open_default.upgrade() {
                                        app.update(cx, |this, cx| this.new_tab(window, cx));
                                    }
                                },
                            ),
                        );
                        if !shells.is_empty() {
                            menu = menu.separator();
                        }
                        for shell in &shells {
                            let spec = ShellSpec {
                                program: shell.program.clone(),
                                args: shell.args.clone(),
                            };
                            let open = app.clone();
                            menu = menu.item(PopupMenuItem::new(shell.label.clone()).on_click(
                                move |_, window, cx| {
                                    if let Some(app) = open.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.new_tab_with_shell(Some(spec.clone()), window, cx);
                                        });
                                    }
                                },
                            ));
                        }
                        menu
                    }),
            ),
        );

        strip
    }
}
