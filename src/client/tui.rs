use std::{io::{stdout}, panic, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc::{Receiver, Sender}}, time::Duration};
use std::io::Write;

use crossterm::{ QueueableCommand, event::{EnableMouseCapture, Event, KeyCode, KeyModifiers, poll, read}, execute, terminal::{EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode}};
use mio::Waker;
use spin_sleep::LoopHelper;
use tui::{Terminal, backend::{CrosstermBackend}, layout::{Constraint, Direction, Layout}, style::{Color, Modifier, Style}, symbols::DOT, text::Spans, widgets::{Block, BorderType, Borders, List, ListItem, ListState, Tabs}};

use crate::common::message_type::Peer;

use super::connection_manager::{ConnectionManager, InterthreadMessage};
use super::{connection_manager::{InterthreadMessage::*}};

pub struct Tui {
    cm_s: Sender<InterthreadMessage>,
    ui_r: Receiver<InterthreadMessage>,
    waker: Arc<Waker>,
    peers: Vec<Peer>,
    contact_list_state: ListState
}


impl Tui {
    pub fn new(cm_s: Sender<InterthreadMessage>, ui_r: Receiver<InterthreadMessage>, waker: Arc<Waker>) -> Tui {
    
        Tui {
            cm_s: cm_s,
            ui_r,
            waker,
            peers: vec![],
            contact_list_state: ListState::default()
        }
    }

    pub fn main_loop(&mut self) {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        let r1 = running.clone();

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
            println!("Set ctrl+c handler");
        }).unwrap();
 
        let err = Arc::new(Mutex::new(String::from("")));
        {
            let err = Arc::clone(&err);
            panic::set_hook(Box::new(move |p| {
                println!("Error: {}", p.to_string());
                let mut err = err.lock().unwrap();
                *err = p.to_string();
                r1.store(false, Ordering::SeqCst);
            }));
        }
        

        enable_raw_mode().unwrap();
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
        
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.clear().unwrap();

        let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.5) 
        .build_with_target_rate(5.0);

        while running.load(Ordering::SeqCst) {
            loop_helper.loop_start();

            // Handle interthread messaging
            for msg in self.ui_r.try_iter() {
                match msg {
                    AnnounceResponse(msg) => self.peers = msg,
                    PeerDisconnected(p_key) => {
                        let pos = self.peers.iter().position(|x| x.public_key == p_key).unwrap();
                        self.peers.remove(pos);
                    },
                    Call(p) => {
                        panic!("Received call from peer: {}", p.public_key);
                    }
                    _ => unreachable!()
                }
            }
            
            // Handle keyboard and mouse events
            loop {
                if poll(Duration::from_secs(0)).unwrap() {
                    match read() {
                        Ok(Event::Key(e)) => {
                            match e.code {                               
                                KeyCode::Backspace => {}
                                KeyCode::Enter => {
                                    if self.peers.len() == 0 {continue;}
                                    match self.contact_list_state.selected() {
                                        Some(i) => {
                                            ConnectionManager::call(&self.cm_s, &self.waker, self.peers.get(i).unwrap().clone());
                                        }
                                        None => continue
                                    }
                                }
                                KeyCode::Left => {}
                                KeyCode::Right => {}
                                KeyCode::Up => {
                                    if self.peers.len() == 0 {continue;}
                                    match self.contact_list_state.selected() {
                                        None => self.contact_list_state.select(Some(0)),
                                        Some(i) => {
                                            if i == 0 {
                                                self.contact_list_state.select(Some(self.peers.len() - 1))
                                            }
                                            else {
                                                self.contact_list_state.select(Some(i-1));
                                            }
                                        }
                                    }
                                }
                                KeyCode::Down => {
                                    if self.peers.len() == 0 {continue;}
                                    match self.contact_list_state.selected() {
                                        None => self.contact_list_state.select(Some(0)),
                                        Some(i) => {
                                            if i == self.peers.len() - 1 {
                                                self.contact_list_state.select(Some(0))
                                            }
                                            else {
                                                self.contact_list_state.select(Some(i+1));
                                            }
                                        }
                                    }
                                }
                                KeyCode::Tab => {} // Todo advance tabs
                                KeyCode::BackTab => {} // Todo deadvance tabs
                                KeyCode::Delete => {}
                                KeyCode::Insert => {}
                                KeyCode::F(_) => {}
                                KeyCode::Char(c) => {
                                }
                                KeyCode::Null => {}
                                KeyCode::Esc => {}
                                _ => {}
                            }
                            if e.code == KeyCode::Char('q') || (e.code == KeyCode::Char('c') && e.modifiers == KeyModifiers::CONTROL) {
                                running.store(false, Ordering::SeqCst);
                            }
                        },
                        Ok(Event::Mouse(e)) => {

                        },
                        Ok(Event::Resize(x,y)) => {
                            
                        },
                        Err(err) => println!("Error while reading events: {}", err.to_string()),
                        _ => unimplemented!()
                    }
                }
                else {
                    break;
                }
            }

            // Tries to avoid a crash which happens if a terminal's height is too low
            if terminal.size().unwrap().height <= 1 {
                continue;
            }

            // Render the final image
            terminal.draw(|f| {
                let size = f.size();

                let tab_divider = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(2)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Percentage(100)
                    ].as_ref())
                    .split(f.size());

                let main_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .margin(0)
                    .constraints([
                        Constraint::Percentage(15),
                        Constraint::Percentage(85)
                    ].as_ref())
                    .split(tab_divider[1]);

                let titles = ["Main", "Debug"].iter().cloned().map(Spans::from).collect();
                let tabs = Tabs::new(titles)
                    .block(Block::default().title("Tabs").borders(Borders::ALL))
                    .style(Style::default().fg(Color::White))
                    .highlight_style(Style::default().fg(Color::Yellow))
                    .divider(DOT)
                    .select(0);
                f.render_widget(tabs, tab_divider[0]);
                
                let contact_list = List::new(self.peers.to_vec().iter().map(|p| ListItem::new(p.public_key.to_string().clone())).collect::<Vec<ListItem>>())
                    .block(Block::default().title("Contacts").borders(Borders::ALL))
                    .style(Style::default().fg(Color::White))
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                    .highlight_symbol(">");
                f.render_stateful_widget(contact_list, main_layout[0], &mut self.contact_list_state);

                let main_screen = Block::default()
                    .title("Main screen")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray));
                f.render_widget(main_screen, main_layout[1]);
            }).unwrap();
            loop_helper.loop_sleep();
        }
        terminal.backend_mut().queue(LeaveAlternateScreen).unwrap();
        terminal.clear().unwrap();

        let err = err.lock().unwrap();
        if !err.is_empty() {
            println!("Error occured: {}", err);
        }
    }
}

