use MsgTypes::Call;
use mio::{Events, Interest, Poll, Waker, net::{TcpStream, UdpSocket}};
use mpsc::{Receiver, channel};
use std::{io::{self, Read, Write}, net::{Shutdown, SocketAddr}, str::FromStr, sync::{Arc, mpsc::{self, Sender}}, thread::{self, JoinHandle}, time::{Duration, Instant}};

use mio::Token;
use serde::Serialize;

use crate::common::{encryption::{Encryption, NetworkedPublicKey}, lib::read_exact, message_type::{MsgType, MsgTypes, Peer}};

const RENDEZVOUS: Token = Token(0);
const WAKER: Token = Token(1);
const UDP_SOCKET: Token = Token(2);

/// Call decay defined in seconds
const CALL_DECAY: u64 = 60; 
/// Keep alive delay between messages
const KEEP_ALIVE_DELAY: u64 = 10; 
/// Message sending interval when mid-call
const KEEP_ALIVE_DELAY_MIDCALL: u64 = 1; 
/// Message sending interval when announcing
const ANNOUNCE_DELAY: u64 = 1; 

pub struct ConnectionManager {
    rendezvous_socket: TcpStream,
    rendezvous_ip: SocketAddr,
    udp_socket: UdpSocket,
    next_token: usize,
    peers: Vec<Peer>,
    udp_connections: Vec<UdpConnection>,
    poll: Poll,
    ui_s: Sender<InterthreadMessage>,
    encryption: Encryption,
    /// Instant is when the call was sent
    calls_in_progress: Vec<(Call, Instant)>, 
    waker_thread: Sender<InterthreadMessage>
}

struct UdpConnection {
    address: SocketAddr,
    last_keep_alive: Option<Instant>,
    last_announce: Option<Instant>,
    state: UdpConnectionState
}

enum UdpConnectionState {
    /// The punch through is currently being done
    MidCall=0, 
    /// The socket is 'connected' so only keep alive packets need to be sent
    Connected=1,
    /// The socket is waiting for the server to accept the announce
    Unannounced=2
}

pub enum InterthreadMessage {
    //SendMessage(MsgType, ),
    AnnounceResponse(Vec<Peer>),
    Quit(),
    PeerDisconnected(NetworkedPublicKey),
    Call(Peer),
    SetWakepDelay(u64)
}

impl ConnectionManager {
    pub fn start(rend_ip: &str, ui_s: Sender<InterthreadMessage>) -> (Sender<InterthreadMessage>, JoinHandle<()>, Arc<Waker>) {
        let mut udp_connections = Vec::new();
        let mut next_token: usize = 0;
        let poll = Poll::new().unwrap();
        let (cm_s, cm_r) = mpsc::channel();
        
        let rend_ip = SocketAddr::from_str(rend_ip).unwrap();

        let mut rendezvous_socket = TcpStream::connect(rend_ip).unwrap();
        poll.registry().register(&mut rendezvous_socket, Token(next_token), Interest::READABLE).unwrap();
        next_token += 1;

        let waker = Arc::new(Waker::new(poll.registry(), Token(next_token)).unwrap());
        next_token += 1;

        let mut udp_socket = UdpSocket::bind(SocketAddr::from_str("127.0.0.1:0").unwrap()).unwrap();
        udp_connections.push(UdpConnection{
            address: rend_ip,
            last_keep_alive: None,
            last_announce: None,
            state: UdpConnectionState::Unannounced,
        });
        poll.registry().register(&mut udp_socket, UDP_SOCKET, Interest::READABLE).unwrap();
        next_token += 1;

        let waker_thread = ConnectionManager::set_up_waker_thread(waker.clone());

        let encryption = Encryption::new();

        let mut mgr = ConnectionManager {
            rendezvous_socket,
            rendezvous_ip: rend_ip,
            udp_socket,
            next_token,
            peers: Vec::new(),
            udp_connections,
            poll,
            ui_s,
            encryption,
            calls_in_progress: Vec::new(),
            waker_thread,
        };
        mgr.announce();
        let thr = thread::spawn(move || {
            mgr.event_loop(cm_r);
        });
        (cm_s, thr, waker)
    }

    fn set_up_waker_thread(waker: Arc<Waker>) -> Sender<InterthreadMessage> {
        let (s, r) = channel();
        thread::spawn(move || {
            let mut delay = KEEP_ALIVE_DELAY;
            let mut elapsed = 0;
            loop {
                thread::sleep(Duration::from_secs(1));
                elapsed += 1;
                for e in r.try_iter() {
                    match e {
                        InterthreadMessage::SetWakepDelay(n) => delay = n, //TODO: Reset wake up delay if no call requests are currently sent
                        InterthreadMessage::Quit() => return,
                        _ => unreachable!()
                    }
                }
                if elapsed > delay {
                    waker.wake().unwrap();
                    elapsed = 0;
                }
            }
        });
        s
    }

    fn event_loop(&mut self, r: Receiver<InterthreadMessage>) {
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
                                ConnectionManager::send_udp_message(&self.udp_socket, addr, MsgType::KeepAlive, &());
                            }
                        }
                    }
                    UdpConnectionState::Unannounced => {
                        match conn.last_announce {
                            Some(time) if time.elapsed().as_secs() < ANNOUNCE_DELAY => {}
                            None | _ => {
                                let announce = MsgTypes::Announce {
                                    public_key: self.encryption.get_public_key(),
                                };
                                ConnectionManager::send_udp_message(&self.udp_socket, self.rendezvous_ip, MsgType::Announce, &announce);
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
                            //InterthreadMessage::SendMessage(t, msg) => self.send_tcp_message(t, msg),
                            InterthreadMessage::Call(p) => {
                                let call = MsgTypes::Call {
                                    callee: p,
                                    caller: None
                                };
                                self.calls_in_progress.push((call.clone(), Instant::now()));
                                self.send_tcp_message(MsgType::Call, &call);
                            }
                            InterthreadMessage::Quit() => {
                                self.rendezvous_socket.shutdown(Shutdown::Both).unwrap();
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
                                            println!("Disconnected from rendezvous server");
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
                                        //e => panic!("err={:?}", e), // Unexpected error
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

    fn announce(&mut self) {
        let msg = MsgTypes::Announce {
            public_key: self.encryption.get_public_key()
        };
        self.send_tcp_message(MsgType::Announce, &msg);
    }

    pub fn call(s: &Sender<InterthreadMessage>, waker: &Waker, p: Peer) {
        s.send(InterthreadMessage::Call(p)).unwrap();
        waker.wake().unwrap();
    }

    pub fn quit(s: &Sender<InterthreadMessage>, waker: &Waker) {
        s.send(InterthreadMessage::Quit()).unwrap();
        waker.wake().unwrap();
    }

    fn read_tcp_message(&mut self, msg_type: u8, token: Token) {
        let sock = &mut self.rendezvous_socket;
        let addr = sock.peer_addr().unwrap();

        let msg_type = num::FromPrimitive::from_u8(msg_type);

        let mut msg_size = [0u8; 8];
        read_exact(sock, &mut msg_size);
        let msg_size: u64 = bincode::deserialize(&msg_size).unwrap();
        let mut msg = vec![0;msg_size as usize];
        read_exact(sock, &mut msg[..]);

        match msg_type {
            Some(MsgType::Announce) => {
                let peers: Vec<Peer> = bincode::deserialize(&msg).unwrap();
                self.ui_s.send(InterthreadMessage::AnnounceResponse(peers)).unwrap();
            }
            Some(MsgType::Call) => {
                let call: MsgTypes::Call = bincode::deserialize(&mut msg[..]).unwrap();

                // FIXME: add the ability to decline the call
                let msg = MsgTypes::CallResponse {
                    call: call.clone(),
                    response: true
                };
                self.send_tcp_message(MsgType::CallResponse, &msg);

                let conn = UdpConnection {
                    address: call.caller.clone().unwrap().udp_addr.unwrap(),
                    last_keep_alive: None,
                    last_announce: None,
                    state: UdpConnectionState::MidCall,
                };

                self.udp_connections.push(conn);
            }
            Some(MsgType::CallResponse) => {
                let call_response: MsgTypes::Call = bincode::deserialize(&mut msg[..]).unwrap();
                self.calls_in_progress.iter()
                .position(|(c, _)| c.callee == call_response.callee)
                .map(|i| self.calls_in_progress.remove(i));

                let conn = UdpConnection {
                    address: call_response.callee.udp_addr.unwrap(),
                    last_keep_alive: None,
                    last_announce: None,
                    state: UdpConnectionState::MidCall,
                };
                self.udp_connections.push(conn);
                self.waker_thread.send(InterthreadMessage::SetWakepDelay(KEEP_ALIVE_DELAY_MIDCALL)).unwrap();
            }
            Some(MsgType::Disconnect) => {
                let decoded: MsgTypes::Disconnect = bincode::deserialize(&mut msg[..]).unwrap();
                self.ui_s.send(InterthreadMessage::PeerDisconnected(decoded.public_key)).unwrap();
            }
            _ => unreachable!()
        }
        
    }

    fn read_udp_message(&mut self, read: usize, addr: SocketAddr, buf: &[u8]) {
        let msg_type = buf[0];
        let msg_type = num::FromPrimitive::from_u8(msg_type);

        match msg_type {
            Some(MsgType::Announce) => {
                self.udp_connections.iter_mut()
                .find(|x| x.address == addr).unwrap()
                .state = UdpConnectionState::Connected;
                println!("UDP Announced");
                // TODO: Log that it's connected
            }
            Some(MsgType::KeepAlive) => {
                println!("Received keep alive message");
                self.udp_connections.iter_mut()
                .find(|x| x.address == addr).unwrap()
                .state = UdpConnectionState::Connected;
            }
            Some(MsgType::ChatMessage) => {
                let msg :MsgTypes::ChatMessage = bincode::deserialize(&buf[1..]).unwrap();
                println!("Received chat message");
            }
            _ => unreachable!()
        }
    }

    fn send_tcp_message<T: ?Sized>(&mut self, t:MsgType, msg: &T) where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let msg_size = bincode::serialize(&msg.len()).unwrap();
        let chained: &[u8] = &[&[t], &msg_size[..], &msg].concat()[..];

        self.rendezvous_socket.write_all(chained).unwrap();
    }

    fn send_udp_message<T: ?Sized>(sock: &UdpSocket, addr: SocketAddr, t: MsgType, msg: &T) where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        sock.send_to(chained, addr);
    }
}