//! WSDL documents → SOAP requests.

use std::path::Path;

use apix_core::{Body, Method, Request};
use apix_import::wsdl::{self, Loader};
use apix_import::{Format, PlannedFile};
use roxmltree::Document;

const CALCULATOR: &str = include_str!("fixtures/calculator.wsdl");
const WEATHER: &str = include_str!("fixtures/weather-rpc.wsdl");
const ENVELOPE_11: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const ENVELOPE_12: &str = "http://www.w3.org/2003/05/soap-envelope";
const CALC: &str = "urn:apix:calculator";

fn requests(found: &apix_import::Found) -> Vec<(String, Request)> {
    found.collections[0]
        .1
        .files
        .iter()
        .filter_map(|file| match file {
            PlannedFile::Request { path, request } => {
                Some((path.to_string_lossy().replace('\\', "/"), request.clone()))
            }
            _ => None,
        })
        .collect()
}

fn find<'a>(all: &'a [(String, Request)], path: &str) -> &'a Request {
    &all.iter()
        .find(|(p, _)| p == path)
        .unwrap_or_else(|| {
            panic!(
                "{path} in {:?}",
                all.iter().map(|r| &r.0).collect::<Vec<_>>()
            )
        })
        .1
}

fn header<'a>(request: &'a Request, key: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|h| h.key == key)
        .map(|h| h.value.as_str())
}

fn xml(request: &Request) -> &str {
    match &request.body {
        Body::Xml(text) => text,
        other => panic!("{other:?}"),
    }
}

/// The first element of this name and namespace.
fn element<'a, 'i>(doc: &'a Document<'i>, namespace: &str, name: &str) -> roxmltree::Node<'a, 'i> {
    doc.descendants()
        .find(|n| {
            n.is_element()
                && n.tag_name().name() == name
                && n.tag_name().namespace().unwrap_or_default() == namespace
        })
        .unwrap_or_else(|| panic!("{{{namespace}}}{name} in {}", doc.input_text()))
}

#[test]
fn soap_versions_ports_and_addresses() {
    assert_eq!(apix_import::detect(CALCULATOR).unwrap(), Some(Format::Wsdl));
    let found = apix_import::convert(CALCULATOR).unwrap().unwrap();
    let (_, plan) = &found.collections[0];
    assert_eq!(plan.meta.name, "Calculator");
    assert_eq!(
        plan.meta.description,
        "Adds and divides, for trying SOAP requests."
    );
    // One address for both SOAP ports; the HTTP port is left out, and says so.
    assert_eq!(
        plan.meta.vars.get("baseUrl").map(String::as_str),
        Some("http://127.0.0.1:8773/calculator")
    );
    assert!(
        plan.report
            .warnings
            .iter()
            .any(|w| w.item == "CalculatorHttp" && w.message.contains("not SOAP")),
        "{:?}",
        plan.report.warnings
    );
    assert_eq!(plan.report.requests, 5);
    assert_eq!(plan.report.folders, 2);

    let all = requests(&found);
    let old = find(&all, "calculatorsoap/add.apix");
    assert_eq!(old.method, Method::Post);
    assert_eq!(old.url, "{{baseUrl}}");
    assert_eq!(old.description, "Adds two numbers.");
    assert_eq!(header(old, "Content-Type"), Some("text/xml; charset=utf-8"));
    assert_eq!(
        header(old, "SOAPAction"),
        Some("\"urn:apix:calculator/Add\"")
    );

    let new = find(&all, "calculatorsoap12/add.apix");
    assert_eq!(header(new, "SOAPAction"), None);
    assert_eq!(
        header(new, "Content-Type"),
        Some("application/soap+xml; charset=utf-8; action=\"urn:apix:calculator/Add\"")
    );
    let doc = Document::parse(xml(new)).unwrap();
    element(&doc, ENVELOPE_12, "Envelope");
}

#[test]
fn document_literal_envelopes() {
    let found = apix_import::convert(CALCULATOR).unwrap().unwrap();
    let all = requests(&found);

    let add = find(&all, "calculatorsoap/add.apix");
    let doc = Document::parse(xml(add)).unwrap();
    assert_eq!(doc.root_element().tag_name().namespace(), Some(ENVELOPE_11));
    let body = element(&doc, ENVELOPE_11, "Body");
    let call = body.first_element_child().unwrap();
    assert_eq!(call.tag_name().name(), "Add");
    assert_eq!(call.tag_name().namespace(), Some(CALC));
    let a = element(&doc, CALC, "a");
    assert_eq!(a.text(), Some("1"));
    assert!(xml(add).contains("<tns:b>1</tns:b>"), "{}", xml(add));

    // A part bound to the header goes there, the others to the body.
    let divide = find(&all, "calculatorsoap/divide.apix");
    let doc = Document::parse(xml(divide)).unwrap();
    let head = element(&doc, ENVELOPE_11, "Header");
    assert_eq!(
        head.first_element_child().unwrap().tag_name().name(),
        "Trace"
    );
    let body = element(&doc, ENVELOPE_11, "Body");
    let names: Vec<&str> = body
        .children()
        .filter(|c| c.is_element())
        .map(|c| c.tag_name().name())
        .collect();
    assert_eq!(names, ["Divide"]);
    assert_eq!(element(&doc, CALC, "divisor").text(), Some("1.5"));
    // An operation without an action still sends the header 1.1 requires.
    let order = find(&all, "calculatorsoap/placeorder.apix");
    assert_eq!(header(order, "SOAPAction"), Some("\"\""));
}

#[test]
fn examples_follow_the_schema() {
    let found = apix_import::convert(CALCULATOR).unwrap().unwrap();
    let all = requests(&found);
    let text = xml(find(&all, "calculatorsoap/placeorder.apix"));
    let doc = Document::parse(text).unwrap();

    // Extension: the base's attribute and elements, then its own.
    let order = element(&doc, CALC, "Order");
    assert_eq!(order.attribute("version"), Some("1"));
    assert_eq!(order.attribute("ignored"), None, "only required attributes");
    let names: Vec<&str> = order
        .children()
        .filter(|c| c.is_element())
        .map(|c| c.tag_name().name())
        .collect();
    assert_eq!(
        names,
        ["id", "status", "note", "item", "plain", "card"],
        "{text}"
    );

    assert_eq!(element(&doc, CALC, "status").text(), Some("NEW"));
    assert!(text.contains("<!-- one of: NEW, PAID -->"), "{text}");
    assert!(text.contains("<!-- optional -->"), "{text}");
    assert!(text.contains("<!-- zero or more -->"), "{text}");
    assert!(text.contains("<!-- one of: card, cash -->"), "{text}");
    assert!(text.contains("<!-- any element -->"), "{text}");
    // form="unqualified" against the schema's default.
    assert!(text.contains("<plain>string</plain>"), "{text}");
    assert_eq!(element(&doc, "", "plain").text(), Some("string"));
    // Types by kind; a type inside itself stops there.
    assert_eq!(
        element(&doc, CALC, "created").text(),
        Some("2026-01-31T12:00:00Z")
    );
    assert_eq!(element(&doc, CALC, "active").text(), Some("true"));
    assert!(text.contains("Item: recursive, left out"), "{text}");
    // Nesting is indented by level.
    assert!(
        text.contains("\n      <tns:item>\n        <tns:created>"),
        "{text}"
    );
}

#[test]
fn rpc_style_with_imports() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let base = dir.join("weather-rpc.wsdl");
    let mut loader = Loader::new(&base.to_string_lossy(), WEATHER);
    let mut read = Vec::new();
    while let Some(location) = loader.next_location() {
        read.push(
            Path::new(&location)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
        let text = std::fs::read_to_string(&location).map_err(|e| e.to_string());
        loader.add(location, text);
    }
    read.sort();
    assert_eq!(read, ["weather-more.wsdl", "weather-types.xsd"]);

    let found = wsdl::convert(WEATHER, &loader).unwrap();
    let (_, plan) = &found.collections[0];
    assert_eq!(plan.meta.name, "Weather");
    // Different addresses: a variable per port, a folder per port.
    assert_eq!(
        plan.meta.vars.get("weatherrpcUrl").map(String::as_str),
        Some("https://weather.example.com/soap"),
        "{:?}",
        plan.meta.vars
    );
    assert_eq!(
        plan.meta.vars.get("moreportUrl").map(String::as_str),
        Some("https://more.example.com/soap")
    );
    let warnings = |text: &str| {
        plan.report
            .warnings
            .iter()
            .any(|w| w.message.contains(text))
    };
    assert!(warnings("SOAP encoding"), "{:?}", plan.report.warnings);
    assert!(
        warnings("{urn:apix:types}Nowhere"),
        "{:?}",
        plan.report.warnings
    );

    let all = requests(&found);
    let forecast = find(&all, "weatherservice-weatherrpc/forecast.apix");
    assert_eq!(forecast.url, "{{weatherrpcUrl}}");
    let doc = Document::parse(xml(forecast)).unwrap();
    // The wrapper has the binding's namespace; parts have none; a type from
    // the imported schema keeps its own rules inside.
    let wrapper = element(&doc, "urn:apix:weather", "Forecast");
    let names: Vec<&str> = wrapper
        .children()
        .filter(|c| c.is_element())
        .map(|c| c.tag_name().name())
        .collect();
    assert_eq!(names, ["city", "place", "missing"]);
    assert_eq!(element(&doc, "", "city").text(), Some("string"));
    let place = element(&doc, "", "place");
    assert_eq!(
        place.first_element_child().unwrap().tag_name().name(),
        "lat"
    );
    assert_eq!(element(&doc, "urn:apix:types", "lat").text(), Some("1.5"));
    assert_eq!(
        element(&doc, "urn:apix:types", "Country").text(),
        Some("string")
    );

    // The imported WSDL brought its own service.
    let ping = find(&all, "moreservice-moreport/ping.apix");
    assert_eq!(header(ping, "SOAPAction"), Some("\"urn:ping\""));
    Document::parse(xml(ping)).unwrap();
}

#[test]
fn what_could_not_be_read_is_reported() {
    let mut loader = Loader::new("http://h/w.wsdl", WEATHER);
    while let Some(location) = loader.next_location() {
        loader.add(location, Err("404".into()));
    }
    let found = wsdl::convert(WEATHER, &loader).unwrap();
    let (_, plan) = &found.collections[0];
    let warnings: Vec<&str> = plan
        .report
        .warnings
        .iter()
        .map(|w| w.message.as_str())
        .collect();
    assert!(
        warnings.contains(&"Could not read http://h/weather-types.xsd: 404"),
        "{warnings:?}"
    );
    // The request is still there, with what is known.
    assert_eq!(plan.report.requests, 1);
}

#[test]
fn documents_that_cannot_be_imported_say_why() {
    let empty = Loader::default();
    let wsdl2 = r#"<description xmlns="http://www.w3.org/ns/wsdl"/>"#;
    assert_eq!(apix_import::detect(wsdl2).unwrap(), Some(Format::Wsdl));
    assert!(
        wsdl::convert(wsdl2, &empty)
            .unwrap_err()
            .to_string()
            .contains("WSDL 2.0")
    );
    let no_ports = r#"<definitions xmlns="http://schemas.xmlsoap.org/wsdl/"/>"#;
    assert!(
        wsdl::convert(no_ports, &empty)
            .unwrap_err()
            .to_string()
            .contains("no SOAP port")
    );
    assert!(wsdl::convert("<not-closed", &empty).is_err());
}

#[test]
fn imported_requests_survive_the_file_format() {
    let found = apix_import::convert(CALCULATOR).unwrap().unwrap();
    for (_, request) in requests(&found) {
        let text = toml::to_string_pretty(&request).unwrap();
        let back: Request = toml::from_str(&text).unwrap();
        assert_eq!(request, back);
    }
}
