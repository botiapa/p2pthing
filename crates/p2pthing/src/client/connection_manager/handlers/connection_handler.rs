use std::net::SocketAddr;

use p2pthing_common::{
    encryption::SymmetricEncryption,
    message_type::{msg_types::AnnounceSecret, InterthreadMessage, MsgType},
    ui::UIConn,
};

use crate::client::{connection_manager::ConnectionManager, udp_connection::UdpConnectionState};

use super::ConnectionHandler;

impl ConnectionHandler for ConnectionManager {
    fn on_udp_announce(&mut self, addr: SocketAddr) {
        self.peers.conn_mut(&addr).unwrap().state = UdpConnectionState::Connected;
        self.ui_s.log_info("UDP Announcement has been accepted");
    }

    fn on_keep_alive(&mut self, addr: SocketAddr) {
        self.ui_s.log_info(&format!("Keep alive message received from {}", addr));
        self.check_punchthrough(addr);
    }

    fn on_secret_announce(&mut self, addr: SocketAddr, data: &[u8]) {
        let secret: AnnounceSecret = bincode::deserialize(data).unwrap();
        let secret = &secret.secret[..];

        let conn = self.peers.conn_mut(&addr).unwrap();

        conn.symmetric_key = Some(SymmetricEncryption::new_from_secret(secret));
        conn.upgraded = true;

        self.ui_s.log_info(&format!("Received secret for peer: ({})", conn.associated_peer.as_ref().unwrap()));
        self.check_punchthrough(addr);
    }

    fn check_punchthrough(&mut self, addr: SocketAddr) {
        let conn = self.peers.conn_mut(&addr).unwrap();
        match conn.state {
            UdpConnectionState::MidCall => {
                let p = conn.associated_peer.clone().unwrap();
                conn.state = UdpConnectionState::Connected;
                self.ui_s.log_info(&format!("Punch through successfull. Connected to peer: ({})", p));
                self.ui_s.send(InterthreadMessage::PunchThroughSuccessfull(p)).unwrap();
            }
            _ => {}
        }
    }

    fn on_confirmation_message(&mut self, addr: SocketAddr, data: &[u8]) {
        let id: u32 = bincode::deserialize(data).unwrap();
        let conn = self.peers.conn_mut(&addr).unwrap();

        let removed =
            conn.sent_messages.iter_mut().position(|msg| msg.packet.msg_id == id).map(|i| conn.sent_messages.remove(i));
        match removed {
            Some(packet) => {
                conn.statistics.new_ping(packet.sent.elapsed());
                match packet.msg_type {
                    MsgType::AnnounceSecret => {
                        conn.upgraded = true;
                        self.ui_s
                            .log_info(&format!("Peer received secret: ({})", conn.associated_peer.as_ref().unwrap()));
                        self.check_punchthrough(addr);
                    }
                    MsgType::ChatMessage => {
                        let id = self.msg_confirmations.remove(&packet.custom_id.unwrap()).unwrap();
                        self.ui_s.log_info(&format!(
                            "Chat message confirmed by: ({})",
                            conn.associated_peer.as_ref().unwrap()
                        ));
                        self.ui_s.send(InterthreadMessage::OnChatMessageReceived(id)).unwrap();
                    }
                    MsgType::RequestFileChunks => {}
                    _ => unreachable!(),
                }
            }
            None => self.ui_s.log_warning(&format!("Couldn't find message with confirmation id: ({})", id)),
        }
    }
}
