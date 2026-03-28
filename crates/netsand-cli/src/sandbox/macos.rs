use super::SandboxConfig;
use std::process::Command;

/// macOS sandbox using pf (Packet Filter).
///
/// Strategy:
/// 1. Create a dedicated pf anchor with rules blocking all outbound except localhost
/// 2. Use `pfctl` to load the anchor
/// 3. Spawn the child with proxy env vars
/// 4. On exit, flush the anchor rules
///
/// pf rules are per-user (matching UID) so only the bot process is affected.
/// Requires sudo for pfctl.
pub fn spawn(config: &SandboxConfig, command: &[String]) -> Result<std::process::Child, String> {
    let proxy_url = format!("http://127.0.0.1:{}", config.proxy_port);
    let anchor_name = format!("netsand_{}", config.proxy_port);

    match &config.user {
        Some(user) => {
            // Set up pf rules for this user
            setup_pf(&anchor_name, user)?;

            let child = Command::new("sudo")
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
                .map_err(|e| format!("sudo spawn: {e}"))?;

            // Store anchor for cleanup
            ACTIVE_ANCHORS.lock().unwrap().push(anchor_name);
            Ok(child)
        }
        None => {
            // No user — use proxy env vars only (warn about weak isolation)
            tracing::warn!("no sandbox user set — using proxy env vars only (no OS enforcement on macOS)");
            tracing::warn!("set [sandbox] user in profile for pf-based isolation");
            super::spawn_basic(config, command)
        }
    }
}

pub fn cleanup_all() {
    let anchors: Vec<String> = ACTIVE_ANCHORS.lock().unwrap().drain(..).collect();
    for anchor in anchors {
        teardown_pf(&anchor);
    }
}

static ACTIVE_ANCHORS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn setup_pf(anchor_name: &str, user: &str) -> Result<(), String> {
    // pf rule: block outbound from this user, allow loopback
    let rules = format!(
        "pass out quick on lo0 from any to any\n\
         block out quick proto tcp from any to ! 127.0.0.1 user {}\n\
         block out quick proto udp from any to ! 127.0.0.1 user {}\n",
        user, user
    );

    // Write rules to temp file
    let rules_path = format!("/tmp/netsand-pf-{}.conf", anchor_name);
    std::fs::write(&rules_path, &rules)
        .map_err(|e| format!("write pf rules: {e}"))?;

    // Ensure pf has our anchor referenced in the main ruleset
    // First, load the anchor rules
    let status = Command::new("sudo")
        .args(["pfctl", "-a", anchor_name, "-f", &rules_path])
        .status()
        .map_err(|e| format!("pfctl load: {e}"))?;

    if !status.success() {
        return Err("pfctl: failed to load anchor rules (need sudo)".into());
    }

    // Enable pf if not already enabled
    Command::new("sudo")
        .args(["pfctl", "-e"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok();

    std::fs::remove_file(&rules_path).ok();
    tracing::info!("pf anchor '{}': user {} restricted to localhost", anchor_name, user);
    Ok(())
}

fn teardown_pf(anchor_name: &str) {
    // Flush the anchor rules
    Command::new("sudo")
        .args(["pfctl", "-a", anchor_name, "-F", "all"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok();
    tracing::info!("pf anchor '{}' flushed", anchor_name);
}
