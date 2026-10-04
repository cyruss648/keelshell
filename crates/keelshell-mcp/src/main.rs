use std::{process::ExitCode, time::Duration};

use keelshell_mcp::{DesktopIpcClient, KeelShellMcpServer, serve_stdio};

fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        eprintln!(
            "Usage: keelshell-mcp (stdio server; desktop access requires copied environment)"
        );
        return ExitCode::from(2);
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            eprintln!("MCP runtime could not start");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async {
        match DesktopIpcClient::from_environment() {
            Ok(Some(client)) => client
                .bridge_stdio()
                .await
                .map_err(|error| error.to_string()),
            Ok(None) => serve_stdio(KeelShellMcpServer::default())
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        }
    });
    // Tokio stdin uses a blocking read which cannot be cancelled portably.
    // Bound runtime shutdown so an idle client cannot keep this process alive
    // after the startup deadline. Process exit terminates that owned thread.
    runtime.shutdown_timeout(Duration::from_millis(100));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
