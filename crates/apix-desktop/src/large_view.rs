//! A read-only viewer for response bodies of any size.
//!
//! The scroll position is kept in rows, not pixels. A 100 MB body has around
//! ten million rows, which at 20 px each is far beyond what `f32` pixel
//! coordinates can address exactly, so a pixel-based list would drift and
//! jitter near the end. Here every coordinate stays within one screen: only
//! the rows on screen are laid out, and the scrollbar maps rows to the track.
//! The text and its row index are built once, off the UI thread.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use apix_core::text::{RowIndex, json_tokens};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use crate::theme::{self, Palette};

/// Bodies at least this large use this viewer instead of the code editor,
/// which is comfortable up to a few tens of thousands of lines.
pub const LARGE_BODY_BYTES: usize = 512 * 1024;

/// Longer lines (minified JSON is often one line) are split into rows of
/// this many bytes, so every row stays cheap to shape.
const MAX_ROW_BYTES: usize = 2048;

const ROW_HEIGHT: f32 = 20.;
/// Advance of one JetBrains Mono glyph at 13 px, for horizontal scrolling.
const CHAR_WIDTH: f32 = 7.8;
/// Rows laid out per frame: enough for a 4K screen, clipped to the viewport.
const RENDER_ROWS: usize = 120;
const MIN_THUMB: f32 = 28.;
const SCROLLBAR_WIDTH: f32 = 12.;

actions!(
    large_view,
    [LineUp, LineDown, PageUp, PageDown, Top, Bottom]
);

const CONTEXT: &str = "LargeView";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", LineUp, Some(CONTEXT)),
        KeyBinding::new("down", LineDown, Some(CONTEXT)),
        KeyBinding::new("pageup", PageUp, Some(CONTEXT)),
        KeyBinding::new("pagedown", PageDown, Some(CONTEXT)),
        KeyBinding::new("secondary-home", Top, Some(CONTEXT)),
        KeyBinding::new("secondary-end", Bottom, Some(CONTEXT)),
        KeyBinding::new("secondary-up", Top, Some(CONTEXT)),
        KeyBinding::new("secondary-down", Bottom, Some(CONTEXT)),
    ]);
}

/// Text prepared for viewing. Building it is linear in the text size.
pub struct Document {
    text: String,
    rows: RowIndex,
    json: bool,
    widest_chars: usize,
}

impl Document {
    pub fn new(text: String, json: bool) -> Self {
        let rows = RowIndex::new(&text, MAX_ROW_BYTES);
        let widest_chars = if rows.is_empty() {
            0
        } else {
            rows.row(&text, rows.widest()).0.chars().count()
        };
        Self {
            text,
            rows,
            json,
            widest_chars,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn lines(&self) -> u32 {
        self.rows.lines()
    }
}

struct Drag {
    start_y: f32,
    start_top: usize,
}

pub struct LargeView {
    doc: Arc<Document>,
    palette: Palette,
    focus: FocusHandle,
    /// First row on screen.
    top: usize,
    /// Wheel movement smaller than a row, carried to the next event.
    wheel_rest: f32,
    /// Horizontal offset in pixels.
    left: f32,
    /// Where the viewer was painted last frame.
    viewport: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<Drag>,
}

impl LargeView {
    pub fn new(doc: Arc<Document>, palette: Palette, cx: &mut Context<Self>) -> Self {
        Self {
            doc,
            palette,
            focus: cx.focus_handle(),
            top: 0,
            wheel_rest: 0.,
            left: 0.,
            viewport: Rc::new(Cell::new(Bounds::default())),
            drag: None,
        }
    }

    fn total(&self) -> usize {
        self.doc.rows.len()
    }

    fn visible_rows(&self) -> usize {
        let height = f32::from(self.viewport.get().size.height);
        ((height / ROW_HEIGHT).floor() as usize).max(1)
    }

    fn max_top(&self) -> usize {
        self.total().saturating_sub(self.visible_rows())
    }

    fn gutter_width(&self) -> f32 {
        self.doc.lines().to_string().len() as f32 * 8. + 20.
    }

    fn max_left(&self) -> f32 {
        let content = self.gutter_width() + self.doc.widest_chars as f32 * CHAR_WIDTH + 24.;
        (content - f32::from(self.viewport.get().size.width)).max(0.)
    }

    fn scroll_to(&mut self, top: usize, cx: &mut Context<Self>) {
        let top = top.min(self.max_top());
        if top != self.top {
            self.top = top;
            cx.notify();
        }
    }

    fn scroll_by(&mut self, rows: isize, cx: &mut Context<Self>) {
        let top = self.top.saturating_add_signed(rows);
        self.scroll_to(top, cx);
    }

    fn on_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(ROW_HEIGHT));
        let (mut dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        if event.modifiers.shift && dx == 0. {
            dx = dy;
        } else {
            self.wheel_rest -= dy;
            let rows = (self.wheel_rest / ROW_HEIGHT).trunc();
            self.wheel_rest -= rows * ROW_HEIGHT;
            if rows != 0. {
                self.scroll_by(rows as isize, cx);
            }
        }
        if dx != 0. {
            self.left = (self.left - dx).clamp(0., self.max_left());
            cx.notify();
        }
        cx.stop_propagation();
    }

    /// Thumb height and travel of the vertical scrollbar, in pixels.
    fn thumb_metrics(&self) -> (f32, f32) {
        let height = f32::from(self.viewport.get().size.height);
        let ratio = self.visible_rows() as f32 / self.total().max(1) as f32;
        let thumb = (height * ratio).clamp(MIN_THUMB.min(height), height);
        (thumb, (height - thumb).max(0.))
    }

    fn thumb_top(&self) -> f32 {
        let (_, travel) = self.thumb_metrics();
        let max_top = self.max_top();
        if max_top == 0 {
            0.
        } else {
            travel * (self.top as f64 / max_top as f64) as f32
        }
    }

    fn top_for_thumb(&self, thumb_top: f32) -> usize {
        let (_, travel) = self.thumb_metrics();
        if travel <= 0. {
            return 0;
        }
        let fraction = (thumb_top / travel).clamp(0., 1.) as f64;
        (fraction * self.max_top() as f64).round() as usize
    }

    fn on_track_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let y = f32::from(event.position.y - self.viewport.get().origin.y);
        let (thumb, _) = self.thumb_metrics();
        let thumb_top = self.thumb_top();
        if y < thumb_top || y > thumb_top + thumb {
            // Clicking the track centres the thumb on the click, then keeps dragging.
            let top = self.top_for_thumb(y - thumb / 2.);
            self.scroll_to(top, cx);
        }
        self.drag = Some(Drag {
            start_y: y,
            start_top: self.top,
        });
        cx.stop_propagation();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = &self.drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let y = f32::from(event.position.y - self.viewport.get().origin.y);
        let start_thumb = {
            let max_top = self.max_top().max(1);
            let (_, travel) = self.thumb_metrics();
            travel * (drag.start_top as f64 / max_top as f64) as f32
        };
        let top = self.top_for_thumb(start_thumb + (y - drag.start_y));
        self.scroll_to(top, cx);
    }

    fn row(&self, ix: usize, gutter: f32) -> Div {
        let p = self.palette;
        let (text, line, continuation) = self.doc.rows.row(&self.doc.text, ix);
        let number: SharedString = if continuation {
            SharedString::default()
        } else {
            line.to_string().into()
        };
        let mut styled = StyledText::new(SharedString::from(text.to_owned()));
        if self.doc.json {
            styled = styled.with_highlights(json_tokens(text).into_iter().map(|(range, token)| {
                let style = HighlightStyle {
                    color: Some(theme::json_token_color(&p, token)),
                    ..Default::default()
                };
                (range, style)
            }));
        }

        h_flex()
            .h(px(ROW_HEIGHT))
            .child(
                div()
                    .w(px(gutter))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .pr_3()
                    .text_color(p.text_muted.opacity(0.6))
                    .child(number),
            )
            .child(div().whitespace_nowrap().child(styled))
    }

    fn render_scrollbar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.max_top() == 0 {
            return None;
        }
        let p = self.palette;
        let (thumb, _) = self.thumb_metrics();
        let dragging = self.drag.is_some();
        Some(
            div()
                .id("large-view-scrollbar")
                .absolute()
                .top_0()
                .right_0()
                .h_full()
                .w(px(SCROLLBAR_WIDTH))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_track_down))
                .child(
                    div()
                        .absolute()
                        .top(px(self.thumb_top()))
                        .right(px(2.))
                        .w(px(6.))
                        .h(px(thumb))
                        .rounded_full()
                        .bg(p.text_muted.opacity(if dragging { 0.7 } else { 0.4 })),
                ),
        )
    }
}

impl Focusable for LargeView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for LargeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The viewport may have changed since the last event (window resize).
        self.top = self.top.min(self.max_top());
        self.left = self.left.min(self.max_left());

        let gutter = self.gutter_width();
        let start = self.top;
        let end = (start + RENDER_ROWS).min(self.total());
        let viewport = self.viewport.clone();

        div()
            .id("large-view")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .overflow_hidden()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(13.))
            .line_height(px(ROW_HEIGHT))
            .on_action(cx.listener(|this, _: &LineUp, _, cx| this.scroll_by(-1, cx)))
            .on_action(cx.listener(|this, _: &LineDown, _, cx| this.scroll_by(1, cx)))
            .on_action(cx.listener(|this, _: &PageUp, _, cx| {
                this.scroll_by(-(this.visible_rows() as isize), cx)
            }))
            .on_action(cx.listener(|this, _: &PageDown, _, cx| {
                this.scroll_by(this.visible_rows() as isize, cx)
            }))
            .on_action(cx.listener(|this, _: &Top, _, cx| this.scroll_to(0, cx)))
            .on_action(cx.listener(|this, _: &Bottom, _, cx| this.scroll_to(usize::MAX, cx)))
            .on_scroll_wheel(cx.listener(Self::on_wheel))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| this.focus.focus(window, cx)),
            )
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.drag.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| this.drag = None),
            )
            .child(
                // Records the viewport size; a change triggers one more frame
                // so the row count and scrollbar use the real size.
                canvas(
                    move |bounds, window, _| {
                        if viewport.get() != bounds {
                            viewport.set(bounds);
                            window.refresh();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                v_flex()
                    .absolute()
                    .top_0()
                    .left(px(-self.left))
                    .children((start..end).map(|ix| self.row(ix, gutter))),
            )
            .children(self.render_scrollbar(cx))
    }
}
