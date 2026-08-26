mod server;

use server::OrcServer;
use std::process::ExitCode;

const DEFAULT_PORT: u16 = 8222;

fn main() -> ExitCode {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let plugin_dir = std::env::args().nth(2).unwrap_or_else(|| {
        let exe = std::env::current_exe().expect("Cannot determine executable path");
        let dir = exe.parent().expect("Executable has no parent directory");
        dir.to_string_lossy().into_owned()
    });
    match OrcServer::start(&plugin_dir, port) {
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
