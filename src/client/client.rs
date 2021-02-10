use super::{connection_manager::{ConnectionManager}, tui::Tui};

pub fn start_client() {
    

    let mut tui = Tui::new();

    let (cm_s, cm_thr, waker, own_public_key) = ConnectionManager::start("127.0.0.1:42069", tui.get_notifier());

    let cm_waker = waker.clone();
    
    
    tui.main_loop(cm_s.clone(), own_public_key);

    // If the gui interface exited, then signal the connection manager to stop as well

    ConnectionManager::quit(&cm_s, &waker);

    cm_thr.join().unwrap();
}