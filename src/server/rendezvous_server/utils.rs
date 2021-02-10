use std::{io::Write, net::SocketAddr};

use mio::net::TcpStream;
use serde::Serialize;

use crate::common::message_type::MsgType;

use super::RendezvousServer;

impl RendezvousServer {
    pub fn send_tcp_message<T: ?Sized>(sock: &mut TcpStream, t: MsgType, msg: &T) where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let msg_size = bincode::serialize(&msg.len()).unwrap();
        let chained: &[u8] = &[&[t], &msg_size[..], &msg].concat()[..];

        sock.write_all(chained).unwrap();
    }

    pub fn send_udp_message<T: ?Sized>(&self, addr: SocketAddr, t: MsgType, msg: &T) where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        self.udp_listener.send_to(chained, addr).unwrap();
    }
}