//! Signing for the auth types that compute their header per request: AWS
//! Signature Version 4, OAuth 1.0a and HTTP Digest, plus PKCE and the random
//! values they need. Pure functions, so the HTTP engine and "Copy as" produce
//! the same signatures, and the published test vectors can check them.

use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::{Digest as _, Sha256};
use url::Url;

use crate::model::{AwsSigV4, OAuth1, OAuth1Method};

/// The payload hash for a body that cannot be hashed up front (a stream).
pub const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";

/// Seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// `bytes` random bytes from the OS, as URL-safe base64 without padding.
pub fn random_token(bytes: usize) -> String {
    use rand_core::{OsRng, RngCore as _};
    let mut buf = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

/// The PKCE `code_challenge` for `verifier` (method `S256`, RFC 7636).
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// Percent-encodes everything but RFC 3986's unreserved characters, with
/// upper-case hex, as AWS and OAuth 1.0 both require.
pub fn encode_rfc3986(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `host` or `host:port` when the port is not the scheme's default.
pub fn host_header(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default();
    match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    }
}

/// `20150830T123600Z`
fn amz_date(unix_secs: u64) -> String {
    let iso = crate::dynamic::iso8601(unix_secs as i64 * 1000);
    let compact: String = iso.chars().filter(|c| !matches!(c, '-' | ':')).collect();
    format!("{}Z", &compact[..15])
}

/// The headers that make AWS accept the request: `X-Amz-Date`, for S3
/// `X-Amz-Content-Sha256`, `X-Amz-Security-Token` with temporary credentials,
/// and `Authorization`. `payload_hash` is [`sha256_hex`] of the body, or
/// [`UNSIGNED_PAYLOAD`].
pub fn aws_sigv4(
    aws: &AwsSigV4,
    method: &str,
    url: &Url,
    content_type: Option<&str>,
    payload_hash: &str,
    unix_secs: u64,
) -> Vec<(String, String)> {
    let stamp = amz_date(unix_secs);
    let date = &stamp[..8];
    let s3 = aws.service.eq_ignore_ascii_case("s3");

    let mut signed: Vec<(&str, String)> =
        vec![("host", host_header(url)), ("x-amz-date", stamp.clone())];
    if let Some(content_type) = content_type {
        signed.push(("content-type", content_type.trim().to_owned()));
    }
    if s3 {
        signed.push(("x-amz-content-sha256", payload_hash.to_owned()));
    }
    if !aws.session_token.is_empty() {
        signed.push(("x-amz-security-token", aws.session_token.clone()));
    }
    signed.sort_by(|a, b| a.0.cmp(b.0));
    let headers: String = signed
        .iter()
        .map(|(k, v)| {
            format!(
                "{k}:{}\n",
                v.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect();
    let names = signed.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(";");

    let canonical = format!(
        "{}\n{}\n{}\n{headers}\n{names}\n{payload_hash}",
        method.to_ascii_uppercase(),
        canonical_uri(url, s3),
        canonical_query(url),
    );
    let scope = format!("{date}/{}/{}/aws4_request", aws.region, aws.service);
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{stamp}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );
    let mut key = hmac_sha256(format!("AWS4{}", aws.secret_key).as_bytes(), date);
    for part in [aws.region.as_str(), aws.service.as_str(), "aws4_request"] {
        key = hmac_sha256(&key, part);
    }
    let signature = hex(&hmac_sha256(&key, &to_sign));

    let mut out = vec![("X-Amz-Date".to_owned(), stamp)];
    if s3 {
        out.push(("X-Amz-Content-Sha256".to_owned(), payload_hash.to_owned()));
    }
    if !aws.session_token.is_empty() {
        out.push(("X-Amz-Security-Token".to_owned(), aws.session_token.clone()));
    }
    out.push((
        "Authorization".to_owned(),
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={names}, Signature={signature}",
            aws.access_key
        ),
    ));
    out
}

/// Each path segment encoded by AWS's rules; twice, except for S3.
fn canonical_uri(url: &Url, s3: bool) -> String {
    let path = url.path();
    if path.is_empty() || path == "/" {
        return "/".into();
    }
    path.split('/')
        .map(|segment| {
            let once = encode_rfc3986(&percent_decode(segment));
            if s3 { once } else { encode_rfc3986(&once) }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn canonical_query(url: &Url) -> String {
    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (encode_rfc3986(&k), encode_rfc3986(&v)))
        .collect();
    pairs.sort();
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// The `oauth_*` parameters for one request, `oauth_signature` last.
/// `form` holds the fields of a URL-encoded body, which are signed too.
pub fn oauth1_params(
    o: &OAuth1,
    method: &str,
    url: &Url,
    form: &[(String, String)],
    unix_secs: u64,
    nonce: &str,
) -> Vec<(String, String)> {
    let mut oauth: Vec<(String, String)> = vec![
        ("oauth_consumer_key".into(), o.consumer_key.clone()),
        ("oauth_nonce".into(), nonce.to_owned()),
        (
            "oauth_signature_method".into(),
            o.signature_method.as_str().into(),
        ),
        ("oauth_timestamp".into(), unix_secs.to_string()),
    ];
    if !o.token.is_empty() {
        oauth.push(("oauth_token".into(), o.token.clone()));
    }
    oauth.push(("oauth_version".into(), "1.0".into()));

    let mut params: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .chain(form.iter().cloned())
        .chain(oauth.iter().cloned())
        .map(|(k, v)| (encode_rfc3986(&k), encode_rfc3986(&v)))
        .collect();
    params.sort();
    let normalized = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    let base_url = format!(
        "{}://{}{}",
        url.scheme(),
        host_header(url).to_ascii_lowercase(),
        url.path()
    );
    let base = format!(
        "{}&{}&{}",
        method.to_ascii_uppercase(),
        encode_rfc3986(&base_url),
        encode_rfc3986(&normalized)
    );
    let key = format!(
        "{}&{}",
        encode_rfc3986(&o.consumer_secret),
        encode_rfc3986(&o.token_secret)
    );
    let signature = match o.signature_method {
        OAuth1Method::HmacSha1 => {
            let mut mac =
                Hmac::<sha1::Sha1>::new_from_slice(key.as_bytes()).expect("any key length");
            mac.update(base.as_bytes());
            STANDARD.encode(mac.finalize().into_bytes())
        }
        OAuth1Method::HmacSha256 => STANDARD.encode(hmac_sha256(key.as_bytes(), &base)),
        OAuth1Method::Plaintext => key,
    };
    oauth.push(("oauth_signature".into(), signature));
    oauth
}

/// `OAuth realm="…", oauth_consumer_key="…", …`
pub fn oauth1_header(params: &[(String, String)], realm: &str) -> String {
    let mut parts = Vec::with_capacity(params.len() + 1);
    if !realm.is_empty() {
        parts.push(format!("realm=\"{}\"", encode_rfc3986(realm)));
    }
    parts.extend(
        params
            .iter()
            .map(|(k, v)| format!("{k}=\"{}\"", encode_rfc3986(v))),
    );
    format!("OAuth {}", parts.join(", "))
}

/// A server's `WWW-Authenticate: Digest …` challenge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DigestChallenge {
    pub realm: String,
    pub nonce: String,
    pub opaque: Option<String>,
    /// As the server wrote it, e.g. `MD5` or `SHA-256-sess`. Empty means MD5.
    pub algorithm: String,
    /// The offered qop values, e.g. `["auth", "auth-int"]`.
    pub qop: Vec<String>,
}

/// Reads the Digest challenge out of a `WWW-Authenticate` value, which may
/// also hold challenges for other schemes.
pub fn parse_digest_challenge(header: &str) -> Option<DigestChallenge> {
    let start = header.to_ascii_lowercase().find("digest ")?;
    let mut rest = &header[start + 7..];
    let mut challenge = DigestChallenge::default();
    loop {
        rest = rest.trim_start_matches([' ', ',', '\t']);
        let Some(eq) = rest.find('=') else { break };
        let name = rest[..eq].trim();
        // A word without `=` before it starts the next scheme's challenge.
        if name.is_empty() || name.contains(' ') {
            break;
        }
        rest = &rest[eq + 1..];
        let value = if let Some(quoted) = rest.strip_prefix('"') {
            let mut value = String::new();
            let mut chars = quoted.char_indices();
            let mut end = quoted.len();
            while let Some((i, c)) = chars.next() {
                match c {
                    '\\' => value.extend(chars.next().map(|(_, c)| c)),
                    '"' => {
                        end = i + 1;
                        break;
                    }
                    c => value.push(c),
                }
            }
            rest = &quoted[end..];
            value
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            let value = rest[..end].trim().to_owned();
            rest = &rest[end..];
            value
        };
        match name.to_ascii_lowercase().as_str() {
            "realm" => challenge.realm = value,
            "nonce" => challenge.nonce = value,
            "opaque" => challenge.opaque = Some(value),
            "algorithm" => challenge.algorithm = value,
            "qop" => {
                challenge.qop = value
                    .split(',')
                    .map(|q| q.trim().to_owned())
                    .filter(|q| !q.is_empty())
                    .collect()
            }
            _ => {}
        }
    }
    (!challenge.nonce.is_empty()).then_some(challenge)
}

/// The `Authorization` value answering `challenge`. `uri` is the request
/// target (path and query); `cnonce` is a fresh random value.
pub fn digest_authorization(
    challenge: &DigestChallenge,
    username: &str,
    password: &str,
    method: &str,
    uri: &str,
    cnonce: &str,
) -> Result<String, String> {
    let algorithm = if challenge.algorithm.is_empty() {
        "MD5"
    } else {
        challenge.algorithm.as_str()
    };
    let upper = algorithm.to_ascii_uppercase();
    let (base, sess) = match upper.strip_suffix("-SESS") {
        Some(base) => (base.to_owned(), true),
        None => (upper.clone(), false),
    };
    let hash = |text: &str| -> String {
        if base == "MD5" {
            hex(&md5::Md5::digest(text.as_bytes()))
        } else {
            sha256_hex(text.as_bytes())
        }
    };
    if base != "MD5" && base != "SHA-256" {
        return Err(format!("Digest algorithm {algorithm} is not supported"));
    }
    let qop = if challenge.qop.is_empty() {
        None
    } else if challenge.qop.iter().any(|q| q.eq_ignore_ascii_case("auth")) {
        Some("auth")
    } else {
        return Err("the server only offers Digest qop=auth-int, which is not supported".into());
    };

    let nc = "00000001";
    let mut ha1 = hash(&format!("{username}:{}:{password}", challenge.realm));
    if sess {
        ha1 = hash(&format!("{ha1}:{}:{cnonce}", challenge.nonce));
    }
    let ha2 = hash(&format!("{}:{uri}", method.to_ascii_uppercase()));
    let response = match qop {
        Some(qop) => hash(&format!(
            "{ha1}:{}:{nc}:{cnonce}:{qop}:{ha2}",
            challenge.nonce
        )),
        None => hash(&format!("{ha1}:{}:{ha2}", challenge.nonce)),
    };

    let quote = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    let mut parts = vec![
        format!("username={}", quote(username)),
        format!("realm={}", quote(&challenge.realm)),
        format!("nonce={}", quote(&challenge.nonce)),
        format!("uri={}", quote(uri)),
        format!("algorithm={algorithm}"),
        format!("response=\"{response}\""),
    ];
    if let Some(qop) = qop {
        parts.push(format!("qop={qop}"));
        parts.push(format!("nc={nc}"));
        parts.push(format!("cnonce={}", quote(cnonce)));
    }
    if let Some(opaque) = &challenge.opaque {
        parts.push(format!("opaque={}", quote(opaque)));
    }
    Ok(format!("Digest {}", parts.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const AWS_TIME: u64 = 1_440_938_160; // 2015-08-30T12:36:00Z

    fn aws(service: &str) -> AwsSigV4 {
        AwsSigV4 {
            access_key: "AKIDEXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
            session_token: String::new(),
            region: "us-east-1".into(),
            service: service.into(),
        }
    }

    fn header<'a>(headers: &'a [(String, String)], name: &str) -> &'a str {
        &headers.iter().find(|(k, _)| k == name).unwrap().1
    }

    #[test]
    fn aws_sigv4_matches_the_test_suite() {
        // aws-sig-v4-test-suite: get-vanilla
        let url = Url::parse("https://example.amazonaws.com/").unwrap();
        let headers = aws_sigv4(
            &aws("service"),
            "GET",
            &url,
            None,
            &sha256_hex(b""),
            AWS_TIME,
        );
        assert_eq!(header(&headers, "X-Amz-Date"), "20150830T123600Z");
        assert_eq!(
            header(&headers, "Authorization"),
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;x-amz-date, \
             Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        );

        // The IAM ListUsers example from the AWS documentation.
        let url =
            Url::parse("https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08").unwrap();
        let headers = aws_sigv4(
            &aws("iam"),
            "GET",
            &url,
            Some("application/x-www-form-urlencoded; charset=utf-8"),
            &sha256_hex(b""),
            AWS_TIME,
        );
        assert!(
            header(&headers, "Authorization").ends_with(
                "SignedHeaders=content-type;host;x-amz-date, \
                 Signature=5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7"
            ),
            "{}",
            header(&headers, "Authorization")
        );
    }

    #[test]
    fn aws_sigv4_adds_s3_and_session_headers() {
        let mut creds = aws("s3");
        creds.session_token = "tok".into();
        let url = Url::parse("https://bucket.s3.amazonaws.com/my%20file.txt").unwrap();
        let headers = aws_sigv4(&creds, "PUT", &url, None, UNSIGNED_PAYLOAD, AWS_TIME);
        assert_eq!(header(&headers, "X-Amz-Content-Sha256"), UNSIGNED_PAYLOAD);
        assert_eq!(header(&headers, "X-Amz-Security-Token"), "tok");
        assert!(
            header(&headers, "Authorization").contains(
                "SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token"
            )
        );
        assert_eq!(canonical_uri(&url, true), "/my%20file.txt");
        assert_eq!(canonical_uri(&url, false), "/my%2520file.txt");
    }

    #[test]
    fn oauth1_matches_the_classic_example() {
        // The worked example from Twitter's "Creating a signature" guide.
        let o = OAuth1 {
            consumer_key: "xvz1evFS4wEEPTGEFPHBog".into(),
            consumer_secret: "kAcSOqF21Fu85e7zjz7ZN2U4ZRhfV3WpwPAoE3Z7kBw".into(),
            token: "370773112-GmHxMAgYyLbNEtIKZeRNFsMKPR9EyMZeS9weJAEb".into(),
            token_secret: "LswwdoUaIvS8ltyTt5jkRh4J50vUPVVHtR2YPi5kE".into(),
            ..OAuth1::default()
        };
        let url =
            Url::parse("https://api.twitter.com/1.1/statuses/update.json?include_entities=true")
                .unwrap();
        let form = [(
            "status".to_owned(),
            "Hello Ladies + Gentlemen, a signed OAuth request!".to_owned(),
        )];
        let params = oauth1_params(
            &o,
            "POST",
            &url,
            &form,
            1_318_622_958,
            "kYjzVBB8Y0ZFabxSWbWovY3uYSQ2pTgmZeNu2VS4cg",
        );
        // The result printed in that guide for exactly these inputs.
        assert_eq!(params.last().unwrap().1, "hCtSmYh+iHYCEqBWrE7C7hYmtUk=");
        let header = oauth1_header(&params, "");
        assert!(header.starts_with("OAuth oauth_consumer_key=\"xvz1evFS4wEEPTGEFPHBog\""));
        assert!(header.ends_with("oauth_signature=\"hCtSmYh%2BiHYCEqBWrE7C7hYmtUk%3D\""));

        let plain = OAuth1 {
            signature_method: OAuth1Method::Plaintext,
            ..o
        };
        let params = oauth1_params(&plain, "GET", &url, &[], 1, "n");
        assert_eq!(
            params.last().unwrap().1,
            "kAcSOqF21Fu85e7zjz7ZN2U4ZRhfV3WpwPAoE3Z7kBw&LswwdoUaIvS8ltyTt5jkRh4J50vUPVVHtR2YPi5kE"
        );
    }

    #[test]
    fn digest_matches_rfc_7616() {
        let header = "Digest realm=\"http-auth@example.org\", qop=\"auth, auth-int\", \
                      algorithm=SHA-256, nonce=\"7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v\", \
                      opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\"";
        let mut challenge = parse_digest_challenge(header).unwrap();
        assert_eq!(challenge.qop, ["auth", "auth-int"]);
        let cnonce = "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ";
        let answer = |c: &DigestChallenge| {
            digest_authorization(
                c,
                "Mufasa",
                "Circle of Life",
                "GET",
                "/dir/index.html",
                cnonce,
            )
            .unwrap()
        };
        assert!(
            answer(&challenge).contains(
                "response=\"753927fa0e85d155564e2e272a28d1802ca10daf4496794697cf8db5856cb6c1\""
            ),
            "{}",
            answer(&challenge)
        );
        challenge.algorithm = "MD5".into();
        let md5 = answer(&challenge);
        assert!(
            md5.contains("response=\"8ca523f5e9506fed4657c9700eebdbec\""),
            "{md5}"
        );
        assert!(
            md5.contains("qop=auth, nc=00000001, cnonce=\"f2/wE4"),
            "{md5}"
        );
        assert!(md5.ends_with("opaque=\"FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS\""));

        // Followed by another scheme's challenge in the same header.
        let mixed = parse_digest_challenge("Digest realm=\"r\", nonce=\"n\", Basic realm=\"r\"");
        assert_eq!(mixed.unwrap().nonce, "n");
        assert!(parse_digest_challenge("Basic realm=\"r\"").is_none());
    }

    #[test]
    fn pkce_matches_rfc_7636() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let token = random_token(32);
        assert_eq!(token.len(), 43);
        assert_ne!(token, random_token(32));
    }
}
