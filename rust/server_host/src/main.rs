use server_host::server::OrcServer;
use std::process::ExitCode;

const DEFAULT_PORT: u16 = 8222;

fn main() -> ExitCode {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    match OrcServer::start(port) {
        Ok(server) => {
            println!("orc server listening on port {port}");
            server.join();
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Failed to start server: {e}");
            ExitCode::FAILURE
        }
    }
}
