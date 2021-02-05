use MsgTypes::Call;
use dxgcap::DXGIManager;
use serde::Serialize;
use std::{collections::HashMap, io, net::SocketAddr, process::{Command, Stdio}, str::FromStr, time::{Instant}};
use std::io::{Write, Read};
use std::thread;
use std::sync::mpsc;
use spin_sleep::LoopHelper;
//use scrap;
use mio::{Events, Interest, Poll, Token, net::UdpSocket};
use mio::net::{TcpListener, TcpStream};
use crate::common::{lib::read_exact, message_type::{MsgType, MsgTypes, Peer}};

const TCP_LISTENER: Token = Token(0);
const UDP_LISTENER: Token = Token(1);


struct CallRequest {
    caller: Peer,
    callee: Peer,
    time: Instant
}

pub struct RendezvousServer {
    poll: Poll,
    next_token: usize,
    tcp_listener: TcpListener,
    udp_listener: UdpSocket,
    addresses: HashMap<SocketAddr, Token>,
    tcp_connections: HashMap<Token, TcpStream>,
    /// List of announced peers
    peers: Vec<Peer>,
    /// List of ongoing calls
    calls: Vec<CallRequest>
}

impl RendezvousServer {
    pub fn start_server(){
        let poll = Poll::new().unwrap();
        let mut next_token = 0;

        let mut tcp_listener = TcpListener::bind(SocketAddr::from_str("0.0.0.0:42069").unwrap()).unwrap();
        poll.registry().register(&mut tcp_listener, Token(next_token), Interest::READABLE).unwrap();
        next_token += 1;
        
        let mut udp_listener = UdpSocket::bind(SocketAddr::from_str("0.0.0.0:42069").unwrap()).unwrap();
        poll.registry().register(&mut udp_listener, Token(next_token), Interest::READABLE).unwrap();
        next_token += 1;
        
        
        let mut s = RendezvousServer {
            poll,
            next_token,
            tcp_listener,
            udp_listener,
            addresses: HashMap::new(),
            tcp_connections: HashMap::new(),
            peers: Vec::new(),
            calls: Vec::new()
        };
        s.event_loop();
    }
    
    fn event_loop(&mut self) {
        loop {
            let mut events = Events::with_capacity(1024);
            self.poll.poll(&mut events, None).unwrap();
            for event in events.iter() {
                match event.token() {
                    TCP_LISTENER => {
                        loop {
                            match self.tcp_listener.accept() {
                                Ok((mut sock, addr)) => {
                                    println!("Peer ({}) connected", sock.peer_addr().unwrap());
                                    let token = Token(self.next_token);
                                    self.next_token += 1;

                                    self.poll.registry().register(&mut sock, token, Interest::READABLE).unwrap();
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
                    UDP_LISTENER => {
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
                    token => {
                        let mut msg_type = [0;1];
                        
                        loop {
                            let sock = self.tcp_connections.get_mut(&token).unwrap();
                            let addr = sock.peer_addr().unwrap();
                            match sock.read(&mut msg_type) {
                                Ok(0) => {
                                    self.on_disconnect(addr, token);
                                    break;
                                }
                                Ok(_) => {
                                    self.read_message(msg_type[0], token);
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    // Socket is not ready anymore, stop reading
                                    break;
                                }
                                e => {
                                    println!("Peer disconnected with an error={:?}", e);
                                    self.on_disconnect(addr, token);
                                    break;
                                },
                            }
                        }
                    }
                }
            }
        }
    }

    fn on_disconnect(&mut self, addr: SocketAddr, token: Token) {
        println!("Peer ({}) disconnected", addr);

        // Notify other clients
        let sock = self.tcp_connections.get(&token).unwrap().peer_addr().unwrap();
        let p_key = self.peers.iter_mut().find(|x| x.addr.unwrap() == addr).unwrap().public_key.clone();
        for c in self.tcp_connections.values_mut() {
            if c.peer_addr().unwrap() != sock // Only broadcast to other clients
            {
                RendezvousServer::send_message(c, MsgType::Disconnect, &MsgTypes::Disconnect{public_key: p_key.clone()});
            }
        }

        // Remove from database
        self.addresses.remove(&addr);
        self.peers.iter()
        .position(|p| p.addr.unwrap() == addr)
        .map(|i| self.peers.remove(i));
        self.tcp_connections.remove(&token);
    }

    fn read_message(&mut self, msg_type: u8, token: Token) {
        let sock = self.tcp_connections.get_mut(&token).unwrap();
        let addr = sock.peer_addr().unwrap();

        let msg_type = num::FromPrimitive::from_u8(msg_type);

        let mut msg_size = [0u8; 8];
        
        read_exact(sock, &mut msg_size);
        let msg_size: u64 = bincode::deserialize(&msg_size).unwrap();
        let mut msg = vec![0;msg_size as usize];
        read_exact(sock, &mut msg[..]);

        match msg_type {
            Some(MsgType::Announce) => {
                let decoded: MsgTypes::Announce = bincode::deserialize(&mut msg[..]).unwrap();
                let p = Peer {
                    addr: Some(addr),
                    udp_addr: None,
                    public_key: decoded.public_key,
                };
                println!("Received public key for peer ({}): {}", p.addr.unwrap(), p.public_key);
                self.peers.push(p);

                for c in self.tcp_connections.values_mut() {
                    RendezvousServer::send_message(c, MsgType::Announce,
                    &self.peers.to_vec().iter_mut()
                        .filter(|x| x.addr.unwrap() != c.peer_addr().unwrap())
                        .map(|x| {x.addr = None; x.udp_addr=None; x}).collect::<Vec<&mut Peer>>()
                    );
                }
                
            }
            Some(MsgType::Call) => {
                let mut call: MsgTypes::Call = bincode::deserialize(&mut msg[..]).unwrap();
                if let Some(caller) = self.peers.iter().find(|x| x.addr.unwrap() == addr) {
                    if let Some(callee) = self.peers.iter().find(|x| x.public_key == call.callee.public_key) {
                        if caller.udp_addr.is_none() || callee.udp_addr.is_none() {
                            // TODO: Send error message to caller
                            println!("Error routing a call from ({}; {}) to ({}; {}) udp address hasn't been found", addr, caller.public_key, callee.addr.unwrap(), callee.public_key);
                            return;
                        }
                        let req = CallRequest{
                            caller: caller.clone(),
                            callee: callee.clone(),
                            time: Instant::now()
                        };
                        self.calls.push(req);
                        // Don't trust the client
                        call.caller = Some(caller.clone());
                        let token = self.addresses.get(&callee.addr.unwrap()).unwrap();
                        let mut callee_socket = self.tcp_connections.get_mut(token).unwrap();
                        RendezvousServer::send_message(&mut callee_socket, MsgType::Call, &call);
                        println!("Routed a call from ({}; {}) to ({}; {})", addr, caller.public_key, callee_socket.peer_addr().unwrap(), callee.public_key);
                    }
                    else {
                        println!("Callee haven't announced itself yet");
                    }
                }
                else {
                    println!("Caller haven't announced itself yet. ({})", addr);
                }

            }
            Some(MsgType::CallResponse) => {
                let call_response: MsgTypes::CallResponse = bincode::deserialize(&mut msg[..]).unwrap();
                let callee = call_response.call.callee;
                let caller = call_response.call.caller.unwrap();
                match self.calls.iter().position(|x| x.callee.public_key == callee.public_key && x.caller.public_key == caller.public_key) {
                    Some(index) => {
                        if call_response.response {
                            let call_request = self.calls.get(index).unwrap();
                            println!("Peer ({}) accepted the call request from ({})", callee.public_key, caller.public_key);
                            
                            let mut sock = self.tcp_connections.values_mut().find(|x| x.peer_addr().unwrap() == caller.addr.unwrap()).unwrap();
                            let callee = self.peers.iter().find(|p| p.public_key == callee.public_key).unwrap().clone(); // Get the callee so the address is included
                            let msg = MsgTypes::CallResponse {
                                call: Call {
                                    callee,
                                    caller: Some(caller),
                                },
                                response: call_response.response,
                            };
                            RendezvousServer::send_message(&mut sock, MsgType::CallResponse, &msg);
                        }
                        else {
                            self.calls.remove(index);
                        }
                    }
                    None => {
                        unreachable!();
                    }
                }
            }
            _ => unreachable!()
        }
        
    }

    fn read_udp_message(&mut self, read: usize, addr: SocketAddr, buf: &[u8]) {
        let msg_type = buf[0];
        let msg_type = num::FromPrimitive::from_u8(msg_type);

        match msg_type {
            Some(MsgType::Announce) => {
                let announce: MsgTypes::Announce = bincode::deserialize(&buf[1..]).unwrap();
                match self.peers.iter_mut().find(|p| p.public_key == announce.public_key) {
                    Some(p) => {
                        p.udp_addr = Some(addr);
                        println!("Associated UDP adress ({}) with peer: ({})", addr, p.public_key);
                        self.send_udp_message(addr, MsgType::Announce, &());
                    },
                    None => {}
                }
            }
            Some(MsgType::KeepAlive) => {}
            _ => unreachable!()
        }
    }
    
    fn send_message<T: ?Sized>(sock: &mut TcpStream, t: MsgType, msg: &T) where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let msg_size = bincode::serialize(&msg.len()).unwrap();
        let chained: &[u8] = &[&[t], &msg_size[..], &msg].concat()[..];

        sock.write_all(chained).unwrap();
    }

    fn send_udp_message<T: ?Sized>(&self, addr: SocketAddr, t: MsgType, msg: &T) where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        self.udp_listener.send_to(chained, addr).unwrap();
    }

    fn handle_client(&self, mut stream: TcpStream) {
        let msg = "Hello from the server";
        stream.write(msg.as_bytes()).unwrap();
    }
}


fn start_recording() {
    let mut dxgi = DXGIManager::new(100).unwrap();
    dxgi.set_capture_source_index(0);
    
    let (w,h) = dxgi.geometry();
    println!("Dimensions: ({}:{})", w, h);

    let framerate: usize = 30;
    let mut child = Command::new("cmd")
        .args(&[
            "/C",
            format!(
                //"ffmpeg -f rawvideo -pix_fmt bgra -s {width}x{height} -i - -c:v libx264rgb -r {framerate} -vsync 1 -b:v 10M -maxrate 10M -bufsize 20M -g 120 -preset veryfast -tune zerolatency -y {output}",
                "ffmpeg -r {framerate} -re -f rawvideo -pix_fmt bgra -s {width}x{height} -i - -r {framerate} -vsync 1 -f h264 -y {output} ",
                //"ffmpeg -i asd.mp4 -pix_fmt rgb24 -f rawvideo -",
                //"ffmpeg -hwaccel_output_format cuda -vsync 0 -r 60 -hwaccel cuvid -f rawvideo -pix_fmt bgra -s {width}x{height} -i - -c:v hevc_nvenc -preset p7 -b:v 50M -maxrate:v 55M -y {output}",
                width = w,
                height = h,
                framerate = framerate,
                output = "pipe:1"
            ).as_str()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute child");
    
    let (tx, rx) = mpsc::channel();
    let mut child_stdin = child.stdin.take().unwrap();
    let mut child_stdout = child.stdout.take().unwrap();
    
    thread::spawn(move || {
        let mut buf = vec![];

        let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.5) 
        .build_with_target_rate(60.0);

        loop {
                
            loop_helper.loop_start();
            let res = rx.try_recv();
            match res {
                Ok(x) => buf = x,
                Err(_) => {},
            }

            child_stdin.write(&buf[..]).unwrap();
            loop_helper.loop_sleep();
        }
    });

    thread::spawn(move || {
        let mut buf = vec![0;2560*1080*3];
        for i in 0..1000 {
            
            child_stdout.read_exact(&mut buf).unwrap();
        
            //image::save_buffer(format!("test{i}.png", i=i), &buf, 2560, 1080, image::ColorType::Rgb8).unwrap();
        }
    });

    loop {
        let (buf, (_,_)) = dxgi.capture_frame_components().unwrap();
        tx.send(buf).unwrap();
    }
    
    /*child_stdin.flush().unwrap();
    child.wait().expect("child process wasn't running");*/

}
