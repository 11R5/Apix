//! Right-to-left layout for Arabic.
//!
//! GPUI shapes Arabic text right to left, but it has no layout direction:
//! rows always run left to right and `pl` / `border_r` are physical sides.
//! Layout that should follow the reading direction goes through these
//! helpers instead. They read the interface language when a view renders, so
//! a language change in Settings mirrors the window on the next frame.
//!
//! Some things stay left to right in every language: the code and response
//! viewers, the URL bar, key–value tables and other text that holds code,
//! URLs or data. Inputs always lay out their text from the left.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{Dialog, DialogClose};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{IconName, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::i18n;

mod paragraph;

/// The interface reads right to left.
pub fn rtl() -> bool {
    i18n::current().rtl
}

/// Text that may wrap onto several lines: notes, hints, descriptions,
/// messages. gpui cannot wrap right-to-left text, so in RTL it is broken into
/// lines by [`paragraph::Paragraph`]; otherwise it is plain text.
pub fn para(text: impl Into<SharedString>) -> AnyElement {
    let text = text.into();
    if rtl() {
        paragraph::Paragraph::new(text).into_any_element()
    } else {
        text.into_any_element()
    }
}

/// Direction-aware styling. "Start" is the side reading starts from: left,
/// or right in an RTL language.
pub trait Bidi: Styled + Sized {
    /// Lays a row's children out from the reading side. `justify_start` /
    /// `justify_end` then mean the reading start and end too.
    fn dir_row(self) -> Self {
        if rtl() { self.flex_row_reverse() } else { self }
    }

    /// Lines of text start on the reading side. Inherited by children.
    fn text_start(self) -> Self {
        if rtl() {
            self.text_right()
        } else {
            self.text_left()
        }
    }

    /// Lines of text end on the reading side, e.g. numbers in a column.
    fn text_end(self) -> Self {
        if rtl() {
            self.text_left()
        } else {
            self.text_right()
        }
    }

    /// Left to right whatever the language: code, URLs, data.
    fn ltr(self) -> Self {
        self.text_left()
    }

    /// Padding on the reading-start side.
    fn ps(self, length: impl Into<DefiniteLength> + Clone) -> Self {
        if rtl() {
            self.pr(length)
        } else {
            self.pl(length)
        }
    }

    /// Padding on the reading-end side.
    fn pe(self, length: impl Into<DefiniteLength> + Clone) -> Self {
        if rtl() {
            self.pl(length)
        } else {
            self.pr(length)
        }
    }

    /// A border on the reading-end side, e.g. between the sidebar and the editor.
    fn border_e_1(self) -> Self {
        if rtl() {
            self.border_l_1()
        } else {
            self.border_r_1()
        }
    }
}

impl<E: Styled> Bidi for E {}

/// The chevron of a collapsed tree node: it points into the reading direction.
pub fn chevron_collapsed() -> IconName {
    if rtl() {
        IconName::ChevronLeft
    } else {
        IconName::ChevronRight
    }
}

/// Where a dropdown opens from its button: along the reading direction.
pub fn menu_anchor() -> Anchor {
    if rtl() {
        Anchor::TopRight
    } else {
        Anchor::TopLeft
    }
}

/// Fills a gpui-kit [`TabBar`] so its tabs run in the reading direction. The
/// bar lays its tabs out itself, so in RTL they are given in reverse and the
/// indices are mapped back: `selected` and `on_click` use the logical order.
/// The caller pushes the bar to the reading side of its row.
pub fn fill_tabs(
    bar: TabBar,
    selected: Option<usize>,
    tabs: Vec<Tab>,
    on_click: impl Fn(&usize, &mut Window, &mut App) + 'static,
) -> TabBar {
    if !rtl() {
        return bar
            .when_some(selected, |bar, ix| bar.selected_index(ix))
            .children(tabs)
            .on_click(on_click);
    }
    let count = tabs.len();
    let flip = move |ix: usize| count.saturating_sub(1).saturating_sub(ix);
    bar.when_some(selected, |bar, ix| bar.selected_index(flip(ix)))
        .children(tabs.into_iter().rev())
        .on_click(move |ix: &usize, window, cx| on_click(&flip(*ix), window, cx))
}

/// An underlined tab strip across its pane, tabs starting from the reading side.
pub fn tab_bar(
    id: &'static str,
    selected: usize,
    tabs: Vec<Tab>,
    on_click: impl Fn(&usize, &mut Window, &mut App) + 'static,
) -> TabBar {
    let bar = TabBar::new(id).underline();
    if !rtl() {
        return fill_tabs(bar, Some(selected), tabs, on_click);
    }
    // An empty tab that takes the free space pushes the tabs to the right.
    // (A bar only as wide as its tabs measures itself short and clips one.)
    // Shown at 0, the filler shifts the reversed tabs by one: logical `l` is shown at `count - l`.
    let count = tabs.len();
    bar.selected_index(count - selected.min(count))
        .child(Tab::new().flex_1().disabled(true))
        .children(tabs.into_iter().rev())
        .on_click(move |ix: &usize, window, cx| {
            if (1..=count).contains(ix) {
                on_click(&(count - ix), window, cx)
            }
        })
}

/// Direction for gpui-kit dialogs, which lay out left to right.
pub trait DialogBidi {
    /// The dialog's title, with text and the close button mirrored in RTL.
    /// gpui-kit pins its close button to the top right, so RTL draws its own
    /// on the left.
    fn bidi_title(self, title: impl Into<SharedString>) -> Self;

    /// The title of a dialog without a close button, mirrored in RTL.
    fn bidi_heading(self, title: impl Into<SharedString>) -> Self;
}

/// Dialogs paint their own background: with a see-through window, the one
/// gpui-kit puts under everything is transparent, and text behind a dialog
/// must not show through it.
fn solid(dialog: Dialog) -> Dialog {
    dialog.bg(crate::theme::current().surface_raised)
}

impl DialogBidi for Dialog {
    fn bidi_heading(self, title: impl Into<SharedString>) -> Self {
        solid(self)
            .text_start()
            .close_button(false)
            .title(title.into())
    }

    fn bidi_title(self, title: impl Into<SharedString>) -> Self {
        let title = title.into();
        let this = solid(self);
        if !rtl() {
            return this.title(title);
        }
        this.text_right().close_button(false).title(
            h_flex()
                .w_full()
                .flex_row_reverse()
                .items_center()
                .gap_2()
                .child(div().flex_1().min_w_0().child(title))
                .child(
                    div()
                        .flex_none()
                        .child(DialogClose::new().trigger(|button: Button| {
                            button.small().ghost().icon(IconName::Close)
                        })),
                ),
        )
    }
}
