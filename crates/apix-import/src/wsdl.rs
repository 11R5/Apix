//! WSDL 1.1 → one collection with a SOAP request per operation, a folder per
//! port when there are several. Requests are ordinary POSTs: the address is a
//! collection variable, the headers follow the binding's SOAP version, and
//! the body is an envelope with example values built from the XML schema.
//!
//! Documents may import others (`wsdl:import`, `xsd:import`, `xsd:include`).
//! This crate does not fetch anything: [`Loader`] says what is needed next,
//! and the caller reads it from a file or the network.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use apix_core::soap::SoapVersion;
use apix_core::store::{CollectionMeta, VarMap};
use apix_core::{Auth, Body, Method, Request, RequestSettings};
use roxmltree::{Document, Node};

use crate::{Found, ImportError, Plan, PlannedFile, Report, request_file, slug, unique};

const WSDL: &str = "http://schemas.xmlsoap.org/wsdl/";
const WSDL2: &str = "http://www.w3.org/ns/wsdl";
const XSD: &str = "http://www.w3.org/2001/XMLSchema";
const SOAP11: &str = "http://schemas.xmlsoap.org/wsdl/soap/";
const SOAP12: &str = "http://schemas.xmlsoap.org/wsdl/soap12/";

/// Documents read for one import, at most.
const MAX_DOCUMENTS: usize = 64;
/// Levels of nested elements in an example.
const MAX_DEPTH: usize = 16;

/// A namespace and a local name.
type QName = (String, String);

fn is_element(node: &Node<'_, '_>, namespace: &str, name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == name
        && node.tag_name().namespace() == Some(namespace)
}

fn parse_document(text: &str) -> Result<Document<'_>, roxmltree::Error> {
    Document::parse(text.trim_start_matches('\u{feff}'))
}

/// Whether `text` is a WSDL document (1.1, or 2.0, which is refused later
/// with a reason).
pub(crate) fn is_wsdl(text: &str) -> bool {
    let start = text.trim_start_matches('\u{feff}').trim_start();
    if !start.starts_with('<') || !text.contains("wsdl") {
        return false;
    }
    let Ok(doc) = parse_document(text) else {
        return false;
    };
    let root = doc.root_element();
    is_element(&root, WSDL, "definitions") || is_element(&root, WSDL2, "description")
}

/// Where `location` (from a document at `base`) is. A document from the
/// network cannot point at local files.
fn resolve(base: &str, location: &str) -> Option<String> {
    let base_is_url = base.contains("://");
    if let Some((scheme, _)) = location.split_once("://") {
        let web = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
        return web.then(|| location.to_owned());
    }
    if base_is_url {
        return url::Url::parse(base)
            .ok()?
            .join(location)
            .ok()
            .map(|u| u.to_string());
    }
    let dir = Path::new(base).parent().unwrap_or(Path::new(""));
    Some(dir.join(location).to_string_lossy().into_owned())
}

/// The other documents `text` (found at `base`) refers to.
fn references(base: &str, text: &str) -> Vec<String> {
    let Ok(doc) = parse_document(text) else {
        return Vec::new();
    };
    doc.descendants()
        .filter_map(|node| {
            let location = if is_element(&node, WSDL, "import") {
                node.attribute("location")
            } else if is_element(&node, XSD, "import") || is_element(&node, XSD, "include") {
                node.attribute("schemaLocation")
            } else {
                None
            };
            resolve(base, location?.trim())
        })
        .collect()
}

/// Follows a document's imports: ask [`Loader::next_location`] what to read, give the
/// result to [`Loader::add`], and hand the loader to [`convert`].
#[derive(Debug, Default)]
pub struct Loader {
    queue: VecDeque<String>,
    seen: HashSet<String>,
    documents: Vec<String>,
    failed: Vec<(String, String)>,
}

impl Loader {
    /// For the main document, found at `base` (a path or a URL).
    pub fn new(base: &str, text: &str) -> Self {
        let mut loader = Self::default();
        loader.seen.insert(base.to_owned());
        loader.follow(base, text);
        loader
    }

    fn follow(&mut self, base: &str, text: &str) {
        for location in references(base, text) {
            if self.seen.len() >= MAX_DOCUMENTS {
                self.failed
                    .push((location, "too many documents were imported".into()));
            } else if self.seen.insert(location.clone()) {
                self.queue.push_back(location);
            }
        }
    }

    /// The next document to read.
    pub fn next_location(&mut self) -> Option<String> {
        self.queue.pop_front()
    }

    /// What reading `location` gave.
    pub fn add(&mut self, location: String, read: Result<String, String>) {
        match read {
            Ok(text) => {
                self.follow(&location, &text);
                self.documents.push(text);
            }
            Err(why) => self.failed.push((location, why)),
        }
    }
}

/// A part of a message: an element, or a type under the part's own name.
struct Part {
    name: String,
    element: Option<QName>,
    kind: Option<QName>,
}

struct BoundOperation<'a, 'i> {
    node: Node<'a, 'i>,
    name: String,
    action: String,
    style: Option<String>,
    encoded: bool,
    namespace: String,
    parts: Option<Vec<String>>,
    headers: Vec<(QName, String)>,
}

struct Binding<'a, 'i> {
    port_type: QName,
    /// `None` for a binding that is not SOAP (HTTP GET and POST).
    version: Option<SoapVersion>,
    style: String,
    operations: Vec<BoundOperation<'a, 'i>>,
}

struct Port {
    name: String,
    service: String,
    binding: QName,
    address: String,
}

/// Everything the documents declare.
#[derive(Default)]
struct Model<'a, 'i> {
    title: String,
    description: String,
    messages: HashMap<QName, Vec<Part>>,
    /// Port type → operation → (input message, documentation).
    port_types: HashMap<QName, HashMap<String, (Option<QName>, String)>>,
    bindings: HashMap<QName, Binding<'a, 'i>>,
    ports: Vec<Port>,
    elements: HashMap<QName, Node<'a, 'i>>,
    types: HashMap<QName, Node<'a, 'i>>,
    groups: HashMap<QName, Node<'a, 'i>>,
    attribute_groups: HashMap<QName, Node<'a, 'i>>,
}

fn qname(node: Node<'_, '_>, value: &str) -> QName {
    let (prefix, local) = match value.trim().split_once(':') {
        Some((prefix, local)) => (Some(prefix), local),
        None => (None, value.trim()),
    };
    let namespace = node.lookup_namespace_uri(prefix).unwrap_or_default();
    (namespace.to_owned(), local.to_owned())
}

fn attribute_qname(node: Node<'_, '_>, name: &str) -> Option<QName> {
    node.attribute(name).map(|value| qname(node, value))
}

fn documentation(node: Node<'_, '_>) -> String {
    node.children()
        .find(|c| is_element(c, WSDL, "documentation"))
        .map(|d| {
            d.descendants()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .unwrap_or_default()
}

impl<'a, 'i> Model<'a, 'i> {
    fn read_document(&mut self, doc: &'a Document<'i>) {
        let root = doc.root_element();
        if is_element(&root, XSD, "schema") {
            self.read_schema(root);
        } else if is_element(&root, WSDL, "definitions") {
            self.read_definitions(root);
        }
    }

    fn read_schema(&mut self, schema: Node<'a, 'i>) {
        let namespace = schema.attribute("targetNamespace").unwrap_or_default();
        for child in schema.children().filter(Node::is_element) {
            if child.tag_name().namespace() != Some(XSD) {
                continue;
            }
            let Some(name) = child.attribute("name") else {
                continue;
            };
            let key = (namespace.to_owned(), name.to_owned());
            match child.tag_name().name() {
                "element" => {
                    self.elements.entry(key).or_insert(child);
                }
                "complexType" | "simpleType" => {
                    self.types.entry(key).or_insert(child);
                }
                "group" => {
                    self.groups.entry(key).or_insert(child);
                }
                "attributeGroup" => {
                    self.attribute_groups.entry(key).or_insert(child);
                }
                _ => {}
            }
        }
    }

    fn read_definitions(&mut self, root: Node<'a, 'i>) {
        let target = root.attribute("targetNamespace").unwrap_or_default();
        if self.title.is_empty() {
            self.title = root.attribute("name").unwrap_or_default().to_owned();
        }
        if self.description.is_empty() {
            self.description = documentation(root);
        }
        for child in root.children().filter(Node::is_element) {
            if child.tag_name().namespace() != Some(WSDL) {
                continue;
            }
            let key = (
                target.to_owned(),
                child.attribute("name").unwrap_or_default().to_owned(),
            );
            match child.tag_name().name() {
                "types" => {
                    for schema in child.children().filter(|c| is_element(c, XSD, "schema")) {
                        self.read_schema(schema);
                    }
                }
                "message" => {
                    let parts = child
                        .children()
                        .filter(|c| is_element(c, WSDL, "part"))
                        .map(|part| Part {
                            name: part.attribute("name").unwrap_or_default().to_owned(),
                            element: attribute_qname(part, "element"),
                            kind: attribute_qname(part, "type"),
                        })
                        .collect();
                    self.messages.insert(key, parts);
                }
                "portType" => {
                    let operations = child
                        .children()
                        .filter(|c| is_element(c, WSDL, "operation"))
                        .map(|operation| {
                            let input = operation
                                .children()
                                .find(|c| is_element(c, WSDL, "input"))
                                .and_then(|i| attribute_qname(i, "message"));
                            (
                                operation.attribute("name").unwrap_or_default().to_owned(),
                                (input, documentation(operation)),
                            )
                        })
                        .collect();
                    self.port_types.insert(key, operations);
                }
                "binding" => {
                    if let Some(binding) = read_binding(child) {
                        self.bindings.insert(key, binding);
                    }
                }
                "service" => {
                    let service = child.attribute("name").unwrap_or_default();
                    if self.title.is_empty() {
                        self.title = service.to_owned();
                    }
                    for port in child.children().filter(|c| is_element(c, WSDL, "port")) {
                        let Some(binding) = attribute_qname(port, "binding") else {
                            continue;
                        };
                        let address = port
                            .children()
                            .find(|c| c.is_element() && c.tag_name().name() == "address")
                            .and_then(|a| a.attribute("location"))
                            .unwrap_or_default();
                        self.ports.push(Port {
                            name: port.attribute("name").unwrap_or_default().to_owned(),
                            service: service.to_owned(),
                            binding,
                            address: address.to_owned(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// A global by name; a schema without a namespace of its own (included
    /// into another) is found under the empty one.
    fn find(
        table: &HashMap<QName, Node<'a, 'i>>,
        (namespace, name): &QName,
    ) -> Option<Node<'a, 'i>> {
        table
            .get(&(namespace.clone(), name.clone()))
            .or_else(|| table.get(&(String::new(), name.clone())))
            .copied()
    }
}

fn soap_child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<(Node<'a, 'i>, SoapVersion)> {
    node.children().find_map(|c| {
        if is_element(&c, SOAP11, name) {
            Some((c, SoapVersion::V11))
        } else if is_element(&c, SOAP12, name) {
            Some((c, SoapVersion::V12))
        } else {
            None
        }
    })
}

fn read_binding<'a, 'i>(node: Node<'a, 'i>) -> Option<Binding<'a, 'i>> {
    let soap = soap_child(node, "binding");
    let operations = node
        .children()
        .filter(|c| is_element(c, WSDL, "operation"))
        .map(|operation| {
            let details = soap_child(operation, "operation").map(|(n, _)| n);
            let input = operation.children().find(|c| is_element(c, WSDL, "input"));
            let body = input.and_then(|i| soap_child(i, "body")).map(|(n, _)| n);
            let headers = input
                .map(|i| {
                    i.children()
                        .filter(|c| {
                            is_element(c, SOAP11, "header") || is_element(c, SOAP12, "header")
                        })
                        .filter_map(|h| {
                            Some((
                                attribute_qname(h, "message")?,
                                h.attribute("part")?.to_owned(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            BoundOperation {
                node: operation,
                name: operation.attribute("name").unwrap_or_default().to_owned(),
                action: details
                    .and_then(|d| d.attribute("soapAction"))
                    .unwrap_or_default()
                    .to_owned(),
                style: details
                    .and_then(|d| d.attribute("style"))
                    .map(str::to_owned),
                encoded: body.and_then(|b| b.attribute("use")) == Some("encoded"),
                namespace: body
                    .and_then(|b| b.attribute("namespace"))
                    .unwrap_or_default()
                    .to_owned(),
                parts: body
                    .and_then(|b| b.attribute("parts"))
                    .map(|p| p.split_whitespace().map(str::to_owned).collect()),
                headers,
            }
        })
        .collect();
    Some(Binding {
        port_type: attribute_qname(node, "type")?,
        version: soap.map(|(_, version)| version),
        style: soap
            .and_then(|(n, _)| n.attribute("style"))
            .unwrap_or("document")
            .to_owned(),
        operations,
    })
}

/// What an element holds, gathered from its type and the types it extends.
#[derive(Default)]
struct Content {
    attributes: Vec<(String, String)>,
    children: String,
    text: Option<String>,
    note: Option<String>,
}

/// Writes example XML, and remembers the namespaces it used.
struct Writer<'m, 'a, 'i> {
    model: &'m Model<'a, 'i>,
    /// `(prefix, namespace)`, declared on the envelope.
    namespaces: Vec<(String, String)>,
    missing: Vec<String>,
}

fn escape(text: &str, attribute: bool) -> String {
    let text = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if attribute {
        text.replace('"', "&quot;")
    } else {
        text
    }
}

fn comment(text: &str) -> String {
    format!("<!-- {} -->", text.replace("--", "- -"))
}

/// An example value of a built-in XML schema type.
fn built_in(name: &str) -> &'static str {
    match name {
        "boolean" => "true",
        "byte" | "short" | "int" | "integer" | "long" | "unsignedByte" | "unsignedShort"
        | "unsignedInt" | "unsignedLong" | "nonNegativeInteger" | "negativeInteger"
        | "positiveInteger" | "nonPositiveInteger" => "1",
        "decimal" | "float" | "double" => "1.5",
        "date" => "2026-01-31",
        "dateTime" => "2026-01-31T12:00:00Z",
        "time" => "12:00:00",
        "duration" => "P1D",
        "anyURI" => "http://example.com/",
        "base64Binary" => "QXBpeA==",
        "hexBinary" => "4170",
        "gYear" => "2026",
        _ => "string",
    }
}

fn occurrence(node: Node<'_, '_>) -> Option<&'static str> {
    let optional = node.attribute("minOccurs") == Some("0");
    let repeats = matches!(node.attribute("maxOccurs"), Some(max) if max != "0" && max != "1");
    match (optional, repeats) {
        (true, true) => Some("zero or more"),
        (true, false) => Some("optional"),
        (false, true) => Some("one or more"),
        (false, false) => None,
    }
}

fn xsd_children<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children()
        .filter(|c| c.is_element() && c.tag_name().namespace() == Some(XSD))
}

fn xsd_child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    xsd_children(node).find(|c| c.tag_name().name() == name)
}

/// The schema a declaration is in.
fn schema_of<'a, 'i>(node: Node<'a, 'i>) -> Option<Node<'a, 'i>> {
    node.ancestors().find(|a| is_element(a, XSD, "schema"))
}

fn is_global(node: Node<'_, '_>) -> bool {
    node.parent().is_some_and(|p| is_element(&p, XSD, "schema"))
}

impl<'a, 'i> Writer<'_, 'a, 'i> {
    fn prefix(&mut self, namespace: &str) -> String {
        if namespace == "http://www.w3.org/XML/1998/namespace" {
            return "xml".to_owned();
        }
        if let Some((prefix, _)) = self.namespaces.iter().find(|(_, n)| n == namespace) {
            return prefix.clone();
        }
        let prefix = match self.namespaces.len() {
            0 => "tns".to_owned(),
            n => format!("ns{n}"),
        };
        self.namespaces.push((prefix.clone(), namespace.to_owned()));
        prefix
    }

    fn qualified(&mut self, namespace: &str, name: &str) -> String {
        if namespace.is_empty() {
            name.to_owned()
        } else {
            format!("{}:{name}", self.prefix(namespace))
        }
    }

    /// The name a declared element or attribute is written with.
    fn declared_name(&mut self, declaration: Node<'a, 'i>, default_form: &str) -> String {
        let name = declaration.attribute("name").unwrap_or_default();
        let schema = schema_of(declaration);
        let form = declaration
            .attribute("form")
            .or_else(|| schema.and_then(|s| s.attribute(default_form)));
        if is_global(declaration) || form == Some("qualified") {
            let namespace = schema
                .and_then(|s| s.attribute("targetNamespace"))
                .unwrap_or_default();
            self.qualified(namespace, name)
        } else {
            name.to_owned()
        }
    }

    fn type_note(&mut self, name: &QName) {
        let missing = format!("{{{}}}{}", name.0, name.1);
        if !self.missing.contains(&missing) {
            self.missing.push(missing);
        }
    }

    /// Writes an element declaration (or a reference to one).
    fn element(
        &mut self,
        node: Node<'a, 'i>,
        indent: usize,
        stack: &mut Vec<QName>,
        out: &mut String,
    ) {
        let declaration = match attribute_qname(node, "ref") {
            Some(name) => match Model::find(&self.model.elements, &name) {
                Some(found) => found,
                None => {
                    self.type_note(&name);
                    return;
                }
            },
            None => node,
        };
        let tag = self.declared_name(declaration, "elementFormDefault");
        let mut content = Content::default();
        self.fill_from_declaration(declaration, indent, stack, &mut content);
        self.write(&tag, occurrence(node), content, indent, out);
    }

    /// What an element holds: its named type, or one written inside it.
    fn fill_from_declaration(
        &mut self,
        declaration: Node<'a, 'i>,
        indent: usize,
        stack: &mut Vec<QName>,
        content: &mut Content,
    ) {
        if let Some(name) = attribute_qname(declaration, "type") {
            self.fill_named(&name, indent, stack, content);
        } else if let Some(inline) = xsd_children(declaration)
            .find(|c| matches!(c.tag_name().name(), "complexType" | "simpleType"))
        {
            self.fill(inline, indent, stack, content);
        }
    }

    fn fill_named(
        &mut self,
        name: &QName,
        indent: usize,
        stack: &mut Vec<QName>,
        content: &mut Content,
    ) {
        if name.0 == XSD {
            content.text = Some(built_in(&name.1).to_owned());
            return;
        }
        let Some(found) = Model::find(&self.model.types, name) else {
            self.type_note(name);
            return;
        };
        // A type inside itself ends the example there.
        if stack.contains(name) || stack.len() >= MAX_DEPTH {
            content.note = Some(format!("{}: recursive, left out", name.1));
            return;
        }
        stack.push(name.clone());
        self.fill(found, indent, stack, content);
        stack.pop();
    }

    fn fill(
        &mut self,
        node: Node<'a, 'i>,
        indent: usize,
        stack: &mut Vec<QName>,
        content: &mut Content,
    ) {
        if node.tag_name().name() == "simpleType" {
            self.fill_simple(node, stack, content);
            return;
        }
        for child in xsd_children(node) {
            match child.tag_name().name() {
                "complexContent" | "simpleContent" => {
                    let extension = xsd_child(child, "extension");
                    let derived = extension.or_else(|| xsd_child(child, "restriction"));
                    let Some(derived) = derived else { continue };
                    // A restriction repeats what it keeps, so only an
                    // extension (or simple content's base) adds its base.
                    let simple = child.tag_name().name() == "simpleContent";
                    if (extension.is_some() || simple)
                        && let Some(base) = attribute_qname(derived, "base")
                    {
                        self.fill_named(&base, indent, stack, content);
                    }
                    self.fill(derived, indent, stack, content);
                }
                "attribute" => self.attribute(child, content),
                "attributeGroup" => {
                    let group = attribute_qname(child, "ref")
                        .and_then(|name| Model::find(&self.model.attribute_groups, &name));
                    if let Some(group) = group {
                        self.fill(group, indent, stack, content);
                    }
                }
                _ => {
                    let mut children = std::mem::take(&mut content.children);
                    self.particle(child, indent, stack, &mut children);
                    content.children = children;
                }
            }
        }
    }

    /// Sequences, choices, groups and elements: what goes between the tags.
    fn particle(
        &mut self,
        node: Node<'a, 'i>,
        indent: usize,
        stack: &mut Vec<QName>,
        out: &mut String,
    ) {
        match node.tag_name().name() {
            "element" => self.element(node, indent, stack, out),
            "sequence" | "all" => {
                for child in xsd_children(node) {
                    self.particle(child, indent, stack, out);
                }
            }
            "choice" => {
                let options: Vec<Node<'a, 'i>> = xsd_children(node).collect();
                let Some(first) = options.first() else {
                    return;
                };
                if options.len() > 1 {
                    let names: Vec<&str> = options
                        .iter()
                        .map(|o| {
                            o.attribute("name")
                                .or_else(|| o.attribute("ref"))
                                .unwrap_or_else(|| o.tag_name().name())
                        })
                        .collect();
                    let note = comment(&format!("one of: {}", names.join(", ")));
                    out.push_str(&format!("{:indent$}{note}\n", ""));
                }
                self.particle(*first, indent, stack, out);
            }
            "group" => {
                let group = attribute_qname(node, "ref")
                    .and_then(|name| Model::find(&self.model.groups, &name));
                if let Some(group) = group {
                    for child in xsd_children(group) {
                        self.particle(child, indent, stack, out);
                    }
                }
            }
            "any" => {
                let note = comment("any element");
                out.push_str(&format!("{:indent$}{note}\n", ""));
            }
            _ => {}
        }
    }

    fn fill_simple(&mut self, node: Node<'a, 'i>, stack: &mut Vec<QName>, content: &mut Content) {
        let Some(restriction) = xsd_child(node, "restriction") else {
            content.text = Some("string".to_owned());
            return;
        };
        let values: Vec<&str> = xsd_children(restriction)
            .filter(|c| c.tag_name().name() == "enumeration")
            .filter_map(|c| c.attribute("value"))
            .collect();
        if let Some(first) = values.first() {
            content.text = Some((*first).to_owned());
            if values.len() > 1 {
                content.note = Some(format!("one of: {}", values.join(", ")));
            }
            return;
        }
        match attribute_qname(restriction, "base") {
            Some(base) => self.fill_named(&base, 0, stack, content),
            None => match xsd_child(restriction, "simpleType") {
                Some(inner) => self.fill_simple(inner, stack, content),
                None => content.text = Some("string".to_owned()),
            },
        }
    }

    /// Only required attributes go in the example.
    fn attribute(&mut self, node: Node<'a, 'i>, content: &mut Content) {
        if node.attribute("use") != Some("required") {
            return;
        }
        let name = match attribute_qname(node, "ref") {
            Some((namespace, local)) => self.qualified(&namespace, &local),
            None => self.declared_name(node, "attributeFormDefault"),
        };
        let mut value = Content::default();
        if let Some(fixed) = node.attribute("fixed") {
            value.text = Some(fixed.to_owned());
        } else {
            self.fill_from_declaration(node, 0, &mut Vec::new(), &mut value);
        }
        content
            .attributes
            .push((name, value.text.unwrap_or_else(|| "string".to_owned())));
    }

    fn write(
        &mut self,
        tag: &str,
        occurs: Option<&str>,
        mut content: Content,
        indent: usize,
        out: &mut String,
    ) {
        let notes: Vec<String> = occurs
            .map(str::to_owned)
            .into_iter()
            .chain(content.note.take())
            .collect();
        if !notes.is_empty() {
            let note = comment(&notes.join("; "));
            out.push_str(&format!("{:indent$}{note}\n", ""));
        }
        let attributes: String = content
            .attributes
            .iter()
            .map(|(name, value)| format!(" {name}=\"{}\"", escape(value, true)))
            .collect();
        out.push_str(&format!("{:indent$}<{tag}{attributes}", ""));
        if !content.children.is_empty() {
            // Children were written one level too shallow; move them in.
            out.push_str(">\n");
            for line in content.children.lines() {
                out.push_str(&format!("  {line}\n"));
            }
            out.push_str(&format!("{:indent$}</{tag}>\n", ""));
        } else if let Some(text) = &content.text {
            out.push_str(&format!(">{}</{tag}>\n", escape(text, false)));
        } else {
            out.push_str("/>\n");
        }
    }

    /// A part written as an element: its global element, or its type under
    /// the part's name (without a namespace, as the bindings require).
    fn part(&mut self, part: &Part, indent: usize, out: &mut String) {
        let mut stack = Vec::new();
        if let Some(name) = &part.element {
            match Model::find(&self.model.elements, name) {
                Some(element) => self.element(element, indent, &mut stack, out),
                None => self.type_note(name),
            }
        } else if let Some(name) = &part.kind {
            let mut content = Content::default();
            self.fill_named(name, indent, &mut stack, &mut content);
            self.write(&part.name, None, content, indent, out);
        } else {
            self.write(&part.name, None, Content::default(), indent, out);
        }
    }
}

/// The envelope of one operation.
fn envelope(
    model: &Model<'_, '_>,
    version: SoapVersion,
    binding_style: &str,
    operation: &BoundOperation<'_, '_>,
    input: Option<&QName>,
) -> (String, Vec<String>) {
    let mut writer = Writer {
        model,
        namespaces: Vec::new(),
        missing: Vec::new(),
    };
    let empty = Vec::new();
    let parts = input
        .and_then(|name| model.messages.get(name))
        .unwrap_or(&empty);
    let in_body = |part: &&Part| {
        operation
            .parts
            .as_ref()
            .is_none_or(|names| names.contains(&part.name))
            && !operation
                .headers
                .iter()
                .any(|(message, name)| input == Some(message) && *name == part.name)
    };
    let mut body = String::new();
    let style = operation.style.as_deref().unwrap_or(binding_style);
    if style == "rpc" {
        // A wrapper named after the operation, holding the parts.
        let wrapper = writer.qualified(&operation.namespace, &operation.name);
        let mut inner = String::new();
        for part in parts.iter().filter(in_body) {
            writer.part(part, 4, &mut inner);
        }
        let content = Content {
            children: inner,
            ..Content::default()
        };
        writer.write(&wrapper, None, content, 4, &mut body);
    } else {
        for part in parts.iter().filter(in_body) {
            writer.part(part, 4, &mut body);
        }
    }
    let mut header = String::new();
    for (message, name) in &operation.headers {
        let part = model
            .messages
            .get(message)
            .and_then(|parts| parts.iter().find(|p| p.name == *name));
        if let Some(part) = part {
            writer.part(part, 4, &mut header);
        }
    }
    let text = version.envelope(&writer.namespaces, &header, &body);
    (text, writer.missing)
}

/// Converts a WSDL document. `loader` holds the documents it imports, when
/// they were read; what could not be read is listed in the report.
pub fn convert(source: &str, loader: &Loader) -> Result<Found, ImportError> {
    let main = parse_document(source)
        .map_err(|e| ImportError::Unsupported(format!("This WSDL is not valid XML: {e}")))?;
    if is_element(&main.root_element(), WSDL2, "description") {
        return Err(ImportError::Unsupported(
            "This is WSDL 2.0; Apix reads WSDL 1.1.".into(),
        ));
    }
    if !is_element(&main.root_element(), WSDL, "definitions") {
        return Err(ImportError::Unsupported(
            "This is not a WSDL document.".into(),
        ));
    }
    let mut report = Report::default();
    let mut imported = Vec::new();
    for text in &loader.documents {
        match parse_document(text) {
            Ok(doc) => imported.push(doc),
            Err(e) => report.warn("", format!("An imported document is not valid XML: {e}")),
        }
    }
    for (location, why) in &loader.failed {
        let location = apix_core::display_url(location);
        report.warn("", format!("Could not read {location}: {why}"));
    }
    let mut model = Model::default();
    model.read_document(&main);
    for doc in &imported {
        model.read_document(doc);
    }
    if model.title.is_empty() {
        model.title = "WSDL".to_owned();
    }
    report.collection = model.title.clone();

    let soap_ports: Vec<(&Port, &Binding<'_, '_>)> = model
        .ports
        .iter()
        .filter_map(|port| {
            let binding = model.bindings.get(&port.binding);
            match binding {
                Some(binding) if binding.version.is_some() => Some((port, binding)),
                Some(_) => {
                    report.warn(
                        &port.name,
                        "This port is not SOAP (HTTP GET or POST); it was left out.",
                    );
                    None
                }
                None => {
                    report.warn(&port.name, "The port's binding is not defined.");
                    None
                }
            }
        })
        .collect();
    if soap_ports.is_empty() {
        return Err(ImportError::Unsupported(
            "This WSDL has no SOAP port to call.".into(),
        ));
    }

    // One address is `baseUrl`; several are a variable per port.
    let addresses: HashSet<&str> = soap_ports.iter().map(|(p, _)| p.address.as_str()).collect();
    let one_address = addresses.len() == 1;
    let several_ports = soap_ports.len() > 1;
    let mut vars = VarMap::new();
    let mut files = Vec::new();
    let mut folders = HashSet::new();
    let mut names: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for (port, binding) in &soap_ports {
        let version = binding.version.unwrap_or(SoapVersion::V11);
        if port.address.is_empty() {
            report.warn(&port.name, "The port has no address; fill in the variable.");
        }
        let variable = if one_address {
            "baseUrl".to_owned()
        } else {
            format!("{}Url", slug(&port.name, "port").replace('-', "_"))
        };
        vars.insert(variable.clone(), port.address.clone());
        let dir = if several_ports {
            let folder = if model.ports.iter().any(|p| p.service != port.service) {
                format!("{} {}", port.service, port.name)
            } else {
                port.name.clone()
            };
            PathBuf::from(unique(slug(&folder, "port"), &mut folders))
        } else {
            PathBuf::new()
        };
        let declared = model.port_types.get(&binding.port_type);
        for operation in &binding.operations {
            let item = format!("{} / {}", port.name, operation.name);
            let details = declared.and_then(|operations| operations.get(&operation.name));
            if details.is_none() {
                report.warn(&item, "The port type does not declare this operation.");
            }
            if operation.encoded {
                report.warn(
                    &item,
                    "It uses SOAP encoding; the example has no xsi:type attributes, which the server may want.",
                );
            }
            let input = details.and_then(|(input, _)| input.as_ref());
            let (body, missing) = envelope(&model, version, &binding.style, operation, input);
            for name in missing {
                report.warn(&item, format!("{name} is not defined in what was read."));
            }
            let description = details
                .map(|(_, text)| text.clone())
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| documentation(operation.node));
            let request = Request {
                name: operation.name.clone(),
                description,
                method: Method::Post,
                url: format!("{{{{{variable}}}}}"),
                headers: version.headers(&operation.action),
                body: Body::Xml(body),
                auth: Auth::Inherit,
                settings: RequestSettings::default(),
                ..Request::default()
            };
            let stem = unique(
                slug(&operation.name, "operation"),
                names.entry(dir.clone()).or_default(),
            );
            files.push(PlannedFile::Request {
                path: dir.join(request_file(&stem)),
                request,
            });
            report.requests += 1;
        }
    }
    report.folders = folders.len();
    report.variables = vars.len();
    Ok(Found {
        collections: vec![(
            String::new(),
            Plan {
                meta: CollectionMeta {
                    name: model.title.clone(),
                    description: model.description.clone(),
                    vars,
                    ..CollectionMeta::default()
                },
                files,
                report,
            },
        )],
        ..Found::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_resolve_against_their_document() {
        assert_eq!(
            resolve("http://h/a/b.wsdl", "c.xsd").as_deref(),
            Some("http://h/a/c.xsd")
        );
        assert_eq!(
            resolve("http://h/s?wsdl", "s?xsd=1").as_deref(),
            Some("http://h/s?xsd=1")
        );
        assert_eq!(
            resolve("/x/y.wsdl", "http://h/c.xsd").as_deref(),
            Some("http://h/c.xsd")
        );
        let local = resolve("x/y.wsdl", "c.xsd").unwrap();
        assert_eq!(local.replace('\\', "/"), "x/c.xsd");
        // A document from the network cannot read local files.
        assert_eq!(resolve("http://h/a.wsdl", "file:///etc/passwd"), None);
        assert_eq!(resolve("/x/a.wsdl", "ftp://h/c.xsd"), None);
    }

    #[test]
    fn imports_are_followed_once_and_stop_at_the_limit() {
        let text = |n: usize| {
            format!(
                r#"<xs:schema xmlns:xs="{XSD}"><xs:include schemaLocation="d{n}.xsd"/><xs:include schemaLocation="d0.xsd"/></xs:schema>"#
            )
        };
        let mut loader = Loader::new("/x/main.wsdl", &text(0));
        let mut read = 0;
        while let Some(location) = loader.next_location() {
            read += 1;
            loader.add(location, Ok(text(read)));
        }
        assert!(read < MAX_DOCUMENTS, "{read}");
        assert_eq!(loader.documents.len(), read);
        assert!(!loader.failed.is_empty());
    }

    #[test]
    fn wsdl_is_told_apart() {
        assert!(is_wsdl(&format!(r#"<definitions xmlns="{WSDL}"/>"#)));
        assert!(is_wsdl(&format!(
            "\u{feff}<?xml version=\"1.0\"?><w:definitions xmlns:w=\"{WSDL}\"/>"
        )));
        assert!(is_wsdl(&format!(r#"<description xmlns="{WSDL2}"/>"#)));
        assert!(!is_wsdl(r#"{"wsdl": true}"#));
        assert!(!is_wsdl("<html>wsdl</html>"));
    }
}
