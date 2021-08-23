pub mod message_type;
pub mod encryption;
pub mod debug_message;
pub mod ui;
pub mod statistics;

use std::io::Read;

// TODO: Error handling
pub fn read_exact<T>(sock: &mut T, buf: &mut [u8]) where T: Read {
    let mut read = 0;
    while read < buf.len() {
        match sock.read(&mut buf[read..]) {
            Ok(c) => read += c,
            Err(_) => {}
        }
    }
}