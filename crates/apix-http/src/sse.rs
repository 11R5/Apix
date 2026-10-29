//! Server-sent events (`text/event-stream`), read as they arrive, following
//! the WHATWG HTML spec's event-stream format: `field: value` lines, an
//! event per blank line, `:` comments, and any line ending.

/// One event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event:` type, `message` when none was given.
    pub event: String,
    /// The `data:` lines, joined with `\n`.
    pub data: String,
    /// The last `id:` seen so far (it carries over to later events).
    pub id: Option<String>,
    /// A `retry:` reconnection time sent with this event, in milliseconds.
    pub retry: Option<u64>,
}

/// Reads an event stream piece by piece; pieces may split anywhere, even
/// inside a UTF-8 character or between `\r` and `\n`.
#[derive(Debug, Default)]
pub struct SseParser {
    /// Bytes of a line that has not ended yet.
    pending: Vec<u8>,
    /// The pending `\r` might be the first half of `\r\n`.
    after_cr: bool,
    event: String,
    data: String,
    has_data: bool,
    retry: Option<u64>,
    last_id: Option<String>,
    /// The last `retry:` seen, which outlives the event it came with.
    reconnection_ms: Option<u64>,
    /// The stream's first line may start with a byte order mark.
    started: bool,
}

/// How long to wait before reconnecting when the server never said.
pub const DEFAULT_RETRY_MS: u64 = 3000;

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// The last event id, to send back as `Last-Event-ID` when reconnecting.
    pub fn last_id(&self) -> Option<&str> {
        self.last_id.as_deref().filter(|id| !id.is_empty())
    }

    /// How long to wait before reconnecting: the server's last `retry:`, or
    /// [`DEFAULT_RETRY_MS`].
    pub fn reconnection_time(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.reconnection_ms.unwrap_or(DEFAULT_RETRY_MS))
    }

    /// A parser for the next connection of the same stream: the id and
    /// reconnection time carry over, a half-read event does not.
    pub fn reconnected(&self) -> Self {
        Self {
            last_id: self.last_id.clone(),
            reconnection_ms: self.reconnection_ms,
            started: false,
            ..Self::default()
        }
    }

    /// The events completed by `chunk`.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let mut out = Vec::new();
        for &byte in chunk {
            if self.after_cr {
                self.after_cr = false;
                if byte == b'\n' {
                    continue;
                }
            }
            match byte {
                b'\r' => {
                    self.after_cr = true;
                    self.end_line(&mut out);
                }
                b'\n' => self.end_line(&mut out),
                _ => self.pending.push(byte),
            }
        }
        out
    }

    fn end_line(&mut self, out: &mut Vec<SseEvent>) {
        let raw = std::mem::take(&mut self.pending);
        let mut line = String::from_utf8_lossy(&raw).into_owned();
        if !self.started {
            self.started = true;
            if let Some(rest) = line.strip_prefix('\u{feff}') {
                line = rest.to_owned();
            }
        }
        if line.is_empty() {
            self.dispatch(out);
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_str(), ""),
        };
        match field {
            "event" => self.event = value.to_owned(),
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
                self.has_data = true;
            }
            "id" if !value.contains('\0') => self.last_id = Some(value.to_owned()),
            "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                self.retry = value.parse().ok();
                self.reconnection_ms = self.retry;
            }
            _ => {}
        }
    }

    fn dispatch(&mut self, out: &mut Vec<SseEvent>) {
        let event = std::mem::take(&mut self.event);
        let retry = self.retry.take();
        if !self.has_data {
            return;
        }
        self.has_data = false;
        let mut data = std::mem::take(&mut self.data);
        data.pop();
        out.push(SseEvent {
            event: if event.is_empty() {
                "message".into()
            } else {
                event
            },
            data,
            id: self.last_id.clone(),
            retry,
        });
    }
}

/// Every complete event in a whole stream. A last event without the blank
/// line that ends it is dropped, as browsers do.
pub fn parse_all(body: &[u8]) -> Vec<SseEvent> {
    SseParser::new().feed(body)
}

/// `req` for reconnecting to a stream: with `Last-Event-ID` when an id was
/// seen (replacing one already there).
pub fn reconnect_request(req: &apix_core::Request, last_id: Option<&str>) -> apix_core::Request {
    let mut req = req.clone();
    if let Some(id) = last_id {
        req.headers
            .retain(|h| !h.key.eq_ignore_ascii_case("last-event-id"));
        req.headers
            .push(apix_core::KeyValue::new("Last-Event-ID", id));
    }
    req
}

/// Whether a `Content-Type` value is an event stream.
pub fn is_event_stream(content_type: &str) -> bool {
    content_type
        .split(';')
        .next()
        .is_some_and(|t| t.trim().eq_ignore_ascii_case("text/event-stream"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(event: &str, data: &str, id: Option<&str>) -> SseEvent {
        SseEvent {
            event: event.into(),
            data: data.into(),
            id: id.map(Into::into),
            retry: None,
        }
    }

    #[test]
    fn reads_the_spec_examples() {
        let stream = b"\xEF\xBB\xBF: comment\n\ndata: first\ndata: second\n\nevent: add\ndata:73857293\nid: 7\n\ndata\n\ndata:  two spaces\n\nevent: ignored without data\n\n";
        assert_eq!(
            parse_all(stream),
            [
                ev("message", "first\nsecond", None),
                ev("add", "73857293", Some("7")),
                ev("message", "", Some("7")),
                ev("message", " two spaces", Some("7")),
            ]
        );
    }

    #[test]
    fn pieces_may_split_anywhere() {
        let stream =
            "event: ping\r\ndata: héllo\r\nretry: 3000\r\n\r\ndata: x\r\rdata: y\n\n".as_bytes();
        let whole = parse_all(stream);
        for size in 1..stream.len() {
            let mut parser = SseParser::new();
            let mut got = Vec::new();
            for piece in stream.chunks(size) {
                got.extend(parser.feed(piece));
            }
            assert_eq!(got, whole, "pieces of {size}");
        }
        assert_eq!(whole[0].data, "héllo");
        assert_eq!(whole[0].retry, Some(3000));
        assert_eq!(whole[1].data, "x");
        assert_eq!(whole[2].data, "y");
    }

    #[test]
    fn what_carries_over_to_a_reconnection() {
        let mut parser = SseParser::new();
        assert_eq!(parser.reconnection_time().as_millis(), 3000);
        assert_eq!(parser.last_id(), None);
        // `retry:` without data still counts; the id carries over.
        parser.feed(b"retry: 500\n\nid: 41\ndata: a\n\ndata: half");
        assert_eq!(parser.reconnection_time().as_millis(), 500);
        assert_eq!(parser.last_id(), Some("41"));
        let mut next = parser.reconnected();
        assert_eq!(next.last_id(), Some("41"));
        assert_eq!(next.reconnection_time().as_millis(), 500);
        // The half-read event from before is gone.
        assert_eq!(next.feed(b"data: b\n\n")[0].data, "b");

        let mut req = apix_core::Request::new(apix_core::Method::Get, "http://x/");
        req.headers
            .push(apix_core::KeyValue::new("last-event-id", "old"));
        let again = reconnect_request(&req, Some("41"));
        let ids: Vec<_> = again
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("last-event-id"))
            .map(|h| h.value.as_str())
            .collect();
        assert_eq!(ids, ["41"]);
        assert_eq!(reconnect_request(&req, None), req);
    }

    #[test]
    fn an_unfinished_event_is_dropped() {
        assert!(parse_all(b"data: no blank line").is_empty());
    }

    #[test]
    fn content_types() {
        assert!(is_event_stream("text/event-stream"));
        assert!(is_event_stream("Text/Event-Stream; charset=utf-8"));
        assert!(!is_event_stream("text/plain"));
    }
}
