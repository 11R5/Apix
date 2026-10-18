//! The response's Network tab: where the time went (a waterfall), the
//! connection's two ends, DNS answers, redirects and the server certificate.

use std::time::Duration;

use apix_http::human_duration;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Finished, Workspace};

impl Workspace {
    pub(super) fn render_network(&self, done: &Finished) -> impl IntoElement {
        let p = self.palette;
        let response = &done.response;
        let trace = &response.trace;
        let local = &done.local;
        let phases = trace.phases(response.timing.ttfb, response.timing.total);
        let total = response.timing.total.max(Duration::from_micros(1));

        // One bar per phase, offset by the phases before it.
        let rows = [
            ("DNS lookup", phases.dns, hsla(0.55, 0.7, 0.6, 1.)),
            (
                "Connect (TCP + TLS)",
                phases.connect,
                hsla(0.75, 0.6, 0.7, 1.),
            ),
            ("Waiting for the server", phases.waiting, p.warn),
            ("Download", phases.download, p.ok),
        ];
        let mut offset = Duration::ZERO;
        let waterfall = rows.into_iter().map(|(label, took, color)| {
            let start = offset.as_secs_f32() / total.as_secs_f32();
            let width = (took.as_secs_f32() / total.as_secs_f32()).max(0.004);
            offset += took;
            h_flex()
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
                            .left(relative(start.min(1.)))
                            .w(relative(width.min(1.)))
                            .rounded_sm()
                            .bg(color),
                    ),
                )
                .child(
                    div()
                        .w(px(70.))
                        .flex_none()
                        .text_right()
                        .text_color(p.text)
                        .child(human_duration(took)),
                )
        });

        let row = |label: &'static str, value: String| {
            h_flex()
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
                .child(div().flex_1().min_w_0().text_color(p.text).child(value))
        };
        let section = |title: &'static str| {
            div()
                .pt_3()
                .pb_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text_muted)
                .child(title.to_uppercase())
        };

        let mut body = v_flex()
            .gap_1()
            .child(section("Timing"))
            .children(waterfall)
            .child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(format!(
                        "Total {} · first byte after {}",
                        human_duration(response.timing.total),
                        human_duration(response.timing.ttfb)
                    )),
            )
            .child(section("Connection"));
        if let Some(remote) = trace.remote_addr {
            body = body.child(row("Server", format!("{remote} · {}", response.version)));
        }
        let via = match (&local.interface, &local.kind) {
            (Some(name), Some(kind)) if name != kind => format!("{name} ({kind})"),
            (Some(name), _) => name.clone(),
            _ => "unknown interface".into(),
        };
        let address = trace
            .local_addr
            .map(|a| a.to_string())
            .or_else(|| local.local_ip.map(|ip| ip.to_string()))
            .unwrap_or_else(|| "unknown".into());
        body = body.child(row(
            "This computer",
            format!(
                "{address} via {via}{}",
                if local.vpn { " · VPN" } else { "" }
            ),
        ));
        if let Some(gateway) = local.gateway {
            body = body.child(row("Gateway", gateway.to_string()));
        }
        if !local.dns_servers.is_empty() {
            let servers: Vec<String> = local.dns_servers.iter().map(ToString::to_string).collect();
            body = body.child(row("DNS servers", servers.join(", ")));
        }
        body = body.child(row(
            "Proxy",
            match &local.proxy {
                Some((var, value)) => format!("{value} (from {var})"),
                None => "none in the environment".into(),
            },
        ));
        if trace.connects.is_empty() {
            body = body.child(row(
                "Note",
                "No new connection was recorded (reused, or through a proxy).".into(),
            ));
        }

        if !trace.dns.is_empty() {
            body = body.child(section("DNS"));
            for lookup in &trace.dns {
                let addrs: Vec<String> = lookup.addrs.iter().map(ToString::to_string).collect();
                body = body.child(row(
                    "Lookup",
                    format!(
                        "{} → {} in {}",
                        lookup.host,
                        addrs.join(", "),
                        human_duration(lookup.duration)
                    ),
                ));
            }
        }

        if !trace.redirects.is_empty() {
            body = body.child(section("Redirects"));
            for hop in &trace.redirects {
                body = body.child(row("Followed", format!("{} → {}", hop.status, hop.to)));
            }
        }

        match &trace.certificate {
            Some(cert) => {
                let color = match cert.days_left {
                    ..0 => p.err,
                    0..30 => p.warn,
                    _ => p.text,
                };
                let left = if cert.days_left < 0 {
                    format!("expired {} days ago", -cert.days_left)
                } else {
                    format!("{} days left", cert.days_left)
                };
                body = body
                    .child(section("Certificate"))
                    .child(row("Issued to", cert.subject.clone()))
                    .child(row("Issued by", cert.issuer.clone()))
                    .child(
                        h_flex()
                            .gap_3()
                            .text_sm()
                            .child(
                                div()
                                    .w(px(150.))
                                    .flex_none()
                                    .text_color(p.text_muted)
                                    .child("Valid"),
                            )
                            .child(
                                div()
                                    .text_color(p.text)
                                    .child(format!("{} to {}", cert.not_before, cert.not_after)),
                            )
                            .child(div().text_color(color).child(left)),
                    )
                    .when(!cert.names.is_empty(), |b| {
                        b.child(row("Names", cert.names.join(", ")))
                    })
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child("The HTTP engine reports the server's certificate, not the TLS version or cipher."),
                    );
            }
            None if response.version.contains('2')
                || trace.remote_addr.is_some_and(|a| a.port() == 443) =>
            {
                body = body.child(section("Certificate")).child(row(
                    "Note",
                    "Encrypted, but the certificate was not reported.".into(),
                ));
            }
            None => {
                body = body
                    .child(section("Encryption"))
                    .child(row("Note", "Plain HTTP: nothing is encrypted.".into()));
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
