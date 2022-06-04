use std::{io::Write, net::SocketAddr};

use mio::net::TcpStream;
use p2pthing_common::{message_type::{MsgEncryption, MsgType, UdpPacket}, num, serde::Serialize};

use super::RendezvousServer;

impl RendezvousServer {
    pub fn send_tcp_message<T: ?Sized>(sock: &mut TcpStream, t: MsgType, msg: &T) where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let msg_size = bincode::serialize(&msg.len()).unwrap();
        let chained: &[u8] = &[&[t], &msg_size[..], &msg].concat()[..];

        sock.write_all(chained).unwrap();
    }

    pub fn send_udp_message<T: ?Sized>(&mut self, addr: SocketAddr, t: MsgType, msg: &T) where T: Serialize {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];
        let chained: &[u8] = &[&[t], msg].concat()[..];

        let packet = UdpPacket {
            data: chained.to_vec(),
            reliable: false, //FIXME
            msg_id: self.next_msg_id,
            upgraded: MsgEncryption::Unencrypted
        };
        self.next_msg_id += 1;

        let wrapped_data = &bincode::serialize(&packet).unwrap()[..];
        self.udp_listener.send_to(wrapped_data, addr).unwrap();
    }
}