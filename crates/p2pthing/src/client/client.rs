use super::connection_manager::ConnectionManager;
use p2pthing_common::ui::{UIType, UI};
#[cfg(feature = "gui")]
use p2pthing_gui::gui::Gui;
#[cfg(feature = "tui")]
use p2pthing_tui::tui::Tui;

pub use p2pthing_common::enumset;
pub use p2pthing_common::serde;

pub fn start_client(ip: String, ui_type: UIType) {
    let mut ui = if ui_type == UIType::TUI {
        #[cfg(feature = "tui")]
        {
            Box::new(Tui::new()) as Box<dyn UI>
        }
        #[cfg(not(feature = "tui"))]
        panic!("Tried running as tui, but I've been built without tui support")
    } else {
        #[cfg(feature = "gui")]
        {
            Box::new(Gui::new()) as Box<dyn UI>
        }
        #[cfg(not(feature = "gui"))]
        panic!("Tried running as gui, but I've been built without gui support")
    };

    let (cm_s, cm_thr, own_public_key) = ConnectionManager::start(ip, ui.get_notifier());

    ui.main_loop(cm_s.clone(), own_public_key);

    // If the ui interface exited, then signal the connection manager to stop as well

    ConnectionManager::quit(&cm_s);

    cm_thr.join().unwrap();
}
