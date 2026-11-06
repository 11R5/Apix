//! The response's Network tab, in groups that open and close: where the
//! time went (a waterfall), the
//! connection's two ends, DNS answers, redirects, compression, the TLS
//! handshake (version, cipher suite, key exchange, ALPN) and the certificate chain.

use std::time::Duration;

use apix_http::{human_duration, human_size};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Finished, Workspace};
use crate::bidi::{self, Bidi as _};
use crate::tr;

/// The width of the labels, so values line up.
const LABEL_WIDTH: Pixels = px(120.);

/// The groups that start open; the rest show one line until asked.
const OPEN_AT_FIRST: [&str; 2] = ["timing", "connection"];

impl Workspace {
    pub(super) fn render_network(
        &self,
        done: &Finished,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = crate::theme::current();
        let response = &done.response;
        let trace = &response.trace;
        let local = &done.local;
        let phases = trace.phases(response.timing.ttfb, response.timing.total);
        let total = response.timing.total.max(Duration::from_micros(1));

        // One bar per phase, offset by the phases before it; TLS only when encrypted.
        let mut rows = vec![
            (tr!("network.timing.dns"), phases.dns),
            (tr!("network.timing.tcp"), phases.tcp),
        ];
        if trace.tls.is_some() {
            rows.push((tr!("network.timing.tls"), phases.tls));
        }
        rows.push((tr!("network.timing.waiting"), phases.waiting));
        rows.push((tr!("network.timing.download"), phases.download));
        let mut offset = Duration::ZERO;
        let waterfall = rows.into_iter().map(|(label, took)| {
            let start = offset.as_secs_f32() / total.as_secs_f32();
            let width = (took.as_secs_f32() / total.as_secs_f32()).max(0.004);
            offset += took;
            h_flex()
                .dir_row()
                .gap_3()
                .items_center()
                .text_xs()
                .child(
                    div()
                        .w(LABEL_WIDTH)
                        .flex_none()
                        .text_color(p.text_muted)
                        .child(label),
                )
                .child(
                    div().flex_1().h(px(6.)).relative().child(
                        div()
                            .absolute()
                            .top_0()
                            .h_full()
                            // Time runs in the reading direction.
                            .map(|bar| {
                                if bidi::rtl() {
                                    bar.right(relative(start.min(1.)))
                                } else {
                                    bar.left(relative(start.min(1.)))
                                }
                            })
                            .w(relative(width.min(1.)))
                            .rounded_sm()
                            .bg(p.accent),
                    ),
                )
                .child(
                    div()
                        .w(px(60.))
                        .flex_none()
                        .text_end()
                        .text_color(p.text)
                        .child(human_duration(took)),
                )
        });

        let row = |label: SharedString, value: SharedString| {
            h_flex()
                .dir_row()
                .gap_3()
                .items_start()
                .text_sm()
                .child(
                    div()
                        .w(LABEL_WIDTH)
                        .flex_none()
                        .text_color(p.text_muted)
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(p.text)
                        .child(bidi::para(value)),
                )
                .into_any_element()
        };
        let mut sections: Vec<AnyElement> = Vec::new();
        let mut timing: Vec<AnyElement> = waterfall.map(|r| r.into_any_element()).collect();
        timing.push(
            div()
                .pt_1()
                .text_xs()
                .text_color(p.text_muted)
                .child(bidi::para(tr!(
                    "network.timing.summary",
                    total = human_duration(response.timing.total),
                    ttfb = human_duration(response.timing.ttfb)
                )))
                .into_any_element(),
        );
        sections.push(self.network_section(
            "timing",
            tr!("network.section.timing"),
            Some(human_duration(response.timing.total).into()),
            timing,
            cx,
        ));
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(remote) = trace.remote_addr {
            rows.push(row(
                tr!("network.connection.server"),
                format!("{remote} · {}", response.version).into(),
            ));
            rows.push(
                h_flex()
                    .dir_row()
                    .gap_3()
                    .child(div().w(LABEL_WIDTH).flex_none())
                    .child(
                        div()
                            .id("network-route")
                            .text_xs()
                            .text_color(p.accent)
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .child(tr!("route.from_network"))
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(super::OpenRoute), cx)
                            }),
                    )
                    .into_any_element(),
            );
        }
        let via: SharedString = match (&local.interface, &local.kind) {
            (Some(name), Some(kind)) if name != kind => format!("{name} ({kind})").into(),
            (Some(name), _) => name.clone().into(),
            _ => tr!("network.connection.unknown_interface"),
        };
        let address: SharedString = trace
            .local_addr
            .map(|a| a.to_string().into())
            .or_else(|| local.local_ip.map(|ip| ip.to_string().into()))
            .unwrap_or_else(|| tr!("network.connection.unknown_address"));
        rows.push(row(
            tr!("network.connection.this_computer"),
            if local.vpn {
                tr!("network.connection.via_vpn", address = address, via = via)
            } else {
                tr!("network.connection.via", address = address, via = via)
            },
        ));
        if let Some(gateway) = local.gateway {
            rows.push(row(
                tr!("network.connection.gateway"),
                gateway.to_string().into(),
            ));
        }
        if !local.dns_servers.is_empty() {
            let servers: Vec<String> = local.dns_servers.iter().map(ToString::to_string).collect();
            rows.push(row(
                tr!("network.connection.dns_servers"),
                servers.join(", ").into(),
            ));
        }
        rows.push(row(
            tr!("network.connection.proxy"),
            match &local.proxy {
                Some((var, value)) => {
                    tr!("network.connection.proxy_from", value = value, var = var)
                }
                None => tr!("network.connection.proxy_none"),
            },
        ));
        if trace.connects.is_empty() {
            rows.push(row(
                tr!("network.note"),
                tr!("network.connection.none_recorded"),
            ));
        }
        let server = trace.remote_addr.map(|a| a.ip().to_string().into());
        sections.push(self.network_section(
            "connection",
            tr!("network.section.connection"),
            server,
            std::mem::take(&mut rows),
            cx,
        ));

        if !trace.dns.is_empty() {
            for lookup in &trace.dns {
                let addrs: Vec<String> = lookup.addrs.iter().map(ToString::to_string).collect();
                rows.push(row(
                    tr!("network.dns.lookup"),
                    tr!(
                        "network.dns.lookup_result",
                        host = lookup.host,
                        addrs = addrs.join(", "),
                        duration = human_duration(lookup.duration)
                    ),
                ));
            }
            let took = human_duration(trace.dns.iter().map(|l| l.duration).sum::<Duration>());
            sections.push(self.network_section(
                "dns",
                tr!("network.section.dns"),
                Some(took.into()),
                std::mem::take(&mut rows),
                cx,
            ));
        }

        if !trace.redirects.is_empty() {
            for hop in &trace.redirects {
                rows.push(row(
                    tr!("network.redirect.followed"),
                    format!("{} → {}", hop.status, apix_core::display_url(&hop.to)).into(),
                ));
            }
            sections.push(self.network_section(
                "redirects",
                tr!("network.section.redirects"),
                Some(trace.redirects.len().to_string().into()),
                std::mem::take(&mut rows),
                cx,
            ));
        }

        if let Some(c) = &trace.compression {
            let saved = 100u64.saturating_sub(c.encoded * 100 / c.decoded.max(1));
            rows.push(row(
                tr!("network.body.compression"),
                tr!(
                    "network.body.compressed",
                    encoding = c.encoding,
                    wire = human_size(c.encoded as usize),
                    decoded = human_size(c.decoded as usize),
                    percent = saved
                ),
            ));
            sections.push(self.network_section(
                "body",
                tr!("network.section.body"),
                Some(c.encoding.clone().into()),
                std::mem::take(&mut rows),
                cx,
            ));
        }

        if let Some(tls) = &trace.tls {
            let facts = [
                (tr!("network.tls.version"), tls.version.clone()),
                (tr!("network.tls.cipher"), tls.cipher.clone()),
                (tr!("network.tls.group"), tls.group.clone()),
                (tr!("network.tls.alpn"), tls.alpn.clone()),
                (
                    tr!("network.tls.server_name"),
                    Some(tls.server_name.clone()).filter(|s| !s.is_empty()),
                ),
                (
                    tr!("network.tls.handshake"),
                    tls.handshake.map(human_duration),
                ),
            ];
            for (label, value) in facts {
                if let Some(value) = value {
                    rows.push(row(label, value.into()));
                }
            }
            sections.push(self.network_section(
                "encryption",
                tr!("network.section.encryption"),
                tls.version.clone().map(Into::into),
                std::mem::take(&mut rows),
                cx,
            ));
        }

        let chain: Vec<&apix_http::Certificate> = match &trace.tls {
            Some(tls) if !tls.chain.is_empty() => tls.chain.iter().collect(),
            _ => trace.certificate.iter().collect(),
        };
        let days_left = |cert: &apix_http::Certificate| match cert.days_left {
            -1 => tr!("network.cert.expired_one_day_ago"),
            ..-1 => tr!("network.cert.expired_days_ago", count = -cert.days_left),
            1 => tr!("network.cert.one_day_left"),
            _ => tr!("network.cert.days_left", count = cert.days_left),
        };
        match chain.first() {
            Some(first) => {
                for (i, cert) in chain.iter().enumerate() {
                    let color = match cert.days_left {
                        ..0 => p.err,
                        0..30 => p.warn,
                        _ => p.text,
                    };
                    // The certificates after the server's own, set apart.
                    if i > 0 {
                        rows.push(div().h(px(1.)).my_1().bg(p.hairline).into_any_element());
                    }
                    rows.push(row(
                        tr!("network.cert.issued_to"),
                        cert.subject.clone().into(),
                    ));
                    rows.push(row(
                        tr!("network.cert.issued_by"),
                        cert.issuer.clone().into(),
                    ));
                    rows.push(
                        h_flex()
                            .dir_row()
                            .gap_3()
                            .text_sm()
                            .child(
                                div()
                                    .w(LABEL_WIDTH)
                                    .flex_none()
                                    .text_color(p.text_muted)
                                    .child(tr!("network.cert.valid")),
                            )
                            .child(div().text_color(p.text).child(bidi::para(tr!(
                                "network.cert.valid_range",
                                from = cert.not_before,
                                to = cert.not_after
                            ))))
                            .child(div().text_color(color).child(days_left(cert)))
                            .into_any_element(),
                    );
                    rows.push(row(tr!("network.cert.key"), cert.key.clone().into()));
                    if i == 0 && !cert.names.is_empty() {
                        rows.push(row(tr!("network.cert.names"), short_list(&cert.names)));
                    }
                    if i == 0 {
                        rows.push(row(tr!("network.cert.sha256"), cert.sha256.clone().into()));
                    }
                }
                let title = if chain.len() > 1 {
                    tr!("network.section.chain", count = chain.len())
                } else {
                    tr!("network.section.certificate")
                };
                let summary = days_left(first);
                sections.push(self.network_section(
                    "certificates",
                    title,
                    Some(summary),
                    std::mem::take(&mut rows),
                    cx,
                ));
            }
            None if response.version.contains('2')
                || trace.remote_addr.is_some_and(|a| a.port() == 443) =>
            {
                rows.push(row(tr!("network.note"), tr!("network.cert.not_reported")));
                sections.push(self.network_section(
                    "certificates",
                    tr!("network.section.certificate"),
                    None,
                    std::mem::take(&mut rows),
                    cx,
                ));
            }
            None => {
                rows.push(row(tr!("network.note"), tr!("network.plain_http")));
                sections.push(self.network_section(
                    "encryption",
                    tr!("network.section.encryption"),
                    None,
                    std::mem::take(&mut rows),
                    cx,
                ));
            }
        }

        div()
            .id("network")
            .track_scroll(&self.network_scroll)
            .size_full()
            .overflow_y_scroll()
            .child(v_flex().gap_2().children(sections))
    }

    /// A group that opens and closes on its header, which shows the gist.
    fn network_section(
        &self,
        key: &'static str,
        title: SharedString,
        summary: Option<SharedString>,
        rows: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = crate::theme::current();
        let open = OPEN_AT_FIRST.contains(&key) != self.network_toggled.contains(key);
        v_flex()
            .flex_none()
            .rounded_lg()
            .bg(p.hover)
            .border_1()
            .border_color(p.hairline)
            .child(
                h_flex()
                    .id(key)
                    .dir_row()
                    .h(px(30.))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .text_sm()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.network_toggled.remove(key) {
                            this.network_toggled.insert(key);
                        }
                        cx.notify();
                    }))
                    .child(
                        Icon::new(if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .xsmall()
                        .text_color(p.text_muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(p.text)
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    )
                    .children(summary.map(|s| {
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(s)
                    })),
            )
            .when(open, |card| {
                card.child(v_flex().px_3().pb_2().gap_1().children(rows))
            })
            .into_any_element()
    }
}

/// The first few of a long list, and how many more there are.
fn short_list(names: &[String]) -> SharedString {
    const SHOWN: usize = 6;
    let list = names[..names.len().min(SHOWN)].join(", ");
    match names.len().saturating_sub(SHOWN) {
        0 => list.into(),
        more => format!("{list} +{more}").into(),
    }
}
