//! HTTP header names and their usual values, to suggest while typing a
//! request's headers.

/// Request headers people set by hand, the common ones first.
pub const NAMES: &[&str] = &[
    "Accept",
    "Authorization",
    "Content-Type",
    "User-Agent",
    "Accept-Encoding",
    "Accept-Language",
    "Cache-Control",
    "Cookie",
    "Origin",
    "Referer",
    "X-Requested-With",
    "X-API-Key",
    "X-Request-ID",
    "X-Correlation-ID",
    "X-Forwarded-For",
    "X-Forwarded-Host",
    "X-Forwarded-Proto",
    "Accept-Charset",
    "Access-Control-Request-Headers",
    "Access-Control-Request-Method",
    "Connection",
    "Content-Disposition",
    "Content-Encoding",
    "Content-Language",
    "Content-Length",
    "Content-MD5",
    "Date",
    "DNT",
    "Expect",
    "Forwarded",
    "From",
    "Host",
    "If-Match",
    "If-Modified-Since",
    "If-None-Match",
    "If-Range",
    "If-Unmodified-Since",
    "Idempotency-Key",
    "Keep-Alive",
    "Max-Forwards",
    "Pragma",
    "Prefer",
    "Proxy-Authorization",
    "Range",
    "Sec-Fetch-Dest",
    "Sec-Fetch-Mode",
    "Sec-Fetch-Site",
    "SOAPAction",
    "TE",
    "Traceparent",
    "Tracestate",
    "Upgrade",
    "Upgrade-Insecure-Requests",
    "Via",
    "Warning",
    "X-CSRF-Token",
    "X-HTTP-Method-Override",
    "X-Real-IP",
];

/// The usual values of a header, by its name (any case).
pub fn values_of(name: &str) -> &'static [&'static str] {
    match name.trim().to_ascii_lowercase().as_str() {
        "accept" => &[
            "application/json",
            "*/*",
            "application/xml",
            "text/html",
            "text/plain",
            "text/event-stream",
            "application/json, text/plain, */*",
        ],
        "content-type" => &[
            "application/json",
            "application/x-www-form-urlencoded",
            "multipart/form-data",
            "text/plain",
            "application/xml",
            "text/xml",
            "text/html",
            "application/octet-stream",
            "application/graphql",
            "application/soap+xml",
        ],
        "authorization" => &["Bearer {{token}}", "Basic "],
        "accept-encoding" => &["gzip, deflate, br", "gzip", "identity"],
        "accept-language" => &["en-US,en;q=0.9", "tr-TR,tr;q=0.9", "*"],
        "cache-control" => &["no-cache", "no-store", "max-age=0"],
        "connection" => &["keep-alive", "close"],
        "pragma" => &["no-cache"],
        "x-requested-with" => &["XMLHttpRequest"],
        "prefer" => &["return=representation", "return=minimal", "respond-async"],
        "expect" => &["100-continue"],
        "dnt" | "upgrade-insecure-requests" => &["1"],
        "sec-fetch-mode" => &["cors", "navigate", "no-cors", "same-origin"],
        "sec-fetch-site" => &["same-origin", "same-site", "cross-site", "none"],
        "sec-fetch-dest" => &["empty", "document"],
        "content-encoding" => &["gzip", "deflate", "br"],
        "x-forwarded-proto" => &["https", "http"],
        "access-control-request-method" => &["GET", "POST", "PUT", "PATCH", "DELETE"],
        _ => &[],
    }
}

/// What fits what is typed: those starting with it first (case ignored),
/// then those containing it, each list in its own order. Nothing when the
/// text is already one of them, or empty and `when_empty` is false.
pub fn suggest(
    choices: &[&'static str],
    typed: &str,
    when_empty: bool,
    most: usize,
) -> Vec<&'static str> {
    let typed = typed.trim().to_ascii_lowercase();
    if typed.is_empty() && !when_empty {
        return Vec::new();
    }
    if choices.iter().any(|c| c.to_ascii_lowercase() == typed) {
        return Vec::new();
    }
    let lower: Vec<String> = choices.iter().map(|c| c.to_ascii_lowercase()).collect();
    let starting = choices
        .iter()
        .zip(&lower)
        .filter(|(_, l)| l.starts_with(&typed));
    let containing = choices
        .iter()
        .zip(&lower)
        .filter(|(_, l)| !l.starts_with(&typed) && l.contains(&typed));
    starting
        .chain(containing)
        .map(|(c, _)| *c)
        .take(most)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_start_with_it_come_first() {
        assert_eq!(
            suggest(NAMES, "con", false, 4),
            [
                "Content-Type",
                "Connection",
                "Content-Disposition",
                "Content-Encoding"
            ]
        );
        let forwarded = suggest(NAMES, "forw", false, 10);
        assert_eq!(forwarded[0], "Forwarded");
        assert!(forwarded.contains(&"X-Forwarded-For"));
        assert!(suggest(NAMES, "", false, 5).is_empty());
        assert_eq!(suggest(NAMES, "", true, 2), ["Accept", "Authorization"]);
        // Once typed in full there is nothing left to suggest.
        assert!(suggest(NAMES, "content-type", false, 5).is_empty());
        assert!(suggest(NAMES, "zzz", false, 5).is_empty());
    }

    #[test]
    fn values_follow_the_name() {
        let json = suggest(values_of("content-type"), "json", true, 5);
        assert_eq!(json, ["application/json"]);
        assert_eq!(
            suggest(values_of(" Accept "), "", true, 2),
            ["application/json", "*/*"]
        );
        assert!(values_of("X-Custom").is_empty());
    }
}
