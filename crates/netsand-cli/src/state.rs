use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DaemonState {
    pub pid: u32,
    pub profiles: Vec<ProfilePort>,
    pub processes: Vec<ProcessInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfilePort {
    pub name: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub profile: String,
    pub command: String,
    pub started_at: String,
    pub proxy_port: u16,
}

impl DaemonState {
    pub fn state_dir() -> PathBuf {
        dirs::state_dir()
            .or_else(|| dirs::data_local_dir())
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("netsand")
    }

    pub fn state_file() -> PathBuf {
        Self::state_dir().join("daemon.json")
    }

    pub fn pid_file() -> PathBuf {
        Self::state_dir().join("daemon.pid")
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::state_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("create state dir: {e}"))?;
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("serialize: {e}"))?;
        std::fs::write(Self::state_file(), json).map_err(|e| format!("write state: {e}"))?;
        Ok(())
    }

    pub fn load() -> Option<Self> {
        let content = std::fs::read_to_string(Self::state_file()).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn get_port(&self, profile_name: &str) -> Option<u16> {
        self.profiles.iter().find(|p| p.name == profile_name).map(|p| p.port)
    }

    pub fn is_daemon_running() -> bool {
        if let Some(state) = Self::load() {
            is_pid_alive(state.pid)
        } else {
            false
        }
    }

    pub fn cleanup() {
        std::fs::remove_file(Self::state_file()).ok();
        std::fs::remove_file(Self::pid_file()).ok();
    }
}

fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let status = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        matches!(status, Ok(s) if s.success())
    }

    #[cfg(windows)]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .output();
        match output {
            Ok(o) => {
                let out = String::from_utf8_lossy(&o.stdout);
                out.contains(&pid.to_string())
            }
            Err(_) => false,
        }
    }

    #[cfg(not(any(unix, windows)))]
    false
}
