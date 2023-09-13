use std::net::SocketAddr;

use p2pthing_common::{message_type::msg_types::AnnouncePublic, ui::UIConn};

use crate::client::{
    connection_manager::{ConnectionManager, MULTICAST_MAGIC},
    peer::{Peer, PeerSource, PeerType},
    udp_connection::{UdpConnection, UdpConnectionState},
};

use super::MulticastHandler;

impl MulticastHandler for ConnectionManager {
    fn read_multicast_message(&mut self, _: usize, addr: SocketAddr, buf: &[u8]) {
        // First u32 is the magic packet
        let magic = &buf[0..4];
        let magic: u32 = bincode::deserialize(magic).unwrap();
        if magic == MULTICAST_MAGIC {
            let announce: AnnouncePublic = bincode::deserialize(&buf[4..]).unwrap();
            // If the peer is not this peer
            if announce.public_key != self.encryption.get_public_key() {
                let conn = UdpConnection::new(
                    UdpConnectionState::Unknown,
                    addr,
                    self.udp_socket.clone(),
                    None,
                    self.encryption.clone(),
                );
                if let Some(p) = self.peers.peer_mut(&announce.public_key) {
                    if p.udp_conn.is_none() {
                        p.udp_conn = Some(conn);
                    }
                    p.source = p.source | PeerSource::Multicast;
                } else {
                    self.peers.push(Peer {
                        addr: None,
                        tcp_conn: None,
                        udp_conn: Some(conn),
                        sym_key: None,
                        public_key: Some(announce.public_key.clone()),
                        source: PeerSource::Multicast.into(),
                        peer_type: PeerType::ClientPeer,
                    });
                }
                self.ui_s.log_info(&format!("Received multicast announce message: {:?}", announce.public_key))
            }
        }
    }
}
