use std::sync::{Arc, Mutex, mpsc::Receiver};

use mio::{Poll, Token, Waker};
use mio_misc::{NotificationId, channel::{Sender, channel}, queue::NotificationQueue};
use p2pthing_common::{message_type::InterthreadMessage, ui::UI};
use tauri::{Manager, Window};

pub struct Gui {
    ui_s: Sender<InterthreadMessage>,
    ui_r: Option<Receiver<InterthreadMessage>>,
}

impl Gui {
    pub fn new() -> Gui {
        let poll = Poll::new().unwrap();

        let waker = Arc::new(Waker::new(poll.registry(), Token(0)).unwrap());
        let queue = Arc::new(NotificationQueue::new(waker.clone()));
        let (ui_s, ui_r) = channel(queue, NotificationId::gen_next());

        Gui {
            ui_s,
            ui_r: Some(ui_r)
        }
    }

    fn relay_messages(ui_r: Arc<Mutex<Receiver<InterthreadMessage>>>, window: Window) {
        std::thread::spawn(move || {
            let ui_r = ui_r.lock().unwrap();
            loop {
                let msg = ui_r.recv().unwrap();
                if let Err(err) = window.emit("client-event", msg) {
                    println!("Failed to relay event to gui: {}", err);
                }
            }
        });
    }
}


impl UI for Gui {
    fn get_notifier(&self) -> Sender<p2pthing_common::message_type::InterthreadMessage> {
        return self.ui_s.clone();
    }

    fn main_loop(&mut self, cm_s: Sender<p2pthing_common::message_type::InterthreadMessage>, own_public_key: p2pthing_common::encryption::NetworkedPublicKey) {
        let ui_r = self.ui_r.take().unwrap();
        let ui_r = Arc::new(Mutex::new(ui_r));
        tauri::Builder::default()
            .setup(move |app| {
                let main_window = app.get_window("main").unwrap();
                Gui::relay_messages(ui_r.clone(), main_window.clone());

                println!("Starting tauri application");
                Ok(())
            })
            .invoke_handler(tauri::generate_handler![])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }
}

