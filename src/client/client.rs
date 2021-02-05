use std::sync::mpsc;

use super::{connection_manager::{ConnectionManager}, tui::Tui};

pub fn start_client() {
    let (ui_s, ui_r) = mpsc::channel();

    let (cm_s, cm_thr, waker) = ConnectionManager::start("127.0.0.1:42069", ui_s.clone());

    let wake_for_tui = waker.clone();
    
    let mut tui = Tui::new(cm_s.clone(), ui_r, wake_for_tui);
    tui.main_loop();

    // If the gui interface exited, then signal the connection manager to stop as well

    ConnectionManager::quit(&cm_s, &waker);

    cm_thr.join().unwrap();
}