use std::{
    io::{self, Read},
    net::Shutdown,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use base64::Engine;
use chrono::Utc;
use io::ErrorKind;
use mio::{net::TcpStream, Events, Interest};
use p2pthing_common::sha2::{Digest, Sha256};
use p2pthing_common::{
    message_type::{
        msg_types::{self, AnnouncePublic},
        InterthreadMessage, MsgType,
    },
    ui::UIConn,
};
use tracing::{instrument, trace_span};

use crate::client::udp_connection::UdpConnectionState;

use super::{
    handlers::MulticastHandler, ConnectionManager, ANNOUNCE_DELAY, BROADCAST_DELAY, CALL_DECAY, KEEP_ALIVE_DELAY,
    KEEP_ALIVE_DELAY_MIDCALL, MULTICAST_ADDRESS, MULTICAST_MAGIC, MULTICAST_SOCKET, RECONNECT_DELAY, RENDEZVOUS,
    STATS_UPDATE_DELAY, UDP_SOCKET, WAKER,
};

impl ConnectionManager {
    #[instrument(skip(self, r), name = "client event loop")]
    pub fn event_loop(&mut self, r: &mut Receiver<InterthreadMessage>) {
        let mut running = true;
        while running {
            trace_span!("client event loop iteration").in_scope(|| {
                let mut events = Events::with_capacity(1024);

                #[cfg(feature = "audio")]
                if !self.audio.started && self.peers.connections().any(|c| c.upgraded && c.associated_peer.is_some()) {
                    self.audio.init();
                }

                // Calculate the next timeout
                let mut durations = vec![];
                self.get_next_timeouts(&mut durations);

                trace_span!("polling for events", next_poll_duration = durations.first().cloned().unwrap().as_millis())
                    .in_scope(|| {
                        if let Err(err) = self.poll.poll(&mut events, durations.first().cloned()) {
                            self.ui_s.log_error(&format!("Error while polling: {}", err.to_string()));
                        }
                    });

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
            });
        }
    }

    #[instrument(skip(self))]
    fn send_keep_alive_messages(&mut self) {
        for conn in &mut self.peers.connections_mut() {
            match conn.state {
                UdpConnectionState::MidCall | UdpConnectionState::Connected => {
                    let delay = match conn.state {
                        UdpConnectionState::MidCall => KEEP_ALIVE_DELAY_MIDCALL,
                        UdpConnectionState::Connected => KEEP_ALIVE_DELAY,
                        _ => unreachable!(),
                    };
                    match conn.last_message_sent {
                        Some(time) if time.elapsed() < delay => {}
                        None | _ => match conn.associated_peer.clone() {
                            Some(public_key) => {
                                if let Err(err) = conn.send_raw_message(MsgType::KeepAlive, &(), false, None) {
                                    self.ui_s.log_error(&format!(
                                        "Failed to send keep alive message to ({}) with error: {}",
                                        public_key, err
                                    ));
                                } else {
                                    self.ui_s.log_info(&format!("Sent keep alive message to ({})", public_key));
                                }
                            }
                            None => {
                                if let Err(err) = conn.send_raw_message(MsgType::KeepAlive, &(), false, None) {
                                    self.ui_s.log_error(&format!(
                                        "Failed to send keep alive message to the rendezvous server with error: {}",
                                        err
                                    ));
                                } else {
                                    self.ui_s.log_info("Sent keep alive message to the rendezvous server");
                                }
                            }
                        },
                    }
                }
                UdpConnectionState::Unannounced => match conn.last_announce {
                    Some(time) if time.elapsed() < ANNOUNCE_DELAY => {}
                    None | _ => {
                        let announce = msg_types::AnnouncePublic { public_key: self.encryption.get_public_key() };
                        if let Err(err) = conn.send_raw_message(MsgType::Announce, &announce, false, None) {
                            self.ui_s.log_error(&format!(
                                "Failed to send announce message to the rendezvous server with error: {}",
                                err
                            ));
                        }
                        conn.last_announce = Some(Instant::now());
                    }
                },
                UdpConnectionState::Unknown | UdpConnectionState::Pending => {}
            };
        }
    }

    #[instrument(skip(self))]
    fn send_multicast_messages(&mut self) {
        if self.last_broadcast.elapsed() > BROADCAST_DELAY {
            let announce = AnnouncePublic { public_key: self.encryption.get_public_key().clone() };
            let mut announce = bincode::serialize(&announce).unwrap();
            let mut magic = bincode::serialize(&MULTICAST_MAGIC).unwrap();
            magic.append(&mut announce);

            let multicast_address = MULTICAST_ADDRESS.parse().expect("Failed parsing multicast address");
            match self.udp_socket.send_to(&magic[..], multicast_address) {
                Ok(_) => self.ui_s.log_info(&"Sent broadcast message"),
                Err(err) => self.ui_s.log_error(&format!("Failed sending broadcast message: {}", err)),
            }

            self.last_broadcast = Instant::now();
        }
    }

    #[instrument(skip(self))]
    fn send_reliable_messages(&mut self) {
        for conn in &mut self.peers.connections_mut() {
            match conn.state {
                UdpConnectionState::Connected => {
                    conn.resend_reliable_messages();
                }
                _ => {}
            };
        }
    }

    #[instrument(skip(self, r, running))]
    fn handle_interthread_messages(&mut self, r: &mut Receiver<InterthreadMessage>, running: &mut bool) {
        loop {
            let s = trace_span!("Interthread message iteration");
            let _enter = s.enter();
            match r.try_recv() {
                Ok(msg) => {
                    match msg {
                        InterthreadMessage::SendChatMessage(p, msg, files) => {
                            let files = match files {
                                Some(files) => match self.file_manager.send_files(files) {
                                    Ok(files) => Some(files),
                                    Err(e) => {
                                        self.ui_s.log_error(&format!(
                                            "Error while trying to send a file send request: {}",
                                            e.to_string()
                                        ));
                                        continue;
                                    }
                                },
                                None => None,
                            };
                            let dt = Utc::now();
                            let msg_id = [
                                &msg.as_bytes(),
                                &dt.timestamp().to_be_bytes()[..],
                                &dt.timestamp_subsec_micros().to_be_bytes()[..],
                            ]
                            .concat();
                            let msg_id = Sha256::digest(&msg_id);
                            let msg_id = base64::engine::general_purpose::URL_SAFE.encode(msg_id);

                            let msg = msg_types::ChatMessage {
                                author: self.encryption.get_public_key(),
                                recipient: p.clone(),
                                msg,
                                attachments: files,
                                id: msg_id,
                                dt,
                            };

                            self.msg_confirmations.insert(self.next_custom_id, msg.id.clone());
                            self.ui_s.send(InterthreadMessage::OnChatMessage(msg.clone())).unwrap();
                            match self.send_udp_message(Some(p), MsgType::ChatMessage, &msg, true, true) {
                                Ok(_) => {}
                                Err(e) => self.ui_s.log_error(&format!(
                                    "Error while trying to send a chat message: {}",
                                    e.to_string()
                                )),
                            }
                        }
                        #[cfg(feature = "audio")]
                        InterthreadMessage::OpusPacketReady(data) => {
                            for conn in &mut self.peers.connections_mut() {
                                if conn.upgraded && conn.associated_peer.is_some() {
                                    conn.send_udp_message(MsgType::OpusPacket, &data, false, None).unwrap()
                                    // TODO: Indexing packets
                                }
                            }
                        }
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioDataReadyToBeProcessed(data) => {
                            self.audio.process_and_send_packet(data)
                        }
                        InterthreadMessage::OnChatMessage(msg) => {
                            self.ui_s.send(InterthreadMessage::OnChatMessage(msg)).unwrap()
                        }
                        InterthreadMessage::ConnectToServer() => {
                            let rendezvous =
                                self.peers.rendezvous_servers_mut().next().expect("No rendezvous server found");
                            rendezvous.tcp_conn = Some(
                                TcpStream::connect(self.rendezvous_ip)
                                    .expect("Failed to connect to TCP rendezvous server socket"),
                            );
                            self.poll
                                .registry()
                                .register(rendezvous.tcp_conn.as_mut().unwrap(), RENDEZVOUS, Interest::READABLE)
                                .unwrap();
                            self.ui_s.log_info("Trying to connect to server");
                        }
                        InterthreadMessage::CallAccepted(p) => {
                            let msg = msg_types::CallResponse {
                                call: msg_types::Call {
                                    callee: self.encryption.get_public_key().clone(),
                                    caller: Some(p.clone()),
                                    udp_address: None,
                                },
                                response: true,
                            };

                            self.send_tcp_message(MsgType::CallResponse, &msg).unwrap();

                            let p = self.peers.peer_mut(&p).unwrap();
                            p.udp_conn.as_mut().unwrap().state = UdpConnectionState::MidCall;

                            self.ui_s.log_info(&format!(
                                "Accepted call from peer ({};{}), starting the punch through protocol",
                                p.public_key.as_ref().unwrap(),
                                p.udp_conn.as_ref().unwrap().address
                            ));
                        }
                        InterthreadMessage::CallDenied(p) => {
                            let msg = msg_types::CallResponse {
                                call: msg_types::Call {
                                    callee: self.encryption.get_public_key().clone(),
                                    caller: Some(p.clone()),
                                    udp_address: None,
                                },
                                response: false,
                            };

                            self.send_tcp_message(MsgType::CallResponse, &msg).unwrap();

                            let p = self.peers.peer_mut(&p).unwrap();
                            let conn = p.udp_conn.take().unwrap();

                            self.ui_s.log_info(&format!(
                                "Denied call from peer ({};{})",
                                p.public_key.as_ref().unwrap(),
                                conn.address
                            ));
                        }
                        InterthreadMessage::Call(p) => {
                            let peer = self.peers.peer(&p).unwrap(); //FIXME: Unwrap err here
                                                                     // FIXME: Knowing the UDP address does not mean we are connected
                            if let Some(conn) = peer.udp_conn.as_ref() {
                                if conn.state == UdpConnectionState::Connected {
                                    self.ui_s
                                        .log_warning(&format!("Tried to call a peer which is already connected {}", p));
                                    continue;
                                }
                            }
                            let call = msg_types::Call { callee: p.clone(), caller: None, udp_address: None };
                            match self.calls_in_progress.iter().find(|(c, _)| c == &call) {
                                Some(_) => self
                                    .ui_s
                                    .log_warning(&format!("Tried to call a peer which has already been called: {}", p)),
                                None => {
                                    self.ui_s.log_info(&format!("Calling peer: {}", p));

                                    self.calls_in_progress.push((call.clone(), Instant::now()));
                                    self.send_tcp_message(MsgType::Call, &call).unwrap();
                                    //TODO: Error handling
                                }
                            }
                        }
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangeInputDevice(d) => self.audio.change_input_device(d),
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangeOutputDevice(d) => self.audio.change_output_device(d),
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangePreferredKbits(kbits) => {
                            self.audio.change_preferred_kbits(kbits)
                        }
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangeMuteState(muted) => self.audio.change_mute_state(muted),
                        //InterthreadMessage::AudioChangeDenoiserState(denoiser_state) => self.audio.change_denoiser_state(denoiser_state),
                        #[cfg(feature = "audio")]
                        InterthreadMessage::AudioChangeDenoiserState(denoiser_state) => {
                            self.ui_s.log_error("Denoiser is currently disabled")
                        }
                        InterthreadMessage::Quit() => {
                            if let Some(rendezvous) = self.peers.rendezvous_servers_mut().next().as_mut() {
                                if let Some(tcp_conn) = rendezvous.tcp_conn.as_mut() {
                                    if let Err(err) = tcp_conn.shutdown(Shutdown::Both) {
                                        self.ui_s.log_error(&format!(
                                            "Error while trying to shutdown TCP rendezvous server socket: {}",
                                            err.to_string()
                                        ));
                                    }
                                }
                            }
                            *running = false;
                            return;
                        }
                        _ => unreachable!(),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
    }

    #[instrument(skip(self, events))]
    fn handle_io_events(&mut self, events: &Events) {
        for event in events.iter() {
            match event.token() {
                token => {
                    loop {
                        match token {
                            WAKER => break,
                            RENDEZVOUS => {
                                let mut msg_type = [0; 1];
                                let rendezvous_socket = self
                                    .peers
                                    .rendezvous_servers_mut()
                                    .next()
                                    .expect("No rendezvous server found")
                                    .tcp_conn
                                    .as_mut()
                                    .expect("Rendezvous server has no TCP connection");
                                match rendezvous_socket.read(&mut msg_type) {
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
                                    Err(e)
                                        if e.kind() == ErrorKind::ConnectionReset
                                            || e.kind() == ErrorKind::NotConnected
                                            || e.kind() == ErrorKind::ConnectionRefused =>
                                    {
                                        match e.kind() {
                                            ErrorKind::ConnectionReset => self.ui_s.log_warning(&format!(
                                                "Disconnected from rendezvous server, reconnecting in {}",
                                                RECONNECT_DELAY.as_secs()
                                            )),
                                            ErrorKind::NotConnected | ErrorKind::ConnectionRefused => {
                                                self.ui_s.log_warning(&format!(
                                                    "Reconnecting failed to rendezvous server, retrying in {}",
                                                    RECONNECT_DELAY.as_secs()
                                                ))
                                            }
                                            _ => unreachable!(),
                                        }

                                        self.poll.registry().deregister(rendezvous_socket).unwrap();
                                        self.try_server_reconnect();
                                        break;
                                    }
                                    e => panic!("err={:?}", e), // Unexpected error
                                }
                            }
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
                            }
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
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }

    #[instrument(skip(self))]
    fn check_new_chunks(&mut self) {
        if let Some(chunks) = self.file_manager.get_requested_chunks() {
            for (peer, chunks) in chunks {
                if let Err(e) = self.send_udp_message(
                    Some(peer),
                    MsgType::RequestFileChunks,
                    &msg_types::RequestFileChunks { chunks },
                    true,
                    false,
                ) {
                    self.ui_s.log_error(&format!("Error while trying to send a file chunk request: {}", e.to_string()))
                }
            }
        }
    }

    #[instrument(skip(self))]
    fn send_ui_updates(&mut self) {
        if self.last_stats_update.elapsed() > STATS_UPDATE_DELAY {
            let transfer_stats = self.file_manager.transfer_statistics.clone();
            let mut conn_stats = vec![];
            for c in &mut self.peers.connections() {
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
    #[instrument(skip(self, durations))]
    fn get_next_timeouts(&mut self, durations: &mut Vec<Duration>) {
        for conn in &mut self.peers.connections_mut() {
            match conn.next_resendable() {
                Some(d) => durations.push(d),
                None => {}
            }
            if let Some(next_keep_alive) = conn.next_keep_alive() {
                durations.push(next_keep_alive);
            }
        }
        let next_stats_update = (self.last_stats_update + STATS_UPDATE_DELAY)
            .checked_duration_since(self.last_stats_update)
            .unwrap_or(Duration::from_secs(0));
        let next_broadcast = (self.last_broadcast + BROADCAST_DELAY)
            .checked_duration_since(self.last_broadcast)
            .unwrap_or(Duration::from_secs(0));
        durations.push(next_stats_update);
        durations.push(next_broadcast);
        durations.sort_by(|a, b| a.cmp(b));
    }

    #[instrument(skip(self))]
    fn try_server_reconnect(&mut self) {
        let cm_s = self.cm_s.clone();
        thread::spawn(move || {
            thread::sleep(RECONNECT_DELAY);
            cm_s.send(InterthreadMessage::ConnectToServer()).unwrap();
        });
    }
}
