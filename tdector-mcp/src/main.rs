//! Native, single-project MCP adapter. Standard output is protocol-only.

mod dto;
mod server;
mod tools;
mod transport;
mod worker;

use clap::Parser;
use dto::Limits;
use rmcp::ServiceExt;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Serve one existing tdector project over MCP stdio. Read-only by default; edits require --write and an explicit project_save. Unsaved changes are lost on exit."
)]
struct Args {
    #[arg(long, allow_hyphen_values = true)]
    project: PathBuf,
    #[arg(long)]
    write: bool,
    #[arg(long, default_value_t = 32 * 1024 * 1024, value_parser = clap::value_parser!(u64).range(1..=1024 * 1024 * 1024))]
    max_project_bytes: u64,
    #[arg(long, default_value_t = 1024 * 1024, value_parser = clap::value_parser!(u64).range(1024..=64 * 1024 * 1024))]
    max_message_bytes: u64,
    #[arg(long, default_value_t = 256 * 1024, value_parser = clap::value_parser!(u64).range(4096..=64 * 1024 * 1024))]
    max_result_bytes: u64,
    #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u64).range(1..=4096))]
    queue_capacity: u64,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u64).range(1..=10000))]
    max_batch_commands: u64,
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u64).range(1..=200))]
    max_page_size: u64,
    #[arg(long, default_value_t = 30000, value_parser = clap::value_parser!(u64).range(1..=3600000))]
    operation_timeout_ms: u64,
}

fn main() -> std::process::ExitCode {
    let args = Args::parse();
    let limits = Limits {
        project_bytes: args.max_project_bytes as usize,
        message_bytes: args.max_message_bytes as usize,
        result_bytes: args.max_result_bytes as usize,
        queued_operations: args.queue_capacity as usize,
        batch_commands: args.max_batch_commands as usize,
        page_size: args.max_page_size as usize,
        default_page_size: (args.max_page_size as usize).min(50),
        operation_timeout_ms: args.operation_timeout_ms,
        output_write_timeout_ms: transport::OUTPUT_TIMEOUT.as_millis() as u64,
        shutdown_timeout_ms: transport::SHUTDOWN_TIMEOUT.as_millis() as u64,
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create MCP runtime");
    let result = runtime.block_on(run(args.project, args.write, limits));
    // Tokio's stdin uses a blocking read that cannot be cancelled. An invalid initial project must exit even when the host still holds stdin open.
    runtime.shutdown_background();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tdector-mcp: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run(project: PathBuf, writable: bool, limits: Limits) -> Result<(), String> {
    let mut owner = worker::Owner::spawn(project, writable, limits.clone())?;
    let input = transport::BoundedInput::new(
        tokio::io::stdin(),
        limits.message_bytes,
        owner.shutdown.clone(),
    );
    let server = server::Server::new(owner.client.clone(), writable, limits);
    let serve = server.serve(transport::StdioTransport::new(input, tokio::io::stdout()));
    tokio::pin!(serve);
    let result = tokio::select! {
        result = &mut serve => match result {
            Ok(service) => {
                let cancellation = service.cancellation_token();
                let waiting = service.waiting();
                tokio::pin!(waiting);
                tokio::select! {
                    result = &mut waiting => result.map(|_| ()).map_err(|e| e.to_string()),
                    result = &mut owner.finished => {
                        let result = worker::finished_result(result);
                        if result.is_err() {
                            // Deliver pending load errors where possible before teardown.
                            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        }
                        // Output failure can stop the worker while the host keeps stdin open. Wake the SDK in that case as well as on EOF.
                        cancellation.cancel();
                        // The SDK bounds its drain, but transport.close can still wait on a blocked stdout writer. Finish the owner first, then bound protocol teardown independently of the host.
                        let drained = tokio::time::timeout(
                            transport::SHUTDOWN_TIMEOUT, &mut waiting,
                        ).await.map_err(|_| "MCP output did not drain during shutdown".to_owned())
                            .and_then(|r| r.map(|_| ()).map_err(|e| e.to_string()));
                        result.and(drained)
                    }
                }
            }
            Err(error) => Err(error.to_string()),
        },
        result = &mut owner.finished => worker::finished_result(result),
    };
    owner.shutdown.stop();
    owner.join()?;
    result
}
