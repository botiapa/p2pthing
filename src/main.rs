#[cfg(any(feature = "client-tui", feature = "client-gui"))]
mod client;
mod server;

#[cfg(any(feature = "client-tui", feature = "client-gui"))]
use client::client::start_client;
use p2pthing_common::ui::UIType;
use server::rendezvous_server::RendezvousServer;

use std::env;

pub fn main() {
    let args: Vec::<String> = env::args().collect();
    match args.len() {
        1 if cfg!(feature = "client-gui") => {
            println!("No argument found, assuming client-gui role.");
            init_client(None, UIType::GUI);
        }
        1 if cfg!(feature = "client-tui") => {
            println!("No argument found, assuming client-tui role.");
            init_client(None, UIType::TUI);
        }
        1 if cfg!(feature = "server") => {
            println!("No argument found, assuming server role.");
            init_server();
        }
        1 | 2 | 3 if cfg!(feature = "client-tui") && args[1].starts_with("t") => {
            init_client(args.get(2).cloned(), UIType::TUI);
        }
        1 | 2 | 3 if cfg!(feature = "client-gui") && args[1].starts_with("g") => {
            init_client(args.get(2).cloned(), UIType::GUI);
        }
        1 | 2 | 3 if !cfg!(feature = "client-tui") && args[1].starts_with("t") => {
            println!("Tried running as client-tui, but I've been built without client-tui support")
        }
        1 | 2 | 3 if !cfg!(feature = "client-gui") && args[1].starts_with("g") => {
            println!("Tried running as client-gui, but I've been built without client-gui support")
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
}

fn init_client(ip: Option<String>, ui_type: UIType) {
    let ip = ip.unwrap_or(String::from("127.0.0.1:42069"));
    println!("Starting as client. Rendezvous ip: {}", ip);
    #[cfg(any(feature = "client-tui", feature = "client-gui"))]
    start_client(ip, ui_type);
}

fn init_server() {
    println!("Starting as server");
    #[cfg(feature = "server")]
    let _ = RendezvousServer::start_server();
}