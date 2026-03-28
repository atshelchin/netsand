#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::process::Command;

pub struct SandboxConfig {
    pub proxy_port: u16,
    pub user: Option<String>,
}

/// Spawn a command inside the network sandbox.
pub fn spawn_sandboxed(
    config: &SandboxConfig,
    command: &[String],
) -> Result<std::process::Child, String> {
    if command.is_empty() {
        return Err("no command specified".into());
    }

    #[cfg(target_os = "linux")]
    return linux::spawn(config, command);

    #[cfg(target_os = "macos")]
    return macos::spawn(config, command);

    #[cfg(target_os = "windows")]
    return windows::spawn(config, command);

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return spawn_basic(config, command);
}

/// Fallback: just set proxy env vars, no OS-level enforcement.
#[allow(dead_code)]
pub fn spawn_basic(config: &SandboxConfig, command: &[String]) -> Result<std::process::Child, String> {
    let proxy_url = format!("http://127.0.0.1:{}", config.proxy_port);
    Command::new(&command[0])
        .args(&command[1..])
        .env("HTTP_PROXY", &proxy_url)
        .env("HTTPS_PROXY", &proxy_url)
        .env("http_proxy", &proxy_url)
        .env("https_proxy", &proxy_url)
        .env("ALL_PROXY", &proxy_url)
        .env("NO_PROXY", "")
        .spawn()
        .map_err(|e| format!("spawn: {e}"))
}
