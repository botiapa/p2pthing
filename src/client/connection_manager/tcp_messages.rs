use std::net::SocketAddr;

use msg_types::{AnnounceRequest, CallResponse, Disconnect};
use mio::Token;

use crate::{client::tui::Tui, common::{debug_message::DebugMessageType, lib::read_exact, message_type::{InterthreadMessage, MsgType, msg_types::{self, Call}, Peer}}};

use super::{ConnectionManager, KEEP_ALIVE_DELAY_MIDCALL, UdpConnection, UdpConnectionState};

impl ConnectionManager {
    pub fn read_tcp_message(&mut self, msg_type: u8, token: Token) {
        let sock = &mut self.rendezvous_socket;
        let addr = sock.peer_addr().unwrap();

        let msg_type = num::FromPrimitive::from_u8(msg_type);

        let mut msg_size = [0u8; 8];
        read_exact(sock, &mut msg_size);
        let msg_size: u64 = bincode::deserialize(&msg_size).unwrap();
        let mut msg = vec![0;msg_size as usize];
        read_exact(sock, &mut msg[..]);

        match msg_type {
            Some(MsgType::AnnounceRequest) => {
                let announcement: AnnounceRequest = bincode::deserialize(&msg).unwrap();
                self.on_announce_request(addr, announcement);
            }
            Some(MsgType::Announce) => {
                let peers: Vec<Peer> = bincode::deserialize(&msg).unwrap();
                self.on_tcp_announce(addr, peers);
            }
            Some(MsgType::Call) => {
                let call: msg_types::Call = bincode::deserialize(&mut msg[..]).unwrap();
                self.on_call(addr, call);
            }
            Some(MsgType::CallResponse) => {
                let call_response: msg_types::CallResponse = bincode::deserialize(&mut msg[..]).unwrap();
                self.on_call_response(addr, call_response);
            }
            Some(MsgType::Disconnect) => {
                let disconnect_peer: msg_types::Disconnect = bincode::deserialize(&mut msg[..]).unwrap();
                self.on_disconnect(addr, disconnect_peer);
            }
            _ => unreachable!()
        }
        
    }

    fn on_announce_request(&mut self, addr: SocketAddr, announcement: AnnounceRequest) {
        self.rendezvous_public_key = Some(announcement.public_key);
        let announce_secret = msg_types::AnnounceSecret {
            secret: self.rendezvous_syn_key.secret.clone()
        };
        self.send_tcp_message_public_key(MsgType::AnnounceSecret, &announce_secret).unwrap();
        
        let announce_public = msg_types::AnnouncePublic {
            public_key: self.encryption.get_public_key().clone()
        };
        self.send_tcp_message(MsgType::Announce, &announce_public).unwrap();
    }

    fn on_tcp_announce(&mut self, addr: SocketAddr, peers: Vec<Peer>) {
        for new_p in peers {
            if !self.peers.iter().any(|p| p.public_key == new_p.public_key) {
                self.peers.push(new_p);
            }
        }
        self.ui_s.send(InterthreadMessage::AnnounceResponse(self.peers.clone())).unwrap();
    }

    /// Handle incoming call
    fn on_call(&mut self, addr: SocketAddr, call: Call) {
        let caller = call.clone().caller.unwrap();
        let udp_address = caller.udp_addr.unwrap();

        // FIXME: add the ability to decline the call

        let msg = msg_types::CallResponse {
            call: call.clone(),
            response: true
        };
        
        // Notify the UI of the incoming call
        self.ui_s.send(InterthreadMessage::Call(caller.public_key.clone())).unwrap();

        self.send_tcp_message(MsgType::CallResponse, &msg).unwrap();

        let p = self.peers.iter_mut().find(|p| p.public_key == caller.public_key).unwrap();
        p.udp_addr = Some(udp_address);

        let conn = UdpConnection {
            associated_peer: Some(p.public_key.clone()),
            address: udp_address,
            last_keep_alive: None,
            last_announce: None,
            state: UdpConnectionState::MidCall,
        };
        Tui::debug_message(
            &format!("Accepted call from peer ({};{}), starting the punch through protocol", 
                caller.public_key, conn.address), 
        DebugMessageType::Log, &self.ui_s);

        self.udp_connections.push(conn);
        self.waker_thread.send(InterthreadMessage::SetWakepDelay(KEEP_ALIVE_DELAY_MIDCALL)).unwrap();
    }

    /// Handle the response to a sent call
    fn on_call_response(&mut self, addr: SocketAddr, call_response: CallResponse) {
        let call = call_response.call;
        if !call_response.response {
            let i = self.calls_in_progress.iter()
            .position(|(c, _)| c.callee == call.callee)
            .unwrap();
            self.calls_in_progress.remove(i);
            self.ui_s.send(InterthreadMessage::CallDenied(call.callee.public_key)).unwrap();
        }
        else {
            let udp_address = call.callee.udp_addr.unwrap();
        
            let p = self.peers.iter_mut().find(|p| p.public_key == call.callee.public_key).unwrap();
            p.udp_addr = Some(udp_address);
            
            let i = self.calls_in_progress.iter()
            .position(|(c, _)| c.callee == call.callee)
            .unwrap();
            self.calls_in_progress.remove(i);
    
            let conn = UdpConnection {
                associated_peer: Some(p.public_key.clone()),
                address: udp_address,
                last_keep_alive: None,
                last_announce: None,
                state: UdpConnectionState::MidCall,
            };
            Tui::debug_message(
                &format!("A sent call has been accepted by peer ({};{}), starting the punch through protocol", 
                call.callee.public_key, conn.address),
            DebugMessageType::Log, &self.ui_s);
            self.ui_s.send(InterthreadMessage::CallAccepted(p.public_key.clone())).unwrap();
            self.udp_connections.push(conn);
            self.waker_thread.send(InterthreadMessage::SetWakepDelay(KEEP_ALIVE_DELAY_MIDCALL)).unwrap();
        }
    }

    fn on_disconnect(&mut self, addr: SocketAddr, disconnect_peer: Disconnect) {
        let p = self.peers.iter_mut().find(|p| p.public_key == disconnect_peer.public_key).unwrap();
        Tui::debug_message(&format!("Peer ({}) disconnected", p.public_key),DebugMessageType::Log, &self.ui_s);
        match p.udp_addr {
            Some(addr) => {
                self.udp_connections.iter_mut()
                .position(|conn| conn.address == addr)
                .map(|i| self.udp_connections.remove(i)).unwrap();
            }
            None => {}
        }
        self.peers.iter_mut()
        .position(|p| p.public_key == disconnect_peer.public_key)
        .map(|i| self.peers.remove(i));
        self.ui_s.send(InterthreadMessage::PeerDisconnected(disconnect_peer.public_key)).unwrap();
    }
}