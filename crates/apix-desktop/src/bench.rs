//! Opt-in timings for `scripts/measure.ps1`. Off unless `APIX_BENCH` is set:
//!
//! - `APIX_BENCH=startup`: report the time from `main` to the first frame, then quit.
//! - `APIX_BENCH=json:<url>`: send `GET <url>` on start and report the time from
//!   sending until the response is on screen, then quit.
//!
//! Results go to the file in `APIX_BENCH_OUT` (release builds have no console),
//! else to stdout, as `name=milliseconds` lines.

use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bench {
    Startup,
    Json(String),
}

/// Call first thing in `main`.
pub fn mark_start() {
    START.get_or_init(Instant::now);
}

pub fn mode() -> Option<Bench> {
    let value = std::env::var("APIX_BENCH").ok()?;
    match value.split_once(':') {
        _ if value == "startup" => Some(Bench::Startup),
        Some(("json", url)) => Some(Bench::Json(url.to_owned())),
        _ => None,
    }
}

/// Milliseconds since `mark_start`.
pub fn since_start_ms() -> f64 {
    START
        .get()
        .map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0)
}

pub fn report(name: &str, ms: f64) {
    let line = format!("{name}={ms:.1}\n");
    match std::env::var_os("APIX_BENCH_OUT") {
        Some(path) => {
            use std::io::Write as _;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = file.write_all(line.as_bytes());
            }
        }
        None => print!("{line}"),
    }
}
