use std::net::SocketAddr;

use p2pthing_common::{encryption::SymmetricEncryption, message_type::{InterthreadMessage, MsgType, UdpPacket, msg_types::{self, AnnounceSecret, ChatMessage, AnnouncePublic}}, ui::UIConn};
use p2pthing_tui::tui::Tui;

use crate::client::udp_connection::UdpConnectionState;

use super::{ConnectionManager, MULTICAST_MAGIC};

impl ConnectionManager {
    pub fn read_udp_message(&mut self, _: usize, addr: SocketAddr, buf: &[u8]) {
        let conn = match self.udp_connections.iter_mut().find(|x| x.address == addr) {
            Some(c) => c,
            None => {
                self.ui_s.log_warning(&format!("Tried reading from ({}), but couldn't find the associated connection", addr));
                return;
            }
        };

        //TODO: Move all this logic to udp_connection.rs

        let udp_packet: UdpPacket = bincode::deserialize(&buf).unwrap();
        conn.statistics.received_bytes(bincode::serialized_size(&udp_packet).unwrap());
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
            Some(MsgType::FileChunks) => {
                self.on_file_chunks(addr, &buf[1..]);
            }
            _ => unreachable!()
        }
    }

    pub fn read_multicast_message(&mut self, _: usize, addr: SocketAddr, buf: &[u8]) {
        //TODO: Move all this logic to udp_connection.rs

        // First u32 is the magic packet
        let magic = &buf[0..4];
        let magic: u32 = bincode::deserialize(magic).unwrap();
        if magic == MULTICAST_MAGIC {
            let announce: AnnouncePublic = bincode::deserialize(&buf[4..]).unwrap();
            if announce.public_key != self.encryption.get_public_key() {
                self.ui_s.log_info(&format!("Received correct multicast announce message: {:?}", announce.public_key))
            }
            
        }
    }

    fn check_punchthrough(&mut self, addr: SocketAddr) {
        let conn = self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap();
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

    fn on_secret_announce(&mut self, addr: SocketAddr, data: &[u8]) {
        let secret: AnnounceSecret = bincode::deserialize(data).unwrap();
        let secret = &secret.secret[..];

        let conn = self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap();
        
        conn.symmetric_key = Some(SymmetricEncryption::new_from_secret(secret));
        conn.upgraded = true;

        self.ui_s.log_info(&format!("Received secret for peer: ({})", conn.associated_peer.as_ref().unwrap()));
        self.check_punchthrough(addr);
    }

    fn on_confirmation_message(&mut self, addr: SocketAddr, data: &[u8]) {
        let id: u32 = bincode::deserialize(data).unwrap();
        let conn = self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap();

        let removed = conn.sent_messages.iter_mut().position(|msg| msg.packet.msg_id == id).map(|i| conn.sent_messages.remove(i));
        match removed {
            Some(packet) => {
                conn.statistics.new_ping(packet.sent.elapsed());
                match packet.msg_type {
                    MsgType::AnnounceSecret => {
                        conn.upgraded = true;
                        self.ui_s.log_info(&format!("Peer received secret: ({})", conn.associated_peer.as_ref().unwrap()));
                        self.check_punchthrough(addr);
                    }
                    MsgType::ChatMessage => {
                        let id = self.msg_confirmations.remove(&packet.custom_id.unwrap()).unwrap();
                        self.ui_s.log_info(&format!("Chat message confirmed by: ({})", conn.associated_peer.as_ref().unwrap()));
                        self.ui_s.send(InterthreadMessage::OnChatMessageReceived(id)).unwrap();
                    }
                    MsgType::RequestFileChunks => {}
                    _ => unreachable!()
                }
            }
            None => self.ui_s.log_warning(&format!("Couldn't find message with confirmation id: ({})", id))
        }
    }

    fn on_udp_announce(&mut self, addr: SocketAddr) {
        self.udp_connections.iter_mut()
        .find(|x| x.address == addr).unwrap()
        .state = UdpConnectionState::Connected;
        self.ui_s.log_info("UDP Announcement has been accepted");
    }

    fn on_keep_alive(&mut self, addr: SocketAddr) {
        self.ui_s.log_info(&format!("Keep alive message received from {}", addr));
        self.check_punchthrough(addr);
    }

    fn on_chat_message(&mut self, addr: SocketAddr, data: &[u8]) {
        let msg: msg_types::ChatMessage = bincode::deserialize(data).unwrap();
        let p = self.peers.iter().find(|p| p.udp_addr.unwrap() == addr).unwrap();
        self.ui_s.send(InterthreadMessage::OnChatMessage(msg.clone())).unwrap();
        
        //TODO: Ability to accept or deny file download
        if let Some(files) = msg.attachments {
            for file in files {
                if let Err(e) = self.file_manager.start_receiving_file(file.clone(), p.public_key.clone()) {
                    self.ui_s.log_error(&format!("Failed preparing to receive file: {}", e));
                }
            }
        }
    }

    #[cfg(feature = "audio")]
    fn on_opus_packet(&mut self, addr: SocketAddr, data: &[u8]) {
        let data: Vec<u8> = bincode::deserialize(data).unwrap();
        let p = self.peers.iter().find(|p| p.udp_addr.unwrap() == addr).unwrap();

        self.audio.decode_and_queue_packet(&data[..], p.public_key.clone());
    }

    fn on_request_file_chunks(&mut self, addr: SocketAddr, data: &[u8]) {
        let data: msg_types::RequestFileChunks = bincode::deserialize(data).unwrap();
        let p = self.peers.iter().find(|p| p.udp_addr.unwrap() == addr).unwrap();
        let public_key = p.public_key.clone();

        //TODO: Ability to accept or deny file download
        match self.file_manager.get_file_chunks(data) {
            Ok(chunks) => {
                if let Err(e) = self.send_udp_message(Some(public_key), MsgType::FileChunks, &msg_types::FileChunks {chunks,}, false, false) {
                    self.ui_s.log_error(&format!("Error while trying to send a file chunk request: {}", e.to_string()));
                }
            },
            Err(e) => self.ui_s.log_error(&format!("Failed reading file chunks: {}", &e)),
        }
    }

    fn on_file_chunks(&mut self, addr: SocketAddr, data: &[u8]) {
        let data: msg_types::FileChunks = bincode::deserialize(data).unwrap();

        //TODO: Ability to accept or deny file download
        if let Err(e) = self.file_manager.store_file_chunks(data) {
            self.ui_s.log_error(&format!("Error while trying to save a file chunk: {}", e)); 
        }
    }
    
}