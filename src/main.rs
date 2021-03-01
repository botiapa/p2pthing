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
    match args.len() {
        1 if cfg!(feature = "client") => {
            println!("No argument found, assuming client role.");
            init_client(None);
        }
        1 if cfg!(feature = "server") => {
            println!("No argument found, assuming server role.");
            init_server();
        }
        1 | 2 if cfg!(feature = "client") && args[1].starts_with("c") => {
            init_client(None);
        }
        1 | 2 if !cfg!(feature = "client") && args[1].starts_with("c") => {
            println!("Tried running as client, but I've been built without client support")
        }
        3 if cfg!(feature = "client") && args[1].starts_with("c") => {
            init_client(Some(args[2].clone()));
        }
        3 if !cfg!(feature = "client") && args[1].starts_with("c") => {
            println!("Tried running as client, but I've been built without client support")
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

fn init_client(ip: Option<String>) {
    let ip = ip.unwrap_or(String::from("127.0.0.1:42069"));
    println!("Starting as client. Rendezvous ip: {}", ip);
    #[cfg(feature = "client")]
    start_client(ip);
}

fn init_server() {
    println!("Starting as server");
    #[cfg(feature = "server")]
    let _ = RendezvousServer::start_server();
}