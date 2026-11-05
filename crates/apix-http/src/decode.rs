//! Compressed response bodies. Apix asks for `gzip`, `deflate` and `br`
//! itself (unless the request names its own `Accept-Encoding`) and decodes
//! the answer as it arrives, rather than letting reqwest do it: reqwest
//! would drop `Content-Encoding` and the size on the wire, which the
//! response headers and the Network tab should show as they were.

use std::io::Write;
use std::sync::{Arc, Mutex};

/// What a request asks for when it does not say.
pub(crate) const ACCEPT_ENCODING: &str = "gzip, deflate, br";

/// Where a decoder writes; shared so the output can be taken piece by piece.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Sink {
    fn take(&self) -> Vec<u8> {
        self.0
            .lock()
            .map(|mut v| std::mem::take(&mut *v))
            .unwrap_or_default()
    }
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut v) = self.0.lock() {
            v.extend_from_slice(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

enum Kind {
    Gzip(flate2::write::GzDecoder<Sink>),
    /// `deflate` as the standard means it: zlib.
    Zlib(flate2::write::ZlibDecoder<Sink>),
    /// `deflate` as some servers send it: a raw deflate stream.
    Deflate(flate2::write::DeflateDecoder<Sink>),
    Brotli(Box<brotli_decompressor::DecompressorWriter<Sink>>),
    /// `deflate` before its first bytes say which of the two it is.
    Undecided,
}

/// Decodes one body.
pub(crate) struct Decoding {
    sink: Sink,
    kind: Kind,
    pub(crate) encoding: String,
    /// Bytes taken in so far.
    pub(crate) encoded: u64,
}

impl Decoding {
    /// A decoder for the response's `Content-Encoding`, or `None` when there
    /// is none, or it is one Apix cannot decode (then the body stays as sent).
    pub(crate) fn for_headers(headers: &[(String, String)]) -> Option<Self> {
        let value = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-encoding"))?
            .1
            .trim()
            .to_ascii_lowercase();
        let sink = Sink::default();
        let kind = match value.as_str() {
            "gzip" | "x-gzip" => Kind::Gzip(flate2::write::GzDecoder::new(sink.clone())),
            "deflate" => Kind::Undecided,
            "br" => Kind::Brotli(Box::new(brotli_decompressor::DecompressorWriter::new(
                sink.clone(),
                16 * 1024,
            ))),
            _ => return None,
        };
        Some(Self {
            sink,
            kind,
            encoding: value,
            encoded: 0,
        })
    }

    /// Takes in a piece of the body as sent; returns what it decodes to so far.
    pub(crate) fn push(&mut self, chunk: &[u8]) -> std::io::Result<Vec<u8>> {
        if chunk.is_empty() {
            return Ok(Vec::new());
        }
        self.encoded += chunk.len() as u64;
        if matches!(self.kind, Kind::Undecided) {
            self.kind = if is_zlib_header(chunk) {
                Kind::Zlib(flate2::write::ZlibDecoder::new(self.sink.clone()))
            } else {
                Kind::Deflate(flate2::write::DeflateDecoder::new(self.sink.clone()))
            };
        }
        match &mut self.kind {
            Kind::Gzip(d) => d.write_all(chunk)?,
            Kind::Zlib(d) => d.write_all(chunk)?,
            Kind::Deflate(d) => d.write_all(chunk)?,
            Kind::Brotli(d) => d.write_all(chunk)?,
            Kind::Undecided => {}
        }
        Ok(self.sink.take())
    }

    /// The end of the body: what is left, or an error when it stopped early.
    pub(crate) fn finish(self) -> std::io::Result<Vec<u8>> {
        let truncated = || {
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the compressed body ends too early",
            )
        };
        match self.kind {
            Kind::Gzip(d) => {
                d.finish()?;
            }
            Kind::Zlib(d) => {
                d.finish()?;
            }
            Kind::Deflate(d) => {
                d.finish()?;
            }
            Kind::Brotli(mut d) => {
                d.close().map_err(|_| truncated())?;
            }
            Kind::Undecided => {}
        }
        Ok(self.sink.take())
    }
}

/// A zlib stream starts with a header whose two bytes are a multiple of 31
/// and name the deflate method.
fn is_zlib_header(bytes: &[u8]) -> bool {
    match bytes {
        [cmf, flg, ..] => cmf & 0x0F == 8 && (u16::from(*cmf) << 8 | u16::from(*flg)) % 31 == 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(encoding: &str) -> Vec<(String, String)> {
        vec![("Content-Encoding".into(), encoding.into())]
    }

    /// Decodes `bytes` fed in pieces of `step`.
    fn decode(encoding: &str, bytes: &[u8], step: usize) -> std::io::Result<Vec<u8>> {
        let mut d = Decoding::for_headers(&headers(encoding)).unwrap();
        let mut out = Vec::new();
        for piece in bytes.chunks(step) {
            out.extend(d.push(piece)?);
        }
        assert_eq!(d.encoded, bytes.len() as u64);
        out.extend(d.finish()?);
        Ok(out)
    }

    fn text() -> Vec<u8> {
        "{\"users\": [1, 2, 3]}\n".repeat(500).into_bytes()
    }

    #[test]
    fn gzip_zlib_and_raw_deflate() {
        use flate2::Compression;
        let plain = text();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(&plain).unwrap();
        let gz = gz.finish().unwrap();
        assert!(gz.len() < plain.len() / 10);
        assert_eq!(decode("gzip", &gz, 7).unwrap(), plain);

        let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), Compression::default());
        zlib.write_all(&plain).unwrap();
        assert_eq!(
            decode("deflate", &zlib.finish().unwrap(), 100).unwrap(),
            plain
        );

        let mut raw = flate2::write::DeflateEncoder::new(Vec::new(), Compression::default());
        raw.write_all(&plain).unwrap();
        assert_eq!(decode("Deflate", &raw.finish().unwrap(), 1).unwrap(), plain);

        // Cut short.
        assert!(decode("gzip", &gz[..gz.len() / 2], 50).is_err());
    }

    #[test]
    fn brotli() {
        let plain = text();
        let mut br = Vec::new();
        {
            let mut w = brotli::CompressorWriter::new(&mut br, 4096, 9, 22);
            w.write_all(&plain).unwrap();
        }
        assert!(br.len() < plain.len() / 10);
        assert_eq!(decode("br", &br, 13).unwrap(), plain);
        assert!(decode("br", &br[..br.len() / 2], 13).is_err());
    }

    #[test]
    fn unknown_or_absent_encodings_are_left_alone() {
        assert!(Decoding::for_headers(&headers("zstd")).is_none());
        assert!(Decoding::for_headers(&headers("gzip, br")).is_none());
        assert!(Decoding::for_headers(&[]).is_none());
        assert!(is_zlib_header(&[0x78, 0x9c]));
        assert!(!is_zlib_header(&[0x1f, 0x8b]));
    }
}
