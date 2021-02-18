use super::{connection_manager::{ConnectionManager}, tui::Tui};

pub fn start_client() {
    let mut tui = Tui::new();

    let (cm_s, cm_thr, own_public_key) = ConnectionManager::start("138.68.69.243:42069".to_string(), tui.get_notifier());
    
    tui.main_loop(cm_s.clone(), own_public_key);

    // If the gui interface exited, then signal the connection manager to stop as well

    ConnectionManager::quit(&cm_s);

    cm_thr.join().unwrap();
}