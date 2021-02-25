#[cfg(feature = "client")]
mod client;
mod server;
mod common;

#[cfg(feature = "client")]
use client::client::start_client;
use server::rendezvous_server::RendezvousServer;

use std::env;


pub fn main() {
    let args: Vec::<String> = env::args().collect();
    if args.len() == 1 {
        println!("No argument found, assuming client role.");
        init_client();
    }
    else if args.len() >= 2 && args.len() <= 3 {
        if cfg!(feature = "client") && args[1].starts_with("c") {
            init_client();
        }
        else if args[1].starts_with("s") {
            init_server();
        }
    }
    else {
        println!("Too many argument received, exiting...")
    }
}

fn init_client() {
    println!("Starting as client");
    #[cfg(feature = "client")]
    start_client();
}

fn init_server() {
    println!("Starting as server");
    let _ = RendezvousServer::start_server();
}