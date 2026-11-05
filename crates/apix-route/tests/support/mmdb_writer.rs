//! A small MaxMind DB writer (IPv4, 24-bit records) for the tests and the
//! demo database; Apix itself only reads the format.

#![allow(dead_code)]

/// A value in the MaxMind DB data section.
pub enum V {
    Str(&'static str),
    F64(f64),
    U16(u16),
    U32(u32),
    U64(u64),
    Arr(Vec<V>),
    Map(Vec<(&'static str, V)>),
}

fn control(out: &mut Vec<u8>, kind: u8, size: usize) {
    assert!(size < 29);
    if kind <= 7 {
        out.push((kind << 5) | size as u8);
    } else {
        out.push(size as u8);
        out.push(kind - 7);
    }
}

fn number(out: &mut Vec<u8>, kind: u8, value: u64) {
    let bytes = value.to_be_bytes();
    let skip = bytes.iter().take_while(|b| **b == 0).count();
    control(out, kind, 8 - skip);
    out.extend_from_slice(&bytes[skip..]);
}

fn encode(out: &mut Vec<u8>, value: &V) {
    match value {
        V::Str(s) => {
            control(out, 2, s.len());
            out.extend_from_slice(s.as_bytes());
        }
        V::F64(f) => {
            control(out, 3, 8);
            out.extend_from_slice(&f.to_be_bytes());
        }
        V::U16(n) => number(out, 5, u64::from(*n)),
        V::U32(n) => number(out, 6, u64::from(*n)),
        V::U64(n) => number(out, 9, *n),
        V::Arr(items) => {
            control(out, 11, items.len());
            for item in items {
                encode(out, item);
            }
        }
        V::Map(pairs) => {
            control(out, 7, pairs.len());
            for (key, value) in pairs {
                encode(out, &V::Str(key));
                encode(out, value);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Child {
    Empty,
    Node(usize),
    Data(usize),
}

/// An IPv4 database of `(network, prefix length, record)`.
pub fn database(networks: &[([u8; 4], u32, V)]) -> Vec<u8> {
    let mut nodes: Vec<[Child; 2]> = vec![[Child::Empty; 2]];
    let mut data = Vec::new();
    for (addr, len, record) in networks {
        let offset = data.len();
        encode(&mut data, record);
        let bits = u32::from_be_bytes(*addr);
        let mut node = 0;
        for depth in 0..*len {
            let bit = ((bits >> (31 - depth)) & 1) as usize;
            if depth + 1 == *len {
                nodes[node][bit] = Child::Data(offset);
            } else {
                node = match nodes[node][bit] {
                    Child::Node(next) => next,
                    _ => {
                        nodes.push([Child::Empty; 2]);
                        let next = nodes.len() - 1;
                        nodes[node][bit] = Child::Node(next);
                        next
                    }
                };
            }
        }
    }
    let count = nodes.len();
    let mut out = Vec::new();
    for node in &nodes {
        for child in node {
            let value = match *child {
                Child::Empty => count,
                Child::Node(i) => i,
                Child::Data(offset) => count + 16 + offset,
            } as u32;
            out.extend_from_slice(&value.to_be_bytes()[1..]);
        }
    }
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(&data);
    out.extend_from_slice(b"\xAB\xCD\xEFMaxMind.com");
    encode(
        &mut out,
        &V::Map(vec![
            ("node_count", V::U32(count as u32)),
            ("record_size", V::U16(24)),
            ("ip_version", V::U16(4)),
            ("database_type", V::Str("Apix-Test-City")),
            ("languages", V::Arr(vec![V::Str("en")])),
            ("binary_format_major_version", V::U16(2)),
            ("binary_format_minor_version", V::U16(0)),
            ("build_epoch", V::U64(1_700_000_000)),
            ("description", V::Map(vec![("en", V::Str("test"))])),
        ]),
    );
    out
}

pub fn city(name: &'static str, country: &'static str, lat: f64, lon: f64) -> V {
    V::Map(vec![
        (
            "city",
            V::Map(vec![("names", V::Map(vec![("en", V::Str(name))]))]),
        ),
        ("country", V::Map(vec![("iso_code", V::Str(country))])),
        (
            "location",
            V::Map(vec![("latitude", V::F64(lat)), ("longitude", V::F64(lon))]),
        ),
    ])
}
