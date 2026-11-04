//! Header names, and the usual values of the header in a row, offered under
//! the row being typed in. Enter takes the first; a click takes any.

use std::time::Duration;

use apix_core::KeyValue;
use apix_core::headers;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::InputEvent;
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex};
use gpui_kit::*;

use super::{KvRow, Workspace, var_chips};
use crate::bidi::Bidi as _;

/// The Headers field being typed in: its row (by the key input), and
/// whether it is the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HeaderFocus {
    pub row: EntityId,
    pub value: bool,
}

const MOST: usize = 8;

impl Workspace {
    /// A Headers row that offers names and values while it is typed in.
    pub(super) fn new_header_row(
        &mut self,
        kv: &KeyValue,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> KvRow {
        let mut row = KvRow::new(kv, window, cx);
        let row_id = row.key.entity_id();
        for (input, value) in [(row.key.clone(), false), (row.value.clone(), true)] {
            let here = HeaderFocus { row: row_id, value };
            row._sync.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Focus => {
                        this.header_focus = Some(here);
                        this.header_blur = None;
                        cx.notify();
                    }
                    // Late, so a click on a suggestion still finds it.
                    InputEvent::Blur => {
                        this.header_blur = Some(cx.spawn_in(window, async move |this, cx| {
                            cx.background_executor()
                                .timer(Duration::from_millis(200))
                                .await;
                            this.update(cx, |this, cx| {
                                if this.header_focus == Some(here) {
                                    this.header_focus = None;
                                    cx.notify();
                                }
                            })
                            .ok();
                        }));
                    }
                    InputEvent::Change => cx.notify(),
                    InputEvent::PressEnter { .. } if this.header_focus == Some(here) => {
                        this.take_header_hint(0, window, cx);
                    }
                    _ => {}
                },
            ));
        }
        row
    }

    /// What to offer under the row being typed in, and where it is.
    pub(super) fn header_hints(&self, cx: &App) -> Option<(usize, bool, Vec<&'static str>)> {
        let focus = self.header_focus?;
        let ix = self
            .headers
            .iter()
            .position(|r| r.key.entity_id() == focus.row)?;
        let row = &self.headers[ix];
        let key = row.key.read(cx).value().to_string();
        let hints = if focus.value {
            let typed = row.value.read(cx).value().to_string();
            headers::suggest(headers::values_of(&key), &typed, true, MOST)
        } else {
            // A header already in the table is not offered twice.
            let taken: Vec<String> = self
                .headers
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != ix)
                .map(|(_, r)| r.key.read(cx).value().trim().to_ascii_lowercase())
                .collect();
            let free: Vec<&'static str> = headers::NAMES
                .iter()
                .copied()
                .filter(|n| !taken.contains(&n.to_ascii_lowercase()))
                .collect();
            headers::suggest(&free, &key, true, MOST)
        };
        (!hints.is_empty()).then_some((ix, focus.value, hints))
    }

    /// Puts the `choice`-th hint into its field; after a name, the value is next.
    fn take_header_hint(&mut self, choice: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((ix, value, hints)) = self.header_hints(cx) else {
            return;
        };
        let Some(text) = hints.get(choice) else {
            return;
        };
        let row = &self.headers[ix];
        let (target, next) = if value {
            (row.value.clone(), None)
        } else {
            (row.key.clone(), Some(row.value.clone()))
        };
        target.update(cx, |state, cx| {
            state.set_value(var_chips::chip_content(text), window, cx)
        });
        match next {
            Some(value) => value.update(cx, |state, cx| state.focus(window, cx)),
            None => self.header_focus = None,
        }
        cx.notify();
    }

    /// The suggestions under row `ix` of the Headers table, if it is the one typed in.
    pub(super) fn render_header_hints(
        &self,
        ix: usize,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (at, _, hints) = self.header_hints(cx)?;
        if at != ix {
            return None;
        }
        Some(
            h_flex()
                .flex_wrap()
                .gap_1()
                .ps(px(30.))
                .pb_1()
                .children(hints.into_iter().enumerate().map(|(n, hint)| {
                    Button::new(("header-hint", n))
                        .ghost()
                        .xsmall()
                        .label(hint)
                        .selected(n == 0)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.take_header_hint(n, window, cx)
                        }))
                }))
                .into_any_element(),
        )
    }
}
