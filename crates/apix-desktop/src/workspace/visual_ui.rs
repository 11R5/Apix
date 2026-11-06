//! The visualizer: a JSON response (or what the filter picked out) as a
//! table, or as a bar chart of one of its number columns. The records are
//! found by `apix_core::table`, the same as `apix send --table`.
//!
//! Postman draws `pm.visualizer.set` templates in a web view; Apix has no web
//! view, so its visualizer is built in and needs no template.

use std::cell::RefCell;
use std::sync::Arc;

use apix_core::table::{self, Table};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use super::filter_ui::FilterState;
use super::{Finished, Workspace};
use crate::bidi::{self, Bidi as _};
use crate::tr;

/// Bodies larger than this are not turned into a table (filter them first).
const MAX_BODY: usize = 8 * 1024 * 1024;
/// Rows drawn.
const MAX_ROWS: usize = 1000;
/// Bars drawn.
const MAX_BARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum VisualMode {
    #[default]
    Text,
    Table,
    Chart,
    /// The visualizer plugin at this place among the installed ones.
    Plugin(usize),
}

/// A visualizer plugin's view of a body: still running, or done.
#[derive(Clone)]
enum PluginView {
    Running,
    Done(Result<apix_plugins::View, String>),
}

/// The installed plugins that offer a view, with their labels.
fn visualizers() -> Vec<(String, String)> {
    apix_plugins::installed()
        .iter()
        .filter_map(|p| p.as_ref().ok())
        .filter_map(|p| {
            Some((
                p.manifest.name.clone(),
                p.manifest.visualize.as_ref()?.label.clone(),
            ))
        })
        .collect()
}

/// What the table was made from: the body's bytes, or the filter's values.
type Key = (usize, usize, usize);
/// A table, or why there is none.
type Made = Result<Arc<Table>, SharedString>;

#[derive(Default)]
pub(super) struct VisualState {
    pub(super) mode: VisualMode,
    /// The chart's columns, by index; `None` picks one.
    label: Option<usize>,
    value: Option<usize>,
    cache: RefCell<Option<(Key, Made)>>,
    /// The last plugin view: for which body and plugin.
    plugin: RefCell<Option<(Key, String, PluginView)>>,
}

impl Workspace {
    /// The table for the response in front, made once per body or filter result.
    fn visual_table(&self, done: &Finished) -> Result<Arc<Table>, SharedString> {
        let body = &done.response.body;
        let values = match &self.filter_state {
            FilterState::Shown { values, .. } => Some(values.clone()),
            _ => None,
        };
        let key: Key = (
            body.as_ptr() as usize,
            body.len(),
            values.as_ref().map_or(0, |v| Arc::as_ptr(v) as usize),
        );
        if let Some((cached, result)) = self.visual.cache.borrow().as_ref()
            && *cached == key
        {
            return result.clone();
        }
        let value = match values {
            Some(values) if values.len() == 1 => Ok(values[0].clone()),
            Some(values) => Ok(serde_json::Value::Array(values.to_vec())),
            None if body.len() > MAX_BODY => Err(tr!("visual.too_big")),
            None => serde_json::from_slice(body).map_err(|e| e.to_string().into()),
        };
        let result = value.and_then(|v| {
            table::from_json(&v)
                .map(Arc::new)
                .ok_or_else(|| tr!("visual.no_records"))
        });
        *self.visual.cache.borrow_mut() = Some((key, result.clone()));
        result
    }

    pub(super) fn set_visual(&mut self, mode: VisualMode, cx: &mut Context<Self>) {
        self.visual.mode = mode;
        cx.notify();
    }

    /// Text, Table, Chart: at the end of the filter row.
    pub(super) fn render_visual_switch(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.visual.mode;
        let button = |id: &'static str, label: SharedString, mode: VisualMode| {
            let b = Button::new(id).xsmall().label(label);
            let b = if current == mode {
                b.primary()
            } else {
                b.ghost()
            };
            b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_visual(mode, cx)))
        };
        h_flex()
            .dir_row()
            .gap_1()
            .flex_none()
            .child(button("visual-text", tr!("visual.text"), VisualMode::Text))
            .child(button(
                "visual-table",
                tr!("visual.table"),
                VisualMode::Table,
            ))
            .child(button(
                "visual-chart",
                tr!("visual.chart"),
                VisualMode::Chart,
            ))
            .children(
                visualizers()
                    .into_iter()
                    .enumerate()
                    .map(|(i, (_, label))| {
                        let b = Button::new(("visual-plugin", i)).xsmall().label(label);
                        let b = if current == VisualMode::Plugin(i) {
                            b.primary()
                        } else {
                            b.ghost()
                        };
                        b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.set_visual(VisualMode::Plugin(i), cx)
                        }))
                    }),
            )
    }

    /// The Pretty tab while the Table or Chart view is chosen.
    pub(super) fn render_visual(&self, done: &Finished, cx: &mut Context<Self>) -> AnyElement {
        let p = crate::theme::current();
        if let VisualMode::Plugin(i) = self.visual.mode {
            return self.render_plugin_view(i, done, cx);
        }
        let table = match self.visual_table(done) {
            Ok(table) => table,
            Err(why) => {
                return div()
                    .p_3()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(bidi::para(why))
                    .into_any_element();
            }
        };
        let header = div()
            .text_xs()
            .text_color(p.text_muted)
            .pb_1()
            .child(bidi::para(tr!(
                "visual.summary",
                rows = table.rows.len(),
                columns = table.columns.len() + table.more_columns,
                source = table.source.clone()
            )));
        match self.visual.mode {
            VisualMode::Chart => self.render_chart(&table, header, cx),
            _ => render_grid(&table, header),
        }
    }

    fn render_chart(&self, t: &Table, header: Div, cx: &mut Context<Self>) -> AnyElement {
        let p = crate::theme::current();
        let numbers = t.numeric_columns();
        if numbers.is_empty() {
            return v_flex()
                .child(header)
                .child(
                    div()
                        .text_sm()
                        .text_color(p.text_muted)
                        .child(bidi::para(tr!("visual.no_numbers"))),
                )
                .into_any_element();
        }
        let value = self
            .visual
            .value
            .filter(|v| numbers.contains(v))
            .unwrap_or(numbers[0]);
        let label = self
            .visual
            .label
            .filter(|l| *l < t.columns.len())
            .or_else(|| t.label_column())
            .unwrap_or(0);
        let view = cx.entity().downgrade();
        let picker = |id: &'static str, current: usize, choices: Vec<usize>, set_value: bool| {
            let view = view.clone();
            let names: Vec<(usize, String)> =
                choices.iter().map(|&c| (c, t.columns[c].clone())).collect();
            Button::new(id)
                .xsmall()
                .outline()
                .label(t.columns[current].clone())
                .dropdown_menu(move |menu, _, _| {
                    names.iter().fold(menu, |menu, (ix, name)| {
                        let (view, ix) = (view.clone(), *ix);
                        menu.item(
                            PopupMenuItem::new(name.clone())
                                .checked(ix == current)
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        if set_value {
                                            this.visual.value = Some(ix);
                                        } else {
                                            this.visual.label = Some(ix);
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                                }),
                        )
                    })
                })
        };
        let rows: Vec<(String, f64)> = t
            .rows
            .iter()
            .take(MAX_BARS)
            .map(|r| {
                (
                    table::cell_text(&r[label]),
                    table::cell_number(&r[value]).unwrap_or(0.0),
                )
            })
            .collect();
        let max = rows
            .iter()
            .map(|(_, v)| v.abs())
            .fold(0.0_f64, f64::max)
            .max(f64::MIN_POSITIVE);
        let bars = rows.into_iter().enumerate().map(|(i, (name, v))| {
            let colour = if v < 0.0 { p.err } else { p.accent };
            h_flex()
                .dir_row()
                .id(("bar", i))
                .gap_2()
                .items_center()
                .text_xs()
                .py(px(2.))
                .child(
                    div()
                        .w(px(180.))
                        .flex_none()
                        .truncate()
                        .text_color(p.text)
                        .child(name),
                )
                .child(
                    div().flex_1().h(px(12.)).child(
                        div()
                            .h_full()
                            .rounded_sm()
                            .bg(colour)
                            .w(relative(((v.abs() / max) as f32).max(0.003))),
                    ),
                )
                .child(
                    div()
                        .w(px(90.))
                        .flex_none()
                        .text_end()
                        .text_color(p.text_muted)
                        .child(format_number(v)),
                )
        });
        let all_columns: Vec<usize> = (0..t.columns.len()).collect();
        v_flex()
            .size_full()
            .child(header)
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .pb_2()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(tr!("visual.label"))
                    .child(picker("visual-label", label, all_columns, false))
                    .child(tr!("visual.value"))
                    .child(picker("visual-value", value, numbers, true)),
            )
            .child(
                div()
                    .id("visual-chart")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(bars),
            )
            .when(t.rows.len() > MAX_BARS, |d| {
                d.child(div().text_xs().text_color(p.text_muted).child(tr!(
                    "visual.first_rows",
                    shown = MAX_BARS,
                    total = t.rows.len()
                )))
            })
            .into_any_element()
    }
}

use gpui_kit::prelude::FluentBuilder as _;

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{v:.0}")
    } else {
        let text = format!("{v:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// The records as a grid: a header row and one row per record.
impl Workspace {
    /// The view of visualizer plugin `i`, worked out in the background once
    /// per body.
    fn render_plugin_view(&self, i: usize, done: &Finished, cx: &mut Context<Self>) -> AnyElement {
        let p = crate::theme::current();
        let muted = |text: SharedString| {
            div()
                .p_3()
                .text_sm()
                .text_color(p.text_muted)
                .child(bidi::para(text))
                .into_any_element()
        };
        let Some((name, _)) = visualizers().into_iter().nth(i) else {
            return muted(tr!("visual.plugin_gone"));
        };
        let body = done.response.body.clone();
        let key: Key = (body.as_ptr() as usize, body.len(), 0);
        let known = self
            .visual
            .plugin
            .borrow()
            .as_ref()
            .filter(|(k, n, _)| *k == key && *n == name)
            .map(|(_, _, view)| view.clone());
        let view = match known {
            Some(view) => view,
            None => {
                *self.visual.plugin.borrow_mut() = Some((key, name.clone(), PluginView::Running));
                let (status, headers) = (done.response.status, done.response.headers.clone());
                let (plugin_name, name) = (name.clone(), name.clone());
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_spawn(async move {
                            let plugin = apix_plugins::find(&plugin_name)
                                .ok_or_else(|| format!("{plugin_name} is not installed"))?;
                            plugin.visualize(status, &headers, &String::from_utf8_lossy(&body))
                        })
                        .await;
                    this.update(cx, |this, cx| {
                        if let Some((k, n, view)) = this.visual.plugin.borrow_mut().as_mut()
                            && *k == key
                            && *n == name
                        {
                            *view = PluginView::Done(result);
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
                PluginView::Running
            }
        };
        match view {
            PluginView::Running => muted(tr!("visual.plugin_running")),
            PluginView::Done(Err(e)) => div()
                .p_3()
                .text_sm()
                .text_color(p.err)
                .child(bidi::para(e))
                .into_any_element(),
            PluginView::Done(Ok(view)) => {
                let header = div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .pb_1()
                    .child(bidi::para(tr!(
                        "visual.plugin_by",
                        title = view.title.clone(),
                        plugin = name.clone()
                    )));
                if view.columns.is_empty() {
                    return v_flex()
                        .p_2()
                        .child(header)
                        .child(
                            div()
                                .text_sm()
                                .font_family(crate::fonts::MONO)
                                .child(view.text.clone()),
                        )
                        .into_any_element();
                }
                let table = Table {
                    columns: view.columns.clone(),
                    rows: view
                        .rows
                        .iter()
                        .map(|r| r.iter().cloned().map(serde_json::Value::String).collect())
                        .collect(),
                    source: "$".into(),
                    more_columns: 0,
                };
                render_grid(&table, header)
            }
        }
    }
}

fn render_grid(t: &Table, header: Div) -> AnyElement {
    let p = crate::theme::current();
    const WIDTH: f32 = 170.;
    let width = px(WIDTH * t.columns.len() as f32 + 16.);
    let cell = |text: String, head: bool| {
        div()
            .w(px(WIDTH))
            .flex_none()
            .px_2()
            .py(px(3.))
            .truncate()
            .text_xs()
            .when(head, |d| {
                d.font_weight(FontWeight::SEMIBOLD).text_color(p.text_muted)
            })
            .child(text)
    };
    let head = h_flex()
        .w(width)
        .border_b_1()
        .border_color(p.hairline)
        .children(t.columns.iter().map(|c| cell(c.clone(), true)));
    let rows = t.rows.iter().take(MAX_ROWS).enumerate().map(|(i, r)| {
        h_flex()
            .w(width)
            .when(i % 2 == 1, |d| d.bg(p.hover))
            .children(r.iter().map(|v| cell(table::cell_text(v), false)))
    });
    v_flex()
        .size_full()
        .child(header)
        .child(
            div()
                .id("visual-grid")
                .flex_1()
                .min_h_0()
                .overflow_scroll()
                .child(v_flex().ltr().child(head).children(rows)),
        )
        .when(t.rows.len() > MAX_ROWS, |d| {
            d.child(div().text_xs().text_color(p.text_muted).child(tr!(
                "visual.first_rows",
                shown = MAX_ROWS,
                total = t.rows.len()
            )))
        })
        .when(t.more_columns > 0, |d| {
            d.child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(tr!("visual.more_columns", count = t.more_columns)),
            )
        })
        .into_any_element()
}
