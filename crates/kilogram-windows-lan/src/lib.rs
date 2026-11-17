//! Dormant Win32 foundation. Inspect the selected local network on a blocking
//! worker, then use the fixed discovery socket on that network.

#![cfg(windows)]

use kilogram_protocol::ordered_dialog_v2::lan_discovery::{DATAGRAM_BYTES, GROUP};
use std::net::{Ipv4Addr, SocketAddrV4};

#[allow(unsafe_code)]
mod native;

pub use native::{Binding, Cancellation, DiscoverySocket, NetworkScope, Receive, inspect};

const IPV4_UDP_HEADERS: usize = 28;
const MIN_MTU: u32 = (DATAGRAM_BYTES + IPV4_UDP_HEADERS) as u32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketInfo {
    pub source: SocketAddrV4,
    pub destination: Ipv4Addr,
    pub if_index: u32,
}

/// Check the outer route only. The codec and runtime check locator and peer authority.
pub fn is_peer_datagram(info: PacketInfo, expected_peer: Ipv4Addr, selected_if_index: u32) -> bool {
    selected_if_index != 0
        && info.if_index == selected_if_index
        && *info.source.ip() == expected_peer
        && info.destination == GROUP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lan_receive_scope_matches_all_three_fields() {
        let peer = Ipv4Addr::new(192, 168, 48, 6);
        let info = PacketInfo {
            source: SocketAddrV4::new(peer, 50_372),
            destination: GROUP,
            if_index: 12,
        };
        assert!(is_peer_datagram(info, peer, 12));
        assert!(!is_peer_datagram(info, peer, 13));
        assert!(!is_peer_datagram(info, Ipv4Addr::new(192, 168, 48, 7), 12));
        assert!(!is_peer_datagram(
            PacketInfo {
                destination: peer,
                ..info
            },
            peer,
            12
        ));
        assert!(!is_peer_datagram(
            PacketInfo {
                if_index: 0,
                ..info
            },
            peer,
            0
        ));
    }
}
