//! Writes a small demonstration IP location database, for trying the route
//! map without downloading one: `cargo run -p apix-route --example demo_geo --
//! OUT.mmdb [ADDRESS=LAT,LON,CITY,CC ...]`. Each address stands for its /24.
//! The places are whatever is given; they are not real lookups.

#[path = "../tests/support/mmdb_writer.rs"]
mod mmdb_writer;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .expect("usage: demo_geo OUT.mmdb [ADDRESS=LAT,LON,CITY,CC ...]");
    let mut networks = Vec::new();
    for arg in args {
        let (address, place) = arg.split_once('=').expect("ADDRESS=LAT,LON,CITY,CC");
        let ip: std::net::Ipv4Addr = address.parse().expect("an IPv4 address");
        let parts: Vec<&str> = place.split(',').collect();
        let [lat, lon, city, country] = parts[..] else {
            panic!("LAT,LON,CITY,CC: {place}")
        };
        let mut octets = ip.octets();
        octets[3] = 0;
        let leak = |s: &str| -> &'static str { Box::leak(s.to_owned().into_boxed_str()) };
        networks.push((
            octets,
            24,
            mmdb_writer::city(
                leak(city),
                leak(country),
                lat.parse().expect("latitude"),
                lon.parse().expect("longitude"),
            ),
        ));
    }
    networks.sort_by_key(|n| n.0);
    networks.dedup_by_key(|n| n.0);
    std::fs::write(&out, mmdb_writer::database(&networks)).expect("writing the database");
    println!("wrote {out} ({} networks)", networks.len());
}
