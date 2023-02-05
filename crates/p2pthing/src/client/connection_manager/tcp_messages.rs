use std::net::SocketAddr;

use mio::Token;
use p2pthing_common::{encryption::{SymmetricEncryption, NetworkedPublicKey}, message_type::{InterthreadMessage, MsgType, msg_types::{self, AnnounceRequest, AnnounceSecret, Call, CallResponse, Disconnect}}, read_exact, ui::UIConn, num};

use crate::client::peer::{Peer, PeerType, PeerSource};

use super::{ConnectionManager, UdpConnection, UdpConnectionState};

impl ConnectionManager {
    pub fn read_tcp_message(&mut self, msg_type: u8, _: Token) {
        let sock = self.peers.rendezvous_servers_mut().next().expect("Rendezvous server not found").tcp_conn.as_mut().expect("Rendezvous server does not have TCP connection associated with it");
        
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
                let peers: Vec<NetworkedPublicKey> = bincode::deserialize(&msg).unwrap();
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

    /// The rendezvous server has asked us to announce ourselves
    fn on_announce_request(&mut self, addr: SocketAddr, announcement: AnnounceRequest) {
        let conn = self.peers.conn(&addr).unwrap();

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

    fn on_tcp_announce(&mut self, _: SocketAddr, peers: Vec<NetworkedPublicKey>) {
        for new_p in peers {
            // If the peer is not already in the peerlist
            if self.peers.peer(&new_p).is_none() {
                self.peers.push(Peer {
                    addr: None,
                    tcp_conn: None,
                    udp_conn: None,
                    sym_key: None,
                    public_key: Some(new_p),
                    source: PeerSource::Rendezvous.into(),
                    peer_type: PeerType::ClientPeer,
                });
            }
        }
        self.ui_s.send(InterthreadMessage::AnnounceResponse(self.peers.inner.iter().filter(|p| p.peer_type == PeerType::ClientPeer).map(|p| p.public_key.as_ref().unwrap().clone()).collect::<Vec<NetworkedPublicKey>>().clone())).unwrap();
    }

    /// Handle incoming call
    fn on_call(&mut self, _: SocketAddr, call: Call) {
        let caller = call.caller.unwrap();
        let udp_address = call.udp_address.unwrap();

        let mut conn = UdpConnection::new(UdpConnectionState::Pending, udp_address, self.udp_socket.clone(), None, self.encryption.clone());
        conn.associated_peer = Some(caller.clone());

        let p = self.peers.peer_mut(&caller.clone()).unwrap();
        p.udp_conn = Some(conn);

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
        
            let p = self.peers.peer_mut(&call.callee).unwrap();
            
            if let Some(i) = self.calls_in_progress.iter().position(|(c, _)| c.callee == call.callee) {
                self.calls_in_progress.remove(i);
    
                let sym_key = SymmetricEncryption::new();
                let mut conn = UdpConnection::new(UdpConnectionState::MidCall, udp_address, self.udp_socket.clone(), Some(sym_key), self.encryption.clone());
                conn.associated_peer = Some(call.callee.clone());
                self.ui_s.log_info(
                &format!("A sent call has been accepted by peer ({};{}), starting the punch through protocol", call.callee, conn.address));
    
                conn.send_udp_message_with_public_key(MsgType::AnnounceSecret, &AnnounceSecret{secret: conn.symmetric_key.as_ref().unwrap().secret.clone()}, true, None).unwrap();
    
                p.udp_conn = Some(conn);
                self.ui_s.send(InterthreadMessage::CallAccepted(p.public_key.as_ref().unwrap().clone())).unwrap();
            }
            else {
                self.ui_s.log_warning(
                    &format!("An invalid call has been accepted by address ({}), discarding", udp_address));
            }
        }
    }

    fn on_disconnect(&mut self, _: SocketAddr, disconnect_peer: Disconnect) {
        let p = self.peers.peer_mut(&disconnect_peer.public_key).unwrap();
        self.ui_s.log_info(&format!("Peer ({}) disconnected", p.public_key.as_ref().unwrap()));
        self.peers.remove(&disconnect_peer.public_key);
        self.ui_s.send(InterthreadMessage::PeerDisconnected(disconnect_peer.public_key)).unwrap();
    }
}