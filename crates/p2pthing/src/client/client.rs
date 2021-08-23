use super::connection_manager::ConnectionManager;
use p2pthing_common::ui::{UI, UIType};
use p2pthing_gui::gui::Gui;
use p2pthing_tui::tui::Tui;


pub fn start_client(ip: String, ui_type: UIType) {
    let mut ui = match ui_type {
        UIType::TUI => Box::new(Tui::new()) as Box<dyn UI>,
        UIType::GUI => Box::new(Gui::new()) as Box<dyn UI>,
    };

    let (cm_s, cm_thr, own_public_key) = ConnectionManager::start(ip, ui.get_notifier());
    
    ui.main_loop(cm_s.clone(), own_public_key);
    

    // If the ui interface exited, then signal the connection manager to stop as well

    ConnectionManager::quit(&cm_s);

    cm_thr.join().unwrap();
}