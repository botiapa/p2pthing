use std::io::{self, Write};

use p2pthing_common::num;
use p2pthing_common::serde::Serialize;
use p2pthing_common::{encryption::NetworkedPublicKey, message_type::MsgType, ui::UIConn};
use tracing::instrument;

use super::ConnectionManager;

impl ConnectionManager {
    #[instrument(skip(self, msg), fields(msg_type = ?t))]
    pub fn send_tcp_message<T: ?Sized>(&mut self, t: MsgType, msg: &T) -> io::Result<()>
    where
        T: Serialize,
    {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];

        let conn = self.peers.conn(&self.rendezvous_ip).unwrap();

        let encrypted = &conn.symmetric_key.as_ref().unwrap().encrypt(&[&[t], msg].concat()[..])[..];

        let msg_size = bincode::serialize(&encrypted.len()).unwrap();
        let chained: &[u8] = &[&msg_size[..], encrypted].concat()[..];

        self.peers
            .rendezvous_servers_mut()
            .next()
            .as_mut()
            .expect("Rendezvous server not found")
            .tcp_conn
            .as_mut()
            .expect("Rendezvous server does not have TCP connection associated with it")
            .write_all(&chained[..])?;
        Ok(())
    }

    #[instrument(skip(self, msg), fields(msg_type = ?t))]
    pub fn send_tcp_message_public_key<T: ?Sized>(&mut self, t: MsgType, msg: &T) -> io::Result<()>
    where
        T: Serialize,
    {
        let t: u8 = num::ToPrimitive::to_u8(&t).unwrap();
        let msg = &bincode::serialize(msg).unwrap()[..];

        let key = self.rendezvous_public_key.as_ref().unwrap();
        let encrypted = &key.encrypt(&[&[t], msg].concat()[..]);

        let msg_size = bincode::serialize(&encrypted.len()).unwrap();
        let chained: &[u8] = &[&msg_size[..], encrypted].concat()[..];

        self.peers
            .rendezvous_servers_mut()
            .next()
            .as_mut()
            .expect("Rendezvous server not found")
            .tcp_conn
            .as_mut()
            .expect("Rendezvous server does not have TCP connection associated with it")
            .write_all(&chained[..])?;
        Ok(())
    }

    /// Send a UDP packet which optionally can be reliable
    #[instrument(skip(self, msg), fields(msg_type = ?t))]
    pub fn send_udp_message<T: ?Sized>(
        &mut self,
        public_key: Option<NetworkedPublicKey>,
        t: MsgType,
        msg: &T,
        reliable: bool,
        custom_id: bool,
    ) -> Result<(), String>
    where
        T: Serialize,
    {
        let rendezvous_ip = self.rendezvous_ip.clone();

        let conn = match public_key {
            Some(public_key) => match self.peers.peer_mut(&public_key).map(|x| x.udp_conn.as_mut()) {
                Some(conn) => conn.unwrap(),
                None => {
                    self.ui_s.log_error(&format!("Cannot find udp connection with public key: ({})", public_key));
                    return Err("Cannot find udp connection".into());
                }
            },
            None => self.peers.conn_mut(&rendezvous_ip).unwrap(),
        };

        if custom_id {
            conn.send_udp_message(t, msg, reliable, Some(self.next_custom_id))?;
            self.next_custom_id += 1;
        } else {
            conn.send_udp_message(t, msg, reliable, None)?
        }

        Ok(())
    }
}
