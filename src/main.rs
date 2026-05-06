mod cas;
mod consts;
mod server;
mod upstream;

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;

/// USTC Chat API Proxy
///
/// This tool provides an OpenAI-compatible API interface that proxies requests to the USTC Chat API.
#[derive(Parser)]
#[command(name = "ustchat")]
struct Cli {
    /// USTC CAS username
    #[arg(long, short, env = "USTCHAT_USERNAME")]
    username: Option<String>,

    /// USTC CAS password
    #[arg(long, short, env = "USTCHAT_PASSWORD", hide_env_values = true)]
    password: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start the HTTP proxy server
    Run {
        /// Port to listen on
        #[arg(long, default_value = "28080")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Local API keys that clients must present.
        ///
        /// If omitted, no local authentication is required.
        #[arg(long = "auth", short, value_name = "KEY")]
        auth_keys: Vec<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            port,
            host,
            auth_keys,
        } => {
            let username = cli.username.unwrap_or_else(|| {
                eprintln!(
                    "Username is required. Provide --username or set USTCHAT_USERNAME env var."
                );
                std::process::exit(1);
            });
            let password = cli.password.unwrap_or_else(|| {
                eprintln!(
                    "Password is required. Provide --password or set USTCHAT_PASSWORD env var."
                );
                std::process::exit(1);
            });

            eprintln!("Logging in as {username}...");
            let ustc_token = cas::login(&username, &password).await.unwrap_or_else(|e| {
                eprintln!("Login failed: {e}");
                std::process::exit(1);
            });

            if !auth_keys.is_empty() {
                eprintln!("Local auth enabled with {} key(s)", auth_keys.len());
            }

            let http_client = reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_else(|e| {
                    eprintln!("Failed to create HTTP client: {e}");
                    std::process::exit(1);
                });

            let state = Arc::new(server::AppState {
                http_client,
                ustc_token: Some(ustc_token),
                local_api_keys: auth_keys,
            });

            if let Err(e) = server::run(state, &host, port).await {
                eprintln!("Server error: {e}");
                std::process::exit(1);
            }
        }
    }
}
