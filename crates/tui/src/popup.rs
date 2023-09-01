use std::io::Stdout;

use crossterm::event::Event;
use p2pthing_common::encryption::NetworkedPublicKey;
use tui::{backend::CrosstermBackend, layout::Rect, Frame};

pub mod call_popup;

pub enum PopupReturn {
    AcceptCall(NetworkedPublicKey),
    DenyCall(NetworkedPublicKey),
}

pub trait Popup {
    fn draw(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect);
    fn handle_event(&mut self, e: Event) -> Option<PopupReturn>;
}
