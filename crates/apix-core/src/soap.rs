//! SOAP is an XML envelope sent over HTTP POST, so a SOAP request is an
//! ordinary request. What it adds is here: the two versions' headers and
//! envelopes, and reading the fault a server may answer with.

use std::fmt;

use crate::KeyValue;

/// Bodies bigger than this are not searched for a fault.
const MAX_FAULT_BODY: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoapVersion {
    V11,
    V12,
}

impl SoapVersion {
    pub fn label(self) -> &'static str {
        match self {
            SoapVersion::V11 => "SOAP 1.1",
            SoapVersion::V12 => "SOAP 1.2",
        }
    }

    pub fn envelope_namespace(self) -> &'static str {
        match self {
            SoapVersion::V11 => "http://schemas.xmlsoap.org/soap/envelope/",
            SoapVersion::V12 => "http://www.w3.org/2003/05/soap-envelope",
        }
    }

    /// The headers of a call: 1.1 has `SOAPAction`, 1.2 puts the action in
    /// the content type.
    pub fn headers(self, action: &str) -> Vec<KeyValue> {
        match self {
            SoapVersion::V11 => vec![
                KeyValue::new("Content-Type", "text/xml; charset=utf-8"),
                KeyValue::new("SOAPAction", format!("\"{action}\"")),
            ],
            SoapVersion::V12 if action.is_empty() => vec![KeyValue::new(
                "Content-Type",
                "application/soap+xml; charset=utf-8",
            )],
            SoapVersion::V12 => vec![KeyValue::new(
                "Content-Type",
                format!("application/soap+xml; charset=utf-8; action=\"{action}\""),
            )],
        }
    }

    /// An envelope around this header and body, already indented by four
    /// spaces; `namespaces` are declared on the envelope.
    pub fn envelope(self, namespaces: &[(String, String)], header: &str, body: &str) -> String {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        out.push_str(&format!(
            "<soap:Envelope xmlns:soap=\"{}\"",
            self.envelope_namespace()
        ));
        for (prefix, namespace) in namespaces {
            out.push_str(&format!("\n               xmlns:{prefix}=\"{namespace}\""));
        }
        out.push_str(">\n");
        if !header.is_empty() {
            out.push_str("  <soap:Header>\n");
            out.push_str(header);
            out.push_str("  </soap:Header>\n");
        }
        out.push_str("  <soap:Body>\n");
        out.push_str(body);
        out.push_str("  </soap:Body>\n</soap:Envelope>\n");
        out
    }

    /// An empty envelope to start from.
    pub fn template(self) -> String {
        self.envelope(&[], "", "    <!-- the operation goes here -->\n")
    }
}

/// A SOAP fault: what went wrong, in the server's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub code: String,
    pub message: String,
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.code.is_empty(), self.message.is_empty()) {
            (false, false) => write!(f, "{}: {}", self.code, self.message),
            (false, true) => f.write_str(&self.code),
            _ => f.write_str(&self.message),
        }
    }
}

/// The fault in a response body, if it is a SOAP envelope holding one.
pub fn fault(body: &[u8]) -> Option<Fault> {
    if body.len() > MAX_FAULT_BODY {
        return None;
    }
    let text = std::str::from_utf8(body).ok()?;
    if !text.contains("Fault") {
        return None;
    }
    let doc = roxmltree::Document::parse(text.trim_start_matches('\u{feff}')).ok()?;
    let envelope = doc.root_element();
    let namespace = envelope.tag_name().namespace()?;
    if envelope.tag_name().name() != "Envelope"
        || (namespace != SoapVersion::V11.envelope_namespace()
            && namespace != SoapVersion::V12.envelope_namespace())
    {
        return None;
    }
    let fault = child(child(envelope, "Body")?, "Fault")?;
    if namespace == SoapVersion::V11.envelope_namespace() {
        return Some(Fault {
            code: text_of(child(fault, "faultcode")),
            message: text_of(child(fault, "faultstring")),
        });
    }
    let code = child(fault, "Code");
    let mut codes = vec![text_of(code.and_then(|c| child(c, "Value")))];
    let mut sub = code.and_then(|c| child(c, "Subcode"));
    while let Some(node) = sub {
        codes.push(text_of(child(node, "Value")));
        sub = child(node, "Subcode");
    }
    Some(Fault {
        code: codes.join(" / "),
        message: text_of(child(fault, "Reason").and_then(|r| child(r, "Text"))),
    })
}

fn child<'a, 'i>(node: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn text_of(node: Option<roxmltree::Node<'_, '_>>) -> String {
    node.and_then(|n| n.text())
        .unwrap_or_default()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value<'a>(headers: &'a [KeyValue], key: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|h| h.key == key)
            .map(|h| h.value.as_str())
    }

    #[test]
    fn headers_follow_the_version() {
        let old = SoapVersion::V11.headers("urn:add");
        assert_eq!(value(&old, "Content-Type"), Some("text/xml; charset=utf-8"));
        assert_eq!(value(&old, "SOAPAction"), Some("\"urn:add\""));
        // 1.1 sends the header even without an action.
        let none = SoapVersion::V11.headers("");
        assert_eq!(value(&none, "SOAPAction"), Some("\"\""));

        let new = SoapVersion::V12.headers("urn:add");
        assert_eq!(
            value(&new, "Content-Type"),
            Some("application/soap+xml; charset=utf-8; action=\"urn:add\"")
        );
        assert_eq!(value(&new, "SOAPAction"), None);
        assert_eq!(
            value(&SoapVersion::V12.headers(""), "Content-Type"),
            Some("application/soap+xml; charset=utf-8")
        );
    }

    #[test]
    fn templates_are_envelopes() {
        for version in [SoapVersion::V11, SoapVersion::V12] {
            let text = version.template();
            let doc = roxmltree::Document::parse(&text).unwrap();
            let root = doc.root_element();
            assert_eq!(root.tag_name().name(), "Envelope");
            assert_eq!(
                root.tag_name().namespace(),
                Some(version.envelope_namespace())
            );
        }
    }

    #[test]
    fn faults_of_both_versions() {
        let old = br#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body>
  <s:Fault><faultcode>s:Client</faultcode><faultstring> Division by zero </faultstring>
  <detail><x>1</x></detail></s:Fault>
</s:Body></s:Envelope>"#;
        let found = fault(old).unwrap();
        assert_eq!(found.code, "s:Client");
        assert_eq!(found.message, "Division by zero");
        assert_eq!(found.to_string(), "s:Client: Division by zero");

        let new = br#"<env:Envelope xmlns:env="http://www.w3.org/2003/05/soap-envelope"><env:Body>
  <env:Fault>
    <env:Code><env:Value>env:Sender</env:Value>
      <env:Subcode><env:Value>m:BadInput</env:Value></env:Subcode></env:Code>
    <env:Reason><env:Text xml:lang="en">Nope</env:Text></env:Reason>
  </env:Fault>
</env:Body></env:Envelope>"#;
        let found = fault(new).unwrap();
        assert_eq!(found.code, "env:Sender / m:BadInput");
        assert_eq!(found.message, "Nope");
    }

    #[test]
    fn other_bodies_are_not_faults() {
        let answer = br#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body>
            <AddResponse><note>No Fault here</note></AddResponse></s:Body></s:Envelope>"#;
        assert_eq!(fault(answer), None);
        assert_eq!(fault(b"<Fault>not soap</Fault>"), None);
        assert_eq!(fault(br#"{"Fault": 1}"#), None);
        assert_eq!(fault(&[0xff, 0xfe]), None);
    }
}
