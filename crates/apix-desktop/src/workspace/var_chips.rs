//! `{{variable}}` chips in the URL bar and the key/value fields of Params,
//! Headers and form bodies. Every reference is one atomic token,
//! coloured by how it resolves: defined, dynamic (`{{$guid}}`) or missing.
//! Hovering a chip shows its value and the scope it comes from; secrets stay
//! masked.

use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use apix_core::dynamic::DYNAMIC_VARIABLES;
use gpui_kit::component::input::{InlineToken, Input, InputContent, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Sizable, v_flex};
use gpui_kit::*;

use super::{Workspace, key_hint};
use crate::bidi;
use crate::tr;

/// The chip under the pointer, with its bounds in window coordinates.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct HoveredVar {
    name: String,
    bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Defined,
    Dynamic,
    Missing,
}

/// Every `{{name}}` in `text`: its byte range and the trimmed name.
fn references(text: &str) -> Vec<(Range<usize>, String)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(start) = text[from..].find("{{").map(|i| from + i) {
        let after = start + 2;
        let Some(len) = text[after..].find("}}") else {
            break;
        };
        let inner = &text[after..after + len];
        let name = inner.trim();
        // `{{{{x}}` or a stray `{{` before the real one: retry from the next brace.
        if name.is_empty() || inner.contains('{') {
            from = start + 1;
            continue;
        }
        out.push((start..after + len + 2, name.to_owned()));
        from = after + len + 2;
    }
    out
}

fn token(name: &str, text: &str) -> InlineToken {
    InlineToken::new(name.to_owned(), text.to_owned()).with_label(name.to_owned())
}

/// Turns references typed or pasted into `state` into chips. Called on every
/// change. `last` is the text chipped last time: right after an undo of a
/// chip the text is exactly that, so it stays text, and undoing a chip works.
pub(super) fn chip_state(
    state: &Entity<InputState>,
    last: &mut Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let current = state.read(cx);
    let text = current.value().to_string();
    if last.as_deref() == Some(text.as_str()) {
        return;
    }
    *last = None;
    let taken: Vec<Range<usize>> = current.tokens().iter().map(|s| s.range()).collect();
    let fresh: Vec<(Range<usize>, String)> = references(&text)
        .into_iter()
        .filter(|(range, _)| {
            !taken
                .iter()
                .any(|t| t.start < range.end && range.start < t.end)
        })
        .collect();
    if fresh.is_empty() {
        return;
    }
    *last = Some(text.clone());
    state.update(cx, |state, cx| {
        // From the end, so earlier ranges stay valid.
        for (range, name) in fresh.into_iter().rev() {
            let piece = text[range.clone()].to_owned();
            let _ = state.replace_range_with_token(range, token(&name, &piece), window, cx);
        }
    });
}

/// An input that starts with `text`, references as chips, and keeps turning
/// new references into chips as they are typed. Keep the subscription.
pub(super) fn chipped_input(
    text: &str,
    placeholder: SharedString,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<InputState>, Subscription) {
    let content = chip_content(text);
    let state = cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder(placeholder);
        state.set_value(content, window, cx);
        state
    });
    let mut last = None;
    let subscription =
        window.subscribe(&state, cx, move |state, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Change) {
                chip_state(&state, &mut last, window, cx);
            }
        });
    (state, subscription)
}

/// `text` with every reference as a chip, for `InputState::set_value`.
pub(super) fn chip_content(text: &str) -> InputContent {
    references(text).into_iter().fold(
        InputContent::new(text.to_owned()),
        |content, (range, name)| {
            let piece = text[range.clone()].to_owned();
            match content.clone().with_token(range, token(&name, &piece)) {
                Ok(next) => next,
                Err(_) => content,
            }
        },
    )
}

impl Workspace {
    fn kind(&self, name: &str) -> Kind {
        if self.vars.get(name).is_some() {
            Kind::Defined
        } else if name
            .strip_prefix('$')
            .is_some_and(|n| DYNAMIC_VARIABLES.iter().any(|(d, _)| *d == n))
        {
            Kind::Dynamic
        } else {
            Kind::Missing
        }
    }

    fn chip_color(&self, kind: Kind) -> Hsla {
        match kind {
            Kind::Defined => crate::theme::current().accent,
            Kind::Dynamic => crate::theme::current().keyword,
            Kind::Missing => crate::theme::current().err,
        }
    }

    /// Turns references typed or pasted into the URL into chips.
    pub(super) fn chip_url(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url.clone();
        chip_state(&url, &mut self.last_chipped, window, cx);
    }

    /// The URL field with chips and the hover card hook.
    pub(super) fn url_input(&self, cx: &mut Context<Self>) -> Input {
        self.chip_input(&self.url, cx.entity().downgrade(), cx)
    }

    /// `state` as an input whose chips are coloured by how they resolve and
    /// show the hover card of `view`.
    pub(super) fn chip_input(
        &self,
        state: &Entity<InputState>,
        view: WeakEntity<Workspace>,
        cx: &App,
    ) -> Input {
        let colors: Rc<HashMap<String, Hsla>> = Rc::new(
            state
                .read(cx)
                .tokens()
                .iter()
                .map(|span| {
                    let name = span.token().id().to_string();
                    let color = self.chip_color(self.kind(&name));
                    (name, color)
                })
                .collect(),
        );
        Input::new(state)
            .small()
            .token(move |chip, _, cx| {
                let color = colors
                    .get(chip.token().id().as_ref())
                    .copied()
                    .unwrap_or(cx.theme().muted_foreground);
                div()
                    .flex()
                    .items_center()
                    .h(chip.line_height())
                    .max_w(chip.available_width())
                    .px(px(5.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(if chip.is_selected() {
                        color
                    } else {
                        color.opacity(0.35)
                    })
                    .bg(color.opacity(if chip.is_selected() { 0.30 } else { 0.14 }))
                    .text_color(color)
                    .child(
                        div()
                            .min_w_0()
                            .text_ellipsis()
                            .child(chip.token().label().clone()),
                    )
            })
            .on_token_hover(move |event, _, cx| {
                let hovered = HoveredVar {
                    name: event.token().id().to_string(),
                    bounds: event.bounds(),
                };
                let entered = event.is_hovered();
                view.update(cx, |this, cx| {
                    if entered {
                        this.hovered_var = Some(hovered);
                    } else if this
                        .hovered_var
                        .as_ref()
                        .is_some_and(|h| h.name == hovered.name)
                    {
                        this.hovered_var = None;
                    }
                    cx.notify();
                })
                .ok();
            })
    }

    /// The card under a hovered chip: value, scope, or why it is missing.
    pub(super) fn render_var_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let hovered = self.hovered_var.as_ref()?;
        let p = crate::theme::current();
        let name = hovered.name.as_str();
        let kind = self.kind(name);
        let mono = cx.theme().mono_font_family.clone();
        let line = |text: SharedString, color: Hsla| {
            div().text_xs().text_color(color).child(bidi::para(text))
        };

        let mut card = v_flex()
            .gap_1()
            .max_w(px(420.))
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(p.hairline)
            .bg(p.surface_raised)
            .shadow_lg()
            .child(
                div()
                    .text_sm()
                    .font_family(mono.clone())
                    .text_color(self.chip_color(kind))
                    .child(format!("{{{{{name}}}}}")),
            );
        match kind {
            Kind::Defined => {
                let raw = self.vars.get(name).unwrap_or_default();
                let origin = self.vars.origin(name);
                let secret = origin.is_some_and(|o| o.secret);
                let mut missing = Default::default();
                let resolved = self.vars.interpolate(raw, &mut missing);
                let shown = if secret {
                    "•".repeat(resolved.chars().count().clamp(6, 24))
                } else if resolved.is_empty() {
                    tr!("chips.empty").into()
                } else {
                    resolved.clone()
                };
                card = card.child(
                    div()
                        .text_sm()
                        .font_family(mono)
                        .text_color(p.text)
                        .child(shown),
                );
                if raw != resolved && !secret {
                    card = card.child(line(format!("= {raw}").into(), p.text_muted));
                }
                let scope = origin.map_or(tr!("chips.set_for_run"), |o| scope_label(&o.scope));
                let scope = if secret {
                    tr!("chips.secret_scope", scope = scope)
                } else {
                    scope
                };
                card = card.child(line(scope, p.text_muted));
            }
            Kind::Dynamic => {
                let about = DYNAMIC_VARIABLES
                    .iter()
                    .find(|(d, _)| Some(*d) == name.strip_prefix('$'))
                    .map_or("", |(_, about)| *about);
                card = card
                    .child(line(about.into(), p.text))
                    .child(line(tr!("chips.dynamic"), p.text_muted));
            }
            Kind::Missing => {
                let env = match &self.environment {
                    Some(env) => tr!("chips.missing_in_env", env = env),
                    None => tr!("chips.missing_no_env"),
                };
                card = card.child(line(env, p.err)).child(line(
                    tr!("chips.add_with", key = key_hint("E")),
                    p.text_muted,
                ));
            }
        }
        let at = hovered.bounds.bottom_left() + point(px(0.), px(6.));
        Some(
            deferred(
                anchored()
                    .position(at)
                    .snap_to_window_with_margin(px(8.))
                    .child(card),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

/// Where a value comes from, in the interface language. apix-core names it
/// as `Project "P"`, `Collection "C"`, `Environment "E"` or
/// `Environment "E" of "C"`; anything else is shown as it is.
fn scope_label(scope: &str) -> SharedString {
    let quoted: Vec<&str> = scope.split('"').skip(1).step_by(2).collect();
    match (
        scope.split(' ').next().unwrap_or_default(),
        quoted.as_slice(),
    ) {
        (apix_core::scripts::SCRIPT_SCOPE, []) => tr!("chips.scope.script"),
        ("Project", [name]) => tr!("chips.scope.project", name = name),
        ("Collection", [name]) => tr!("chips.scope.collection", name = name),
        ("Environment", [name]) => tr!("chips.scope.environment", name = name),
        ("Environment", [name, collection]) => tr!(
            "chips.scope.environment_of",
            name = name,
            collection = collection
        ),
        _ => scope.to_owned().into(),
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's `test` macro would shadow the built-in `#[test]`.
    use super::{chip_content, references};

    #[test]
    fn finds_references() {
        let found = references("{{base}}/users/{{ id }}?x={{$guid}}&y={{}}&z={{open");
        let names: Vec<&str> = found.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, ["base", "id", "$guid"]);
        assert_eq!(found[1].0, 15..23);
        assert_eq!(references("{{{{x}}")[0].0, 2..7);
    }

    #[test]
    fn content_has_one_token_per_reference() {
        let content = chip_content("{{a}}-{{b}}");
        assert_eq!(content.tokens().len(), 2);
        assert_eq!(content.tokens()[1].range(), 6..11);
    }
}
