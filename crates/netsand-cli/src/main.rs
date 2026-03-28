mod daemon;
mod process;
mod profile;
mod sandbox;
mod state;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::error;

#[derive(Parser)]
#[command(name = "netsand", about = "Process-level network sandbox manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Profile directory (default: ~/.config/netsand/profiles or NETSAND_PROFILE_DIR)
    #[arg(long, global = true)]
    profile_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage the background proxy daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Run a command inside a network sandbox
    Run {
        /// Profile name
        profile: String,
        /// Command to run (use -- to separate)
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
    /// Show daemon status and running processes
    Status,
    /// List available profiles
    Profiles,
    /// Validate a profile config
    Check {
        /// Profile name
        profile: String,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon (loads all profiles)
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(long, short)]
        foreground: bool,
    },
    /// Stop the daemon
    Stop,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("netsand=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    let profile_dir = get_profile_dir(cli.profile_dir);

    let result = match cli.command {
        Commands::Daemon { action } => match action {
            DaemonAction::Start { foreground } => {
                daemon::start(&profile_dir, foreground).await
            }
            DaemonAction::Stop => daemon::stop(),
        },
        Commands::Run { profile, command } => {
            match process::run(&profile, &command) {
                Ok(code) => std::process::exit(code),
                Err(e) => Err(e),
            }
        }
        Commands::Status => daemon::status(),
        Commands::Profiles => list_profiles(&profile_dir),
        Commands::Check { profile } => check_profile(&profile_dir, &profile),
    };

    if let Err(e) = result {
        error!("{e}");
        std::process::exit(1);
    }
}

fn get_profile_dir(override_dir: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = override_dir {
        return dir;
    }
    if let Ok(dir) = std::env::var("NETSAND_PROFILE_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("netsand")
        .join("profiles")
}

fn list_profiles(dir: &PathBuf) -> Result<(), String> {
    let profiles = profile::load_all(dir)?;
    if profiles.is_empty() {
        println!("No profiles found in {}", dir.display());
        return Ok(());
    }
    println!("{:<16} {:<8} {:<8} {}", "NAME", "PORT", "USER", "DOMAINS");
    for p in &profiles {
        let user = p.sandbox_user.as_deref().unwrap_or("-");
        let domain_count = p.policy.allowed_domains.len();
        println!("{:<16} {:<8} {:<8} {} domains", p.name, p.listen_port, user, domain_count);
    }
    Ok(())
}

fn check_profile(dir: &PathBuf, name: &str) -> Result<(), String> {
    let path = dir.join(format!("{}.toml", name));
    let profile = profile::Profile::load(&path, 8001)?;
    println!("Profile '{}' is valid", profile.name);
    println!("  Port: {}", profile.listen_port);
    println!("  Allowed domains: {:?}", profile.policy.allowed_domains);
    println!("  Allowed IPs: {:?}", profile.policy.allowed_ips);
    if let Some(ref up) = profile.policy.upstream {
        println!("  Upstream: {:?} {}:{}", up.proxy_type, up.host, up.port);
        println!("  Upstream domains: {:?}", up.domains);
    }
    if let Some(ref user) = profile.sandbox_user {
        println!("  Sandbox user: {}", user);
    }
    Ok(())
}
