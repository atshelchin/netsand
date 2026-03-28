use crate::profile::{self};
use crate::state::{DaemonState, ProfilePort};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

pub async fn start(profile_dir: &Path, foreground: bool) -> Result<(), String> {
    if DaemonState::is_daemon_running() {
        return Err("daemon is already running".into());
    }

    let profiles = profile::load_all(profile_dir)?;
    if profiles.is_empty() {
        return Err(format!("no profiles found in {}", profile_dir.display()));
    }

    info!("loaded {} profiles", profiles.len());

    // Save state
    let state = DaemonState {
        pid: std::process::id(),
        profiles: profiles.iter().map(|p| ProfilePort {
            name: p.name.clone(),
            port: p.listen_port,
        }).collect(),
        processes: vec![],
    };
    state.save()?;

    // Start listeners
    let mut handles = Vec::new();
    for p in profiles {
        let addr = format!("127.0.0.1:{}", p.listen_port);
        let policy = Arc::new(p.policy);
        let handle = netsand_proxy::run_listener(&addr, policy, &p.name).await?;
        info!("profile '{}' → 127.0.0.1:{}", p.name, p.listen_port);
        handles.push(handle);
    }

    info!("daemon started (PID {})", std::process::id());

    if foreground {
        // Wait for Ctrl+C
        tokio::signal::ctrl_c().await.ok();
        info!("shutting down...");
    } else {
        // Background: wait forever (or until signal)
        tokio::signal::ctrl_c().await.ok();
    }

    DaemonState::cleanup();
    info!("daemon stopped");
    Ok(())
}

pub fn stop() -> Result<(), String> {
    let state = DaemonState::load()
        .ok_or("daemon is not running")?;

    // Terminate the daemon process
    #[cfg(unix)]
    let status = std::process::Command::new("kill")
        .arg(state.pid.to_string())
        .status()
        .map_err(|e| format!("kill: {e}"))?;

    #[cfg(windows)]
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &state.pid.to_string(), "/F"])
        .status()
        .map_err(|e| format!("taskkill: {e}"))?;

    if status.success() {
        DaemonState::cleanup();
        info!("daemon stopped (PID {})", state.pid);
        Ok(())
    } else {
        Err("failed to stop daemon".into())
    }
}

pub fn status() -> Result<(), String> {
    let state = DaemonState::load();

    match state {
        Some(s) => {
            let running = DaemonState::is_daemon_running();
            println!("Daemon: {} (PID {})", if running { "running" } else { "dead" }, s.pid);
            println!();
            println!("Profiles:");
            for p in &s.profiles {
                let proc_count = s.processes.iter().filter(|pr| pr.profile == p.name).count();
                println!("  {:<16} :{:<6} [{} processes]", p.name, p.port, proc_count);
            }
            if !s.processes.is_empty() {
                println!();
                println!("Running processes:");
                for p in &s.processes {
                    println!("  PID {:<8} {:<16} {} (since {})", p.pid, p.profile, p.command, p.started_at);
                }
            }
            Ok(())
        }
        None => {
            println!("Daemon: not running");
            Ok(())
        }
    }
}
