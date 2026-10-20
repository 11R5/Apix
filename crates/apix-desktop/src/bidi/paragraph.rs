//! Wrapped text that reads right to left.
//!
//! gpui wraps text by shaping the whole paragraph as one line and cutting it
//! at glyph x positions, which it assumes grow from left to right. Right-to-left
//! runs place their glyphs leftwards, so a wrapped Arabic paragraph came out as
//! a fragment on the first line and an overflowing second line. This element
//! breaks the text into lines itself, in reading order at spaces, measures each
//! candidate line on its own and paints every line as a separate unwrapped line,
//! each marked right to left (see `i18n::display`).

use std::rc::Rc;

use gpui_kit::*;

/// What laying out and painting the text needs, shared by both.
pub struct Shaper {
    /// Without direction marks; each line gets its own.
    text: String,
    run: TextRun,
    font_size: Pixels,
    line_height: Pixels,
    wraps: bool,
}

impl Shaper {
    fn shape(&self, line: &str, window: &Window) -> ShapedLine {
        let line = crate::i18n::display(line);
        let run = TextRun {
            len: line.len(),
            ..self.run.clone()
        };
        window
            .text_system()
            .shape_line(line, self.font_size, &[run], None)
    }

    /// The text broken into lines for `width` (`None`: no wrapping).
    fn lines(&self, width: Option<Pixels>, window: &Window) -> Vec<ShapedLine> {
        let width = width.filter(|_| self.wraps);
        break_lines(&self.text, width, |line| self.shape(line, window).width)
            .iter()
            .map(|line| self.shape(line, window))
            .collect()
    }
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
    type RequestLayoutState = Rc<Shaper>;
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
        let shaper = Rc::new(Shaper {
            text: crate::i18n::strip_marks(&self.text),
            run: style.to_run(0),
            font_size,
            line_height,
            wraps: style.white_space == WhiteSpace::Normal,
        });
        let layout_id = window.request_measured_layout(Default::default(), {
            let shaper = shaper.clone();
            move |known, available, window, _| {
                // At its narrowest every word has a line of its own, so a row
                // holding this text and buttons can still shrink to fit.
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    AvailableSpace::MinContent => Some(Pixels::ZERO),
                    AvailableSpace::MaxContent => None,
                });
                let lines = shaper.lines(width, window);
                let widest = lines
                    .iter()
                    .map(|l| l.width)
                    .fold(Pixels::ZERO, Pixels::max);
                size(widest, shaper.line_height * lines.len().max(1) as f32)
            }
        });
        (layout_id, shaper)
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
        shaper: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Broken again for the final width: the layout may have measured
        // other widths last. Shaping is cached for the frame.
        let align = window.text_style().text_align;
        let line_height = shaper.line_height;
        for (ix, line) in shaper
            .lines(Some(bounds.size.width + px(0.5)), window)
            .iter()
            .enumerate()
        {
            let origin = point(bounds.origin.x, bounds.origin.y + line_height * ix as f32);
            line.paint(
                origin,
                line_height,
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
