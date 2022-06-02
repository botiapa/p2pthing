use std::{io::{self, Read}, net::{Shutdown, UdpSocket, Ipv4Addr, SocketAddr}, sync::mpsc::{self, Receiver}, thread, time::{Duration, Instant}};

use base64::encode_config;
use chrono::Utc;
use io::ErrorKind;
use mio::{Events, Interest, net::TcpStream};
use p2pthing_common::{message_type::{InterthreadMessage, MsgType, msg_types::{self, AnnouncePublic}, UdpPacket}, ui::UIConn};
use p2pthing_tui::tui::Tui;
use sha2::{Digest, Sha256};
use socket2::{Socket, Type, Protocol, SockAddr, Domain};

use crate::client::{file_manager::FileManager, udp_connection::UdpConnectionState};

use super::{ANNOUNCE_DELAY, CALL_DECAY, ConnectionManager, KEEP_ALIVE_DELAY, KEEP_ALIVE_DELAY_MIDCALL, RECONNECT_DELAY, RENDEZVOUS, STATS_UPDATE_DELAY, UDP_SOCKET, WAKER, BROADCAST_DELAY, MULTICAST_SOCKET, MULTICAST_ADDRESS, MULTICAST_MAGIC};

impl ConnectionManager {
    pub fn event_loop(&mut self, r: &mut Receiver<InterthreadMessage>) {
        let mut running = true;
        while running {
            let mut events = Events::with_capacity(1024);

            #[cfg(feature = "audio")]
            if !self.audio.started && self.udp_connections.iter().any(|c| c.upgraded && c.associated_peer.is_some()) {
                self.audio.init();
            }

            // Calculate the next timeout
            let mut durations = vec![];
            self.get_next_timeouts(&mut durations);

            self.poll.poll(&mut events, durations.first().cloned()).unwrap();

            // Remove old call requests
            self.calls_in_progress.retain(|(_, time)| {
                return time.elapsed() < CALL_DECAY;
            });

            // Send keep alive messages
            self.send_keep_alive_messages();

            // Send broadcast messages
            self.send_multicast_messages();

            // Send reliable messages
            self.send_reliable_messages();

            // Handle interthread messages 
            self.handle_interthread_messages(r, &mut running);

            // Handle IO events
            self.handle_io_events(&events);

            // Check for new requestable chunks
            self.check_new_chunks();
        
            // Send UI updates
            self.send_ui_updates();
        }
    }

    fn send_keep_alive_messages(&mut self) {
        for conn in &mut self.udp_connections {
            match conn.state {
                UdpConnectionState::MidCall | UdpConnectionState::Connected => {
                    let delay = match conn.state { 
                        UdpConnectionState::MidCall => KEEP_ALIVE_DELAY_MIDCALL,
                        UdpConnectionState::Connected => KEEP_ALIVE_DELAY,
                        _ => unreachable!()
                    };
                    match conn.last_message_sent {
                        Some(time) if time.elapsed() < delay => {}
                        None | _ => {
                            match conn.associated_peer.clone() {
                                Some(public_key) => {
                                    conn.send_raw_message(MsgType::KeepAlive, &(), false, None); //TODO: Error handling
                                    self.ui_s.log_info(&format!("Sent keep alive message to ({})", public_key));
                                }
                                None => {
                                    conn.send_raw_message(MsgType::KeepAlive, &(), false, None); //TODO: Error handling
                                    self.ui_s.log_info("Sent keep alive message to the rendezvous server");
                                }
                            }
                            
                        }
                    }
                }
                UdpConnectionState::Unannounced => {
                    match conn.last_announce {
                        Some(time) if time.elapsed() < ANNOUNCE_DELAY => {}
                        None | _ => {
                            let announce = msg_types::AnnouncePublic {
                                public_key: self.encryption.get_public_key()
                            };
                            conn.send_raw_message(MsgType::Announce, &announce, false, None);
                            conn.last_announce = Some(Instant::now());
                        }
                    }
                }
                UdpConnectionState::Pending => {}
            };
        }
    }

   
    fn send_multicast_messages(&mut self) {
        if self.last_broadcast.elapsed() > BROADCAST_DELAY {
            let announce = AnnouncePublic{ public_key: self.encryption.get_public_key().clone() };
            let mut announce = bincode::serialize(&announce).unwrap();
            let mut magic = bincode::serialize(&MULTICAST_MAGIC).unwrap();
            magic.append(&mut announce);

            self.udp_socket.send_to(&magic[..], MULTICAST_ADDRESS.parse().unwrap()).unwrap();
            self.ui_s.log_info(&"Sent broadcast message");
            self.last_broadcast = Instant::now();
        }
        
    }

    fn send_reliable_messages(&mut self) {
        for conn in &mut self.udp_connections {
            match conn.state {
                UdpConnectionState::Connected => {
                    conn.resend_reliable_messages();
                },
                _ => {}
            };
        }
    }

    fn handle_interthread_messages(&mut self, r: &mut Receiver<InterthreadMessage>, running: &mut bool) {
        loop {
            match r.try_recv() {
                Ok(msg) => {
                    match msg {
                        InterthreadMessage::SendChatMessage(p, msg, files) => {
                            let files = match files {
                                Some(files) => match self.file_manager.send_files(files) {
                                    Ok(files) => Some(files),
                                    Err(e) => {
                                        self.ui_s.log_error(&format!("Error while trying to send a file send request: {}", e.to_string()));
                                        continue;
                                    },
                                },
                                None => None
                            };
                            let dt = Utc::now();
                            let msg_id = [&msg.as_bytes(), &dt.timestamp().to_be_bytes()[..], &dt.timestamp_subsec_micros().to_be_bytes()[..]].concat();
                            let msg_id = Sha256::digest(&msg_id);
                            let msg_id = encode_config(msg_id, base64::URL_SAFE);
                            
                            let msg = msg_types::ChatMessage {author: self.encryption.get_public_key(), recipient: p.clone(), msg, attachments: files, id: msg_id, dt};
                            
                            self.msg_confirmations.insert(self.next_custom_id, msg.id.clone());
                            self.ui_s.send(InterthreadMessage::OnChatMessage(msg.clone())).unwrap();
                            match self.send_udp_message(Some(p), MsgType::ChatMessage, &msg, true, true) {
                                Ok(_) => {}
                                Err(e) => self.ui_s.log_error(&format!("Error while trying to send a chat message: {}", e.to_string()))
                            }
                        },
                        #[cfg(feature = "audio")]
                        InterthreadMessage::OpusPacketReady(data) => {
                            for conn in &mut self.udp_connections {
                                if conn.upgraded && conn.associated_peer.is_some() {
                                    conn.send_udp_message(MsgType::OpusPacket, &data, false, None).unwrap() // TODO: Indexing packets
                                }
                            }
                        }
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioDataReadyToBeProcessed(data) => self.audio.process_and_send_packet(data),
                        InterthreadMessage::OnChatMessage(msg) => self.ui_s.send(InterthreadMessage::OnChatMessage(msg)).unwrap(),
                        InterthreadMessage::ConnectToServer() => {
                            self.rendezvous_socket = TcpStream::connect(self.rendezvous_ip).unwrap();
                            self.poll.registry().register(&mut self.rendezvous_socket, RENDEZVOUS, Interest::READABLE).unwrap();
                            self.ui_s.log_info("Trying to connect to server");
                        }
                        InterthreadMessage::CallAccepted(p) => {
                            let msg = msg_types::CallResponse {
                                call: msg_types::Call {
                                    callee: self.encryption.get_public_key().clone(),
                                    caller: Some(p.clone()),
                                    udp_address: None
                                },
                                response: true
                            };

                            self.send_tcp_message(MsgType::CallResponse, &msg).unwrap();

                            let conn = self.udp_connections.iter_mut().find(|c| c.associated_peer.is_some() && c.associated_peer.clone().unwrap() == p).unwrap();
                            conn.state = UdpConnectionState::MidCall;
                            let peer = self.peers.iter_mut().find(|peer| peer.public_key == p).unwrap();
                            peer.udp_addr = Some(conn.address);
                            
                            self.ui_s.log_info(&format!("Accepted call from peer ({};{}), starting the punch through protocol", p, conn.address));
                        }
                        InterthreadMessage::CallDenied(p) => {
                            let msg = msg_types::CallResponse {
                                call: msg_types::Call {
                                    callee: self.encryption.get_public_key().clone(),
                                    caller: Some(p.clone()),
                                    udp_address: None
                                },
                                response: false
                            };

                            self.send_tcp_message(MsgType::CallResponse, &msg).unwrap();

                            let i = self.udp_connections.iter().position(|c| c.associated_peer.is_some() && c.associated_peer.clone().unwrap() == p).unwrap();
                            let conn = self.udp_connections.remove(i);
                            self.ui_s.log_info(&format!("Denied call from peer ({};{})", p, conn.address));
                        }
                        InterthreadMessage::Call(p) => {
                            let peer = self.peers.iter().find(|peer| peer.public_key == p).unwrap(); //FIXME: Unwrap err here
                            if peer.udp_addr.is_some() {
                                self.ui_s.log_warning(&format!("Tried to call a peer which is already connected {}", p));
                                continue;
                            }
                            let call = msg_types::Call {
                                callee: p.clone(),
                                caller: None,
                                udp_address: None
                            };
                            match self.calls_in_progress.iter().find(|(c, _)| c == &call) {
                                Some(_) => self.ui_s.log_warning(&format!("Tried to call a peer which has already been called: {}", p),),
                                None => {
                                    self.ui_s.log_info(&format!("Calling peer: {}", p));
                            
                                    self.calls_in_progress.push((call.clone(), Instant::now()));
                                    self.send_tcp_message(MsgType::Call, &call).unwrap(); //TODO: Error handling
                                }
                            }
                        }
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangeInputDevice(d) => self.audio.change_input_device(d),
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangeOutputDevice(d) => self.audio.change_output_device(d),
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangePreferredKbits(kbits) => self.audio.change_preferred_kbits(kbits),
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangeMuteState(muted) => self.audio.change_mute_state(muted),
                        //InterthreadMessage::AudioChangeDenoiserState(denoiser_state) => self.audio.change_denoiser_state(denoiser_state),
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangeDenoiserState(denoiser_state) => self.ui_s.log_error("Denoiser is currently disabled"),
                        InterthreadMessage::Quit() => {
                            match self.rendezvous_socket.shutdown(Shutdown::Both) {
                                _ => {}
                            }
                            *running = false;
                            return;
                        }
                        _ => unreachable!()
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => break
            }
        }
    }

    fn handle_io_events(&mut self, events: &Events) {
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
                                        self.ui_s.log_warning("Disconnected from rendezvous server");
                                        break;
                                    }
                                    Ok(_) => {
                                        self.read_tcp_message(msg_type[0], token);
                                    }
                                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                        // Socket is not ready anymore, stop reading
                                        break;
                                    }
                                    Err(e) if e.kind() == ErrorKind::ConnectionReset || e.kind() == ErrorKind::NotConnected => {
                                        match e.kind() {
                                            ErrorKind::ConnectionReset => 
                                            self.ui_s.log_warning(&format!("Disconnected from rendezvous server, reconnecting in {}", RECONNECT_DELAY.as_secs())),
                                            ErrorKind::NotConnected => 
                                            self.ui_s.log_warning(&format!("Reconnecting failed to rendezvous server, retrying in {}", RECONNECT_DELAY.as_secs())),
                                            _ => {}
                                        }
                                        
                                        self.poll.registry().deregister(&mut self.rendezvous_socket).unwrap();
                                        self.try_server_reconnect();
                                        break;
                                    },
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
                                        self.ui_s.log_error("Couldn't read from udp socket (ConnectionReset) ");
                                    }
                                    e => println!("err={:?}", e), // Unexpected error
                                }
                            },
                            MULTICAST_SOCKET => {
                                let mut buf = [0; 65536];
                                match self.multicast_socket.recv_from(&mut buf) {
                                    Ok(r) => {
                                        let (read, addr) = r;
                                        self.read_multicast_message(read, addr, &buf);
                                    }
                                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                        // Socket is not ready anymore, stop reading
                                        break;
                                    }
                                    Err(ref e) if e.kind() == io::ErrorKind::ConnectionReset => {
                                        self.ui_s.log_error("Couldn't read from multicast socket (ConnectionReset) ");
                                    }
                                    e => println!("err={:?}", e), // Unexpected error
                                }
                            }
                            _ => unreachable!()
                        }
                    }
                }
            }
        }
    }

    fn check_new_chunks(&mut self) {
        if let Some(chunks) = self.file_manager.get_requested_chunks() {
            for (peer, chunks) in chunks {
                if let Err(e) = self.send_udp_message(Some(peer), MsgType::RequestFileChunks, &msg_types::RequestFileChunks {chunks,}, true, false) {
                    self.ui_s.log_error(&format!("Error while trying to send a file chunk request: {}", e.to_string()))
                }
            }
        }
    }

    fn send_ui_updates(&mut self) {
        if self.last_stats_update.elapsed() > STATS_UPDATE_DELAY {
            let transfer_stats = self.file_manager.transfer_statistics.clone();
            let mut conn_stats = vec![];
            for c in &mut self.udp_connections {
                if let Some(p) = &c.associated_peer {
                    conn_stats.push((p.clone(), c.statistics.clone()));
                }
            }
            
            self.ui_s.send(InterthreadMessage::ConnectionStatistics(conn_stats)).unwrap();
            self.ui_s.send(InterthreadMessage::TransferStatistics(transfer_stats)).unwrap();
            self.last_stats_update = Instant::now();
        }
    }

    /// Lists the next timeouts, and also sorts the list, so the first one is always the smallest
    fn get_next_timeouts(&mut self, durations: &mut Vec<Duration>) {
        for conn in &mut self.udp_connections {
            match conn.next_resendable() {
                Some(d) => durations.push(d),
                None => {}
            }
            durations.push(conn.next_keep_alive());
        }
        let next_stats_update = (self.last_stats_update + STATS_UPDATE_DELAY).checked_duration_since(self.last_stats_update).unwrap_or(Duration::from_secs(0));
        let next_broadcast = (self.last_broadcast + BROADCAST_DELAY).checked_duration_since(self.last_broadcast).unwrap_or(Duration::from_secs(0));
        durations.push(next_stats_update);
        durations.push(next_broadcast);
        durations.sort_by(|a,b| a.cmp(b));
    }

    fn try_server_reconnect(&mut self) {
        let cm_s = self.cm_s.clone();
        thread::spawn(move || {
            thread::sleep(RECONNECT_DELAY);
            cm_s.send(InterthreadMessage::ConnectToServer()).unwrap();
        });
    }
}