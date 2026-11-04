//! The URL bar and the Params tab are one thing seen twice: typing a query
//! into the URL fills the table, editing the table rewrites the URL. The
//! URL's `:name` segments get a row each under Path variables.

use apix_core::KeyValue;
use apix_core::urls;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use super::{KvRow, Workspace, var_chips};
use crate::bidi::{self, Bidi as _};
use crate::tr;

/// The value of one `:name` segment.
pub(super) struct PathVarRow {
    pub name: String,
    pub value: Entity<InputState>,
    _chips: Subscription,
}

impl Workspace {
    /// A Params row whose edits go into the URL.
    pub(super) fn new_param_row(
        &mut self,
        kv: &KeyValue,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> KvRow {
        let mut row = KvRow::new(kv, window, cx);
        for input in [row.key.clone(), row.value.clone()] {
            row._sync.push(cx.subscribe_in(
                &input,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.sync_url_from_params(window, cx);
                    }
                },
            ));
        }
        row
    }

    fn param_rows(&self, cx: &App) -> Vec<KeyValue> {
        self.params
            .iter()
            .filter_map(|r| r.to_key_value(cx))
            .collect()
    }

    /// The table changed: the URL shows its enabled rows as the query.
    pub(super) fn sync_url_from_params(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.param_rows(cx);
        let text = urls::query_text(&rows);
        if text == self.params_synced {
            return;
        }
        self.params_synced = text;
        let current = self.url.read(cx).value().to_string();
        let url = urls::with_query(&current, &rows);
        self.last_url_len = url.chars().count();
        self.url.update(cx, |state, cx| {
            state.set_value(var_chips::chip_content(&url), window, cx)
        });
        cx.notify();
    }

    /// The URL changed: its query becomes the table's enabled rows (disabled
    /// ones stay, after them), and its `:name` segments the path variables.
    pub(super) fn sync_params_from_url(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url.read(cx).value().to_string();
        let (_, query, _) = urls::split(&url);
        let rows = urls::query_rows(query.unwrap_or_default());
        let text = urls::query_text(&rows);
        if text != self.params_synced {
            self.params_synced = text;
            let mut kept: Vec<KvRow> = std::mem::take(&mut self.params)
                .into_iter()
                .filter(|r| !r.enabled)
                .collect();
            let mut params: Vec<KvRow> = rows
                .iter()
                .map(|kv| self.new_param_row(kv, window, cx))
                .collect();
            params.append(&mut kept);
            self.params = params;
        }
        self.sync_path_vars(&url, &[], window, cx);
        cx.notify();
    }

    /// One row per `:name` in `url`, keeping what is typed in rows that stay;
    /// `saved` gives values to rows that are new.
    pub(super) fn sync_path_vars(
        &mut self,
        url: &str,
        saved: &[KeyValue],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let names = urls::path_variables(url);
        if names.len() == self.path_vars.len()
            && names.iter().zip(&self.path_vars).all(|(n, r)| *n == r.name)
        {
            return;
        }
        let mut old = std::mem::take(&mut self.path_vars);
        self.path_vars = names
            .into_iter()
            .map(|name| {
                if let Some(ix) = old.iter().position(|r| r.name == name) {
                    return old.remove(ix);
                }
                let value = saved
                    .iter()
                    .find(|v| v.key == name)
                    .map(|v| v.value.clone())
                    .unwrap_or_default();
                let (value, chips) =
                    var_chips::chipped_input(&value, tr!("table.value"), window, cx);
                PathVarRow {
                    name,
                    value,
                    _chips: chips,
                }
            })
            .collect();
    }

    /// The path variables as the request file keeps them.
    pub(super) fn path_var_values(&self, cx: &App) -> Vec<KeyValue> {
        self.path_vars
            .iter()
            .map(|r| KeyValue::new(r.name.clone(), r.value.read(cx).value().to_string()))
            .collect()
    }

    /// Under the Params table, when the URL has `:name` segments.
    pub(super) fn render_path_vars(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.path_vars.is_empty() {
            return None;
        }
        let p = crate::theme::current();
        let view = cx.entity().downgrade();
        let mono = cx.theme().mono_font_family.clone();
        let rows = self.path_vars.iter().map(|row| {
            h_flex()
                .dir_row()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .ltr()
                        .px_2()
                        .text_sm()
                        .font_family(mono.clone())
                        .text_color(p.text_muted)
                        .child(format!(":{}", row.name)),
                )
                .child(div().flex_1().min_w_0().child(self.chip_input(
                    &row.value,
                    view.clone(),
                    cx,
                )))
                // Lines up with the Params rows' delete button.
                .child(div().w(px(22.)).flex_none())
        });
        Some(
            v_flex()
                .gap_1()
                .pt_3()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text_muted)
                        .child(tr!("request.path_vars")),
                )
                .children(rows)
                .child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(bidi::para(tr!("request.path_vars_hint"))),
                )
                .into_any_element(),
        )
    }
}
