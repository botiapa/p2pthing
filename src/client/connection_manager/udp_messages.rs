use std::net::SocketAddr;

use msg_types::{AnnounceSecret, ChatMessage};

use crate::{client::{tui::Tui, udp_connection::UdpConnectionState}, common::{debug_message::DebugMessageType, encryption::SymmetricEncryption, message_type::{InterthreadMessage, MsgType, UdpPacket, msg_types}}};

use super::ConnectionManager;

impl ConnectionManager {
    pub fn read_udp_message(&mut self, _: usize, addr: SocketAddr, buf: &[u8]) {
        let conn = match self.udp_connections.iter_mut().find(|x| x.address == addr) {
            Some(c) => c,
            None => {
                Tui::debug_message(&format!("Tried reading from ({}), but couldn't find the associated connection", addr), DebugMessageType::Warning, &self.ui_s);
                return;
            }
        };
        let udp_packet: UdpPacket = bincode::deserialize(&buf).unwrap();
        conn.read_bytes += bincode::serialized_size(&udp_packet).unwrap();
        if conn.received_messages.contains(&udp_packet.msg_id) { // If already received this message
            return;
        }
        conn.received_messages.push(udp_packet.msg_id);

        if udp_packet.reliable {
            conn.send_confirmation(udp_packet.msg_id);
        }
        
        let buf = match conn.decrypt(udp_packet) {
            Ok(buf) => buf,
            Err(_) => return
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
                let chat_message: msg_types::ChatMessage = bincode::deserialize(&buf[1..]).unwrap();
                self.on_chat_message(addr, chat_message);
            }
            Some(MsgType::AnnounceSecret) => {
                self.on_secret_announce(addr, &buf[1..]);
            }
            Some(MsgType::MessageConfirmation) => {
                self.on_confirmation_message(addr, &buf[1..])
            }
            Some(MsgType::OpusPacket) => {
                self.on_opus_packet(addr, &buf[1..])
            }
            _ => unreachable!()
        }
    }

    fn check_punchthrough(&mut self, addr: SocketAddr) {
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
    }

    fn on_secret_announce(&mut self, addr: SocketAddr, data: &[u8]) {
        let secret: AnnounceSecret = bincode::deserialize(data).unwrap();
        let secret = &secret.secret[..];

        let conn = self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap();
        
        conn.symmetric_key = Some(SymmetricEncryption::new_from_secret(secret));
        conn.upgraded = true;

        Tui::debug_message(&format!("Received secret for peer: ({})", conn.associated_peer.as_ref().unwrap()), DebugMessageType::Log, &self.ui_s);
        self.check_punchthrough(addr);
    }

    fn on_confirmation_message(&mut self, addr: SocketAddr, data: &[u8]) {
        let id: u32 = bincode::deserialize(data).unwrap();
        let conn = self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap();

        let removed = conn.sent_messages.iter_mut().position(|msg| msg.packet.msg_id == id).map(|i| conn.sent_messages.remove(i));
        match removed {
            Some(msg) => {
                match msg.msg_type {
                    MsgType::AnnounceSecret => {
                        conn.upgraded = true;
                        Tui::debug_message(&format!("Peer received secret: ({})", conn.associated_peer.as_ref().unwrap()), DebugMessageType::Log, &self.ui_s);
                        self.check_punchthrough(addr);
                    }
                    MsgType::ChatMessage => {
                        self.ui_s.send(InterthreadMessage::OnChatMessageReceived(msg.custom_id.unwrap())).unwrap();
                        Tui::debug_message(&format!("Chat message confirmed by: ({})", conn.associated_peer.as_ref().unwrap()), DebugMessageType::Log, &self.ui_s);
                    }
                    _ => unreachable!()
                }
            }
            None => Tui::debug_message(&format!("Couldn't find message with confirmation id: ({})", id), DebugMessageType::Warning, &self.ui_s)
        }
    }

    fn on_udp_announce(&mut self, addr: SocketAddr) {
        self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap()
        .state = UdpConnectionState::Connected;
        Tui::debug_message("UDP Announcement has been accepted", DebugMessageType::Log, &self.ui_s);
    }

    fn on_keep_alive(&mut self, addr: SocketAddr) {
        Tui::debug_message(&format!("Keep alive message received from {}", addr), DebugMessageType::Log, &self.ui_s);
        let conn = self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap();
        
        self.check_punchthrough(addr);
    }

    fn on_chat_message(&mut self, addr: SocketAddr, chat_message: ChatMessage) {
        let p = self.peers.iter().find(|p| p.udp_addr.unwrap() == addr).unwrap();
        Tui::on_chat_message(&self.ui_s, p.clone(), chat_message.msg);
    }

    fn on_opus_packet(&mut self, addr: SocketAddr, data: &[u8]) {
        let data: Vec<u8> = bincode::deserialize(data).unwrap();
        let p = self.peers.iter().find(|p| p.udp_addr.unwrap() == addr).unwrap();


        self.audio.decode_and_queue_packet(&data[..], p.public_key.clone());
    }
}