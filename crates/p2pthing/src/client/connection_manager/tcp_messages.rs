use std::net::SocketAddr;

use mio::Token;
use p2pthing_common::{encryption::SymmetricEncryption, message_type::{InterthreadMessage, MsgType, Peer, msg_types::{self, AnnounceRequest, AnnounceSecret, Call, CallResponse, Disconnect}}, read_exact, ui::UIConn};

use super::{ConnectionManager, UdpConnection, UdpConnectionState};

impl ConnectionManager {
    pub fn read_tcp_message(&mut self, msg_type: u8, _: Token) {
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
        let conn = self.udp_connections.iter()
        .find(|x| x.address == addr).unwrap();

        self.rendezvous_public_key = Some(announcement.public_key);
        let announce_secret = msg_types::AnnounceSecret {
            secret: conn.symmetric_key.as_ref().unwrap().secret.clone()
        };
        self.send_tcp_message_public_key(MsgType::AnnounceSecret, &announce_secret).unwrap();
        
        let announce_public = msg_types::AnnouncePublic {
            public_key: self.encryption.get_public_key().clone()
        };
        self.send_tcp_message(MsgType::Announce, &announce_public).unwrap();
    }

    fn on_tcp_announce(&mut self, _: SocketAddr, peers: Vec<Peer>) {
        for new_p in peers {
            if !self.peers.iter().any(|p| p.public_key == new_p.public_key) {
                self.peers.push(new_p);
            }
        }
        self.ui_s.send(InterthreadMessage::AnnounceResponse(self.peers.clone())).unwrap();
    }

    /// Handle incoming call
    fn on_call(&mut self, _: SocketAddr, call: Call) {
        let caller = call.caller.unwrap();
        let udp_address = call.udp_address.unwrap();

        let mut conn = UdpConnection::new(UdpConnectionState::Pending, udp_address, self.udp_socket.clone(), None, self.encryption.clone());
        conn.associated_peer = Some(caller.clone());
        self.udp_connections.push(conn);

        // Notify the UI of the incoming call
        self.ui_s.send(InterthreadMessage::Call(caller)).unwrap();
    }

    /// Handle the response to a sent call
    fn on_call_response(&mut self, _: SocketAddr, call_response: CallResponse) {
        let call = call_response.call;
        if !call_response.response {
            let i = self.calls_in_progress.iter()
            .position(|(c, _)| c.callee == call.callee)
            .unwrap();
            self.calls_in_progress.remove(i);
            self.ui_s.send(InterthreadMessage::CallDenied(call.callee)).unwrap();
        }
        else {
            let udp_address = call.udp_address.unwrap();
        
            let p = self.peers.iter_mut().find(|p| p.public_key == call.callee).unwrap();
            p.udp_addr = Some(udp_address);
            
            let i = self.calls_in_progress.iter()
            .position(|(c, _)| c.callee == call.callee)
            .unwrap();
            self.calls_in_progress.remove(i);
    
            let sym_key = SymmetricEncryption::new();
            let mut conn = UdpConnection::new(UdpConnectionState::MidCall, udp_address, self.udp_socket.clone(), Some(sym_key), self.encryption.clone());
            conn.associated_peer = Some(call.callee.clone());
            self.ui_s.log_info(
            &format!("A sent call has been accepted by peer ({};{}), starting the punch through protocol", call.callee, conn.address));

            conn.send_udp_message_with_public_key(MsgType::AnnounceSecret, &AnnounceSecret{secret: conn.symmetric_key.as_ref().unwrap().secret.clone()}, true, None).unwrap();

            self.ui_s.send(InterthreadMessage::CallAccepted(p.public_key.clone())).unwrap();
            self.udp_connections.push(conn);
        }
    }

    fn on_disconnect(&mut self, _: SocketAddr, disconnect_peer: Disconnect) {
        let p = self.peers.iter_mut().find(|p| p.public_key == disconnect_peer.public_key).unwrap();
        self.ui_s.log_info(&format!("Peer ({}) disconnected", p.public_key));
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