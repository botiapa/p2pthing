mod client;
mod server;
mod common;

use client::client::start_client;
use server::server::RendezvousServer;

use std::env;


pub fn main() {
    let args: Vec::<String> = env::args().collect();
    if args.len() == 1 {
        println!("No argument found, assuming server role.");
        let server = RendezvousServer::start_server();
    }
    else if args.len() >= 2 && args.len() <= 3 {
        if args[1].starts_with("c") {
            println!("Starting as client");
            start_client();
        }
        else if args[1].starts_with("s") {
            println!("Starting as server");
            let server = RendezvousServer::start_server();
        }
    }
    else {
        println!("Too many argument received, exiting...")
    }
}