//! The Scripts tab: the request's pre-request and test scripts, saved next
//! to it as `<name>.prerequest.js` and `<name>.test.js`. Folder and
//! collection scripts (`_folder.*.js`, `_collection.*.js`) run around them;
//! the tab says how many there are.

use std::path::Path;

use apix_core::scripts::{self, Event};
use apix_core::{AssertOp, Assertion};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{IconName, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
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
    view: View,
    pre: Entity<EditorState>,
    test: Entity<EditorState>,
    /// Folder and collection scripts that also run, per event.
    around: (usize, usize),
    /// The request's checks without code (`[[assert]]`).
    asserts: Vec<AssertRow>,
    _subscriptions: Vec<Subscription>,
}

/// What the tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Script(Event),
    Asserts,
}

/// One editable check: target, operator, value.
struct AssertRow {
    target: Entity<InputState>,
    op: AssertOp,
    value: Entity<InputState>,
    enabled: bool,
    _changes: [Subscription; 2],
}

impl AssertRow {
    fn new(assertion: &Assertion, window: &mut Window, cx: &mut Context<ScriptEditor>) -> Self {
        let (target, value) = (assertion.target.clone(), assertion.value.clone());
        let target = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("status · body.users[0].id · header.Content-Type")
                .default_value(target)
        });
        let value = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr!("table.value"))
                .default_value(value)
        });
        // Typing changes the unsaved marker of the request.
        let notify = |cx: &mut Context<ScriptEditor>, input: &Entity<InputState>| {
            cx.subscribe(input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
        };
        let changes = [notify(cx, &target), notify(cx, &value)];
        Self {
            target,
            op: assertion.op,
            value,
            enabled: assertion.enabled,
            _changes: changes,
        }
    }

    fn to_assertion(&self, cx: &App) -> Option<Assertion> {
        let target = self.target.read(cx).value().trim().to_string();
        (!target.is_empty()).then(|| Assertion {
            target,
            op: self.op,
            value: self.value.read(cx).value().to_string(),
            enabled: self.enabled,
        })
    }
}

impl EventEmitter<SendRequested> for ScriptEditor {}

impl ScriptEditor {
    /// Shows the request's checks without code.
    pub(super) fn set_asserts(
        &mut self,
        asserts: &[Assertion],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.asserts = asserts
            .iter()
            .map(|a| AssertRow::new(a, window, cx))
            .collect();
        cx.notify();
    }

    /// The checks as they are in the table; rows without a target are left out.
    pub(super) fn asserts(&self, cx: &App) -> Vec<Assertion> {
        self.asserts
            .iter()
            .filter_map(|row| row.to_assertion(cx))
            .collect()
    }

    fn render_asserts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.asserts.iter().enumerate().map(|(ix, row)| {
            let op = row.op;
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    Checkbox::new(("assert-on", ix))
                        .checked(row.enabled)
                        .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                            if let Some(row) = this.asserts.get_mut(ix) {
                                row.enabled = *checked;
                            }
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(&row.target).small()),
                )
                .child(
                    Button::new(("assert-op", ix))
                        .small()
                        .outline()
                        .w(px(104.))
                        .label(op.as_str())
                        .dropdown_menu({
                            let view = cx.entity().downgrade();
                            move |mut menu, _, _| {
                                for choice in AssertOp::ALL {
                                    let view = view.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(choice.as_str())
                                            .checked(choice == op)
                                            .on_click(move |_, _, cx| {
                                                view.update(cx, |this, cx| {
                                                    if let Some(row) = this.asserts.get_mut(ix) {
                                                        row.op = choice;
                                                    }
                                                    cx.notify();
                                                })
                                                .ok();
                                            }),
                                    );
                                }
                                menu
                            }
                        }),
                )
                .child(div().flex_1().min_w_0().when(op.takes_value(), |d| {
                    d.child(Input::new(&row.value).small())
                }))
                .child(
                    Button::new(("assert-del", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if ix < this.asserts.len() {
                                this.asserts.remove(ix);
                            }
                            cx.notify();
                        })),
                )
        });
        v_flex()
            .id("assert-rows")
            .gap_1()
            .size_full()
            .overflow_y_scroll()
            .children(rows)
            .child(
                h_flex().pt_1().child(
                    Button::new("assert-add")
                        .ghost()
                        .small()
                        .icon(IconName::Plus)
                        .label(tr!("table.add"))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            let row = AssertRow::new(
                                &Assertion::new("status", AssertOp::Eq, "200"),
                                window,
                                cx,
                            );
                            this.asserts.push(row);
                            cx.notify();
                        })),
                ),
            )
    }

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
            view: View::Script(Event::PreRequest),
            pre,
            test,
            around: (0, 0),
            asserts: Vec::new(),
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
        // Open where there is something to read (checks are set just before).
        self.view = if !scripts.pre.trim().is_empty() {
            View::Script(Event::PreRequest)
        } else if !scripts.test.trim().is_empty() {
            View::Script(Event::Test)
        } else if !self.asserts.is_empty() {
            View::Asserts
        } else {
            View::Script(Event::PreRequest)
        };
        cx.notify();
    }

    /// Adds a line at the end of the Tests script and shows it.
    pub(super) fn append_test(&mut self, line: &str, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.test.read(cx).value().to_string();
        let text = if current.trim().is_empty() {
            format!("{line}\n")
        } else {
            format!("{}\n{line}\n", current.trim_end())
        };
        self.test
            .update(cx, |state, cx| state.set_value(text, window, cx));
        self.view = View::Script(Event::Test);
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
        let checks = self.asserts.len();
        bidi::fill_tabs(
            TabBar::new("script-event").segmented().small(),
            Some(match self.view {
                View::Script(Event::PreRequest) => 0,
                View::Script(Event::Test) => 1,
                View::Asserts => 2,
            }),
            vec![
                Tab::new().label(label(tr!("scripts.pre"), !scripts.pre.trim().is_empty())),
                Tab::new().label(label(tr!("scripts.test"), !scripts.test.trim().is_empty())),
                Tab::new().label(if checks == 0 {
                    tr!("scripts.asserts")
                } else {
                    tr!("scripts.asserts_count", count = checks)
                }),
            ],
            cx.listener(|this, ix: &usize, _, cx| {
                this.view = match *ix {
                    0 => View::Script(Event::PreRequest),
                    1 => View::Script(Event::Test),
                    _ => View::Asserts,
                };
                cx.notify();
            }),
        )
    }
}

impl Render for ScriptEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let (content, note) = match self.view {
            View::Asserts => (
                self.render_asserts(cx).into_any_element(),
                tr!("scripts.asserts_hint"),
            ),
            View::Script(event) => {
                let (editor, hint, around) = match event {
                    Event::PreRequest => (&self.pre, tr!("scripts.pre_hint"), self.around.0),
                    Event::Test => (&self.test, tr!("scripts.test_hint"), self.around.1),
                };
                let note = if around > 0 {
                    tr!("scripts.around", hint = hint, count = around)
                } else {
                    hint
                };
                (Editor::new(editor).h_full().into_any_element(), note)
            }
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
            .child(div().flex_1().min_h_0().child(content))
    }
}
