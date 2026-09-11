use std::net::SocketAddr;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use crate::build::run_command;
use crate::server::run_server;

#[derive(Debug, Parser)]
#[command(
    name = "cargo-leaderboard",
    bin_name = "cargo leaderboard",
    version,
    about = "Compare the unreasonable size of Rust builds and cleans"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Connect your GitHub account through your browser.
    Login(LoginArgs),
    /// Revoke this CLI credential and remove it from this machine.
    Logout,
    /// Configure a nickname for a local, self-hosted SQLite board.
    Setup(SetupArgs),
    /// Check configuration, Cargo, and the server without submitting anything.
    Doctor,
    /// Build and submit the total artifact footprint.
    Build(BuildArgs),
    /// Clean and submit the bytes reclaimed. Runs the real cargo clean.
    Clean(BuildArgs),
    /// Run a local SQLite-backed leaderboard server.
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
struct LoginArgs {
    /// Connect to another GitHub-authenticated leaderboard.
    #[arg(long)]
    api_url: Option<String>,
    /// Print the approval URL without opening a browser (useful for agents and SSH).
    #[arg(long)]
    no_browser: bool,
}

#[derive(Debug, Args)]
struct SetupArgs {
    /// Public nickname (1–40 characters). Prompts when omitted.
    #[arg(long)]
    nickname: Option<String>,
    /// Use a private/self-hosted server instead of the public leaderboard.
    #[arg(long)]
    api_url: Option<String>,
}

#[derive(Debug, Args)]
struct BuildArgs {
    /// Measure locally without sending an event.
    #[arg(long)]
    no_submit: bool,
    /// Public project label; use this to avoid publishing a private repository name.
    #[arg(long)]
    repo: Option<String>,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    cargo_args: Vec<String>,
}

#[derive(Debug, Args)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:3000")]
    bind: SocketAddr,
    #[arg(long, default_value = "sqlite://leaderboard.db")]
    database_url: String,
    #[arg(long)]
    auth_token: Option<String>,
}

pub async fn run() -> Result<u8> {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "leaderboard") {
        args.remove(1);
    }
    let cli = Cli::parse_from(args);

    match cli.command {
        Commands::Login(args) => {
            crate::auth::login(args.api_url, args.no_browser).await?;
            Ok(0)
        }
        Commands::Logout => {
            crate::auth::logout().await?;
            Ok(0)
        }
        Commands::Setup(args) => {
            crate::config::setup(args.nickname, args.api_url)?;
            Ok(0)
        }
        Commands::Doctor => {
            crate::config::doctor().await?;
            Ok(0)
        }
        Commands::Build(args) => {
            run_command("build", args.cargo_args, args.no_submit, args.repo).await
        }
        Commands::Clean(args) => {
            run_command("clean", args.cargo_args, args.no_submit, args.repo).await
        }
        Commands::Serve(args) => {
            run_server(args.bind, &args.database_url, args.auth_token).await?;
            Ok(0)
        }
    }
}
