use std::net::SocketAddr;

mod audio_handler;
mod chat_handler;
mod connection_handler;
mod file_handler;
mod multicast_handler;

pub(crate) trait AudioHandler {
    #[cfg(feature = "audio")]
    fn on_opus_packet(&mut self, addr: SocketAddr, data: &[u8]);
}

pub(crate) trait ChatHandler {
    fn on_chat_message(&mut self, addr: SocketAddr, data: &[u8]);
}

pub(crate) trait ConnectionHandler {
    fn on_udp_announce(&mut self, addr: SocketAddr);
    fn on_keep_alive(&mut self, addr: SocketAddr);
    fn on_secret_announce(&mut self, addr: SocketAddr, data: &[u8]);
    fn check_punchthrough(&mut self, addr: SocketAddr);
    fn on_confirmation_message(&mut self, addr: SocketAddr, data: &[u8]);
}

pub(crate) trait FileHandler {
    fn on_request_file_chunks(&mut self, addr: SocketAddr, data: &[u8]);
    fn on_file_chunks(&mut self, addr: SocketAddr, data: &[u8]);
}

pub(crate) trait MulticastHandler {
    fn read_multicast_message(&mut self, _: usize, addr: SocketAddr, buf: &[u8]);
}
