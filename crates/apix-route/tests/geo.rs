//! Places read from an IP location database. The database is a small one
//! written here in the MaxMind DB format, so nothing is downloaded.

use std::net::IpAddr;

use apix_route::{Geo, Place};

#[path = "support/mmdb_writer.rs"]
mod mmdb_writer;
use mmdb_writer::{V, city, database};

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

#[test]
fn places_come_from_the_database() {
    let dir = std::env::temp_dir().join(format!("apix-geo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bytes = database(&[
        ([1, 1, 1, 0], 24, city("Sydney", "AU", -33.86, 151.2)),
        ([81, 212, 0, 0], 16, city("Istanbul", "TR", 41.01, 28.98)),
        // Only a country, no city or coordinates.
        (
            [5, 5, 0, 0],
            16,
            V::Map(vec![("country", V::Map(vec![("iso_code", V::Str("DE"))]))]),
        ),
    ]);
    std::fs::write(dir.join("b.mmdb"), &bytes).unwrap();
    std::fs::write(dir.join("a.mmdb"), &bytes).unwrap();
    std::fs::write(dir.join("notes.txt"), "not a database").unwrap();

    let path = Geo::find(&dir).unwrap();
    assert_eq!(path.file_name().unwrap(), "a.mmdb");
    let geo = Geo::open(&path).unwrap();

    assert_eq!(
        geo.locate(ip("1.1.1.1")),
        Some(Place {
            latitude: -33.86,
            longitude: 151.2,
            city: Some("Sydney".into()),
            country: Some("AU".into()),
        })
    );
    assert_eq!(
        geo.locate(ip("81.212.73.33")).unwrap().label(),
        "Istanbul, TR"
    );
    // Not in the database, no coordinates, or a private address.
    assert_eq!(geo.locate(ip("8.8.8.8")), None);
    assert_eq!(geo.locate(ip("5.5.5.5")), None);
    assert_eq!(geo.locate(ip("192.168.1.1")), None);

    assert!(Geo::open(&dir.join("notes.txt")).is_err());
    assert!(Geo::find(&dir.join("missing")).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The system's own traceroute to this machine: one hop, the address itself.
#[test]
fn a_trace_to_localhost() {
    let trace = match apix_route::Trace::start("127.0.0.1", 3) {
        Ok(trace) => trace,
        // A Linux without traceroute or tracepath.
        Err(e) if e.contains("not installed") => return,
        Err(e) => panic!("{e}"),
    };
    let hops: Vec<_> = trace.collect();
    assert_eq!(hops.first().map(|h| h.ttl), Some(1), "{hops:?}");
    assert_eq!(hops[0].ip, Some(ip("127.0.0.1")), "{hops:?}");
    assert!(apix_route::Trace::start("-x", 3).is_err());
    assert!(apix_route::Trace::start("a b", 3).is_err());
}

/// A downloaded database is unpacked, checked and kept in place of the old one.
#[test]
fn a_download_is_installed() {
    use std::io::Write as _;
    let dir = std::env::temp_dir().join(format!("apix-geo-install-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = database(&[([1, 1, 1, 0], 24, city("Sydney", "AU", -33.86, 151.2))]);
    std::fs::write(dir.join("old.mmdb"), &db).unwrap();

    // Not a database: refused, the old one stays.
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(b"<html>not found</html>").unwrap();
    let error = apix_route::install(&gz.finish().unwrap(), &dir, "new.mmdb").unwrap_err();
    assert!(error.contains("not an IP location database"), "{error}");
    assert!(dir.join("old.mmdb").exists());
    assert!(!dir.join("new.mmdb.part").exists());

    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&db).unwrap();
    let path = apix_route::install(&gz.finish().unwrap(), &dir, "new.mmdb").unwrap();
    assert!(!dir.join("old.mmdb").exists());
    assert_eq!(Geo::find(&dir), Some(path.clone()));
    let geo = Geo::open(&path).unwrap();
    assert_eq!(geo.locate(ip("1.1.1.9")).unwrap().label(), "Sydney, AU");
    // Not compressed works too.
    assert!(apix_route::install(&db, &dir, "plain.mmdb").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}
