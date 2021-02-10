use std::{sync::{Arc, mpsc::{Sender, channel}}, thread, time::Duration};

use mio::Waker;

use crate::common::message_type::InterthreadMessage;

use super::{ConnectionManager, KEEP_ALIVE_DELAY};

impl ConnectionManager {
    pub fn set_up_waker_thread(waker: Arc<Waker>) -> Sender<InterthreadMessage> {
        let (s, r) = channel();
        thread::spawn(move || {
            let mut delay = KEEP_ALIVE_DELAY;
            let mut elapsed = 0;
            loop {
                thread::sleep(Duration::from_secs(1));
                elapsed += 1;
                for e in r.try_iter() {
                    match e {
                        InterthreadMessage::SetWakepDelay(n) => delay = n, //TODO: Reset wake up delay if no call requests are currently sent
                        InterthreadMessage::Quit() => return,
                        _ => unreachable!()
                    }
                }
                if elapsed > delay {
                    waker.wake().unwrap();
                    elapsed = 0;
                }
            }
        });
        s
    }
}