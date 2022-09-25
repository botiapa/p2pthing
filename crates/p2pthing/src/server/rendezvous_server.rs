use std::{collections::HashMap, env, net::SocketAddr, str::FromStr};
//use scrap;
use mio::{Interest, Poll, Token, net::UdpSocket};
use mio::net::{TcpListener, TcpStream};
use p2pthing_common::encryption::{AsymmetricEncryption, SymmetricEncryption, NetworkedPublicKey};
use p2pthing_common::message_type::{MsgType, msg_types};

mod event_loop;
mod utils;
mod tcp_message;
mod udp_message;

struct CallRequest {
    caller: ServerPeer,
    callee: ServerPeer
}

struct ServerPeer {
    pub public_key: NetworkedPublicKey,
    pub addr: Option<SocketAddr>,
    pub udp_addr: Option<SocketAddr>,
    pub sym_key: Option<SymmetricEncryption>
}

impl Clone for ServerPeer {
    fn clone(&self) -> Self {
        Self { public_key: self.public_key.clone(), addr: self.addr.clone(), udp_addr: self.udp_addr.clone(), sym_key: None }
    }
}

pub struct RendezvousServer {
    poll: Poll,
    next_token: usize,
    tcp_listener: TcpListener,
    udp_listener: UdpSocket,
    addresses: HashMap<SocketAddr, Token>,
    tcp_connections: HashMap<Token, TcpStream>,
    /// List of pending symmetric keys
    sym_keys: HashMap<SocketAddr, SymmetricEncryption>,
    /// List of announced peers
    peers: Vec<ServerPeer>,
    /// List of ongoing calls
    calls: Vec<CallRequest>,
    encryption: AsymmetricEncryption,
    next_msg_id: u32
}

impl RendezvousServer {
    pub fn start_server(){
        let poll = Poll::new().unwrap();
        let mut next_token = 0;

        let port = match env::vars().find(|(k, _)| k == "PORT") {
            Some((_, v)) => v.parse::<i32>().unwrap(),
            None => 42069
        };
        println!("Starting server with PORT: {}", port);

        let ip = &format!("0.0.0.0:{}", port);

        let mut tcp_listener = TcpListener::bind(SocketAddr::from_str(ip).unwrap()).unwrap();
        poll.registry().register(&mut tcp_listener, Token(next_token), Interest::READABLE).unwrap();
        next_token += 1;
        
        let mut udp_listener = UdpSocket::bind(SocketAddr::from_str(ip).unwrap()).unwrap();
        poll.registry().register(&mut udp_listener, Token(next_token), Interest::READABLE).unwrap();
        next_token += 1;
        
        let encryption = AsymmetricEncryption::new();
        
        let mut s = RendezvousServer {
            poll,
            next_token,
            tcp_listener,
            udp_listener,
            addresses: HashMap::new(),
            tcp_connections: HashMap::new(),
            sym_keys: HashMap::new(),
            peers: Vec::new(),
            calls: Vec::new(),
            encryption,
            next_msg_id: 0,
        };
        s.event_loop();
    }

    fn on_disconnect(&mut self, addr: SocketAddr, token: Token) {
        println!("Peer ({}) disconnected", addr);

        // Notify other clients
        let sock = self.tcp_connections.get(&token).unwrap().peer_addr().unwrap();
        let peer = self.peers.iter_mut().find(|x| x.addr.unwrap() == addr);
        match peer {
            Some(peer) => {
                let p_key = peer.public_key.clone();
                for c in self.tcp_connections.values_mut() {
                    if c.peer_addr().unwrap() != sock // Only broadcast to other clients
                    {
                        RendezvousServer::send_tcp_message(c, MsgType::Disconnect, &msg_types::Disconnect{public_key: p_key.clone()});
                    }
                }
                self.peers.iter()
                .position(|p| p.addr.unwrap() == addr)
                .map(|i| self.peers.remove(i));
            }
            None => {} // The peer wasn't announced
        }
        // Remove from database
        self.addresses.remove(&addr);
        self.tcp_connections.remove(&token);
    }
    
    
}

/*
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
 */