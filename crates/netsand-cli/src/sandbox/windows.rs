use super::SandboxConfig;
use std::process::Command;

/// Windows sandbox: use Windows Firewall (netsh) to restrict the spawned process.
///
/// Strategy:
/// 1. Create a firewall rule that blocks ALL outbound for the bot executable
/// 2. Create an allow rule for localhost only
/// 3. Spawn the process with proxy env vars
/// 4. On exit, remove the firewall rules
///
/// This requires Administrator privileges for netsh commands.
pub fn spawn(config: &SandboxConfig, command: &[String]) -> Result<std::process::Child, String> {
    let proxy_url = format!("http://127.0.0.1:{}", config.proxy_port);
    let exe_path = resolve_exe(&command[0])?;
    let rule_name = format!("netsand-{}", sanitize_rule_name(&exe_path));

    // Set up firewall rules
    setup_firewall(&rule_name, &exe_path, config.proxy_port)?;

    // Spawn the child
    let child = Command::new(&command[0])
        .args(&command[1..])
        .env("HTTP_PROXY", &proxy_url)
        .env("HTTPS_PROXY", &proxy_url)
        .env("http_proxy", &proxy_url)
        .env("https_proxy", &proxy_url)
        .env("ALL_PROXY", &proxy_url)
        .env("NO_PROXY", "")
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;

    // Register cleanup on drop
    // Note: firewall rules persist if process crashes without cleanup.
    // `netsand run` handles cleanup in process.rs after waitpid.
    // Store rule name for cleanup.
    ACTIVE_RULES.lock().unwrap().push(rule_name);

    Ok(child)
}

/// Remove firewall rules for a completed process.
pub fn cleanup_firewall(exe_path: &str) {
    let rule_name = format!("netsand-{}", sanitize_rule_name(exe_path));
    remove_firewall(&rule_name);
}

/// Clean up all netsand firewall rules.
pub fn cleanup_all() {
    let rules: Vec<String> = ACTIVE_RULES.lock().unwrap().drain(..).collect();
    for rule in rules {
        remove_firewall(&rule);
    }
}

static ACTIVE_RULES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn setup_firewall(rule_name: &str, exe_path: &str, proxy_port: u16) -> Result<(), String> {
    // Remove old rules if they exist
    remove_firewall(rule_name);

    // Rule 1: Block all outbound for this program
    run_netsh(&[
        "advfirewall", "firewall", "add", "rule",
        &format!("name={}-block", rule_name),
        "dir=out",
        "action=block",
        &format!("program={}", exe_path),
        "enable=yes",
    ])?;

    // Rule 2: Allow localhost (127.0.0.1) for this program
    run_netsh(&[
        "advfirewall", "firewall", "add", "rule",
        &format!("name={}-allow-local", rule_name),
        "dir=out",
        "action=allow",
        &format!("program={}", exe_path),
        "remoteip=127.0.0.1",
        "enable=yes",
    ])?;

    tracing::info!("Windows Firewall: {} restricted to localhost", exe_path);
    Ok(())
}

fn remove_firewall(rule_name: &str) {
    // Silently remove rules (may not exist)
    run_netsh(&[
        "advfirewall", "firewall", "delete", "rule",
        &format!("name={}-block", rule_name),
    ]).ok();
    run_netsh(&[
        "advfirewall", "firewall", "delete", "rule",
        &format!("name={}-allow-local", rule_name),
    ]).ok();
}

fn run_netsh(args: &[&str]) -> Result<(), String> {
    let status = Command::new("netsh")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("netsh: {e} (are you running as Administrator?)"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("netsh {} failed (exit {}). Run as Administrator.", args.join(" "), status))
    }
}

fn resolve_exe(cmd: &str) -> Result<String, String> {
    // Try to get absolute path
    if let Ok(path) = std::fs::canonicalize(cmd) {
        return Ok(path.to_string_lossy().to_string());
    }
    // Try which/where on Windows
    let output = Command::new("where")
        .arg(cmd)
        .output()
        .map_err(|e| format!("where {}: {e}", cmd))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout);
        if let Some(first_line) = path.lines().next() {
            return Ok(first_line.trim().to_string());
        }
    }
    // Fallback: use as-is
    Ok(cmd.to_string())
}

fn sanitize_rule_name(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}
