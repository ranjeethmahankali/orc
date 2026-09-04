use server_host::server::OrcServer;
use std::net::UdpSocket;
use std::process::ExitCode;

const DEFAULT_PORT: u16 = 8222;

fn local_ip() -> Option<std::net::IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

fn main() -> ExitCode {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    match OrcServer::start(port) {
        Ok(server) => {
            let port = server.port();
            if let Some(ip) = local_ip() {
                println!("orc server listening on {ip}:{port}");
            } else {
                println!("orc server listening on port {port}");
            }
            server.join();
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Failed to start server: {e}");
            ExitCode::FAILURE
        }
    }
}
