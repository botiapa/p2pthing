use std::io::{self, Read};

use mio::{Events, Interest, Token};
use p2pthing_common::message_type::{msg_types::AnnounceRequest, MsgType};

use super::RendezvousServer;

const TCP_LISTENER: Token = Token(0);
const UDP_LISTENER: Token = Token(1);

impl RendezvousServer {
    pub fn event_loop(&mut self) {
        loop {
            let mut events = Events::with_capacity(1024);
            if let Err(err) = self.poll.poll(&mut events, None) {
                println!("Error while polling: {:?}", err);
                continue;
            }
            for event in events.iter() {
                match event.token() {
                    TCP_LISTENER => {
                        self.accept_tcp_connections();
                    }
                    UDP_LISTENER => {
                        self.read_udp_events();
                    }
                    token => {
                        self.read_tcp_events(token);
                    }
                }
            }
        }
    }

    fn accept_tcp_connections(&mut self) {
        loop {
            match self.tcp_listener.accept() {
                Ok((mut sock, addr)) => {
                    println!("Peer ({}) connected", sock.peer_addr().unwrap());
                    let token = Token(self.next_token);
                    self.next_token += 1;

                    self.poll.registry().register(&mut sock, token, Interest::READABLE).unwrap();

                    let announce_request = AnnounceRequest { public_key: self.encryption.get_public_key() };
                    RendezvousServer::send_tcp_message(&mut sock, MsgType::AnnounceRequest, &announce_request);

                    self.tcp_connections.insert(token, sock);
                    self.addresses.insert(addr, token);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                e => panic!("err={:?}", e), // Unexpected error
            }
        }
    }

    fn read_udp_events(&mut self) {
        loop {
            let mut buf = [0; 65536];
            match self.udp_listener.recv_from(&mut buf) {
                Ok((size, addr)) => {
                    self.read_udp_message(size, addr, &buf);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(_) => {}
            }
        }
    }

    fn read_tcp_events(&mut self, token: Token) {
        let mut msg_size = [0u8; 8];
        loop {
            let sock = self.tcp_connections.get_mut(&token).unwrap();
            let addr = sock.peer_addr().unwrap(); // TODO: Dont't rely on address, since it can be null if disconnected while handling event. Relyon token instead
            match sock.read(&mut msg_size) {
                Ok(0) => {
                    self.on_disconnect(addr, token);
                    break;
                }
                Ok(_) => {
                    self.read_tcp_message(&msg_size, token);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Socket is not ready anymore, stop reading
                    break;
                }
                e => {
                    println!("Peer disconnected with an error={:?}", e);
                    self.on_disconnect(addr, token);
                    break;
                }
            }
        }
    }
}
