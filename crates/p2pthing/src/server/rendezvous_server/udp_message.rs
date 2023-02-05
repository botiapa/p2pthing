use std::net::SocketAddr;

use p2pthing_common::{message_type::{MsgType, UdpPacket, msg_types}, num};

use super::RendezvousServer;

impl RendezvousServer {
    pub fn read_udp_message(&mut self, _: usize, addr: SocketAddr, buf: &[u8]) {
        let udp_packet: UdpPacket = bincode::deserialize(&buf).unwrap();

        let buf = udp_packet.data;
        let msg_type = buf[0];
        let msg_type = num::FromPrimitive::from_u8(msg_type);

        match msg_type {
            Some(MsgType::Announce) => {
                let announce: msg_types::AnnouncePublic = bincode::deserialize(&buf[1..]).unwrap();
                match self.peers.iter_mut().find(|p| p.public_key == announce.public_key) {
                    Some(p) => {
                        p.udp_addr = Some(addr);
                        println!("Associated UDP address ({}) with peer: ({})", addr, p.public_key);
                        self.send_udp_message(addr, MsgType::Announce, &());
                    },
                    None => {}
                }
            }
            Some(MsgType::KeepAlive) => {}
            _ => unreachable!()
        }
    }
}