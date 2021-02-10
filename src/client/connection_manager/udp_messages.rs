use std::net::SocketAddr;

use msg_types::ChatMessage;

use crate::{client::tui::Tui, common::{debug_message::DebugMessageType, message_type::{InterthreadMessage, MsgType, msg_types}}};

use super::{ConnectionManager, KEEP_ALIVE_DELAY, UdpConnectionState};

impl ConnectionManager {
    pub fn read_udp_message(&mut self, read: usize, addr: SocketAddr, buf: &[u8]) {
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
                let chat_message: msg_types::ChatMessage = bincode::deserialize(&buf[1..]).unwrap();
                self.on_chat_message(addr, chat_message);
            }
            _ => unreachable!()
        }
    }

    fn on_udp_announce(&mut self, addr: SocketAddr) {
        self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap()
        .state = UdpConnectionState::Connected;
        Tui::debug_message("UDP Announcement has been accepted", DebugMessageType::Log, &self.ui_s);
        self.waker_thread.send(InterthreadMessage::SetWakepDelay(KEEP_ALIVE_DELAY)).unwrap();
    }

    fn on_keep_alive(&mut self, addr: SocketAddr) {
        Tui::debug_message(&format!("Keep alive message received from {}", addr), DebugMessageType::Log, &self.ui_s);
        let conn = self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap();
        match conn.state {
            UdpConnectionState::MidCall => {
                let p = conn.associated_peer.clone().unwrap();
                conn.state = UdpConnectionState::Connected;
                Tui::debug_message(&format!("Punch through successfull. Connected to peer: ({})", p), DebugMessageType::Log, &self.ui_s);
                self.ui_s.send(InterthreadMessage::PunchThroughSuccessfull(p)).unwrap();
            }
            _ => {}
        }
        // If this was the last punchthrough waiting to be done, slow done the keep alive message delays
        if self.udp_connections.iter_mut().filter(|conn| match conn.state { UdpConnectionState::MidCall => true, _ => false}).count() == 0 {
            self.waker_thread.send(InterthreadMessage::SetWakepDelay(KEEP_ALIVE_DELAY)).unwrap();
        }
    }

    fn on_chat_message(&mut self, addr: SocketAddr, chat_message: ChatMessage) {
        let p = self.peers.iter().find(|p| p.udp_addr.unwrap() == addr).unwrap();
        Tui::on_chat_message(&self.ui_s, p.clone(), chat_message.msg);
    }
}