pub mod debug_message;
pub mod encryption;
pub mod message_type;
pub mod statistics;
pub mod ui;

// Reexport common deps
pub use aes_gcm_siv;
pub use enumset;
pub use mio_misc;
pub use num;
pub use num_derive;
pub use num_traits;
pub use rand;
pub use rand_core;
pub use rsa;
pub use serde;
pub use sha2;
pub use tracing;
#[cfg(feature = "tracing")]
pub use tracing_chrome;
#[cfg(feature = "tracing")]
pub use tracing_subscriber;

use std::io::Read;

// TODO: Error handling
pub fn read_exact<T>(sock: &mut T, buf: &mut [u8])
where
    T: Read,
{
    let mut read = 0;
    while read < buf.len() {
        match sock.read(&mut buf[read..]) {
            Ok(c) => read += c,
            Err(_) => {}
        }
    }
}
