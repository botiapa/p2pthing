use std::{io::Read, };

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