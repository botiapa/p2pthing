use std::io::Stdout;

use crossterm::event::{Event, KeyCode};
use p2pthing_common::encryption::NetworkedPublicKey;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::{Popup, PopupReturn};

pub struct CallPopup {
    peer: NetworkedPublicKey,
    text: String,
    selected_button: SelectedButton,
}

#[derive(PartialEq)]
enum SelectedButton {
    Yes,
    No,
}

impl Popup for CallPopup {
    fn draw(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let popup_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(50), Constraint::Percentage(25)])
            .split(area);
        let popup_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(40), Constraint::Percentage(35)])
            .split(popup_area[1]);

        let container = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title("Popup");
        f.render_widget(Clear, popup_area[1]);
        f.render_widget(container, popup_area[1]);

        let inside = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)])
            .split(popup_area[1].inner(&Margin { vertical: 2, horizontal: 2 }));

        let label = Paragraph::new(self.text.clone()).alignment(Alignment::Center).wrap(Wrap { trim: true });
        f.render_widget(label, inside[0].inner(&Margin { vertical: 3, horizontal: 0 }));

        let button_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inside[1].inner(&Margin { vertical: 0, horizontal: 0 }));

        let yes_button =
            Paragraph::new("(Y)es").alignment(Alignment::Center).style(self.get_button_style(SelectedButton::Yes));
        f.render_widget(yes_button, button_area[0]);

        let yes_button =
            Paragraph::new("(N)o").alignment(Alignment::Center).style(self.get_button_style(SelectedButton::No));
        f.render_widget(yes_button, button_area[1]);
    }

    fn handle_event(&mut self, e: &Event) -> Option<PopupReturn> {
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Left => {
                        self.selected_button = match &mut self.selected_button {
                            SelectedButton::Yes => SelectedButton::No,
                            SelectedButton::No => SelectedButton::Yes,
                        }
                    }
                    KeyCode::Right => {
                        self.selected_button = match &mut self.selected_button {
                            SelectedButton::Yes => SelectedButton::No,
                            SelectedButton::No => SelectedButton::Yes,
                        }
                    }
                    KeyCode::Enter => match &mut self.selected_button {
                        SelectedButton::Yes => return Some(PopupReturn::AcceptCall(self.peer.clone())),
                        SelectedButton::No => return Some(PopupReturn::DenyCall(self.peer.clone())),
                    },
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        return Some(PopupReturn::AcceptCall(self.peer.clone()));
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        return Some(PopupReturn::DenyCall(self.peer.clone()));
                    }
                    _ => {}
                };
            }
            _ => {}
        };
        None
    }
}

impl CallPopup {
    pub fn new(peer: NetworkedPublicKey) -> Self {
        CallPopup {
            text: format!("Do you want to accept the call coming from ({})?", peer),
            peer,
            selected_button: SelectedButton::Yes,
        }
    }

    fn get_button_style(&mut self, btn: SelectedButton) -> Style {
        match self.selected_button == btn {
            true => Style::default().fg(Color::Yellow),
            false => Style::default(),
        }
    }
}
