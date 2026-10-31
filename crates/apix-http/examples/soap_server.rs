//! A SOAP server for trying Apix's SOAP requests:
//!
//! ```sh
//! cargo run -p apix-http --example soap_server
//! ```
//!
//! Import `http://127.0.0.1:8773/calculator?wsdl` (`apix import` or File ›
//! Import from URL…). `Add` adds, `Divide` divides and answers a fault with
//! HTTP 500 when the divisor is 0, and `PlaceOrder` answers a fault with HTTP
//! 200, as some servers do. A call in SOAP 1.1 without the `SOAPAction` header
//! is refused, and SOAP 1.2 is answered in SOAP 1.2.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const WSDL: &str = include_str!("../../apix-import/tests/fixtures/calculator.wsdl");
const ENVELOPE_11: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const ENVELOPE_12: &str = "http://www.w3.org/2003/05/soap-envelope";

/// The request's head and body (as much as Content-Length says).
async fn read_request(socket: &mut TcpStream) -> String {
    let mut data = Vec::new();
    let mut buf = vec![0u8; 8192];
    while let Ok(n) = socket.read(&mut buf).await {
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&data);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            if data.len() >= end + 4 + length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}

/// The text of the first element with this local name, whatever its prefix.
fn value_of(body: &str, name: &str) -> Option<f64> {
    let start = body
        .find(&format!(":{name}>"))
        .or_else(|| body.find(&format!("<{name}>")))?;
    let rest = &body[start..];
    let open = rest.find('>')? + 1;
    let close = rest.find('<')?;
    rest.get(open..close)?.trim().parse().ok()
}

fn envelope(namespace: &str, inner: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"{namespace}\"><s:Body>{inner}</s:Body></s:Envelope>"
    )
}

fn fault(v12: bool, code: &str, message: &str) -> String {
    if v12 {
        envelope(
            ENVELOPE_12,
            &format!(
                "<s:Fault><s:Code><s:Value>s:{code}</s:Value></s:Code><s:Reason><s:Text xml:lang=\"en\">{message}</s:Text></s:Reason></s:Fault>"
            ),
        )
    } else {
        let code = if code == "Sender" { "Client" } else { "Server" };
        envelope(
            ENVELOPE_11,
            &format!(
                "<s:Fault><faultcode>s:{code}</faultcode><faultstring>{message}</faultstring></s:Fault>"
            ),
        )
    }
}

/// The status and body of a call.
fn answer(head: &str, body: &str) -> (&'static str, String) {
    let v12 = body.contains(ENVELOPE_12);
    let header = |name: &str| {
        head.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim())
        })
    };
    if !v12 && header("SOAPAction").is_none() {
        return (
            "500 Internal Server Error",
            fault(false, "Sender", "SOAP 1.1 needs a SOAPAction header"),
        );
    }
    let namespace = if v12 { ENVELOPE_12 } else { ENVELOPE_11 };
    let reply = |inner: String| ("200 OK", envelope(namespace, &inner));
    if body.contains(":Add>") || body.contains("<Add>") {
        let sum = value_of(body, "a").unwrap_or(0.0) + value_of(body, "b").unwrap_or(0.0);
        return reply(format!(
            "<AddResponse xmlns=\"urn:apix:calculator\"><result>{sum}</result></AddResponse>"
        ));
    }
    if body.contains(":Divide>") || body.contains("<Divide>") {
        let divisor = value_of(body, "divisor").unwrap_or(0.0);
        if divisor == 0.0 {
            return (
                "500 Internal Server Error",
                fault(v12, "Sender", "Division by zero"),
            );
        }
        let dividend = value_of(body, "dividend").unwrap_or(0.0);
        return reply(format!(
            "<DivideResponse xmlns=\"urn:apix:calculator\"><dividend>{}</dividend><divisor>1</divisor></DivideResponse>",
            dividend / divisor
        ));
    }
    ("200 OK", fault(v12, "Receiver", "Orders are closed today"))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8773").await?;
    println!("SOAP on http://127.0.0.1:8773/calculator (WSDL: add ?wsdl)");
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let request = read_request(&mut socket).await;
            let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
            let wsdl = head.starts_with("GET ");
            let (status, kind, content) = if wsdl {
                ("200 OK", "text/xml; charset=utf-8", WSDL.to_owned())
            } else {
                let (status, content) = answer(head, body);
                let kind = if body.contains(ENVELOPE_12) {
                    "application/soap+xml; charset=utf-8"
                } else {
                    "text/xml; charset=utf-8"
                };
                (status, kind, content)
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{content}",
                content.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        });
    }
}
