use std::{io::{self, Read}, net::Shutdown, sync::mpsc::{self, Receiver}, thread, time::{Duration, Instant}};

use mio::{Events, Interest, net::TcpStream};

use crate::{client::tui::Tui, common::{debug_message::DebugMessageType, message_type::{InterthreadMessage, MsgType, Peer, msg_types}}};

use super::{ANNOUNCE_DELAY, CALL_DECAY, ConnectionManager, KEEP_ALIVE_DELAY, KEEP_ALIVE_DELAY_MIDCALL, UdpConnectionState, WAKER, RENDEZVOUS, UDP_SOCKET, RECONNECT_DELAY};

impl ConnectionManager {
    pub fn event_loop(&mut self, r: Receiver<InterthreadMessage>) {
        loop {
            let mut events = Events::with_capacity(1024);
            self.poll.poll(&mut events, None).unwrap();

            // Remove old call requests
            self.calls_in_progress.retain(|(_, time)| {
                return time.elapsed().as_secs() < CALL_DECAY;
            });

            // Send keep alive messages
            for conn in &mut self.udp_connections {
                match conn.state {
                    UdpConnectionState::MidCall | UdpConnectionState::Connected => {
                        let delay = match conn.state { 
                            UdpConnectionState::MidCall => KEEP_ALIVE_DELAY_MIDCALL,
                            UdpConnectionState::Connected => KEEP_ALIVE_DELAY,
                            _ => unreachable!()
                        };
                        match conn.last_keep_alive {
                            Some(time) if time.elapsed().as_secs() < delay => {}
                            None | _ => {
                                conn.last_keep_alive = Some(Instant::now());
                                let addr = conn.address.clone();
                                ConnectionManager::send_udp_message_to(&self.udp_socket, addr, MsgType::KeepAlive, &()).unwrap(); //TODO: Error handling
                                match conn.associated_peer.clone() {
                                    Some(public_key) => Tui::debug_message(&format!("Sent keep alive message to ({})", public_key), DebugMessageType::Log, &self.ui_s),
                                    None => Tui::debug_message("Sent keep alive message to the rendezvous server", DebugMessageType::Log, &self.ui_s)
                                }
                                
                            }
                        }
                    }
                    UdpConnectionState::Unannounced => {
                        match conn.last_announce {
                            Some(time) if time.elapsed().as_secs() < ANNOUNCE_DELAY => {}
                            None | _ => {
                                let announce = msg_types::AnnouncePublic {
                                    public_key: self.encryption.get_public_key()
                                };
                                let announce = ConnectionManager::send_udp_message_to(&self.udp_socket, self.rendezvous_ip, MsgType::Announce, &announce);
                                match announce {
                                    Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                                        Tui::debug_message("Couldn't send udp announce to the server, not connected ", DebugMessageType::Error, &self.ui_s);
                                    }
                                    _ => {}
                                }
                                conn.last_announce = Some(Instant::now());
                            }
                        }
                    }
                };
                
            }
            
            loop {
                match r.try_recv() {
                    Ok(data) => {
                        match data {
                            InterthreadMessage::SendChatMessage(p, msg) => self.send_udp_message(Some(p), MsgType::ChatMessage, &msg_types::ChatMessage {msg,}).unwrap(),
                            InterthreadMessage::OnChatMessage(p, msg) => Tui::on_chat_message(&self.ui_s, p, msg),
                            InterthreadMessage::ConnectToServer() => {
                                self.rendezvous_socket = TcpStream::connect(self.rendezvous_ip).unwrap();
                                self.poll.registry().register(&mut self.rendezvous_socket, RENDEZVOUS, Interest::READABLE).unwrap();
                                Tui::debug_message("Trying to connect to server", DebugMessageType::Log, &self.ui_s);
                            }
                            InterthreadMessage::Call(p) => {
                                let call = msg_types::Call {
                                    callee: Peer{ addr: None, udp_addr: None, public_key: p.clone(), sym_key: None},
                                    caller: None
                                };
                                match self.calls_in_progress.iter().find(|(c, _)| c == &call) {
                                    Some(c) => Tui::debug_message(&format!("Tried to call a peer which has already been called: {}", p), DebugMessageType::Warning, &self.ui_s),
                                    None => {
                                        Tui::debug_message(&format!("Calling peer: {}", p), DebugMessageType::Log, &self.ui_s);
                                
                                        self.calls_in_progress.push((call.clone(), Instant::now()));
                                        self.send_tcp_message(MsgType::Call, &call).unwrap(); //TODO: Error handling
                                    }
                                }
                            }
                            InterthreadMessage::Quit() => {
                                match self.rendezvous_socket.shutdown(Shutdown::Both) {
                                    _ => {}
                                }
                                self.waker_thread.send(InterthreadMessage::Quit()).unwrap();
                                return;
                            },
                            _ => unreachable!()
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => break,
                    Err(mpsc::TryRecvError::Empty) => break
                }
            }

            for event in events.iter() {
                match event.token() {
                    token => {
                        loop {
                            match token {
                                WAKER => break,
                                RENDEZVOUS => {
                                    let mut msg_type = [0;1];
                                    match self.rendezvous_socket.read(&mut msg_type) {
                                        Ok(0) => {
                                            Tui::debug_message("Disconnected from rendezvous server", DebugMessageType::Warning, &self.ui_s);
                                            //TODO: Should I try to reconnect?
                                            break;
                                        }
                                        Ok(_) => {
                                            self.read_tcp_message(msg_type[0], token);
                                        }
                                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                            // Socket is not ready anymore, stop reading
                                            break;
                                        }
                                        Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                                            Tui::debug_message(&format!("Couldn't connect to the server, retrying in {}", RECONNECT_DELAY), DebugMessageType::Error, &self.ui_s);
                                            self.try_server_reconnect();
                                            break;
                                        }
                                        e => panic!("err={:?}", e), // Unexpected error
                                    }
                                },
                                UDP_SOCKET => {
                                    let mut buf = [0; 65536];
                                    match self.udp_socket.recv_from(&mut buf) {
                                        Ok(r) => {
                                            let (read, addr) = r;
                                            self.read_udp_message(read, addr, &buf);
                                        }
                                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                            // Socket is not ready anymore, stop reading
                                            break;
                                        }
                                        Err(ref e) if e.kind() == io::ErrorKind::ConnectionReset => {
                                            Tui::debug_message("Couldn't read from udp socket (ConnectionReset) ", DebugMessageType::Error, &self.ui_s);
                                        }
                                        e => println!("err={:?}", e), // Unexpected error
                                    }
                                },
                                _ => unreachable!()
                            }
                        }
                    }
                }
            }
        }
    }

    fn try_server_reconnect(&mut self) {
        let cm_s = self.cm_s.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(RECONNECT_DELAY));
            cm_s.send(InterthreadMessage::ConnectToServer()).unwrap();
        });
    }
}