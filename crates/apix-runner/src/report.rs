//! What a run did, and the files it can be saved as: JSON (everything),
//! JUnit XML (for CI test views) and a self-contained HTML page.

use apix_script::{LogLine, TestResult};
use serde::Serialize;

/// One request sent (or not) in one iteration.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ItemResult {
    /// 0-based.
    pub iteration: u32,
    pub name: String,
    /// The request file, relative to the collection.
    pub path: String,
    pub method: String,
    /// The URL as sent, variables filled in.
    pub url: String,
    pub status: Option<u16>,
    pub reason: String,
    pub time_ms: f64,
    pub size: u64,
    pub tests: Vec<TestResult>,
    pub logs: Vec<LogLine>,
    /// Why the request could not be sent (undefined variable, network error …).
    pub error: Option<String>,
    pub pre_error: Option<String>,
    pub test_error: Option<String>,
    /// A pre-request script called `pm.execution.skipRequest()`.
    pub skipped: bool,
    pub passed: bool,
}

impl ItemResult {
    pub fn failed_tests(&self) -> usize {
        self.tests
            .iter()
            .filter(|t| !t.passed && !t.skipped)
            .count()
    }

    /// The script errors and the send error, each saying where it came from.
    pub fn errors(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(e) = &self.pre_error {
            out.push(format!("pre-request script error: {e}"));
        }
        if let Some(e) = &self.error {
            out.push(e.clone());
        }
        if let Some(e) = &self.test_error {
            out.push(format!("test script error: {e}"));
        }
        out
    }
}

/// Counts over a run's items.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub requests: usize,
    pub passed: usize,
    pub failed: usize,
    pub tests: usize,
    pub tests_failed: usize,
}

impl Summary {
    pub fn of(items: &[ItemResult]) -> Self {
        let mut s = Self::default();
        for item in items {
            s.requests += 1;
            if item.passed {
                s.passed += 1;
            } else {
                s.failed += 1;
            }
            s.tests += item.tests.iter().filter(|t| !t.skipped).count();
            s.tests_failed += item.failed_tests();
        }
        s
    }
}

/// A whole run.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RunReport {
    /// The collection's name.
    pub name: String,
    pub environment: Option<String>,
    /// When it started, in milliseconds since 1970 (UTC).
    pub started_ms: u64,
    pub duration_ms: u64,
    pub iterations: u32,
    /// It ended early: `--bail`, or stopped from the Runner.
    pub stopped: bool,
    pub items: Vec<ItemResult>,
}

impl RunReport {
    pub fn summary(&self) -> Summary {
        Summary::of(&self.items)
    }

    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct WithSummary<'a> {
            #[serde(flatten)]
            report: &'a RunReport,
            started: String,
            summary: Summary,
        }
        serde_json::to_string_pretty(&WithSummary {
            report: self,
            started: iso8601(self.started_ms),
            summary: self.summary(),
        })
        .unwrap_or_default()
    }

    /// One `<testsuite>` per request per iteration and one `<testcase>` per
    /// `pm.test`; a request without tests gets one case for its status.
    pub fn to_junit(&self) -> String {
        let summary = self.summary();
        let errors = self.items.iter().filter(|i| !i.errors().is_empty()).count();
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out += &format!(
            "<testsuites name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{errors}\" time=\"{:.3}\" timestamp=\"{}\">\n",
            xml(&self.name),
            self.items.iter().map(cases).sum::<usize>(),
            summary.tests_failed + self.items.iter().filter(|i| status_failed(i)).count(),
            self.duration_ms as f64 / 1000.0,
            iso8601(self.started_ms),
        );
        for item in &self.items {
            let suite = self.label(item);
            let failures = item.failed_tests() + usize::from(status_failed(item));
            let skipped =
                item.tests.iter().filter(|t| t.skipped).count() + usize::from(item.skipped);
            out += &format!(
                "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{failures}\" errors=\"{}\" skipped=\"{skipped}\" time=\"{:.3}\">\n",
                xml(&suite),
                cases(item),
                usize::from(!item.errors().is_empty()),
                item.time_ms / 1000.0,
            );
            let class = xml(&item.path);
            for error in item.errors() {
                out += &format!(
                    "    <testcase name=\"{}\" classname=\"{class}\" time=\"0\">\n      <error message=\"{}\"/>\n    </testcase>\n",
                    xml(&format!("{} {}", item.method, item.name)),
                    xml(&error),
                );
            }
            if item.skipped {
                out += &format!(
                    "    <testcase name=\"{}\" classname=\"{class}\" time=\"0\">\n      <skipped message=\"skipped\"/>\n    </testcase>\n",
                    xml(&format!("{} {}", item.method, item.name)),
                );
            }
            if item.tests.is_empty() && item.status.is_some() {
                let status = status_text(item);
                out += &format!(
                    "    <testcase name=\"{}\" classname=\"{class}\" time=\"{:.3}\">\n",
                    xml(&format!(
                        "{} {} responds with a success status",
                        item.method, item.name
                    )),
                    item.time_ms / 1000.0,
                );
                if status_failed(item) {
                    out += &format!(
                        "      <failure message=\"{}\" type=\"StatusError\"/>\n",
                        xml(&format!("got {status}"))
                    );
                }
                out += "    </testcase>\n";
            }
            for test in &item.tests {
                out += &format!(
                    "    <testcase name=\"{}\" classname=\"{class}\" time=\"0\">\n",
                    xml(&test.name)
                );
                if test.skipped {
                    out += "      <skipped/>\n";
                } else if !test.passed {
                    let why = test.error.as_deref().unwrap_or("failed");
                    out += &format!(
                        "      <failure message=\"{}\" type=\"AssertionError\"/>\n",
                        xml(why)
                    );
                }
                out += "    </testcase>\n";
            }
            if !item.logs.is_empty() {
                let lines: Vec<String> = item
                    .logs
                    .iter()
                    .map(|l| format!("[{}] {}", l.level, l.text))
                    .collect();
                out += &format!("    <system-out>{}</system-out>\n", xml(&lines.join("\n")));
            }
            out += "  </testsuite>\n";
        }
        out += "</testsuites>\n";
        out
    }

    /// A page that opens anywhere, with no files beside it.
    pub fn to_html(&self) -> String {
        let s = self.summary();
        let title = format!("{} · Apix run", self.name);
        let env = self
            .environment
            .as_deref()
            .map(|e| format!(" · {}", html(e)))
            .unwrap_or_default();
        let mut rows = String::new();
        for item in &self.items {
            let (mark, class) = if item.skipped {
                ("–", "skip")
            } else if item.passed {
                ("✓", "ok")
            } else {
                ("✗", "bad")
            };
            let mut details = String::new();
            for error in item.errors() {
                details += &format!("<li class=\"bad\">{}</li>", html(&error));
            }
            for test in &item.tests {
                let (m, c) = if test.skipped {
                    ("–", "skip")
                } else if test.passed {
                    ("✓", "ok")
                } else {
                    ("✗", "bad")
                };
                let why = test
                    .error
                    .as_deref()
                    .map(|e| format!("<div class=\"why\">{}</div>", html(e)))
                    .unwrap_or_default();
                details += &format!(
                    "<li class=\"{c}\"><span class=\"mark\">{m}</span> {}{why}</li>",
                    html(&test.name)
                );
            }
            let console = if item.logs.is_empty() {
                String::new()
            } else {
                let lines: Vec<String> = item
                    .logs
                    .iter()
                    .map(|l| {
                        format!(
                            "<div class=\"log {}\">{}</div>",
                            html(&l.level),
                            html(&l.text)
                        )
                    })
                    .collect();
                format!("<div class=\"console\">{}</div>", lines.join(""))
            };
            let tests = if item.tests.is_empty() {
                String::new()
            } else {
                format!(
                    "{}/{}",
                    item.tests.iter().filter(|t| t.passed && !t.skipped).count(),
                    item.tests.iter().filter(|t| !t.skipped).count()
                )
            };
            let iteration = if self.iterations > 1 {
                format!("<td class=\"num\">{}</td>", item.iteration + 1)
            } else {
                String::new()
            };
            let span = if self.iterations > 1 { 6 } else { 5 };
            let body = if details.is_empty() && console.is_empty() {
                String::new()
            } else {
                format!(
                    "<tr class=\"more\"><td></td><td colspan=\"{span}\"><ul>{details}</ul>{console}</td></tr>"
                )
            };
            rows += &format!(
                "<tr class=\"{class}\"><td class=\"mark\">{mark}</td>{iteration}<td><span class=\"method\">{}</span> {}<div class=\"url\">{}</div></td><td class=\"num\">{}</td><td class=\"num\">{:.0} ms</td><td class=\"num\">{}</td><td class=\"num\">{tests}</td></tr>{body}\n",
                html(&item.method),
                html(&item.name),
                html(&item.url),
                html(&status_text(item)),
                item.time_ms,
                apix_http::human_size(item.size as usize),
            );
        }
        let iteration_head = if self.iterations > 1 {
            "<th>#</th>"
        } else {
            ""
        };
        let stopped = if self.stopped {
            "<p class=\"bad\">The run was stopped before the end.</p>"
        } else {
            ""
        };
        format!(
            r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{ --bg:#f7f9fa; --panel:#fff; --text:#1a2226; --muted:#66757c; --line:#e3e8ea; --ok:#14866d; --bad:#c62f3a; --accent:#0e8fa3; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg:#0d1417; --panel:#141e22; --text:#dce6e9; --muted:#8a9aa0; --line:#22313a; --ok:#3ccf9e; --bad:#ff6b73; --accent:#3cc8dc; }} }}
body {{ margin:0; padding:24px 16px; background:var(--bg); color:var(--text); font:14px/1.45 system-ui,-apple-system,"Segoe UI",sans-serif; }}
main {{ max-width:1100px; margin:0 auto; }}
h1 {{ font-size:20px; margin:0 0 4px; }}
.sub {{ color:var(--muted); margin-bottom:20px; }}
.cards {{ display:flex; flex-wrap:wrap; gap:12px; margin-bottom:20px; }}
.card {{ background:var(--panel); border:1px solid var(--line); border-radius:10px; padding:12px 16px; min-width:120px; }}
.card b {{ display:block; font-size:22px; }}
.card span {{ color:var(--muted); font-size:12px; }}
table {{ width:100%; border-collapse:collapse; background:var(--panel); border:1px solid var(--line); border-radius:10px; overflow:hidden; }}
th, td {{ text-align:left; padding:8px 10px; border-bottom:1px solid var(--line); vertical-align:top; }}
th {{ font-size:12px; color:var(--muted); font-weight:600; }}
.num {{ white-space:nowrap; font-variant-numeric:tabular-nums; }}
.mark {{ width:18px; font-weight:700; }}
.ok .mark, li.ok .mark, .ok b {{ color:var(--ok); }}
.bad .mark, li.bad, .bad b, p.bad {{ color:var(--bad); }}
.skip {{ color:var(--muted); }}
.method {{ font-weight:700; color:var(--accent); font-size:12px; }}
.url {{ color:var(--muted); font-size:12px; word-break:break-all; }}
tr.more td {{ padding-top:0; }}
tr:has(+ tr.more) td {{ border-bottom:none; }}
ul {{ margin:0; padding:0 0 0 2px; list-style:none; }}
li {{ margin:2px 0; }}
.why {{ color:var(--muted); font-size:12px; margin-left:20px; }}
.console {{ margin-top:6px; padding:6px 8px; border-radius:6px; background:var(--bg); font:12px/1.4 ui-monospace,Consolas,monospace; white-space:pre-wrap; }}
.log.warn {{ color:#c98a00; }} .log.error {{ color:var(--bad); }}
footer {{ color:var(--muted); font-size:12px; margin-top:16px; }}
</style>
</head>
<body>
<main>
<h1>{name}</h1>
<div class="sub">{started} UTC{env} · {iterations} iteration(s) · {duration:.1} s</div>
{stopped}
<div class="cards">
<div class="card {req_class}"><b>{passed}/{requests}</b><span>requests passed</span></div>
<div class="card {test_class}"><b>{tests_passed}/{tests}</b><span>tests passed</span></div>
<div class="card"><b>{avg:.0} ms</b><span>average response time</span></div>
</div>
<table>
<thead><tr><th></th>{iteration_head}<th>Request</th><th>Status</th><th>Time</th><th>Size</th><th>Tests</th></tr></thead>
<tbody>
{rows}</tbody>
</table>
<footer>Made by Apix</footer>
</main>
</body>
</html>
"#,
            title = html(&title),
            name = html(&self.name),
            started = iso8601(self.started_ms)
                .replace('T', " ")
                .trim_end_matches('Z'),
            iterations = self.iterations,
            duration = self.duration_ms as f64 / 1000.0,
            req_class = if s.failed > 0 { "bad" } else { "ok" },
            test_class = if s.tests_failed > 0 { "bad" } else { "ok" },
            passed = s.passed,
            requests = s.requests,
            tests_passed = s.tests - s.tests_failed,
            tests = s.tests,
            avg = average_time(&self.items),
        )
    }

    /// `users/get.apix`, or `users/get.apix (iteration 2)` in a run of several.
    fn label(&self, item: &ItemResult) -> String {
        if self.iterations > 1 {
            format!("{} (iteration {})", item.path, item.iteration + 1)
        } else {
            item.path.clone()
        }
    }
}

fn average_time(items: &[ItemResult]) -> f64 {
    let sent: Vec<f64> = items
        .iter()
        .filter(|i| i.status.is_some())
        .map(|i| i.time_ms)
        .collect();
    if sent.is_empty() {
        0.0
    } else {
        sent.iter().sum::<f64>() / sent.len() as f64
    }
}

/// A request without tests that got an error status.
fn status_failed(item: &ItemResult) -> bool {
    item.tests.is_empty() && item.status.is_some_and(|s| s >= 400)
}

fn status_text(item: &ItemResult) -> String {
    match item.status {
        Some(code) if item.reason.is_empty() => code.to_string(),
        Some(code) => format!("{code} {}", item.reason),
        None if item.skipped => "skipped".into(),
        None => "—".into(),
    }
}

/// How many `<testcase>` elements an item gets.
fn cases(item: &ItemResult) -> usize {
    item.errors().len()
        + usize::from(item.skipped)
        + usize::from(item.tests.is_empty() && item.status.is_some())
        + item.tests.len()
}

fn xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\t' | '\r' => out.push(c),
            // Control characters are not allowed in XML 1.0.
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

fn html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `2026-10-24T09:30:00Z` for milliseconds since 1970.
pub fn iso8601(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Howard Hinnant's days-to-civil.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> RunReport {
        RunReport {
            name: "Shop <API>".into(),
            environment: Some("dev".into()),
            started_ms: 1_761_298_200_000,
            duration_ms: 1500,
            iterations: 1,
            stopped: false,
            items: vec![
                ItemResult {
                    name: "List users".into(),
                    path: "users/list.apix".into(),
                    method: "GET".into(),
                    url: "https://x.test/users".into(),
                    status: Some(200),
                    reason: "OK".into(),
                    time_ms: 120.0,
                    tests: vec![
                        TestResult {
                            name: "status is 200".into(),
                            passed: true,
                            skipped: false,
                            error: None,
                        },
                        TestResult {
                            name: "has \"id\"".into(),
                            passed: false,
                            skipped: false,
                            error: Some("expected undefined to exist".into()),
                        },
                    ],
                    ..Default::default()
                },
                ItemResult {
                    name: "Health".into(),
                    path: "health.apix".into(),
                    method: "GET".into(),
                    status: Some(503),
                    passed: false,
                    ..Default::default()
                },
                ItemResult {
                    name: "Login".into(),
                    path: "login.apix".into(),
                    method: "POST".into(),
                    error: Some("undefined variable(s): password".into()),
                    ..Default::default()
                },
            ],
        }
    }

    #[test]
    fn dates() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(1_761_298_200_000), "2025-10-24T09:30:00Z");
        assert_eq!(iso8601(951_782_400_000), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn summary_counts() {
        let s = report().summary();
        assert_eq!(
            s,
            Summary {
                requests: 3,
                passed: 0,
                failed: 3,
                tests: 2,
                tests_failed: 1
            }
        );
    }

    #[test]
    fn junit_has_suites_cases_and_escapes() {
        let xml = report().to_junit();
        assert!(xml.starts_with("<?xml"));
        assert!(
            xml.contains(
                r#"<testsuites name="Shop &lt;API&gt;" tests="4" failures="2" errors="1""#
            )
        );
        assert!(xml.contains(r#"<testcase name="has &quot;id&quot;" classname="users/list.apix""#));
        assert!(
            xml.contains(
                r#"<failure message="expected undefined to exist" type="AssertionError"/>"#
            )
        );
        assert!(xml.contains(r#"<failure message="got 503" type="StatusError"/>"#));
        assert!(xml.contains(r#"<error message="undefined variable(s): password"/>"#));
        assert_eq!(xml.matches("<testsuite ").count(), 3);
        assert_eq!(xml.matches("<testcase ").count(), 4);
    }

    #[test]
    fn json_and_html() {
        let json: serde_json::Value = serde_json::from_str(&report().to_json()).unwrap();
        assert_eq!(json["summary"]["tests_failed"], 1);
        assert_eq!(json["started"], "2025-10-24T09:30:00Z");
        assert_eq!(json["items"][0]["tests"][1]["name"], "has \"id\"");
        let page = report().to_html();
        assert!(page.contains("<title>Shop &lt;API&gt; · Apix run</title>"));
        assert!(page.contains("expected undefined to exist"));
        assert!(page.contains("0/3"));
    }
}
