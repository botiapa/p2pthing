use std::{io::{stdout}, panic, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc::Receiver}};

use crossterm::{ErrorKind, QueueableCommand, event::{DisableMouseCapture, EnableMouseCapture, Event}, execute, terminal::{EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode}};
use mio::{Events, Poll, Token, Waker};
use mio_misc::{NotificationId, channel::{Sender, channel}, queue::NotificationQueue};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use tui::{Terminal, backend::{CrosstermBackend}, widgets::ListState};

use crate::common::{debug_message::{DebugMessage, DebugMessageType}, encryption::NetworkedPublicKey, message_type::{InterthreadMessage, Peer}};

use self::{popup::Popup, ui_peer::UIPeer};

use super::udp_connection::statistics::Statistics;

mod events;
mod blocks;
mod chat_input;
mod ui_peer;
mod popup;

#[derive(PartialEq)]
enum ActiveBlock {
    ContactList,
    ChatMessages,
    ChatInput,
    Tabs,
    InputList,
    OutputList,
    BitRateList
}

enum CallStatus {
    PunchThroughSuccessfull,
    PunchThroughInProgress,
    SentRequest,
    RequestFailed
}

pub struct CallStatusHolder {
    status: CallStatus,
    public_key: NetworkedPublicKey
}

//TODO: Clean up this struct
pub struct Tui {
    cm_s: Option<Sender<InterthreadMessage>>,
    ui_s: Sender<InterthreadMessage>,
    ui_r: Receiver<InterthreadMessage>,
    event_s: Sender<Result<Event, ErrorKind>>,
    event_r: Receiver<Result<Event, ErrorKind>>,
    poll: Poll,
    peers: Vec<UIPeer>,
    contact_list_state: ListState,
    running: Arc<AtomicBool>,
    debug_messages: Vec<DebugMessage>,
    debug_messages_state: ListState,
    chat_messages_length: usize,
    chat_messages_list_state: Option<usize>,
    settings_inputs: Option<Vec<String>>,
    settings_inputs_state: ListState,
    settings_outputs: Option<Vec<String>>,
    settings_outputs_state: ListState,
    settings_kbits_state: ListState,
    muted: bool,
    selected_tab: usize,
    tab_titles: Vec<String>,
    active_block: ActiveBlock,
    is_active: bool,
    own_public_key: Option<NetworkedPublicKey>,
    calls: Vec<CallStatusHolder>,
    next_msg_id: u32,
    active_popup: Option<Box<dyn Popup>>,
    conn_stats: Vec<(NetworkedPublicKey, Statistics)>,
    /// Whether the debug panel is visible above the chat messages
    debug_visible: bool
}

#[derive(FromPrimitive)]
enum TabIndex {
    MAIN = 0,
    SETTINGS = 1,
    DEBUG = 2,
}

const CHOOSABLE_KBITS: [i32; 7] = [2, 8, 16, 32, 64, 128, 256];

impl Tui {
    pub fn new() -> Tui {
        let poll = Poll::new().unwrap();

        let waker = Arc::new(Waker::new(poll.registry(), Token(0)).unwrap());
        let queue = Arc::new(NotificationQueue::new(waker.clone()));
        let (ui_s, ui_r) = channel(queue, NotificationId::gen_next());

        let event_queue = Arc::new(NotificationQueue::new(waker.clone()));
        let (event_s, event_r) = channel(event_queue, NotificationId::gen_next());

        Tui {
            cm_s: None,
            ui_s,
            ui_r,
            event_s,
            event_r,
            poll,
            peers: vec![],
            contact_list_state: ListState::default(),
            running: Arc::new(AtomicBool::new(true)),
            debug_messages: vec![],
            debug_messages_state: ListState::default(),
            chat_messages_length: 0,
            chat_messages_list_state: None,
            settings_inputs: None,
            settings_inputs_state: ListState::default(),
            settings_outputs: None,
            settings_outputs_state: ListState::default(),
            settings_kbits_state: ListState::default(),
            muted: true,
            selected_tab: 0,
            tab_titles: vec!["Main".into(), "Settings".into(), "Debug".into()],
            active_block: ActiveBlock::ContactList,
            is_active: false,
            own_public_key: None,
            calls: vec![],
            next_msg_id: 0,
            active_popup: None,
            conn_stats: vec![],
            debug_visible: false
        }
    }

    pub fn get_notifier(&self) -> Sender<InterthreadMessage>{
        self.ui_s.clone()
    }

    pub fn debug_message(msg: &str, msg_type: DebugMessageType, s: &Sender<InterthreadMessage>) {
        s.send(InterthreadMessage::DebugMessage(msg.into(), msg_type)).unwrap();
    }

    pub fn on_chat_message(s: &Sender<InterthreadMessage>, peer: Peer, msg: String) {
        Tui::debug_message(&format!("Received chat message from: ({})", peer.public_key), DebugMessageType::Log, s);
        s.send(InterthreadMessage::OnChatMessage(peer, msg)).unwrap();
    }

    pub fn main_loop(&mut self, cm_s: Sender<InterthreadMessage>, own_public_key: NetworkedPublicKey) {
        let r = self.running.clone();
        let r1 = self.running.clone();

        self.cm_s = Some(cm_s);
        self.own_public_key = Some(own_public_key);

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
            println!("Set ctrl+c handler");
        }).unwrap();
 
        let s1 = Arc::new(Mutex::new(self.ui_s.clone()));
        let err = Arc::new(Mutex::new(String::from("")));
        {
            let err = Arc::clone(&err);
            panic::set_hook(Box::new(move |p| {
                println!("Error: {}", p.to_string());
                let mut err = err.lock().unwrap();
                *err = p.to_string();
                r1.store(false, Ordering::SeqCst);
                let s1 = s1.lock().unwrap();
                s1.send(InterthreadMessage::WakeUp).unwrap();
            }));
        }
        

        enable_raw_mode().unwrap();
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
        
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.clear().unwrap();
        
        self.receive_keyboard_mouse_events();

        let mut events = Events::with_capacity(1);
        while self.running.load(Ordering::SeqCst) {
            self.poll.poll(&mut events, None).unwrap();

            // Tries to avoid a crash which happens if a terminal's height is too low
            if terminal.size().unwrap().height <= 1 {
                continue;
            }

            self.handle_interthread_events();

            self.handle_keyboard_mouse_events();

            // Render the final image
            terminal.draw(|f| {
                let screen = f.size();

                let tab_divider = self.tab_divider(screen);
                let tab_layout = self.tab_layout(tab_divider[0]);
                self.tabs(f, tab_layout[0]);
                self.status_icons(f, tab_layout[1]);
                
                match FromPrimitive::from_usize(self.selected_tab) {
                    Some(TabIndex::MAIN) => {
                        let main_layout = self.main_layout(tab_divider[1]);
                        self.contact_list(f, main_layout[0]);

                        match self.contact_list_state.selected() {
                            Some(_) => {
                                let main_screen = self.main_screen(main_layout[1]);
                                self.peer_stats(f, main_screen[0]);
                                self.chat_messages(f, main_screen[1]);
                                self.chat_input(f, main_screen[2]);
                            }
                            None => {} // TODO: Display a friendly message here?
                        }
                        
                    }
                    Some(TabIndex::SETTINGS) => {
                        let settings_layout = self.settings_layout(tab_divider[1]);
                        let audio_options_layout = self.setting_audio_options(settings_layout[0]);
                        self.settings_input_list(f, audio_options_layout[0]);
                        self.settings_output_list(f, audio_options_layout[1]);
                        self.settings_kbits_list(f, audio_options_layout[2]);

                    }
                    Some(TabIndex::DEBUG) => {
                        self.debug_messages(f, tab_divider[1]);
                    }
                    _ => unimplemented!("Unimplemented tab received")
                }
                match &mut self.active_popup {
                    Some(popup) => {
                        popup.draw(f, screen);
                    }
                    None => {}
                }
            }).unwrap();
        }
        terminal.backend_mut().queue(LeaveAlternateScreen).unwrap().queue(DisableMouseCapture).unwrap();
        terminal.clear().unwrap();

        let err = err.lock().unwrap();
        if !err.is_empty() {
            println!("Error occured: {}", err);
        }
    }
}

