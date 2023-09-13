use crate::client::connection_manager::ConnectionManager;

use super::AudioHandler;

impl AudioHandler for ConnectionManager {
    #[cfg(feature = "audio")]
    fn on_opus_packet(&mut self, addr: SocketAddr, data: &[u8]) {
        let data: Vec<u8> = bincode::deserialize(data).unwrap();
        let p = self.peers.peer_by_addr(&addr).unwrap();

        self.audio.decode_and_queue_packet(&data[..], p.public_key.as_ref().unwrap().clone());
    }
}
