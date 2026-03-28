use super::SandboxConfig;
use std::io::Write;
use std::process::Command;

pub fn spawn(config: &SandboxConfig, command: &[String]) -> Result<std::process::Child, String> {
    let proxy_url = format!("http://127.0.0.1:{}", config.proxy_port);

    // Generate Seatbelt profile
    let sb_profile = format!(
        r#"(version 1)
(allow default)
(deny network-outbound)
(allow network-outbound (remote ip "localhost:{port}"))
(allow network-outbound (remote ip "127.0.0.1:{port}"))
(allow network-outbound (remote unix-socket))
"#,
        port = config.proxy_port,
    );

    // Write to temp file
    let mut tmp = tempfile().map_err(|e| format!("create temp: {e}"))?;
    tmp.write_all(sb_profile.as_bytes()).map_err(|e| format!("write temp: {e}"))?;
    let tmp_path = tmp.path().to_string();

    Command::new("sandbox-exec")
        .args(["-f", &tmp_path])
        .arg("--")
        .arg("env")
        .arg(format!("HTTP_PROXY={}", proxy_url))
        .arg(format!("HTTPS_PROXY={}", proxy_url))
        .arg(format!("http_proxy={}", proxy_url))
        .arg(format!("https_proxy={}", proxy_url))
        .arg("ALL_PROXY=")
        .arg("NO_PROXY=")
        .args(command)
        .spawn()
        .map_err(|e| format!("sandbox-exec: {e}"))
}

struct TempFile {
    path: String,
}

impl TempFile {
    fn path(&self) -> &str {
        &self.path
    }
}

impl std::io::Write for TempFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::fs::OpenOptions::new()
            .append(true)
            .open(&self.path)?
            .write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

fn tempfile() -> std::io::Result<TempFile> {
    let path = format!("/tmp/netsand-{}.sb", std::process::id());
    std::fs::write(&path, "")?;
    Ok(TempFile { path })
}
