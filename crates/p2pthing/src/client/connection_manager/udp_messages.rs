use std::net::SocketAddr;

use p2pthing_common::{
    encryption::SymmetricEncryption,
    message_type::{
        msg_types::{self, AnnouncePublic, AnnounceSecret},
        InterthreadMessage, MsgType, UdpPacket,
    },
    num,
    ui::UIConn,
};
use tracing::instrument;

use crate::client::{
    peer::{Peer, PeerSource, PeerType},
    udp_connection::{UdpConnection, UdpConnectionState},
};

use super::{
    handlers::{ChatHandler, ConnectionHandler, FileHandler},
    ConnectionManager, MULTICAST_MAGIC,
};

impl ConnectionManager {
    #[instrument(skip(self, buf))]
    pub fn read_udp_message(&mut self, _: usize, addr: SocketAddr, buf: &[u8]) {
        let conn = match self.peers.conn_mut(&addr) {
            Some(c) => c,
            None => {
                self.ui_s.log_warning(&format!(
                    "Tried reading from ({}), but couldn't find the associated connection",
                    addr
                ));
                return;
            }
        };

        //TODO: Move all this logic to udp_connection.rs

        let udp_packet: UdpPacket = bincode::deserialize(&buf).unwrap();
        conn.statistics.received_bytes(bincode::serialized_size(&udp_packet).unwrap());
        if conn.received_messages.contains(&udp_packet.msg_id) {
            // If already received this message
            return;
        }
        conn.received_messages.push(udp_packet.msg_id);

        if udp_packet.reliable {
            conn.send_confirmation(udp_packet.msg_id);
        }

        let buf = match conn.decrypt(udp_packet) {
            Ok(buf) => buf,
            Err(_) => return,
        };

        let msg_type = buf[0];
        let msg_type = num::FromPrimitive::from_u8(msg_type);

        match msg_type {
            Some(MsgType::Announce) => {
                self.on_udp_announce(addr);
            }
            Some(MsgType::KeepAlive) => {
                self.on_keep_alive(addr);
            }
            Some(MsgType::ChatMessage) => {
                self.on_chat_message(addr, &buf[1..]);
            }
            Some(MsgType::AnnounceSecret) => {
                self.on_secret_announce(addr, &buf[1..]);
            }
            Some(MsgType::MessageConfirmation) => {
                self.on_confirmation_message(addr, &buf[1..]);
            }
            Some(MsgType::OpusPacket) => {
                #[cfg(feature = "audio")]
                self.on_opus_packet(addr, &buf[1..]);
            }
            Some(MsgType::RequestFileChunks) => {
                self.on_request_file_chunks(addr, &buf[1..]);
            }
            Some(MsgType::FileChunk) => {
                self.on_file_chunks(addr, &buf[1..]);
            }
            _ => unreachable!(),
        }
    }
}
