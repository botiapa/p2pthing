use super::{connection_manager::{ConnectionManager}, tui::Tui};

pub fn start_client(ip: String) {
    let mut tui = Tui::new();

    let (cm_s, cm_thr, own_public_key) = ConnectionManager::start(ip, tui.get_notifier());
    
    tui.main_loop(cm_s.clone(), own_public_key);

    // If the gui interface exited, then signal the connection manager to stop as well

    ConnectionManager::quit(&cm_s);

    cm_thr.join().unwrap();
}