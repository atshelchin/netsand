use super::SandboxConfig;
use std::process::Command;

pub fn spawn(config: &SandboxConfig, command: &[String]) -> Result<std::process::Child, String> {
    let proxy_url = format!("http://127.0.0.1:{}", config.proxy_port);

    match &config.user {
        Some(user) => {
            // Set up iptables rules for this user, then run as that user
            setup_iptables(user)?;

            Command::new("sudo")
                .arg("-u")
                .arg(user)
                .arg("env")
                .arg(format!("HTTP_PROXY={}", proxy_url))
                .arg(format!("HTTPS_PROXY={}", proxy_url))
                .arg(format!("http_proxy={}", proxy_url))
                .arg(format!("https_proxy={}", proxy_url))
                .arg("ALL_PROXY=")
                .arg("NO_PROXY=")
                .args(command)
                .spawn()
                .map_err(|e| format!("sudo spawn: {e}"))
        }
        None => {
            // No dedicated user — just set env vars (weaker isolation)
            tracing::warn!("no sandbox user set — using proxy env vars only (no OS enforcement)");
            super::spawn_basic(config, command)
        }
    }
}

fn setup_iptables(user: &str) -> Result<(), String> {
    // Allow loopback
    run_cmd("iptables", &[
        "-C", "OUTPUT", "-m", "owner", "--uid-owner", user, "-o", "lo", "-j", "ACCEPT",
    ]).ok(); // ignore if exists
    run_cmd("iptables", &[
        "-I", "OUTPUT", "-m", "owner", "--uid-owner", user, "-o", "lo", "-j", "ACCEPT",
    ])?;

    // Drop everything else
    run_cmd("iptables", &[
        "-C", "OUTPUT", "-m", "owner", "--uid-owner", user, "-j", "DROP",
    ]).ok();
    run_cmd("iptables", &[
        "-A", "OUTPUT", "-m", "owner", "--uid-owner", user, "-j", "DROP",
    ])?;

    tracing::info!("iptables: {} restricted to localhost", user);
    Ok(())
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("{} {}: {e}", cmd, args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} {} failed", cmd, args.join(" ")))
    }
}
