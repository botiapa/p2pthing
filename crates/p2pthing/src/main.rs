#[cfg(any(feature = "tui", feature = "gui"))]
mod client;
mod server;

#[cfg(any(feature = "tui", feature = "gui"))]
use client::client::start_client;
use p2pthing_common::ui::UIType;
use server::rendezvous_server::RendezvousServer;

use std::env;

pub fn main() {
    let args: Vec::<String> = env::args().collect();
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