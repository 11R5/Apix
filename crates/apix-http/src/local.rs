//! This machine's side of a connection: which interface it left through,
//! the gateway and DNS servers, whether that looks like a VPN, and a proxy
//! set in the environment.

use std::net::IpAddr;

use netdev::interface::types::InterfaceType;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalNetwork {
    /// "Wi-Fi", "Ethernet 2", "tailscale0" …
    pub interface: Option<String>,
    /// "Wi-Fi", "Ethernet", "Tunnel" …
    pub kind: Option<String>,
    pub local_ip: Option<IpAddr>,
    pub gateway: Option<IpAddr>,
    pub dns_servers: Vec<IpAddr>,
    /// The interface is a tunnel or named like a VPN (WireGuard, Tailscale, OpenVPN …).
    pub vpn: bool,
    /// A proxy from the environment, as `(variable, value)`.
    pub proxy: Option<(String, String)>,
}

/// Names that give a VPN interface away, lowercased.
const VPN_NAMES: &[&str] = &[
    "tailscale",
    "wireguard",
    "wg",
    "utun",
    "tun",
    "tap",
    "vpn",
    "openvpn",
    "zerotier",
    "nordlynx",
    "proton",
    "mullvad",
];

/// Describes the interface that holds `local_ip` (or the default one).
/// Reads the system's interface table, so call it off the UI thread.
pub fn local_network(local_ip: Option<IpAddr>) -> LocalNetwork {
    let interfaces = netdev::get_interfaces();
    let found = local_ip
        .and_then(|ip| interfaces.iter().find(|i| i.ip_addrs().contains(&ip)))
        .or_else(|| interfaces.iter().find(|i| i.default));
    let mut out = LocalNetwork {
        local_ip,
        proxy: proxy_from_env(),
        ..LocalNetwork::default()
    };
    let Some(interface) = found else {
        return out;
    };
    let name = interface
        .friendly_name
        .clone()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| interface.name.clone());
    let lower = format!(
        "{} {}",
        name.to_lowercase(),
        interface
            .description
            .clone()
            .unwrap_or_default()
            .to_lowercase()
    );
    out.vpn = interface.if_type == InterfaceType::Tunnel
        || interface.is_tun()
        || VPN_NAMES.iter().any(|v| {
            lower
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| word == *v || (v.len() > 3 && word.starts_with(v)))
        });
    out.kind = Some(kind(interface.if_type));
    out.interface = Some(name);
    out.gateway = interface.gateway.as_ref().and_then(|g| {
        g.ipv4
            .first()
            .map(|ip| IpAddr::V4(*ip))
            .or_else(|| g.ipv6.first().map(|ip| IpAddr::V6(*ip)))
    });
    out.dns_servers = interface.dns_servers.clone();
    if out.local_ip.is_none() {
        out.local_ip = interface.ip_addrs().into_iter().next();
    }
    out
}

fn kind(t: InterfaceType) -> String {
    match t {
        InterfaceType::Wireless80211 | InterfaceType::PeerToPeerWireless => "Wi-Fi".into(),
        InterfaceType::Ethernet
        | InterfaceType::FastEthernetT
        | InterfaceType::FastEthernetFx
        | InterfaceType::GigabitEthernet
        | InterfaceType::Ethernet3Megabit => "Ethernet".into(),
        InterfaceType::Tunnel => "Tunnel".into(),
        InterfaceType::Ppp => "PPP".into(),
        InterfaceType::Loopback => "Loopback".into(),
        InterfaceType::Wwan | InterfaceType::Wwanpp | InterfaceType::Wwanpp2 => "Mobile".into(),
        InterfaceType::Bridge => "Bridge".into(),
        other => format!("{other:?}"),
    }
}

/// The proxy reqwest would take from the environment, if any.
fn proxy_from_env() -> Option<(String, String)> {
    [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ]
    .into_iter()
    .find_map(|var| {
        std::env::var(var)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(|value| (var.to_owned(), value))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_some_interface_without_panicking() {
        let net = local_network(None);
        // CI machines differ; only check that whatever was found is consistent.
        if net.interface.is_some() {
            assert!(net.kind.is_some());
        }
        let loopback = local_network(Some(IpAddr::from([127, 0, 0, 1])));
        assert_eq!(loopback.local_ip, Some(IpAddr::from([127, 0, 0, 1])));
    }
}
