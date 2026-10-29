//! Sent requests and their responses, kept per machine in the app's config
//! folder (never inside a collection, so history stays out of git):
//!
//! ```text
//! history/
//! ├── index.jsonl            one Summary per line, oldest first
//! ├── <id>.json              the request (as TOML text) and the response's
//! │                          status, headers and timing
//! └── <id>.body              the response body, cut at MAX_BODY bytes
//! ```
//!
//! The request is stored as it was edited, with its `{{variables}}`: values
//! from environments (tokens, passwords) are never written here.

use std::fs;
use std::io::{self, BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::model::{Method, Request};

/// Entries kept; older ones are dropped.
pub const MAX_ENTRIES: usize = 300;
/// Bytes of each response body kept.
pub const MAX_BODY: usize = 256 * 1024;
/// The index is rewritten once it is this far over `MAX_ENTRIES`, not on
/// every send.
const SLACK: usize = 50;

const INDEX: &str = "index.jsonl";

/// One line of the history list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub id: String,
    /// When it was sent, in milliseconds since the Unix epoch.
    pub at: u64,
    pub method: Method,
    /// The URL as written, `{{variables}}` included.
    pub url: String,
    pub name: String,
    /// The request file it was sent from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// `None` when no response arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Until the last byte (or the failure), in milliseconds.
    pub duration_ms: u64,
    /// The full body size, even when less was kept.
    #[serde(default)]
    pub size: u64,
    /// A gRPC call: `status` is its status code (0 is OK) and the body is
    /// the call's log.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grpc: bool,
}

/// What came back, without the body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredResponse {
    pub status: u16,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub ttfb_us: u64,
    pub total_us: u64,
    /// Whether the body was cut at `MAX_BODY`.
    #[serde(default)]
    pub truncated: bool,
    /// A gRPC call's trailers (its response metadata is in `headers`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailers: Vec<(String, String)>,
    /// A gRPC call's status message.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
}

/// A whole history entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub summary: Summary,
    pub request: Request,
    pub response: Option<StoredResponse>,
    pub body: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct EntryFile {
    summary: Summary,
    /// TOML, exactly like a `.apix` file.
    request: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<StoredResponse>,
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

/// A sortable, unique-enough id: the time, then a few random hex digits.
pub fn new_id(at_ms: u64) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or_default();
    let mix = (nanos ^ std::process::id().rotate_left(16)) & 0xffff;
    format!("{at_ms:013}-{mix:04x}")
}

#[derive(Debug, Clone)]
pub struct History {
    dir: PathBuf,
}

impl History {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, id: &str, ext: &str) -> PathBuf {
        self.dir.join(format!("{id}.{ext}"))
    }

    /// Saves `entry` (its body cut at `MAX_BODY`) and drops the oldest
    /// entries beyond `MAX_ENTRIES`.
    pub fn record(&self, entry: &Entry) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let id = &entry.summary.id;
        let request = toml::to_string(&entry.request).map_err(io::Error::other)?;
        let mut response = entry.response.clone();
        let body = &entry.body[..entry.body.len().min(MAX_BODY)];
        if let Some(r) = response.as_mut() {
            r.truncated |= body.len() < entry.body.len();
        }
        let file = EntryFile {
            summary: entry.summary.clone(),
            request,
            response,
        };
        fs::write(
            self.file(id, "json"),
            serde_json::to_vec(&file).map_err(io::Error::other)?,
        )?;
        if !body.is_empty() {
            fs::write(self.file(id, "body"), body)?;
        }
        let mut line = serde_json::to_string(&entry.summary).map_err(io::Error::other)?;
        line.push('\n');
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(INDEX))?
            .write_all(line.as_bytes())?;
        self.prune(MAX_ENTRIES)
    }

    /// Oldest first; lines that cannot be read are skipped.
    fn read_index(&self) -> Vec<Summary> {
        let Ok(file) = fs::File::open(self.dir.join(INDEX)) else {
            return Vec::new();
        };
        io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect()
    }

    fn write_index(&self, entries: &[Summary]) -> io::Result<()> {
        let mut text = String::new();
        for e in entries {
            text.push_str(&serde_json::to_string(e).map_err(io::Error::other)?);
            text.push('\n');
        }
        let tmp = self.dir.join(format!("{INDEX}.tmp"));
        fs::write(&tmp, text)?;
        fs::rename(tmp, self.dir.join(INDEX))
    }

    fn remove_files(&self, id: &str) {
        let _ = fs::remove_file(self.file(id, "json"));
        let _ = fs::remove_file(self.file(id, "body"));
    }

    fn prune(&self, keep: usize) -> io::Result<()> {
        let entries = self.read_index();
        if entries.len() <= keep + SLACK {
            return Ok(());
        }
        let cut = entries.len() - keep;
        for old in &entries[..cut] {
            self.remove_files(&old.id);
        }
        self.write_index(&entries[cut..])
    }

    /// Newest first.
    pub fn list(&self) -> Vec<Summary> {
        let mut entries = self.read_index();
        entries.reverse();
        entries.truncate(MAX_ENTRIES);
        entries
    }

    pub fn load(&self, id: &str) -> Option<Entry> {
        let text = fs::read(self.file(id, "json")).ok()?;
        let file: EntryFile = serde_json::from_slice(&text).ok()?;
        let request = toml::from_str(&file.request).ok()?;
        let body = fs::read(self.file(id, "body")).unwrap_or_default();
        Some(Entry {
            summary: file.summary,
            request,
            response: file.response,
            body,
        })
    }

    pub fn remove(&self, id: &str) -> io::Result<()> {
        self.remove_files(id);
        let entries: Vec<Summary> = self
            .read_index()
            .into_iter()
            .filter(|e| e.id != id)
            .collect();
        self.write_index(&entries)
    }

    /// Removes every entry.
    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Auth, Body};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apix-history-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn entry(n: u64, body: &[u8]) -> Entry {
        let mut request = Request::new(Method::Post, "{{base}}/users");
        request.name = format!("Create {n}");
        request.auth = Auth::Bearer {
            token: "{{token}}".into(),
        };
        request.body = Body::Json("{\"n\": 1}".into());
        Entry {
            summary: Summary {
                id: format!("{n:013}-0000"),
                at: n,
                method: Method::Post,
                url: request.url.clone(),
                name: request.name.clone(),
                path: Some(PathBuf::from("users/create.apix")),
                status: Some(201),
                error: None,
                duration_ms: 12,
                size: body.len() as u64,
                grpc: false,
            },
            request,
            response: Some(StoredResponse {
                status: 201,
                version: "HTTP/1.1".into(),
                headers: vec![("Content-Type".into(), "application/json".into())],
                ttfb_us: 9_000,
                total_us: 12_000,
                truncated: false,
                trailers: Vec::new(),
                message: String::new(),
            }),
            body: body.to_vec(),
        }
    }

    #[test]
    fn keeps_grpc_calls() {
        let history = History::new(temp("grpc"));
        let mut call = entry(3, br#"[{"at_ms":0,"sent":true,"message":{}}]"#);
        call.summary.grpc = true;
        call.summary.status = Some(5);
        let response = call.response.as_mut().unwrap();
        response.status = 5;
        response.trailers = vec![("x-cause".into(), "gone".into())];
        response.message = "no such user".into();
        history.record(&call).unwrap();
        let id = history.list()[0].id.clone();
        assert_eq!(history.load(&id).unwrap(), call);
        // HTTP entries do not write the gRPC fields.
        history.record(&entry(4, b"")).unwrap();
        let http = fs::read_to_string(history.dir().join(format!("{:013}-0000.json", 4))).unwrap();
        assert!(
            !http.contains("grpc") && !http.contains("trailers"),
            "{http}"
        );
        fs::remove_dir_all(history.dir()).ok();
    }

    #[test]
    fn records_lists_and_loads() {
        let history = History::new(temp("basic"));
        assert!(
            history.list().is_empty(),
            "a missing folder is an empty history"
        );
        history.record(&entry(1, b"{\"id\":1}")).unwrap();
        history.record(&entry(2, b"")).unwrap();

        let list = history.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Create 2", "newest first");

        let loaded = history.load(&list[1].id).unwrap();
        assert_eq!(loaded, entry(1, b"{\"id\":1}"));
        // Variables stay variables: no resolved secrets on disk.
        let json = fs::read_to_string(history.dir().join(format!("{}.json", list[1].id))).unwrap();
        assert!(json.contains("{{token}}"), "{json}");

        history.remove(&list[0].id).unwrap();
        assert_eq!(history.list().len(), 1);
        assert!(history.load(&list[0].id).is_none());
        history.clear().unwrap();
        assert!(history.list().is_empty());
        history.clear().unwrap();
    }

    #[test]
    fn cuts_big_bodies_and_drops_old_entries() {
        let history = History::new(temp("prune"));
        let big = vec![b'x'; MAX_BODY + 10];
        history.record(&entry(1, &big)).unwrap();
        let loaded = history.load(&history.list()[0].id).unwrap();
        assert_eq!(loaded.body.len(), MAX_BODY);
        assert!(loaded.response.unwrap().truncated);
        assert_eq!(
            loaded.summary.size,
            big.len() as u64,
            "the real size is kept"
        );

        for n in 2..=(MAX_ENTRIES + SLACK + 2) as u64 {
            history.record(&entry(n, b"{}")).unwrap();
        }
        let list = history.list();
        assert_eq!(list.len(), MAX_ENTRIES);
        assert_eq!(list[0].at, (MAX_ENTRIES + SLACK + 2) as u64);
        // The first entry was pruned, files included.
        assert!(history.load(&format!("{:013}-0000", 1)).is_none());
        let files = fs::read_dir(history.dir()).unwrap().count();
        assert!(files <= 2 * (MAX_ENTRIES + SLACK) + 1, "{files} files");
        history.clear().unwrap();
    }

    #[test]
    fn ids_sort_by_time() {
        assert!(new_id(5) < new_id(40));
        assert_eq!(new_id(1_700_000_000_000).len(), 18);
    }
}
