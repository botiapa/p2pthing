use std::net::SocketAddr;

use mio::Token;
use p2pthing_common::{encryption::SymmetricEncryption, message_type::{MsgType, Peer, msg_types::{self, AnnouncePublic, AnnounceSecret, Call, CallResponse}, PeerSource}, read_exact, num};

use super::{CallRequest, RendezvousServer};

impl RendezvousServer {
    pub fn read_tcp_message(&mut self, msg_size: &[u8], token: Token) {
        let sock = self.tcp_connections.get_mut(&token).unwrap();
        let addr = sock.peer_addr().unwrap();

        let msg_size: u64 = bincode::deserialize(msg_size).unwrap();
        let mut encrypted = vec![0; msg_size as usize];
        read_exact(sock, &mut encrypted[..]);

        let mut msg = match self.sym_keys.get(&addr) {
            Some(sym_key) => sym_key.decrypt(&mut encrypted[..]), // Peer has already announce, use the symmetric key
            None => {
                match self.peers.iter().find(|p| p.addr.unwrap() == addr) {
                    Some(p) => p.sym_key.as_ref().unwrap().decrypt(&mut encrypted[..]),
                    None => self.encryption.decrypt(&mut encrypted[..]) // Peer hasn't announced yet, use the asymmetric key
                }
            }
        }; 
        let msg_type = num::FromPrimitive::from_u8(msg[0]);

        match msg_type {
            Some(MsgType::AnnounceSecret) => {
                let announcement: msg_types::AnnounceSecret = bincode::deserialize(&mut msg[1..]).unwrap();
                self.on_secret_announce(addr, announcement);
            }
            Some(MsgType::Announce) => {
                let announcement: msg_types::AnnouncePublic = bincode::deserialize(&mut msg[1..]).unwrap();
                self.on_announce(addr, announcement);
            }
            Some(MsgType::Call) => {
                let mut call: msg_types::Call = bincode::deserialize(&mut msg[1..]).unwrap();
                self.on_call(addr, &mut call);
            }
            Some(MsgType::CallResponse) => {
                let call_response: msg_types::CallResponse = bincode::deserialize(&mut msg[1..]).unwrap();
                self.on_call_response(addr, call_response);
            }
            _ => unreachable!()
        }
        
    }

    /// After receiving the secret, wait for the public key to arrive
    fn on_secret_announce(&mut self, addr: SocketAddr, announcement: AnnounceSecret) {
        let secret = SymmetricEncryption::new_from_secret(&announcement.secret[..]);

        self.sym_keys.insert(addr, secret);
    }

    fn on_announce(&mut self, addr: SocketAddr, announcement: AnnouncePublic) {
        let p = Peer {
            addr: Some(addr),
            udp_addr: None,
            public_key: announcement.public_key,
            source: PeerSource::Rendezvous.into(),
            sym_key: Some(self.sym_keys.remove(&addr).unwrap())
        };
        println!("Received public key for peer ({}): {}", p.addr.unwrap(), p.public_key);

        // Notify the new client of the connections
        let sock = self.tcp_connections.iter_mut().find(|(_, c)| c.peer_addr().unwrap() == addr).unwrap().1;
        RendezvousServer::send_tcp_message(sock, MsgType::Announce, &self.peers.to_vec().iter_mut().map(|x| x.safe_clone()).collect::<Vec<_>>());
        
        // Notify everyone else of the new connection
        for c in self.tcp_connections.values_mut().filter(|c| c.peer_addr().unwrap() != addr) {
            RendezvousServer::send_tcp_message(c, MsgType::Announce, &[p.safe_clone()].to_vec());
        }
        self.peers.push(p); 
    }

    fn on_call(&mut self, addr: SocketAddr, call: &mut Call) {
        if let Some(caller) = self.peers.iter().find(|x| x.addr.unwrap() == addr) {
            if let Some(callee) = self.peers.iter().find(|x| x.public_key == call.callee) {
                if caller.udp_addr.is_none() || callee.udp_addr.is_none() {
                    let caller_token = self.addresses.get(&caller.addr.unwrap()).unwrap();
                    let mut caller_socket = self.tcp_connections.get_mut(caller_token).unwrap();
                    RendezvousServer::send_tcp_message(&mut caller_socket, MsgType::CallResponse, &CallResponse{ 
                        call: call.clone(), 
                        response: false
                    });
                    println!("Error routing a call from ({}; {}) to ({}; {}) udp address hasn't been found", addr, caller.public_key, callee.addr.unwrap(), callee.public_key);
                }
                else {
                    let req = CallRequest{
                        caller: caller.clone(),
                        callee: callee.clone(),
                    };
                    self.calls.push(req);
                    // Don't trust the client
                    call.caller = Some(caller.clone().public_key);
                    let token = self.addresses.get(&callee.addr.unwrap()).unwrap();
                    let mut callee_socket = self.tcp_connections.get_mut(token).unwrap();
                    call.udp_address = caller.udp_addr;
                    RendezvousServer::send_tcp_message(&mut callee_socket, MsgType::Call, &call);
                    println!("Routed a call from ({}; {}) to ({}; {})", addr, caller.public_key, callee_socket.peer_addr().unwrap(), callee.public_key);
                }
            }
            else {
                println!("Callee haven't announced itself yet");
            }
        }
        else {
            println!("Caller haven't announced itself yet. ({})", addr);
        }
    }

    fn on_call_response(&mut self, _: SocketAddr, call_response: CallResponse) {
        let callee = call_response.call.callee;
        let caller = call_response.call.caller.unwrap();
        match self.calls.iter().position(|x| x.callee.public_key == callee && x.caller.public_key == caller) {
            Some(index) => {
                if call_response.response {
                    println!("Peer ({}) accepted the call request from ({})", callee, caller);
                    
                    let caller_peer = self.peers.iter().find(|p| p.public_key == caller).unwrap();
                    let callee_peer = self.peers.iter().find(|p| p.public_key == callee).unwrap().clone();
                    let mut sock = self.tcp_connections.values_mut().find(|x| x.peer_addr().unwrap() == caller_peer.addr.unwrap()).unwrap();
                    
                    let msg = msg_types::CallResponse {
                        call: Call {
                            callee,
                            caller: Some(caller),
                            udp_address: Some(callee_peer.udp_addr.unwrap())
                        },
                        response: call_response.response,
                    };
                    RendezvousServer::send_tcp_message(&mut sock, MsgType::CallResponse, &msg);
                }
                else {
                    println!("Peer ({}) denied the call request from ({})", callee, caller);
                }
                self.calls.remove(index);
            }
            None => {
                println!("Peer ({}) accepted call that wasn't in the database", callee);
            }
        }
    }
}