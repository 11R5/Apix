//! Wrapped text that reads right to left.
//!
//! gpui wraps text by shaping the whole paragraph as one line and cutting it
//! at glyph x positions, which it assumes grow from left to right. Right-to-left
//! runs place their glyphs leftwards, so a wrapped Arabic paragraph came out as
//! a fragment on the first line and an overflowing second line. This element
//! breaks the text into lines itself, in reading order at spaces, measures each
//! candidate line on its own and paints every line as a separate unwrapped line.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::*;

/// The lines of the last measure, painted as they are.
#[derive(Default)]
pub struct Lines {
    lines: Vec<ShapedLine>,
    line_height: Pixels,
}

pub struct Paragraph {
    text: SharedString,
}

impl Paragraph {
    pub fn new(text: SharedString) -> Self {
        Self { text }
    }
}

impl IntoElement for Paragraph {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Paragraph {
    type RequestLayoutState = Rc<RefCell<Lines>>;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            style
                .line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );
        let wraps = style.white_space == WhiteSpace::Normal;
        let run = style.to_run(0);
        let state = Rc::new(RefCell::new(Lines::default()));
        let layout_id = window.request_measured_layout(Default::default(), {
            let (state, text) = (state.clone(), self.text.clone());
            move |known, available, window, _| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) if wraps => Some(width),
                    _ => None,
                });
                let shape = |line: &str| {
                    let run = TextRun {
                        len: line.len(),
                        ..run.clone()
                    };
                    window.text_system().shape_line(
                        SharedString::from(line.to_owned()),
                        font_size,
                        &[run],
                        None,
                    )
                };
                let lines: Vec<ShapedLine> = break_lines(&text, width, |line| shape(line).width)
                    .iter()
                    .map(|line| shape(line))
                    .collect();
                let widest = lines
                    .iter()
                    .map(|l| l.width)
                    .fold(Pixels::ZERO, Pixels::max);
                let size = size(widest, line_height * lines.len().max(1) as f32);
                *state.borrow_mut() = Lines { lines, line_height };
                size
            }
        });
        (layout_id, state)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let align = window.text_style().text_align;
        let state = state.borrow();
        for (ix, line) in state.lines.iter().enumerate() {
            let origin = point(
                bounds.origin.x,
                bounds.origin.y + state.line_height * ix as f32,
            );
            line.paint(
                origin,
                state.line_height,
                align,
                Some(bounds.size.width),
                window,
                cx,
            )
            .ok();
        }
    }
}

/// Splits `text` into lines no wider than `width` (as `measure` says), at
/// spaces and in reading order. A word wider than `width` gets a line of its own.
fn break_lines(text: &str, width: Option<Pixels>, measure: impl Fn(&str) -> Pixels) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let Some(width) = width else {
            lines.push(paragraph.to_owned());
            continue;
        };
        let mut line = String::new();
        for word in paragraph.split(' ').filter(|w| !w.is_empty()) {
            if line.is_empty() {
                line.push_str(word);
                continue;
            }
            let candidate = format!("{line} {word}");
            if measure(&candidate) <= width {
                line = candidate;
            } else {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            }
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Pixels, px};

    use super::break_lines;

    /// Ten pixels per character.
    fn measure(text: &str) -> Pixels {
        px(text.chars().count() as f32 * 10.)
    }

    #[test]
    fn breaks_at_spaces_in_reading_order() {
        let lines = break_lines("واحد اثنان ثلاثة أربعة", Some(px(110.)), measure);
        assert_eq!(lines, ["واحد اثنان", "ثلاثة أربعة"]);
    }

    #[test]
    fn keeps_long_words_and_newlines() {
        let lines = break_lines("abcdefghijkl x\ny", Some(px(50.)), measure);
        assert_eq!(lines, ["abcdefghijkl", "x", "y"]);
        assert_eq!(break_lines("a b", None, measure), ["a b"]);
    }
}
