use std::sync::{mpsc::Receiver, Arc, Mutex};

use mio::{Poll, Token, Waker};
use mio_misc::{
  channel::{channel, Sender},
  queue::NotificationQueue,
  NotificationId,
};
use p2pthing_common::{encryption::NetworkedPublicKey, message_type::InterthreadMessage, ui::UI};
use tauri::{Manager, Window};

pub struct Gui {
  ui_s: Sender<InterthreadMessage>,
  ui_r: Option<Receiver<InterthreadMessage>>,
}

struct GuiState(Arc<Mutex<Sender<InterthreadMessage>>>, NetworkedPublicKey);

impl Gui {
  pub fn new() -> Gui {
    let poll = Poll::new().unwrap();

    let waker = Arc::new(Waker::new(poll.registry(), Token(0)).unwrap());
    let queue = Arc::new(NotificationQueue::new(waker.clone()));
    let (ui_s, ui_r) = channel(queue, NotificationId::gen_next());

    Gui {
      ui_s,
      ui_r: Some(ui_r),
    }
  }

  fn relay_messages(ui_r: Arc<Mutex<Receiver<InterthreadMessage>>>, window: Window) {
    let w = window.clone();
    let r = ui_r.clone();
    std::thread::spawn(move || {
      let ui_r = r.lock().unwrap();
      loop {
        let msg = ui_r.recv();

        if let Ok(msg) = msg {
          if let Err(err) = w.emit("client-event", msg) {
            println!("Failed to relay event to gui: {}", err);
          }
        }
      }
    });
  }
}

#[tauri::command]
fn send_event(state: tauri::State<GuiState>, event: InterthreadMessage) {
  state.0.lock().unwrap().send(event).unwrap();
}

#[tauri::command]
fn get_own_public_key(state: tauri::State<GuiState>) -> NetworkedPublicKey {
  return state.1.clone();
}

impl UI for Gui {
  fn get_notifier(&self) -> Sender<p2pthing_common::message_type::InterthreadMessage> {
    return self.ui_s.clone();
  }

  fn main_loop(
    &mut self,
    cm_s: Sender<p2pthing_common::message_type::InterthreadMessage>,
    own_public_key: NetworkedPublicKey,
  ) {
    let ui_r = self.ui_r.take().unwrap();
    let ui_r = Arc::new(Mutex::new(ui_r));
    let state = GuiState(Arc::new(Mutex::new(cm_s)), own_public_key);

    let mut app = tauri::Builder::default()
      .on_window_event(move |event| match event.event() {
        tauri::WindowEvent::Destroyed => {
          println!("Window destroyed");
        }
        _ => {}
      })
      .manage(state)
      .setup(move |app| {
        let main_window = app.get_window("main").unwrap();
        {
          let r = ui_r.clone();
          let win = main_window.clone();
          app.once_global("gui-started", move |_| {
            Gui::relay_messages(r.clone(), win.clone());
          });
        }

        println!("Starting tauri application");
        Ok(())
      })
      .invoke_handler(tauri::generate_handler![send_event, get_own_public_key])
      .build(tauri::generate_context!())
      .expect("error while running tauri application");
    app.run(|_, _| {});
  }
}
