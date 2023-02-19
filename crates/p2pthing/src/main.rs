#[cfg(any(feature = "tui", feature = "gui"))]
mod client;
mod server;

#[cfg(any(feature = "tui", feature = "gui"))]
use client::client::start_client;
use p2pthing_common::ui::UIType;
#[cfg(feature = "tracing")]
use p2pthing_common::{
    tracing_chrome::{ChromeLayerBuilder, FlushGuard},
    tracing_subscriber::{self, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt},
};
use server::rendezvous_server::RendezvousServer;

use std::{
    env, fs,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn main() {
    let args: Vec<String> = env::args().collect();

    #[cfg(feature = "tracing")]
    let _guard = set_up_tracing();

    match args.len() {
        1 if cfg!(feature = "gui") => {
            println!("No argument found, assuming gui role.");
            init_client(None, UIType::GUI);
        }
        1 if cfg!(feature = "tui") => {
            println!("No argument found, assuming tui role.");
            init_client(None, UIType::TUI);
        }
        1 if cfg!(feature = "server") => {
            println!("No argument found, assuming server role.");
            init_server();
        }
        1 | 2 | 3 if cfg!(feature = "tui") && args[1].starts_with("t") => {
            init_client(args.get(2).cloned(), UIType::TUI);
        }
        1 | 2 | 3 if cfg!(feature = "gui") && args[1].starts_with("g") => {
            init_client(args.get(2).cloned(), UIType::GUI);
        }
        1 | 2 | 3 if !cfg!(feature = "tui") && args[1].starts_with("t") => {
            println!("Tried running as tui, but I've been built without tui support")
        }
        1 | 2 | 3 if !cfg!(feature = "gui") && args[1].starts_with("g") => {
            println!("Tried running as gui, but I've been built without gui support")
        }
        2 if cfg!(feature = "server") && args[1].starts_with("s") => {
            init_server();
        }
        2 if !cfg!(feature = "server") && args[1].starts_with("s") => {
            println!("Tried running as server, but I've been built without server support");
        }
        3 if args[1].starts_with("s") => {
            println!("Tried running as server with custom IP, but this is not supported");
        }
        _ => {
            println!("Invalid args."); //TODO: Display help
        }
    }
    #[cfg(feature = "tracing")]
    drop(_guard);
}

fn init_client(ip: Option<String>, ui_type: UIType) {
    let ip = ip.unwrap_or(String::from("127.0.0.1:42069"));
    println!("Starting as client. Rendezvous ip: {}", ip);
    #[cfg(any(feature = "tui", feature = "gui"))]
    start_client(ip, ui_type);
}

fn init_server() {
    println!("Starting as server");
    #[cfg(feature = "server")]
    let _ = RendezvousServer::start_server();
}

#[cfg(feature = "tracing")]
fn set_up_tracing() -> FlushGuard {
    fs::create_dir_all("./tracing").unwrap();
    let fname = format!("./tracing/trace-{}.json", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
    let (chrome_layer, _guard) = ChromeLayerBuilder::new().include_args(true).file(fname).build();

    tracing_subscriber::registry().with(chrome_layer).init();
    _guard
}
