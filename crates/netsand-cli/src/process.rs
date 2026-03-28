use crate::sandbox::{self, SandboxConfig};
use crate::state::{DaemonState, ProcessInfo};
use chrono::Utc;
use tracing::info;

pub fn run(profile_name: &str, command: &[String]) -> Result<i32, String> {
    let state = DaemonState::load()
        .ok_or("daemon is not running — start it with: netsand daemon start")?;

    let port = state.get_port(profile_name)
        .ok_or(format!("profile '{}' not found in daemon state", profile_name))?;

    // Load profile to get sandbox user
    let profile_dir = get_profile_dir();
    let profile_path = profile_dir.join(format!("{}.toml", profile_name));
    let sandbox_user = if profile_path.exists() {
        let profile = crate::profile::Profile::load(&profile_path, port)?;
        profile.sandbox_user
    } else {
        None
    };

    let config = SandboxConfig {
        proxy_port: port,
        user: sandbox_user,
    };

    info!("running in sandbox '{}' (proxy :{}) — {:?}", profile_name, port, command);

    // Register process
    let cmd_str = command.join(" ");
    let mut child = sandbox::spawn_sandboxed(&config, command)?;
    let child_pid = child.id();

    // Update state with running process
    let mut state = DaemonState::load().unwrap_or_default();
    state.processes.push(ProcessInfo {
        pid: child_pid,
        profile: profile_name.to_string(),
        command: cmd_str,
        started_at: Utc::now().to_rfc3339(),
        proxy_port: port,
    });
    state.save().ok();

    // Wait for child
    let status = child.wait().map_err(|e| format!("wait: {e}"))?;

    // Deregister
    if let Some(mut state) = DaemonState::load() {
        state.processes.retain(|p| p.pid != child_pid);
        state.save().ok();
    }

    let code = status.code().unwrap_or(1);
    info!("process exited with code {}", code);
    Ok(code)
}

fn get_profile_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("NETSAND_PROFILE_DIR") {
        return std::path::PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("netsand")
        .join("profiles")
}
