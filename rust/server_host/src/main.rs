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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = !args.iter().any(|a| a == "--no-verbose");
    let port = args
        .iter()
        .find_map(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    match OrcServer::start(port, verbose) {
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
