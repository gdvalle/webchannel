use std::io;
use std::net::{SocketAddr, TcpListener};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::Duration;

const HOST: &str = "127.0.0.1";
pub const CHANNEL_SECRET: &str = "moo";

pub fn get_open_port(host: &str) -> io::Result<SocketAddr> {
    let bind_addr = format!("{}:0", host);
    let listener = TcpListener::bind(bind_addr).expect("Failed to bind on open port");
    listener.local_addr()
}

pub fn start_server(config_file: &str) -> (Child, SocketAddr) {
    let addr = get_open_port(HOST).unwrap();

    let config_file_arg = match config_file {
        "" => "tests/settings/default.toml",
        _ => config_file,
    };

    let handle = Command::new("./target/debug/webchannel")
        .env("WC_SERVER__LISTEN_ADDRESS", addr.to_string())
        .arg(format!("--config-file={}", config_file_arg))
        .spawn()
        .expect("Failed to start server");

    // Wait for server to start.
    sleep(Duration::from_secs(1));

    (handle, addr)
}
