use std::{io::Stdout, time::Duration};

use tui::{Frame, backend::CrosstermBackend, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, symbols::DOT, text::{Span, Spans, Text}, widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap}};

use crate::common::message_type::Peer;

use super::{ActiveBlock, CHOOSABLE_KBITS, CallStatus, Tui};

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

    pub fn tab_layout(&mut self, area: Rect) -> Vec<Rect> {
        Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([
            Constraint::Percentage(95),
            Constraint::Percentage(5)
        ].as_ref())
        .split(area)
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

    pub fn settings_layout(&mut self, main_area: Rect) -> Vec<Rect> {
        Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50)
        ].as_ref())
        .split(main_area)
    }

    pub fn setting_audio_options(&mut self, area: Rect) -> Vec<Rect> {
        Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
        ].as_ref())
        .split(area)
    }

    pub fn settings_input_list(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        match self.settings_inputs.as_ref() {
            Some(devices) => {
                let input_list = List::new(devices.iter().map(|d| ListItem::new(d.clone())).collect::<Vec<ListItem>>())
                .block(Block::default().title("Input devices").borders(Borders::ALL)
                .border_style(Style::default().fg(self.get_fg_color(ActiveBlock::InputList))))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol(">");
                f.render_stateful_widget(input_list, area, &mut self.settings_inputs_state);
            }
            None => {
                let label = Paragraph::new("No input devices found")
                .style(Style::default().fg(Color::Red));
                f.render_widget(label, area);
            }
        }
    }

    pub fn settings_output_list(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        match self.settings_outputs.as_ref() {
            Some(devices) => {
                let output_list = List::new(devices.iter().map(|d| ListItem::new(d.clone())).collect::<Vec<ListItem>>())
                .block(Block::default().title("Output devices").borders(Borders::ALL)
                .border_style(Style::default().fg(self.get_fg_color(ActiveBlock::OutputList))))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol(">");
                f.render_stateful_widget(output_list, area, &mut self.settings_outputs_state);
            }
            None => {
                let label = Paragraph::new("No output devices found")
                .style(Style::default().fg(Color::Red));
                f.render_widget(label, area);
            }
        }
    }

    pub fn settings_kbits_list(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let kbits_list = List::new(CHOOSABLE_KBITS.iter().map(|n| ListItem::new(format!("{} kbit/s", n))).collect::<Vec<ListItem>>())
        .block(Block::default().title("Preferred voice bitrate").borders(Borders::ALL)
        .border_style(Style::default().fg(self.get_fg_color(ActiveBlock::BitRateList))))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">");
        f.render_stateful_widget(kbits_list, area, &mut self.settings_kbits_state);
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

    pub fn status_icons(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let mute_icon = match self.muted{
            true => Span::styled("M", Style::default().fg(Color::Red)),
            false => Span::styled("R", Style::default().fg(Color::Green))
        };
        let icons = Paragraph::new(mute_icon).block(Block::default().borders(Borders::ALL));
        f.render_widget(icons, area);
    }

    pub fn contact_list(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let contact_list = List::new(self.peers.iter().map(|p| ListItem::new(p.get_public_key().to_string().clone())).collect::<Vec<ListItem>>())
        .block(Block::default().title("Contacts").borders(Borders::ALL)
        .border_style(Style::default().fg(self.get_fg_color(ActiveBlock::ContactList))))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">");
        f.render_stateful_widget(contact_list, area, &mut self.contact_list_state);
    }

    /// This screen contains Peer information, the chat, and chat input in a vertical layout
    pub fn main_screen(&mut self, area: Rect) -> Vec<Rect> {
        Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(if self.debug_visible {6} else {0}),
            Constraint::Max(999),
            Constraint::Length(3),
        ].as_ref())
        .split(area)
    }

    pub fn peer_stats(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let selected_contact = self.contact_list_state.selected().unwrap();
        let p = self.peers.get(selected_contact).unwrap();

        let layout = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([
            Constraint::Length(50),
            Constraint::Max(999)
        ].as_ref())
        .split(area);

        let mut spans: Vec<Spans> = vec![];
        spans.push(Spans::from(Span::from(format!("{}\n", p.get_public_key().to_string()))));
        if let Some((_, stats)) = self.conn_stats.iter().find(|(p1, _)| p1 == p.get_public_key()) {
            spans.push(Spans::from(vec![
                Span::from("Sent: "),
                Span::styled(format!("{} bytes\n", stats.get_total_sent().to_string()), Style::default().add_modifier(Modifier::BOLD))
            ]));
            spans.push(Spans::from(vec![
                Span::from("Received: "),
                Span::styled(format!("{} bytes\n", stats.get_total_received().to_string()), Style::default().add_modifier(Modifier::BOLD))
            ]));
            spans.push(Spans::from(vec![
                Span::from("Sent average: "),
                Span::styled(format!("{} bytes/s\n", stats.get_avg_sent(Duration::from_secs(5)).to_string()), Style::default().add_modifier(Modifier::BOLD))
            ]));
            spans.push(Spans::from(vec![
                Span::from("Received average: "),
                Span::styled(format!("{} bytes/s\n", stats.get_avg_received(Duration::from_secs(5)).to_string()), Style::default().add_modifier(Modifier::BOLD))
            ]));
            spans.push(Spans::from(vec![
                Span::from("Last ping: "),
                Span::styled(format!("{} ms\n", stats.get_last_ping().unwrap_or(&Duration::from_secs(0)).as_millis()), Style::default().add_modifier(Modifier::BOLD))
            ]));
        }
        let stats_paragraph = Paragraph::new(spans).wrap(Wrap{ trim: false});
        f.render_widget(stats_paragraph, layout[0]);

        //TODO
        /*let spark_line = Sparkline::default()
        .data(&[0, 2, 3, 4, 1, 4, 10, 5, 4, 02,3, 2,5, 5, 5,4, 8,3, 5, 5,4])
        .style(Style::default().fg(Color::White));*/

        //f.render_widget(spark_line, layout[1]);
    }

    pub fn chat_messages(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect) {
        let selected_contact = self.contact_list_state.selected().unwrap();
        let p = self.peers.get(selected_contact).unwrap();

        let mut chat_items: Vec<ListItem> = Vec::new();
        let mut last_author: Option<Peer> = None;
        for m in &p.chat_messages {
            match &last_author {
                Some(last_author) if last_author == &m.author => {},
                _ => chat_items.push(ListItem::new(format!("{}: \n", m.author.public_key)).style(Style::default().add_modifier(Modifier::BOLD)))
            }
            last_author = Some(m.author.clone());

            let msg = &m.msg.clone();
            let lines = textwrap::wrap(msg, area.width as usize);
            for line in lines {
                chat_items.push(ListItem::new(format!("{}\n", line)).style(match m.received {
                    Some(false) => Style::default().fg(Color::DarkGray),
                    _ => Style::default()
                }));
            }
        }
        self.chat_messages_length = chat_items.len();

        let public_key = p.get_public_key().to_string();
        let title_string = match self.chat_messages_list_state {
            Some(i) => format!("{} - ({})", public_key, i), 
            None => public_key.clone()
        };
        let title = match self.calls.iter().find(|c| &c.public_key == p.get_public_key()) {
            Some(c) => match c.status {
                CallStatus::PunchThroughSuccessfull => Span::styled(title_string, Style::default().fg(Color::Green)),
                CallStatus::PunchThroughInProgress=> Span::styled(public_key, Style::default().fg(Color::Blue)),
                CallStatus::SentRequest => Span::styled(public_key, Style::default().fg(Color::Yellow)),
                CallStatus::RequestFailed => Span::styled(public_key, Style::default().fg(Color::Red)),
            }
            None => Span::styled(public_key, Style::default().fg(Color::DarkGray))
        };

        let chat_messages = List::new(chat_items)
        .block(Block::default().title(title).borders(Borders::ALL)
        .border_style(Style::default().fg(self.get_fg_color(ActiveBlock::ChatMessages))));

        let mut list_state = ListState::default();
        list_state.select(match self.chat_messages_list_state {
            Some(i) => Some(i),
            None if self.chat_messages_length > 0 => Some(self.chat_messages_length - 1),
            _ => None
        });

        f.render_stateful_widget(chat_messages, area, &mut list_state);
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