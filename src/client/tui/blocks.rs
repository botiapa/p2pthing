use std::{io::Stdout};

use tui::{Frame, backend::CrosstermBackend, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, symbols::DOT, text::{Span, Spans, Text}, widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Tabs, Wrap}};

use crate::common::message_type::Peer;

use super::{ActiveBlock, CallStatus, Tui};

impl Tui{
    fn get_fg_color(&mut self, block: ActiveBlock) -> Color {
        if block == self.active_block {
            match self.is_active {
                true => Color::LightBlue,
                false => Color::Yellow
            }
        }
        else {
            Color::White
        }
    }

    pub fn tab_divider(&mut self, screen: Rect) -> Vec<Rect> {
        Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Percentage(100)
        ].as_ref())
        .split(screen)
    }

    pub fn main_layout(&mut self, main_area: Rect) -> Vec<Rect> {
        Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(85)
        ].as_ref())
        .split(main_area)
    }

    pub fn tabs(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let titles = self.tab_titles.iter().cloned().map(Spans::from).collect();
        let tabs = Tabs::new(titles)
        .block(Block::default().title("Tabs").borders(Borders::ALL)
        .border_style(Style::default().fg(self.get_fg_color(ActiveBlock::Tabs))))
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider(DOT)
        .select(self.selected_tab);
        f.render_widget(tabs, area);
    }

    pub fn contact_list(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let contact_list = List::new(self.peers.iter().map(|p| ListItem::new(p.get_public_key().to_string().clone())).collect::<Vec<ListItem>>())
        .block(Block::default().title("Contacts").borders(Borders::ALL)
        .border_style(Style::default().fg(self.get_fg_color(ActiveBlock::ContactList))))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">");
        f.render_stateful_widget(contact_list, area, &mut self.contact_list_state);
    }

    pub fn main_screen(&mut self, area: Rect) -> Vec<Rect> {
        Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Max(9999),
            Constraint::Length(3)
        ].as_ref())
        .split(area)
    }

    pub fn chat_messages(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let selected_contact = self.contact_list_state.selected().unwrap();
        let p = self.peers.get(selected_contact).unwrap();

        let mut spans = Vec::new();
        let mut last_author: Option<Peer> = None;
        for m in &p.chat_messages {
            match &last_author {
                Some(last_author) if last_author == &m.author => {},
                _ => spans.push(Spans::from(Span::styled(format!("{}: \n", m.author.public_key), Style::default().add_modifier(Modifier::BOLD))))
            }
            last_author = Some(m.author.clone());
            spans.push(Spans::from(Span::styled(format!("{}\n", m.msg.clone()), Style::default())));
        }

        let public_key = p.get_public_key().to_string();
        let title = match self.calls.iter().find(|c| &c.public_key == p.get_public_key()) {
            Some(c) => match c.status {
                CallStatus::PunchThroughSuccessfull => Span::styled(public_key, Style::default().fg(Color::Green)),
                CallStatus::PunchThroughInProgress=> Span::styled(public_key, Style::default().fg(Color::Blue)),
                CallStatus::SentRequest => Span::styled(public_key, Style::default().fg(Color::Yellow)),
                CallStatus::RequestFailed => Span::styled(public_key, Style::default().fg(Color::Red)),
            }
            None => Span::styled(public_key, Style::default().fg(Color::DarkGray))
        };

        let chat_messages = Paragraph::new(spans)
        .block(Block::default().title(title).borders(Borders::ALL)
        .border_style(Style::default().fg(self.get_fg_color(ActiveBlock::ChatMessages))))
        .wrap(Wrap { trim: false});
        f.render_widget(chat_messages, area);
    }

    pub fn chat_input(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let selected_contact = self.contact_list_state.selected().unwrap();
        let selected_contact = self.peers.get(selected_contact).unwrap().get_public_key().to_string();
        
        let input = &self.peers.get(self.contact_list_state.selected().unwrap()).unwrap().chat_input;
        let input_string = input.get_string();
        let text = match input_string.is_empty() {
            true => Span::styled(format!("Send message to {}", selected_contact), Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)),
            false => Span::styled(input_string.clone(), Style::default()),
        };

        if !input_string.is_empty() && self.is_active && self.active_block == ActiveBlock::ChatInput {
            f.set_cursor(area.x + input.get_cursor_pos() as u16 + 1, area.y + 1); //FIXME: Add support for multiline
        }

        let chat_input = Paragraph::new(text)
        .style(Style::default().fg(self.get_fg_color(ActiveBlock::ChatInput)))
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded))
        .wrap(Wrap { trim: false});
        f.render_widget(chat_input, area);
    }

    pub fn debug_messages(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let debug_message_list = List::new(self.debug_messages.iter()
        .map(|p| ListItem::new(Text::from(p.to_string()))
        .style(match p.msg_type {
            super::DebugMessageType::Log => Style::default().fg(Color::White),
            super::DebugMessageType::Warning => Style::default().fg(Color::Yellow),
            super::DebugMessageType::Error => Style::default().fg(Color::Red)
        }))
        .collect::<Vec<ListItem>>())
        .block(Block::default().title("Debug Messages").borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">");
        f.render_stateful_widget(debug_message_list, area, &mut self.debug_messages_state);
    }
}