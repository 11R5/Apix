//! The Scripts tab: the request's pre-request and test scripts, saved next
//! to it as `<name>.prerequest.js` and `<name>.test.js`. Folder and
//! collection scripts (`_folder.*.js`, `_collection.*.js`) run around them;
//! the tab says how many there are.

use std::path::Path;

use apix_core::scripts::{self, Event};
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Sizable, h_flex, v_flex};
use gpui_kit::*;

use super::body_editor::SendRequested;
use crate::bidi::{self, Bidi as _};
use crate::theme::Palette;
use crate::tr;

/// The two scripts of a request, as text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Scripts {
    pub pre: String,
    pub test: String,
}

impl Scripts {
    /// The scripts saved next to the request at `path`.
    pub(super) fn read(path: &Path) -> Self {
        Self {
            pre: scripts::read_script(&scripts::request_script(path, Event::PreRequest)),
            test: scripts::read_script(&scripts::request_script(path, Event::Test)),
        }
    }

    /// Saves both next to the request at `path`; a blank one removes its file.
    pub(super) fn write(&self, path: &Path) -> std::io::Result<()> {
        scripts::write_script(&scripts::request_script(path, Event::PreRequest), &self.pre)?;
        scripts::write_script(&scripts::request_script(path, Event::Test), &self.test)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.pre.trim().is_empty() && self.test.trim().is_empty()
    }
}

pub(super) struct ScriptEditor {
    palette: Palette,
    event: Event,
    pre: Entity<EditorState>,
    test: Entity<EditorState>,
    /// Folder and collection scripts that also run, per event.
    around: (usize, usize),
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SendRequested> for ScriptEditor {}

impl ScriptEditor {
    pub(super) fn new(palette: Palette, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pre = cx.new(|cx| EditorState::new(window, cx).language("javascript"));
        let test = cx.new(|cx| EditorState::new(window, cx).language("javascript"));
        let subscriptions = [&pre, &test]
            .into_iter()
            .map(|editor| {
                cx.subscribe(editor, |_, _, event: &InputEvent, cx| match event {
                    InputEvent::PressEnter {
                        secondary: true, ..
                    } => cx.emit(SendRequested),
                    InputEvent::Change => cx.notify(),
                    _ => {}
                })
            })
            .collect();
        Self {
            palette,
            event: Event::PreRequest,
            pre,
            test,
            around: (0, 0),
            _subscriptions: subscriptions,
        }
    }

    /// Shows `scripts`; `around` counts the folder and collection scripts
    /// (pre-request, test) that run with them.
    pub(super) fn set_scripts(
        &mut self,
        scripts: &Scripts,
        around: (usize, usize),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (pre, test) = (scripts.pre.clone(), scripts.test.clone());
        self.pre
            .update(cx, |state, cx| state.set_value(pre, window, cx));
        self.test
            .update(cx, |state, cx| state.set_value(test, window, cx));
        self.around = around;
        // Open where there is something to read.
        if scripts.pre.trim().is_empty() && !scripts.test.trim().is_empty() {
            self.event = Event::Test;
        }
        cx.notify();
    }

    pub(super) fn scripts(&self, cx: &App) -> Scripts {
        Scripts {
            pre: self.pre.read(cx).value().to_string(),
            test: self.test.read(cx).value().to_string(),
        }
    }

    fn picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = |name: SharedString, has: bool| {
            if has {
                tr!("scripts.tab_set", name = name)
            } else {
                name
            }
        };
        let scripts = self.scripts(cx);
        bidi::fill_tabs(
            TabBar::new("script-event").segmented().small(),
            Some(match self.event {
                Event::PreRequest => 0,
                Event::Test => 1,
            }),
            vec![
                Tab::new().label(label(tr!("scripts.pre"), !scripts.pre.trim().is_empty())),
                Tab::new().label(label(tr!("scripts.test"), !scripts.test.trim().is_empty())),
            ],
            cx.listener(|this, ix: &usize, _, cx| {
                this.event = if *ix == 0 {
                    Event::PreRequest
                } else {
                    Event::Test
                };
                cx.notify();
            }),
        )
    }
}

impl Render for ScriptEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let (editor, hint, around) = match self.event {
            Event::PreRequest => (&self.pre, tr!("scripts.pre_hint"), self.around.0),
            Event::Test => (&self.test, tr!("scripts.test_hint"), self.around.1),
        };
        let note = if around > 0 {
            tr!("scripts.around", hint = hint, count = around)
        } else {
            hint
        };
        v_flex()
            .size_full()
            .gap_2()
            .child(h_flex().dir_row().w_full().min_w_0().child(self.picker(cx)))
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(note)),
            )
            .child(div().flex_1().min_h_0().child(Editor::new(editor).h_full()))
    }
}
