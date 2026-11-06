//! The route to a server and where it goes.
//!
//! The hops come from the system's own tool, the way `apix_core::git` uses
//! the installed git: `tracert` on Windows, `traceroute` on macOS and Linux
//! (`tracepath` when traceroute is not installed). Those need no
//! administrator rights, which sending ICMP ourselves would. Only numbers and
//! addresses are read from their output, so a translated Windows still works.
//!
//! Places come from an IP location database on disk (MaxMind DB format, such
//! as DB-IP's free "IP to City Lite" or MaxMind's GeoLite2 City). Nothing is
//! looked up online.

use std::io::{BufRead as _, BufReader};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

/// One step on the way: the router that answered at this distance.
#[derive(Debug, Clone, PartialEq)]
pub struct Hop {
    /// 1 for the first router.
    pub ttl: u32,
    /// `None` when nothing answered in time.
    pub ip: Option<IpAddr>,
    /// The best of the round trips measured, in milliseconds.
    pub rtt_ms: Option<f64>,
}

/// Which tool's output a line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Windows `tracert -d`.
    Tracert,
    /// `traceroute -n` (Linux, macOS, BSD).
    Traceroute,
    /// Linux `tracepath -n`.
    Tracepath,
}

impl Tool {
    fn program(self) -> &'static str {
        match self {
            Tool::Tracert => "tracert",
            Tool::Traceroute => "traceroute",
            Tool::Tracepath => "tracepath",
        }
    }

    fn args(self, target: &str, max_hops: u32) -> Vec<String> {
        let hops = max_hops.to_string();
        let list: Vec<&str> = match self {
            Tool::Tracert => vec!["-d", "-h", &hops, "-w", "1000", target],
            Tool::Traceroute => vec!["-n", "-q", "1", "-w", "1", "-m", &hops, target],
            Tool::Tracepath => vec!["-n", "-m", &hops, target],
        };
        list.into_iter().map(str::to_owned).collect()
    }

    /// The tools to try on this system, in order.
    fn candidates() -> &'static [Tool] {
        if cfg!(windows) {
            &[Tool::Tracert]
        } else {
            &[Tool::Traceroute, Tool::Tracepath]
        }
    }
}

/// Reads one line of a tool's output; `None` for headers and other text.
pub fn parse_line(tool: Tool, line: &str) -> Option<Hop> {
    let line = line.trim();
    let (first, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    // tracepath writes "1:" and "1?:".
    let ttl: u32 = first
        .trim_end_matches(':')
        .trim_end_matches('?')
        .parse()
        .ok()?;
    if ttl == 0 {
        return None;
    }
    let words: Vec<&str> = rest.split_whitespace().collect();
    let ip = words
        .iter()
        .map(|w| w.trim_matches(|c| c == '[' || c == ']' || c == '(' || c == ')'))
        .find_map(|w| w.parse::<IpAddr>().ok());
    let mut times = Vec::new();
    for (i, word) in words.iter().enumerate() {
        // "12 ms", "<1 ms", "0.512 ms", and tracepath's "0.512ms".
        let number = if let Some(n) = word.strip_suffix("ms") {
            if n.is_empty() {
                continue;
            }
            n
        } else if words.get(i + 1) == Some(&"ms") {
            word
        } else {
            continue;
        };
        // tracert's "<1 ms": under a millisecond.
        let (number, under) = match number.strip_prefix('<') {
            Some(n) => (n, true),
            None => (number, false),
        };
        if let Ok(ms) = number.parse::<f64>() {
            times.push(if under { ms / 2.0 } else { ms });
        }
    }
    let rtt_ms = times.into_iter().reduce(f64::min);
    Some(Hop {
        ttl,
        ip: ip.filter(|_| rtt_ms.is_some() || tool != Tool::Tracert),
        rtt_ms,
    })
}

/// A running trace: iterate it for the hops as they arrive. Dropping it (or
/// [`Stopper::stop`]) ends the tool.
pub struct Trace {
    tool: Tool,
    lines: std::io::Lines<BufReader<ChildStdout>>,
    child: Arc<Mutex<Child>>,
    last: u32,
}

/// Ends a [`Trace`] from another thread.
#[derive(Clone)]
pub struct Stopper(Arc<Mutex<Child>>);

impl Stopper {
    pub fn stop(&self) {
        if let Ok(mut child) = self.0.lock() {
            let _ = child.kill();
        }
    }
}

impl Trace {
    /// Starts tracing the route to `target` (an address or a host name) with
    /// the first tool this system has.
    pub fn start(target: &str, max_hops: u32) -> Result<Self, String> {
        if target.is_empty() || target.starts_with('-') || target.contains(char::is_whitespace) {
            return Err(format!("not a host: {target}"));
        }
        let mut missing = Vec::new();
        for &tool in Tool::candidates() {
            let mut command = Command::new(tool.program());
            command
                .args(tool.args(target, max_hops))
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt as _;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                command.creation_flags(CREATE_NO_WINDOW);
            }
            match command.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take().ok_or("no output")?;
                    return Ok(Self {
                        tool,
                        lines: BufReader::new(stdout).lines(),
                        child: Arc::new(Mutex::new(child)),
                        last: 0,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => missing.push(tool.program()),
                Err(e) => return Err(format!("{}: {e}", tool.program())),
            }
        }
        Err(format!("{} is not installed", missing.join(" or ")))
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn stopper(&self) -> Stopper {
        Stopper(self.child.clone())
    }
}

impl Iterator for Trace {
    type Item = Hop;

    fn next(&mut self) -> Option<Hop> {
        loop {
            // The output is the system's code page; only ASCII digits and
            // addresses matter, so anything else may be garbled.
            let line = match self.lines.next()? {
                Ok(line) => line,
                Err(_) => continue,
            };
            if let Some(hop) = parse_line(self.tool, &line) {
                // tracepath repeats a hop when the path MTU changes.
                if hop.ttl <= self.last && hop.ip.is_none() {
                    continue;
                }
                if hop.ttl < self.last {
                    continue;
                }
                self.last = hop.ttl;
                return Some(hop);
            }
        }
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Where an address is.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub latitude: f64,
    pub longitude: f64,
    pub city: Option<String>,
    /// ISO 3166 code, `TR`.
    pub country: Option<String>,
}

impl Place {
    /// `Istanbul, TR`, or whichever of them is known.
    pub fn label(&self) -> String {
        match (&self.city, &self.country) {
            (Some(city), Some(country)) => format!("{city}, {country}"),
            (Some(one), None) | (None, Some(one)) => one.clone(),
            (None, None) => format!("{:.2}, {:.2}", self.latitude, self.longitude),
        }
    }
}

/// An IP location database opened from disk.
pub struct Geo {
    reader: maxminddb::Reader<Vec<u8>>,
    pub path: PathBuf,
}

impl Geo {
    pub fn open(path: &Path) -> Result<Self, String> {
        let reader = maxminddb::Reader::open_readfile(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            reader,
            path: path.to_path_buf(),
        })
    }

    /// The first database in `dir` (`*.mmdb`, by name).
    pub fn find(dir: &Path) -> Option<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("mmdb"))
            })
            .collect();
        found.sort();
        found.into_iter().next()
    }

    /// Where `ip` is, when the database knows. Private and other local
    /// addresses are never looked up.
    pub fn locate(&self, ip: IpAddr) -> Option<Place> {
        if is_local(ip) {
            return None;
        }
        let record: maxminddb::geoip2::City = self.reader.lookup(ip).ok()?;
        let location = record.location?;
        let english = |names: Option<std::collections::BTreeMap<&str, &str>>| {
            names.and_then(|n| n.get("en").map(|s| (*s).to_owned()))
        };
        Some(Place {
            latitude: location.latitude?,
            longitude: location.longitude?,
            city: record.city.and_then(|c| english(c.names)),
            country: record.country.and_then(|c| c.iso_code.map(str::to_owned)),
        })
    }
}

/// Where Apix keeps the location database: `<config>/Apix/geo`.
pub fn geo_dir() -> Option<PathBuf> {
    Some(
        match std::env::var_os("APIX_CONFIG_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => dirs::config_dir()?.join("Apix"),
        }
        .join("geo"),
    )
}

/// The database in [`geo_dir`], opened, when there is one.
pub fn default_geo() -> Option<Result<Geo, String>> {
    geo_dir()
        .and_then(|dir| Geo::find(&dir))
        .map(|path| Geo::open(&path))
}

/// The host of a URL (`https://user@api.test:8443/x` → `api.test`, `[::1]` →
/// `::1`), or the text itself when it is already a host.
pub fn host_of(target: &str) -> String {
    let rest = target.trim();
    let rest = rest.split_once("://").map_or(rest, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if let Some(v6) = authority.strip_prefix('[') {
        return v6.split(']').next().unwrap_or_default().to_owned();
    }
    if authority.matches(':').count() == 1 {
        return authority.split(':').next().unwrap_or_default().to_owned();
    }
    authority.to_owned()
}

/// Where the free DB-IP "IP to City Lite" database of a month is published
/// (gzip-compressed). Licensed CC BY 4.0, which asks for the credit
/// [`DBIP_CREDIT`] wherever its data is shown.
pub fn dbip_url(year: i32, month: u32) -> String {
    format!("https://download.db-ip.com/free/dbip-city-lite-{year}-{month:02}.mmdb.gz")
}

pub const DBIP_CREDIT: &str = "IP geolocation by DB-IP (db-ip.com)";

/// Private, loopback, link-local, carrier-grade NAT and other addresses that
/// say nothing about a place.
pub fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || (a == 100 && (64..128).contains(&b)) // 100.64.0.0/10
                || a >= 224
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (first & 0xfe00) == 0xfc00 // unique local
                || (first & 0xffc0) == 0xfe80 // link local
                || (first & 0xff00) == 0xff00 // multicast
        }
    }
}

/// A point on an equirectangular world map `width` × `height` big
/// (longitude −180…180 left to right, latitude 90…−90 top to bottom).
pub fn project(latitude: f64, longitude: f64, width: f64, height: f64) -> (f64, f64) {
    let x = (longitude.clamp(-180.0, 180.0) + 180.0) / 360.0 * width;
    let y = (90.0 - latitude.clamp(-90.0, 90.0)) / 180.0 * height;
    (x, y)
}

/// The great-circle distance between two places, in kilometres.
pub fn distance_km(a: &Place, b: &Place) -> f64 {
    let (la1, lo1) = (a.latitude.to_radians(), a.longitude.to_radians());
    let (la2, lo2) = (b.latitude.to_radians(), b.longitude.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2)
        + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * 6371.0 * h.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hop(ttl: u32, ip: Option<&str>, rtt: Option<f64>) -> Hop {
        Hop {
            ttl,
            ip: ip.map(|s| s.parse().unwrap()),
            rtt_ms: rtt,
        }
    }

    #[test]
    fn tracert_lines() {
        let t = Tool::Tracert;
        assert_eq!(
            parse_line(t, "Tracing route to 1.1.1.1 over a maximum of 8 hops"),
            None
        );
        assert_eq!(parse_line(t, ""), None);
        assert_eq!(
            parse_line(t, "  1    <1 ms    <1 ms    <1 ms  192.168.1.1 "),
            Some(hop(1, Some("192.168.1.1"), Some(0.5)))
        );
        assert_eq!(
            parse_line(t, "  8     *        5 ms     *     212.156.152.62 "),
            Some(hop(8, Some("212.156.152.62"), Some(5.0)))
        );
        // Translated: "İstek zaman aşımına uğradı." in the system code page.
        assert_eq!(
            parse_line(
                t,
                "  9     *        *        *     \u{fffd}stek zaman a\u{fffd}\u{fffd}m\u{fffd}na u\u{fffd}rad\u{fffd}."
            ),
            Some(hop(9, None, None))
        );
        assert_eq!(
            parse_line(t, " 10    12 ms    11 ms    13 ms  2606:4700:4700::1111 "),
            Some(hop(10, Some("2606:4700:4700::1111"), Some(11.0)))
        );
        assert_eq!(parse_line(t, "Trace complete."), None);
    }

    #[test]
    fn traceroute_and_tracepath_lines() {
        let t = Tool::Traceroute;
        assert_eq!(
            parse_line(
                t,
                "traceroute to 1.1.1.1 (1.1.1.1), 30 hops max, 60 byte packets"
            ),
            None
        );
        assert_eq!(
            parse_line(t, " 1  192.168.1.1  0.512 ms"),
            Some(hop(1, Some("192.168.1.1"), Some(0.512)))
        );
        assert_eq!(parse_line(t, " 2  *"), Some(hop(2, None, None)));
        assert_eq!(
            parse_line(t, " 3  10.0.0.1  1.2 ms !H"),
            Some(hop(3, Some("10.0.0.1"), Some(1.2)))
        );
        let p = Tool::Tracepath;
        assert_eq!(
            parse_line(p, " 1?: [LOCALHOST]                      pmtu 1500"),
            Some(hop(1, None, None))
        );
        assert_eq!(
            parse_line(
                p,
                " 1:  192.168.1.1                                           0.398ms "
            ),
            Some(hop(1, Some("192.168.1.1"), Some(0.398)))
        );
        assert_eq!(parse_line(p, " 5:  no reply"), Some(hop(5, None, None)));
        assert_eq!(parse_line(p, "     Resume: pmtu 1500 hops 9 back 9 "), None);
    }

    #[test]
    fn local_addresses() {
        for local in [
            "192.168.1.1",
            "10.2.3.4",
            "172.16.0.1",
            "127.0.0.1",
            "100.95.0.1",
            "169.254.1.1",
            "::1",
            "fe80::1",
            "fd00::5",
        ] {
            assert!(is_local(local.parse().unwrap()), "{local}");
        }
        for public in ["1.1.1.1", "81.212.73.33", "100.128.0.1", "2606:4700::1111"] {
            assert!(!is_local(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn map_and_distance() {
        assert_eq!(project(0.0, 0.0, 360.0, 180.0), (180.0, 90.0));
        assert_eq!(project(90.0, -180.0, 360.0, 180.0), (0.0, 0.0));
        assert_eq!(project(-90.0, 180.0, 360.0, 180.0), (360.0, 180.0));
        let place = |latitude, longitude| Place {
            latitude,
            longitude,
            city: None,
            country: None,
        };
        // Istanbul to Frankfurt is about 1 860 km.
        let d = distance_km(&place(41.01, 28.98), &place(50.11, 8.68));
        assert!((1800.0..1920.0).contains(&d), "{d}");
        assert_eq!(
            Place {
                city: Some("Istanbul".into()),
                country: Some("TR".into()),
                ..place(0.0, 0.0)
            }
            .label(),
            "Istanbul, TR"
        );
        assert!(dbip_url(2026, 3).ends_with("dbip-city-lite-2026-03.mmdb.gz"));
        assert_eq!(host_of("https://user@api.test:8443/x?y"), "api.test");
        assert_eq!(host_of("http://[::1]:80/"), "::1");
        assert_eq!(host_of("example.com"), "example.com");
        assert_eq!(host_of("2606:4700::1111"), "2606:4700::1111");
        assert_eq!(host_of("1.1.1.1"), "1.1.1.1");
    }
}

pub mod land;

/// The year and month (1–12) of a moment, in UTC.
pub fn year_month(unix_secs: u64) -> (i32, u32) {
    // Howard Hinnant's days-to-civil.
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month)
}

/// The DB-IP database addresses to try now: this month's, then last month's
/// (a month's file appears in its first days). `APIX_GEO_URL` replaces them.
pub fn dbip_urls(unix_secs: u64) -> Vec<String> {
    if let Ok(url) = std::env::var("APIX_GEO_URL")
        && !url.trim().is_empty()
    {
        return vec![url];
    }
    let (year, month) = year_month(unix_secs);
    let (last_year, last_month) = if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    };
    vec![dbip_url(year, month), dbip_url(last_year, last_month)]
}

/// Keeps a downloaded database in `dir` as `name`: unpacks it when it is
/// gzip-compressed, checks that it opens, and only then replaces the
/// databases kept before.
pub fn install(bytes: &[u8], dir: &Path, name: &str) -> Result<PathBuf, String> {
    let data = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(bytes), &mut out)
            .map_err(|e| format!("unpacking: {e}"))?;
        out
    } else {
        bytes.to_vec()
    };
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let partial = dir.join(format!("{name}.part"));
    std::fs::write(&partial, &data).map_err(|e| e.to_string())?;
    if let Err(e) = maxminddb::Reader::from_source(data) {
        let _ = std::fs::remove_file(&partial);
        return Err(format!("not an IP location database: {e}"));
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for old in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            if old
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mmdb"))
            {
                let _ = std::fs::remove_file(old);
            }
        }
    }
    let path = dir.join(name);
    std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod date_tests {
    use super::*;

    #[test]
    fn months() {
        assert_eq!(year_month(0), (1970, 1));
        assert_eq!(year_month(1_709_251_200), (2024, 3)); // 2024-03-01
        assert_eq!(year_month(1_709_251_199), (2024, 2)); // 2024-02-29 23:59:59
        assert_eq!(year_month(1_798_761_600), (2027, 1)); // 2027-01-01
        let urls = dbip_urls(1_798_761_600);
        if std::env::var("APIX_GEO_URL").is_err() {
            assert!(urls[0].ends_with("2027-01.mmdb.gz"));
            assert!(urls[1].ends_with("2026-12.mmdb.gz"));
        }
    }
}
