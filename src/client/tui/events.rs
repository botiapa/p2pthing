use core::panic;
use std::{sync::{atomic::Ordering, mpsc::TryRecvError}, thread};

use chrono::Utc;
use crossterm::event::{Event, KeyCode, KeyModifiers, read};
use num::FromPrimitive;

use super::{ActiveBlock, CHOOSABLE_KBITS, CallStatus, CallStatusHolder, DebugMessageType, TabIndex, Tui, popup::PopupReturn, ui_peer::{ChatMessage, UIPeer}};
use super::super::connection_manager::ConnectionManager;
use crate::common::message_type::{*, Peer};
use super::popup::call_popup::CallPopup;

impl Tui {
    pub fn handle_interthread_events(&mut self) {
        for msg in self.ui_r.try_iter() {
            match msg {
                InterthreadMessage::AnnounceResponse(msg) => {
                    self.peers = msg.iter().map(UIPeer::from).collect();
                    match self.contact_list_state.selected() {
                        None if self.peers.len() > 0 => self.contact_list_state.select(Some(0)),
                        _ => {}
                    }
                }
                InterthreadMessage::PeerDisconnected(p_key) => {
                    let pos = self.peers.iter().position(|x| *x.get_public_key() == p_key).unwrap();
                    self.peers.remove(pos);
                    match self.contact_list_state.selected() {
                        Some(i) if i == pos => self.contact_list_state.select(None),
                        Some(i) if i > pos => self.contact_list_state.select(Some(i-1)),
                        None | Some(_) => {}
                    }
                    match self.active_block {
                        ActiveBlock::ChatInput | ActiveBlock::ChatMessages => self.active_block = ActiveBlock::ContactList,
                        _ => {}
                    }
                },
                InterthreadMessage::Call(public_key) => {
                    if self.active_popup.is_some() {
                        unimplemented!("Need to display a new popup while one is still displayed");
                    }
                    self.active_popup = Some(Box::new(CallPopup::new(
                        public_key.clone()
                    )));
                    match self.calls.iter_mut().find(|c| c.public_key == public_key) {
                        Some(call) => call.status = CallStatus::SentRequest,
                        None => self.calls.push(CallStatusHolder{
                            status: CallStatus::SentRequest, 
                            public_key: public_key.clone()
                        })
                    }
                }
                InterthreadMessage::CallAccepted(public_key) => {
                    match self.calls.iter_mut().find(|c| c.public_key == public_key) {
                        Some(call) => call.status = CallStatus::PunchThroughInProgress,
                        None => self.calls.push(CallStatusHolder{
                            status: CallStatus::PunchThroughInProgress, 
                            public_key: public_key.clone()
                        })
                    }
                },
                InterthreadMessage::CallDenied(public_key) => {
                    match self.calls.iter_mut().find(|c| c.public_key == public_key) {
                        Some(call) => call.status = CallStatus::RequestFailed,
                        None => unreachable!()
                    }
                },
                InterthreadMessage::PunchThroughSuccessfull(public_key) => {
                    match self.calls.iter_mut().find(|c| c.public_key == public_key) {
                        Some(call) => call.status = CallStatus::PunchThroughSuccessfull,
                        None => unreachable!()
                    }
                }
                // FIXME: Duplicated code
                InterthreadMessage::DebugMessage(msg, msg_type) => {
                    self.debug_messages.push(super::DebugMessage {
                        message: msg,
                        time: Utc::now(),
                        msg_type,
                    });
                    self.debug_messages_state.select(Some(self.debug_messages.len() - 1));
                },
                InterthreadMessage::OnChatMessage(p, msg) => {
                    let ui_peer = self.peers.iter_mut().find(|peer| peer.get_public_key() == &p.public_key).unwrap();
                    ui_peer.chat_messages.push(ChatMessage {
                        author: p,
                        msg,
                        custom_id: None,
                        received: None,
                        own: false
                    });
                },
                InterthreadMessage::OnChatMessageReceived(custom_id) => {
                    for p in &mut self.peers {
                        for msg in &mut p.chat_messages {
                            if msg.own && msg.custom_id.unwrap() == custom_id {
                                msg.received = Some(true);
                                break;
                            }
                        }
                    }
                },
                InterthreadMessage::AudioNewInputDevices(devices) => {
                    self.settings_inputs = devices;
                    self.settings_inputs_state.select(None); //FIXME
                },
                InterthreadMessage::AudioNewOutputDevices(devices) => {
                    self.settings_outputs = devices;
                    self.settings_outputs_state.select(None); //FIXME
                },
                InterthreadMessage::ConnectionStatistics(stats) => {
                    self.conn_stats = stats;
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
            ConnectionManager::send_chat_message(&self.cm_s.as_mut().unwrap(), peer.get_public_key().clone(), peer.chat_input.get_string(), self.next_msg_id);
            peer.chat_messages.push(ChatMessage {
                author: Peer {
                    addr: None,
                    udp_addr: None,
                    sym_key: None,
                    public_key: self.own_public_key.clone().unwrap(),
                },
                msg: peer.chat_input.get_string(),
                custom_id: Some(self.next_msg_id),
                received: Some(false),
                own: true
            });
            self.next_msg_id += 1;
            peer.chat_input.clear();
        }
    }

    fn handle_popup_return_value(&mut self, ret: PopupReturn) {
        match ret {
            PopupReturn::AcceptCall(p) => {
                self.cm_s.as_ref().unwrap().send(InterthreadMessage::CallAccepted(p.clone())).unwrap();
                self.calls.iter_mut().find(|c| c.public_key == p).unwrap().status = CallStatus::PunchThroughInProgress;
            },
            PopupReturn::DenyCall(p) => {
                self.cm_s.as_ref().unwrap().send(InterthreadMessage::CallDenied(p.clone())).unwrap();
                let i = self.calls.iter_mut().position(|c| c.public_key == p).unwrap();
                self.calls.remove(i);
            }
        }
        self.active_popup = None;
    }

    pub fn handle_keyboard_mouse_events(&mut self) {
        loop {
            match self.event_r.try_recv() {
                Ok(e) => {
                    match e {
                        Ok(e) => {
                            match &mut self.active_popup {
                                Some(popup) => {
                                    match popup.handle_event(e) {
                                        Some(ret) => self.handle_popup_return_value(ret),
                                        None => {}
                                    }
                                }
                                None => {
                                    match &self.active_block {
                                        ActiveBlock::ContactList => self.handle_contact_list_event(e),
                                        ActiveBlock::ChatMessages => self.handle_chat_messages_event(e),
                                        ActiveBlock::ChatInput => self.handle_chat_input_event(e),
                                        ActiveBlock::InputList => self.handle_input_list_event(e),
                                        ActiveBlock::OutputList => self.handle_output_list_event(e),
                                        ActiveBlock::BitRateList => self.handle_bitrate_list_event(e),
                                        ActiveBlock::Tabs => self.handle_tabs_input_event(e)
                                    }
                                    self.handle_global_event(e);
                                }
                            }
                            self.handle_quit_event(e);
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

    fn handle_quit_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                if e.code == KeyCode::Char('q') || (e.code == KeyCode::Char('c') && e.modifiers == KeyModifiers::CONTROL) {
                    self.running.store(false, Ordering::SeqCst);
                }
            }
            _ => {}
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
                        self.active_block = ActiveBlock::Tabs;
                        self.is_active = false;
                    }
                    KeyCode::BackTab => {
                        if self.selected_tab == 0 {self.selected_tab = self.tab_titles.len() - 1} else {self.selected_tab -= 1;}
                        self.active_block = ActiveBlock::Tabs;
                        self.is_active = false;
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') if self.active_block != ActiveBlock::ChatInput || (self.active_block == ActiveBlock::ChatInput &&!self.is_active) => {
                        self.muted = !self.muted;
                        self.cm_s.as_ref().unwrap().send(InterthreadMessage::AudioChangeMuteState(self.muted)).unwrap();
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') if self.active_block != ActiveBlock::ChatInput || (self.active_block == ActiveBlock::ChatInput &&!self.is_active) => {
                        self.debug_visible = !self.debug_visible;
                    }
                    KeyCode::F(x) => {
                        match x {
                            x if (x as usize) < self.tab_titles.len() + 1 => {
                                self.selected_tab = (x - 1).into();
                                self.active_block = ActiveBlock::Tabs;
                                self.is_active = false;
                            },
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }

    fn handle_input_list_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Up if self.is_active => {
                        match &self.settings_inputs{
                            Some(inputs) => {
                                match self.settings_inputs_state.selected() {
                                    Some(i) if i > 0 => self.settings_inputs_state.select(Some(i - 1)),
                                    _ => self.chat_messages_list_state = Some(inputs.len() - 1),
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Down if self.is_active => {
                        match &self.settings_inputs{
                            Some(inputs) => {
                                match self.settings_inputs_state.selected() {
                                    Some(i) if i < inputs.len() - 1 => self.settings_inputs_state.select(Some(i + 1)),
                                    _ => self.settings_inputs_state.select(Some(0)),
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Enter if self.is_active => {
                        match &self.settings_inputs {
                            Some(inputs) => {
                                match self.settings_inputs_state.selected() {
                                    Some(selected) => {
                                        let d = inputs.get(selected).unwrap();
                                        self.cm_s.as_ref().unwrap().send(InterthreadMessage::AudioChangeInputDevice(d.clone())).unwrap();
                                    }
                                    None => {}
                                }
                            }
                            _ => {}
                        }
                    },
                    KeyCode::Up => self.active_block = ActiveBlock::Tabs,
                    KeyCode::Right if !self.is_active => self.active_block = ActiveBlock::OutputList,
                    KeyCode::Left if !self.is_active => self.active_block = ActiveBlock::BitRateList,
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }

    fn handle_output_list_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Up if self.is_active => {
                        match &self.settings_inputs{
                            Some(inputs) => {
                                match self.settings_outputs_state.selected() {
                                    Some(i) if i > 0 => self.settings_outputs_state.select(Some(i - 1)),
                                    _ => self.chat_messages_list_state = Some(inputs.len() - 1),
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Down if self.is_active => {
                        match &self.settings_outputs{
                            Some(outputs) => {
                                match self.settings_outputs_state.selected() {
                                    Some(i) if i < outputs.len() - 1 => self.settings_outputs_state.select(Some(i + 1)),
                                    _ => self.settings_outputs_state.select(Some(0)),
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Enter if self.is_active => {
                        match &self.settings_outputs {
                            Some(outputs) => {
                                match self.settings_outputs_state.selected() {
                                    Some(selected) => {
                                        let d = outputs.get(selected).unwrap();
                                        self.cm_s.as_ref().unwrap().send(InterthreadMessage::AudioChangeOutputDevice(d.clone())).unwrap();
                                    }
                                    None => {}
                                }
                            }
                            _ => {}
                        }
                    },
                    KeyCode::Up => self.active_block = ActiveBlock::Tabs,
                    KeyCode::Left if !self.is_active => self.active_block = ActiveBlock::InputList,
                    KeyCode::Right if !self.is_active => self.active_block = ActiveBlock::BitRateList,
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }

    fn handle_bitrate_list_event(&mut self, e: Event) {
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Up if self.is_active => {
                        match self.settings_kbits_state.selected() {
                            Some(i) if i > 0 => self.settings_kbits_state.select(Some(i - 1)),
                            _ => self.settings_kbits_state.select(Some(CHOOSABLE_KBITS.len() - 1)),
                        }
                    }
                    KeyCode::Down if self.is_active => {
                        match self.settings_kbits_state.selected() {
                            Some(i) if i < CHOOSABLE_KBITS.len() - 1 => self.settings_kbits_state.select(Some(i + 1)),
                            _ => self.settings_kbits_state.select(Some(0)),
                        }
                    }
                    KeyCode::Enter if self.is_active => {
                        match self.settings_kbits_state.selected() {
                            Some(selected) => {
                                let n = CHOOSABLE_KBITS[selected];
                                self.cm_s.as_ref().unwrap().send(InterthreadMessage::AudioChangePreferredKbits(n)).unwrap();
                            }
                            None => {}
                        }
                    },
                    KeyCode::Up => self.active_block = ActiveBlock::Tabs,
                    KeyCode::Left if !self.is_active => self.active_block = ActiveBlock::OutputList,
                    KeyCode::Right if !self.is_active => self.active_block = ActiveBlock::InputList,
                    _ => {}
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
                    KeyCode::Up => self.active_block = ActiveBlock::Tabs,
                    KeyCode::Right if !self.is_active && self.contact_list_state.selected().is_some() => self.active_block = ActiveBlock::ChatMessages,
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }

    fn handle_chat_messages_event(&mut self, e: Event) {
        if !self.is_active {
            self.chat_messages_list_state = None
        }
        match e {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Up if self.is_active => {
                        match self.chat_messages_list_state {
                            Some(i) if i > 0 => self.chat_messages_list_state = Some(i-1),
                            None if self.chat_messages_length > 1 => self.chat_messages_list_state = Some(self.chat_messages_length - 2),
                            _ => {}
                        }
                    }
                    KeyCode::Down if self.is_active => {
                        match self.chat_messages_list_state {
                            Some(i) if i == self.chat_messages_length - 1 => self.chat_messages_list_state = None,
                            Some(i) if i < self.chat_messages_length - 1 => self.chat_messages_list_state = Some(i+1),
                            _ => {}
                        }
                    }
                    KeyCode::Up => self.active_block = ActiveBlock::Tabs,
                    KeyCode::Left if !self.is_active => self.active_block = ActiveBlock::ContactList,
                    KeyCode::Down if !self.is_active => self.active_block = ActiveBlock::ChatInput,
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
                    KeyCode::Left if !self.is_active => self.active_block = ActiveBlock::ContactList,
                    KeyCode::Up if !self.is_active => self.active_block = ActiveBlock::ChatMessages,
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
                        self.active_block = ActiveBlock::Tabs;
                    }
                    KeyCode::Left if self.is_active => {
                        if self.selected_tab == 0 {self.selected_tab = self.tab_titles.len() - 1} else {self.selected_tab -= 1;}
                        self.active_block = ActiveBlock::Tabs;
                    }
                    KeyCode::Down if !self.is_active => match FromPrimitive::from_usize(self.selected_tab) {
                        Some(TabIndex::MAIN) => self.active_block = ActiveBlock::ContactList,
                        Some(TabIndex::SETTINGS) => self.active_block = ActiveBlock::InputList,
                        _ => {}
                    }
                    _ => {}
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }
}