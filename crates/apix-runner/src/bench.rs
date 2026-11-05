//! Load tests: one request sent by many workers at once, for a number of
//! requests or a length of time, optionally at a steady rate, with the
//! latency percentiles and a second-by-second series. The same for
//! `apix bench` and the app's Load test window.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use apix_http::load::{Hit, Load};
use futures_util::StreamExt as _;
use futures_util::stream::FuturesUnordered;
use serde::Serialize;

/// How hard and how long.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Requests in flight at once.
    pub concurrency: usize,
    /// Stop after this many requests…
    pub requests: Option<u64>,
    /// …or after this long (whichever comes first; with neither, 10 s).
    pub duration: Option<Duration>,
    /// At most this many requests a second, spread evenly.
    pub rate: Option<f64>,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            concurrency: 10,
            requests: None,
            duration: Some(Duration::from_secs(10)),
            rate: None,
        }
    }
}

/// One second of the test.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Second {
    pub requests: u64,
    pub failed: u64,
    /// Mean latency of the requests that finished in this second, in ms.
    pub mean_ms: f64,
    #[serde(skip)]
    total_us: u64,
}

/// Where the test stands, or how it ended.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Summary {
    pub requests: u64,
    /// No error and a status below 400.
    pub ok: u64,
    pub failed: u64,
    pub elapsed_ms: f64,
    pub per_second: f64,
    pub bytes: u64,
    pub min_ms: f64,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub statuses: BTreeMap<u16, u64>,
    pub errors: BTreeMap<String, u64>,
    pub seconds: Vec<Second>,
    pub done: bool,
}

#[derive(Default)]
struct Collector {
    /// Microseconds, in the order they finished.
    latencies: Vec<u32>,
    statuses: BTreeMap<u16, u64>,
    errors: BTreeMap<String, u64>,
    bytes: u64,
    ok: u64,
    failed: u64,
    seconds: Vec<Second>,
    elapsed: Duration,
    done: bool,
}

/// A load test that can be watched and stopped from elsewhere while
/// [`Bench::run`] runs.
#[derive(Clone, Default)]
pub struct Bench {
    collector: Arc<Mutex<Collector>>,
    stop: Arc<AtomicBool>,
    started: Arc<Mutex<Option<Instant>>>,
}

impl Bench {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ends the test: no new requests start, those in flight finish.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Sends `load` as the plan says, and returns the summary.
    pub async fn run(&self, load: &Load, plan: &Plan) -> Summary {
        let started = Instant::now();
        if let Ok(mut s) = self.started.lock() {
            *s = Some(started);
        }
        let duration = match (plan.requests, plan.duration) {
            (None, None) => Some(Duration::from_secs(10)),
            (_, d) => d,
        };
        let deadline = duration.map(|d| started + d);
        let issued = AtomicU64::new(0);
        let rate = plan.rate.filter(|r| *r > 0.0);

        // The next request's number, or `None` when it is time to stop.
        let take = || -> Option<u64> {
            if self.stop.load(Ordering::Relaxed) {
                return None;
            }
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return None;
            }
            let n = issued.fetch_add(1, Ordering::Relaxed);
            if plan.requests.is_some_and(|max| n >= max) {
                return None;
            }
            Some(n)
        };
        let worker = || async {
            while let Some(n) = take() {
                if let Some(rate) = rate {
                    let at = started + Duration::from_secs_f64(n as f64 / rate);
                    if let Some(d) = deadline
                        && at >= d
                    {
                        break;
                    }
                    tokio::time::sleep_until(at.into()).await;
                    if self.stop.load(Ordering::Relaxed) {
                        break;
                    }
                }
                let hit = load.hit().await;
                self.record(started, &hit);
            }
        };
        let mut workers: FuturesUnordered<_> =
            (0..plan.concurrency.max(1)).map(|_| worker()).collect();
        while workers.next().await.is_some() {}

        if let Ok(mut c) = self.collector.lock() {
            c.done = true;
            c.elapsed = started.elapsed();
        }
        self.summary()
    }

    fn record(&self, started: Instant, hit: &Hit) {
        let Ok(mut c) = self.collector.lock() else {
            return;
        };
        let micros = hit.latency.as_micros().min(u128::from(u32::MAX)) as u32;
        c.latencies.push(micros);
        c.bytes += hit.bytes;
        let failed = hit.error.is_some() || hit.status.is_none_or(|s| s >= 400);
        if failed {
            c.failed += 1;
        } else {
            c.ok += 1;
        }
        if let Some(status) = hit.status {
            *c.statuses.entry(status).or_default() += 1;
        }
        if let Some(error) = &hit.error {
            *c.errors.entry(error.clone()).or_default() += 1;
        }
        let second = started.elapsed().as_secs() as usize;
        if c.seconds.len() <= second {
            c.seconds.resize(second + 1, Second::default());
        }
        let s = &mut c.seconds[second];
        s.requests += 1;
        s.total_us += u64::from(micros);
        s.mean_ms = s.total_us as f64 / s.requests as f64 / 1000.0;
        if failed {
            s.failed += 1;
        }
        c.elapsed = started.elapsed();
    }

    /// The numbers so far (or the final ones once [`Bench::run`] returned).
    pub fn summary(&self) -> Summary {
        let Ok(c) = self.collector.lock() else {
            return Summary::default();
        };
        let elapsed = if c.done {
            c.elapsed
        } else {
            self.started
                .lock()
                .ok()
                .and_then(|s| s.map(|s| s.elapsed()))
                .unwrap_or_default()
        };
        let mut sorted = c.latencies.clone();
        sorted.sort_unstable();
        let ms = |us: u32| f64::from(us) / 1000.0;
        let at = |q: f64| -> f64 {
            if sorted.is_empty() {
                return 0.0;
            }
            // Nearest rank.
            let rank = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
            ms(sorted[rank - 1])
        };
        let count = sorted.len() as u64;
        let mean = if count == 0 {
            0.0
        } else {
            sorted.iter().map(|&u| u64::from(u)).sum::<u64>() as f64 / count as f64 / 1000.0
        };
        let secs = elapsed.as_secs_f64();
        Summary {
            requests: count,
            ok: c.ok,
            failed: c.failed,
            elapsed_ms: secs * 1000.0,
            per_second: if secs > 0.0 { count as f64 / secs } else { 0.0 },
            bytes: c.bytes,
            min_ms: sorted.first().map_or(0.0, |&u| ms(u)),
            mean_ms: mean,
            p50_ms: at(0.50),
            p90_ms: at(0.90),
            p95_ms: at(0.95),
            p99_ms: at(0.99),
            max_ms: sorted.last().map_or(0.0, |&u| ms(u)),
            statuses: c.statuses.clone(),
            errors: c.errors.clone(),
            seconds: c.seconds.clone(),
            done: c.done,
        }
    }
}

impl Summary {
    /// For a terminal: the totals, the latency spread and what went wrong.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let push = |out: &mut String, line: String| {
            out.push_str(&line);
            out.push('\n');
        };
        push(
            &mut out,
            format!(
                "{} requests in {:.1} s · {:.1}/s · {} ok, {} failed · {}",
                self.requests,
                self.elapsed_ms / 1000.0,
                self.per_second,
                self.ok,
                self.failed,
                human_bytes(self.bytes)
            ),
        );
        push(
            &mut out,
            format!(
                "latency  min {}  mean {}  p50 {}  p90 {}  p95 {}  p99 {}  max {}",
                ms(self.min_ms),
                ms(self.mean_ms),
                ms(self.p50_ms),
                ms(self.p90_ms),
                ms(self.p95_ms),
                ms(self.p99_ms),
                ms(self.max_ms)
            ),
        );
        if !self.statuses.is_empty() {
            let list: Vec<String> = self
                .statuses
                .iter()
                .map(|(s, n)| format!("{s} × {n}"))
                .collect();
            push(&mut out, format!("status   {}", list.join(", ")));
        }
        for (error, n) in &self.errors {
            push(&mut out, format!("error    {error} × {n}"));
        }
        out
    }

    /// The test's JSON, for CI to keep or compare.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

fn ms(value: f64) -> String {
    if value < 10.0 {
        format!("{value:.2} ms")
    } else {
        format!("{value:.0} ms")
    }
}

fn human_bytes(n: u64) -> String {
    match n {
        0..1024 => format!("{n} B"),
        1024..1_048_576 => format!("{:.1} KB", n as f64 / 1024.0),
        _ => format!("{:.1} MB", n as f64 / 1_048_576.0),
    }
}

/// `30s`, `2m`, `1500ms`, or a plain number of seconds.
pub fn parse_duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    let (number, unit) = match text.find(|c: char| !c.is_ascii_digit() && c != '.') {
        Some(i) => text.split_at(i),
        None => (text, "s"),
    };
    let value: f64 = number.parse().ok()?;
    let secs = match unit.trim() {
        "ms" => value / 1000.0,
        "s" | "sec" => value,
        "m" | "min" => value * 60.0,
        "h" => value * 3600.0,
        _ => return None,
    };
    (secs > 0.0).then(|| Duration::from_secs_f64(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("2m"), Some(Duration::from_secs(120)));
        assert_eq!(parse_duration("1500ms"), Some(Duration::from_millis(1500)));
        assert_eq!(parse_duration("5"), Some(Duration::from_secs(5)));
        assert_eq!(parse_duration("0.5s"), Some(Duration::from_millis(500)));
        assert_eq!(parse_duration("0"), None);
        assert_eq!(parse_duration("fast"), None);
        assert_eq!(parse_duration("3 weeks"), None);
    }

    #[test]
    fn percentiles_by_nearest_rank() {
        let bench = Bench::new();
        let started = Instant::now();
        for ms in 1..=100u64 {
            bench.record(
                started,
                &Hit {
                    status: Some(if ms % 10 == 0 { 500 } else { 200 }),
                    latency: Duration::from_millis(ms),
                    bytes: 10,
                    error: None,
                },
            );
        }
        let s = bench.summary();
        assert_eq!(s.requests, 100);
        assert_eq!((s.ok, s.failed), (90, 10));
        assert_eq!(s.min_ms, 1.0);
        assert_eq!(s.p50_ms, 50.0);
        assert_eq!(s.p90_ms, 90.0);
        assert_eq!(s.p95_ms, 95.0);
        assert_eq!(s.p99_ms, 99.0);
        assert_eq!(s.max_ms, 100.0);
        assert_eq!(s.mean_ms, 50.5);
        assert_eq!(s.statuses[&200], 90);
        assert_eq!(s.statuses[&500], 10);
        assert_eq!(s.bytes, 1000);
        assert_eq!(s.seconds[0].requests, 100);
        assert!(s.to_text().contains("p95 95 ms"), "{}", s.to_text());
        assert!(s.to_json().contains("\"p99_ms\": 99.0"));
    }
}
