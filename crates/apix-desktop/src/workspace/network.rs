//! The response's Network tab: where the time went (a waterfall), the
//! connection's two ends, DNS answers, redirects, compression, the TLS
//! handshake (version, cipher suite, key exchange, ALPN) and the certificate chain.

use std::time::Duration;

use apix_http::{human_duration, human_size};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Finished, Workspace};
use crate::bidi::{self, Bidi as _};
use crate::tr;

impl Workspace {
    pub(super) fn render_network(&self, done: &Finished) -> impl IntoElement {
        let p = crate::theme::current();
        let response = &done.response;
        let trace = &response.trace;
        let local = &done.local;
        let phases = trace.phases(response.timing.ttfb, response.timing.total);
        let total = response.timing.total.max(Duration::from_micros(1));

        // One bar per phase, offset by the phases before it; TLS only when encrypted.
        let mut rows = vec![
            (
                tr!("network.timing.dns"),
                phases.dns,
                hsla(0.55, 0.7, 0.6, 1.),
            ),
            (
                tr!("network.timing.tcp"),
                phases.tcp,
                hsla(0.75, 0.6, 0.7, 1.),
            ),
        ];
        if trace.tls.is_some() {
            rows.push((
                tr!("network.timing.tls"),
                phases.tls,
                hsla(0.85, 0.55, 0.68, 1.),
            ));
        }
        rows.push((tr!("network.timing.waiting"), phases.waiting, p.warn));
        rows.push((tr!("network.timing.download"), phases.download, p.ok));
        let mut offset = Duration::ZERO;
        let waterfall = rows.into_iter().map(|(label, took, color)| {
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
                        .w(px(150.))
                        .flex_none()
                        .text_color(p.text_muted)
                        .child(label),
                )
                .child(
                    div().flex_1().h(px(10.)).relative().child(
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
                            .bg(color),
                    ),
                )
                .child(
                    div()
                        .w(px(70.))
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
                        .w(px(150.))
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
        };
        let section = |title: SharedString| {
            div()
                .pt_3()
                .pb_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text_muted)
                .child(crate::i18n::upper(&title))
        };

        let mut body = v_flex()
            .gap_1()
            .child(section(tr!("network.section.timing")))
            .children(waterfall)
            .child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!(
                        "network.timing.summary",
                        total = human_duration(response.timing.total),
                        ttfb = human_duration(response.timing.ttfb)
                    ))),
            )
            .child(section(tr!("network.section.connection")));
        if let Some(remote) = trace.remote_addr {
            body = body.child(row(
                tr!("network.connection.server"),
                format!("{remote} · {}", response.version).into(),
            ));
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
        body = body.child(row(
            tr!("network.connection.this_computer"),
            if local.vpn {
                tr!("network.connection.via_vpn", address = address, via = via)
            } else {
                tr!("network.connection.via", address = address, via = via)
            },
        ));
        if let Some(gateway) = local.gateway {
            body = body.child(row(
                tr!("network.connection.gateway"),
                gateway.to_string().into(),
            ));
        }
        if !local.dns_servers.is_empty() {
            let servers: Vec<String> = local.dns_servers.iter().map(ToString::to_string).collect();
            body = body.child(row(
                tr!("network.connection.dns_servers"),
                servers.join(", ").into(),
            ));
        }
        body = body.child(row(
            tr!("network.connection.proxy"),
            match &local.proxy {
                Some((var, value)) => {
                    tr!("network.connection.proxy_from", value = value, var = var)
                }
                None => tr!("network.connection.proxy_none"),
            },
        ));
        if trace.connects.is_empty() {
            body = body.child(row(
                tr!("network.note"),
                tr!("network.connection.none_recorded"),
            ));
        }

        if !trace.dns.is_empty() {
            body = body.child(section(tr!("network.section.dns")));
            for lookup in &trace.dns {
                let addrs: Vec<String> = lookup.addrs.iter().map(ToString::to_string).collect();
                body = body.child(row(
                    tr!("network.dns.lookup"),
                    tr!(
                        "network.dns.lookup_result",
                        host = lookup.host,
                        addrs = addrs.join(", "),
                        duration = human_duration(lookup.duration)
                    ),
                ));
            }
        }

        if !trace.redirects.is_empty() {
            body = body.child(section(tr!("network.section.redirects")));
            for hop in &trace.redirects {
                body = body.child(row(
                    tr!("network.redirect.followed"),
                    format!("{} → {}", hop.status, apix_core::display_url(&hop.to)).into(),
                ));
            }
        }

        if let Some(c) = &trace.compression {
            let saved = 100u64.saturating_sub(c.encoded * 100 / c.decoded.max(1));
            body = body.child(section(tr!("network.section.body"))).child(row(
                tr!("network.body.compression"),
                tr!(
                    "network.body.compressed",
                    encoding = c.encoding,
                    wire = human_size(c.encoded as usize),
                    decoded = human_size(c.decoded as usize),
                    percent = saved
                ),
            ));
        }

        if let Some(tls) = &trace.tls {
            body = body.child(section(tr!("network.section.encryption")));
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
                    body = body.child(row(label, value.into()));
                }
            }
        }

        let chain: Vec<&apix_http::Certificate> = match &trace.tls {
            Some(tls) if !tls.chain.is_empty() => tls.chain.iter().collect(),
            _ => trace.certificate.iter().collect(),
        };
        match chain.first() {
            Some(_) => {
                body = body.child(section(if chain.len() > 1 {
                    tr!("network.section.chain", count = chain.len())
                } else {
                    tr!("network.section.certificate")
                }));
                for (i, cert) in chain.iter().enumerate() {
                    let color = match cert.days_left {
                        ..0 => p.err,
                        0..30 => p.warn,
                        _ => p.text,
                    };
                    let left = match cert.days_left {
                        -1 => tr!("network.cert.expired_one_day_ago"),
                        ..-1 => tr!("network.cert.expired_days_ago", count = -cert.days_left),
                        1 => tr!("network.cert.one_day_left"),
                        _ => tr!("network.cert.days_left", count = cert.days_left),
                    };
                    body = body
                        // The certificates after the server's own, set apart.
                        .when(i > 0, |b| b.child(div().h(px(1.)).my_1().bg(p.hairline)))
                        .child(row(
                            tr!("network.cert.issued_to"),
                            cert.subject.clone().into(),
                        ))
                        .child(row(
                            tr!("network.cert.issued_by"),
                            cert.issuer.clone().into(),
                        ))
                        .child(
                            h_flex()
                                .dir_row()
                                .gap_3()
                                .text_sm()
                                .child(
                                    div()
                                        .w(px(150.))
                                        .flex_none()
                                        .text_color(p.text_muted)
                                        .child(tr!("network.cert.valid")),
                                )
                                .child(div().text_color(p.text).child(bidi::para(tr!(
                                    "network.cert.valid_range",
                                    from = cert.not_before,
                                    to = cert.not_after
                                ))))
                                .child(div().text_color(color).child(left)),
                        )
                        .child(row(tr!("network.cert.key"), cert.key.clone().into()))
                        .when(i == 0 && !cert.names.is_empty(), |b| {
                            b.child(row(tr!("network.cert.names"), cert.names.join(", ").into()))
                        })
                        .when(i == 0, |b| {
                            b.child(row(tr!("network.cert.sha256"), cert.sha256.clone().into()))
                        });
                }
            }
            None if response.version.contains('2')
                || trace.remote_addr.is_some_and(|a| a.port() == 443) =>
            {
                body = body
                    .child(section(tr!("network.section.certificate")))
                    .child(row(tr!("network.note"), tr!("network.cert.not_reported")));
            }
            None => {
                body = body
                    .child(section(tr!("network.section.encryption")))
                    .child(row(tr!("network.note"), tr!("network.plain_http")));
            }
        }

        div()
            .id("network")
            .size_full()
            .overflow_y_scroll()
            .px_2()
            .child(body)
    }
}
