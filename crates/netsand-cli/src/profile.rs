use netsand_proxy::config::{parse_proxy_url, Policy, UpstreamProxy};
use serde::Deserialize;
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct ProfileFile {
    pub profile: ProfileMeta,
    pub allow: Option<AllowSection>,
    pub upstream: Option<UpstreamSection>,
    pub sandbox: Option<SandboxSection>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub listen_port: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct AllowSection {
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub ips: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpstreamSection {
    pub proxy: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub ips: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SandboxSection {
    pub user: Option<String>,
}

#[derive(Debug)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub listen_port: u16,
    pub policy: Policy,
    pub sandbox_user: Option<String>,
}

impl Profile {
    pub fn load(path: &Path, default_port: u16) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let file: ProfileFile = toml::from_str(&content)
            .map_err(|e| format!("invalid profile {}: {e}", path.display()))?;

        let allow = file.allow.unwrap_or(AllowSection {
            domains: vec![],
            ips: vec![],
        });

        let allowed_domains: HashSet<String> = allow.domains.iter().map(|d| d.to_lowercase()).collect();
        let mut allowed_ips: HashSet<IpAddr> = HashSet::new();
        for ip_str in &allow.ips {
            allowed_ips.insert(ip_str.parse().map_err(|e| format!("invalid IP '{}': {}", ip_str, e))?);
        }

        let upstream = match file.upstream {
            Some(up) => {
                let (proxy_type, host, port) = parse_proxy_url(&up.proxy)?;
                let domains: HashSet<String> = up.domains.iter().map(|d| d.to_lowercase()).collect();
                let mut ips: HashSet<IpAddr> = HashSet::new();
                for ip_str in &up.ips {
                    ips.insert(ip_str.parse().map_err(|e| format!("invalid upstream IP '{}': {}", ip_str, e))?);
                }
                Some(UpstreamProxy { proxy_type, host, port, domains, ips })
            }
            None => None,
        };

        let listen_port = file.profile.listen_port.unwrap_or(default_port);
        let sandbox_user = file.sandbox.and_then(|s| s.user);

        Ok(Profile {
            name: file.profile.name,
            description: file.profile.description,
            listen_port,
            policy: Policy { allowed_domains, allowed_ips, upstream },
            sandbox_user,
        })
    }
}

/// Load all profiles from a directory.
pub fn load_all(dir: &Path) -> Result<Vec<Profile>, String> {
    if !dir.exists() {
        return Err(format!("profile directory not found: {}", dir.display()));
    }

    let mut profiles = Vec::new();
    let mut next_port = 8001u16;

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("read dir: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "toml").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let profile = Profile::load(&entry.path(), next_port)?;
        next_port = profile.listen_port + 1;
        profiles.push(profile);
    }

    Ok(profiles)
}
