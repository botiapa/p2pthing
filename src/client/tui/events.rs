use core::panic;
use std::{sync::{atomic::Ordering, mpsc::TryRecvError}, thread};

use ActiveBlock::{ChatInput, ChatMessages, Tabs};
use chrono::Utc;
use crossterm::event::{Event, KeyCode, KeyModifiers, read};

use super::{ActiveBlock::{self, ContactList}, CallStatus, CallStatusHolder, DebugMessageType, Tui, ui_peer::{ChatMessage, UIPeer}};
use super::super::connection_manager::ConnectionManager;
use crate::common::message_type::{InterthreadMessage::*, MsgType, Peer};

impl Tui {

    pub fn handle_interthread_events(&mut self) {
        for msg in self.ui_r.try_iter() {
            match msg {
                AnnounceResponse(msg) => {
                    self.peers = msg.iter().map(UIPeer::from).collect();
                    match self.contact_list_state.selected() {
                        None if self.peers.len() > 0 => self.contact_list_state.select(Some(0)),
                        _ => {}
                    }
                }
                PeerDisconnected(p_key) => {
                    let pos = self.peers.iter().position(|x| *x.get_public_key() == p_key).unwrap();
                    self.peers.remove(pos);
                    match self.contact_list_state.selected() {
                        Some(i) if i == pos => self.contact_list_state.select(None),
                        Some(i) if i > pos => self.contact_list_state.select(Some(i-1)),
                        None | Some(_) => {}
                    }
                    match self.active_block {
                        ChatInput | ChatMessages => self.active_block = ContactList,
                        _ => {}
                    }
                },
                Call(public_key) | CallAccepted(public_key) => {
                    match self.calls.iter_mut().find(|c| c.public_key == public_key) {
                        Some(call) => call.status = CallStatus::PunchThroughInProgress,
                        None => self.calls.push(CallStatusHolder{
                            status: CallStatus::PunchThroughInProgress, 
                            public_key: public_key.clone()
                        })
                    }
                },
                CallDenied(public_key) => {
                    match self.calls.iter_mut().find(|c| c.public_key == public_key) {
                        Some(call) => call.status = CallStatus::RequestFailed,
                        None => unreachable!()
                    }
                },
                PunchThroughSuccessfull(public_key) => {
                    match self.calls.iter_mut().find(|c| c.public_key == public_key) {
                        Some(call) => call.status = CallStatus::PunchThroughSuccessfull,
                        None => unreachable!()
                    }
                }
                // FIXME: Duplicated code
                DebugMessage(msg, msg_type) => {
                    self.debug_messages.push(super::DebugMessage {
                        message: msg,
                        time: Utc::now(),
                        msg_type,
                    });
                },
                OnChatMessage(p, msg) => {
                    let ui_peer = self.peers.iter_mut().find(|peer| peer.get_public_key() == &p.public_key).unwrap();
                    ui_peer.chat_messages.push(ChatMessage {
                        author: p,
                        msg
                    });
                }
                _ => unreachable!()
            }
        }
    }

    pub fn log_message(&mut self, msg: String, msg_type: DebugMessageType) {
        self.debug_messages.push(super::DebugMessage {
            message: msg,
            time: Utc::now(),
            msg_type,
        });
    }

    /// Start thread that will pool for keyboard and mouse events
    pub fn receive_keyboard_mouse_events(&mut self) {
        let notifier = self.event_s.clone();
        thread::spawn(move || {
            loop {
                notifier.send(read()).unwrap();
            }
        });
    }

    fn send_chat_message(&mut self) {
        let i = self.contact_list_state.selected().unwrap();
        let peer = self.peers.get_mut(i).unwrap();
        if !peer.chat_input.get_string().is_empty() {
            ConnectionManager::send_chat_message(&self.cm_s.as_mut().unwrap(), peer.get_public_key().clone(), peer.chat_input.get_string());
            peer.chat_messages.push(ChatMessage {
                author: Peer {
                    addr: None,
                    udp_addr: None,
                    sym_key: None,
                    public_key: self.own_public_key.clone().unwrap(),
                }, //FIXME
                msg: peer.chat_input.get_string(),
            });
            peer.chat_input.clear();
        }
    }

    pub fn handle_keyboard_mouse_events(&mut self) {
        loop {
            match self.event_r.try_recv() {
                Ok(e) => {
                    match e {
                        Ok(e) => {
                            match self.active_block {
                                ContactList => self.handle_contact_list_event(e),
                                ChatMessages => self.handle_chat_messages_event(e),
                                ChatInput => self.handle_chat_input_event(e),
                                Tabs => self.handle_tabs_input_event(e)
                            }
                            self.handle_global_event(e);
                        }
                        Err(err) => self.log_message(format!("Error while reading events: {}", err.to_string()), DebugMessageType::Error)
                    }
                }
                Err(e) => {
                    match e {
                        TryRecvError::Empty => break,
                        TryRecvError::Disconnected => unreachable!()
                    }
                }
                
            }
        }
    }

    fn handle_global_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Enter => {
                        self.is_active = true
                    }
                    KeyCode::Esc => {
                        self.is_active = false
                    }
                    KeyCode::Tab => {
                        if self.selected_tab == self.tab_titles.len() - 1 {self.selected_tab = 0} else {self.selected_tab += 1;}
                    }
                    KeyCode::BackTab => {
                        if self.selected_tab == 0 {self.selected_tab = self.tab_titles.len() - 1} else {self.selected_tab -= 1;}
                    }
                    KeyCode::F(x) => {
                        match x {
                            x if (x as usize) < self.tab_titles.len() + 1 => self.selected_tab = (x - 1).into(),
                            _ => {}
                        }
                    }
                    _ => {}
                }
                if e.code == KeyCode::Char('q') || (e.code == KeyCode::Char('c') && e.modifiers == KeyModifiers::CONTROL) {
                    self.running.store(false, Ordering::SeqCst);
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }

    fn handle_contact_list_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Enter if self.is_active => {
                        if self.peers.len() == 0 {return;}
                        match self.contact_list_state.selected() {
                            Some(i) => {
                                let p = self.peers.get(i).unwrap().get_public_key();
                                self.calls.push(CallStatusHolder{
                                    status: CallStatus::SentRequest,
                                    public_key: p.clone()
                                });
                                ConnectionManager::call(&self.cm_s.as_mut().unwrap(), p.clone());
                            }
                            None => {}
                        }
                    }
                    KeyCode::Up if self.is_active => {
                        if self.peers.len() == 0 {return;}
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
                    KeyCode::Down if self.is_active => {
                        if self.peers.len() == 0 {return;}
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
                    KeyCode::Up => self.active_block = Tabs,
                    KeyCode::Right if !self.is_active && self.contact_list_state.selected().is_some() => self.active_block = ChatMessages,
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }

    fn handle_chat_messages_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Up if self.is_active => {
                        // TODO: Scroll
                    }
                    KeyCode::Down if self.is_active => {
                        // TODO: Scroll
                    }
                    KeyCode::Up => self.active_block = Tabs,
                    KeyCode::Left if !self.is_active => self.active_block = ContactList,
                    KeyCode::Down if !self.is_active => self.active_block = ChatInput,
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }
    
    fn handle_chat_input_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                let input = &mut self.peers.get_mut(self.contact_list_state.selected().unwrap()).unwrap().chat_input;
                match e.code {
                    KeyCode::Char(c) if self.is_active => input.push_char(c),
                    KeyCode::Backspace if self.is_active => input.backspace(),
                    KeyCode::Delete if self.is_active => input.delete(),
                    KeyCode::Left if self.is_active => input.deadvance_cursor(),
                    KeyCode::Right if self.is_active => input.advance_cursor(),
                    KeyCode::Enter if self.is_active => self.send_chat_message(),
                    KeyCode::Left if !self.is_active => self.active_block = ContactList,
                    KeyCode::Up if !self.is_active => self.active_block = ChatMessages,
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }
    
    fn handle_tabs_input_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Enter => {
                        if self.peers.len() == 0 {return;}
                        match self.contact_list_state.selected() {
                            Some(i) => {
                                ConnectionManager::call(&self.cm_s.as_mut().unwrap(), self.peers.get(i).unwrap().get_public_key().clone());
                            }
                            None => {}
                        }
                    }
                    KeyCode::Right if self.is_active => {
                        if self.selected_tab == self.tab_titles.len() - 1 {self.selected_tab = 0} else {self.selected_tab += 1;}
                    }
                    KeyCode::Left if self.is_active => {
                        if self.selected_tab == 0 {self.selected_tab = self.tab_titles.len() - 1} else {self.selected_tab -= 1;}
                    }
                    KeyCode::Down if !self.is_active => self.active_block = ContactList,
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }
}