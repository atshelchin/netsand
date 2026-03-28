use serde::Deserialize;
use std::collections::HashSet;
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq)]
pub enum UpstreamProxyType {
    Http,
    Socks5,
}

#[derive(Debug, Clone)]
pub struct UpstreamProxy {
    pub proxy_type: UpstreamProxyType,
    pub host: String,
    pub port: u16,
    pub domains: HashSet<String>,
    pub ips: HashSet<IpAddr>,
}

#[derive(Debug)]
pub struct Policy {
    pub allowed_domains: HashSet<String>,
    pub allowed_ips: HashSet<IpAddr>,
    pub upstream: Option<UpstreamProxy>,
}

impl Policy {
    pub fn is_allowed(&self, host: &str) -> bool {
        let normalized = host.to_lowercase();

        if self.allowed_domains.contains(&normalized) {
            return true;
        }
        for domain in &self.allowed_domains {
            if normalized.ends_with(&format!(".{}", domain)) {
                return true;
            }
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            if self.allowed_ips.contains(&ip) {
                return true;
            }
        }
        if normalized == "localhost" || normalized == "127.0.0.1" || normalized == "::1" {
            return true;
        }
        false
    }

    pub fn needs_upstream(&self, host: &str) -> bool {
        let upstream = match &self.upstream {
            Some(u) => u,
            None => return false,
        };
        let normalized = host.to_lowercase();
        if upstream.domains.contains(&normalized) {
            return true;
        }
        for domain in &upstream.domains {
            if normalized.ends_with(&format!(".{}", domain)) {
                return true;
            }
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            if upstream.ips.contains(&ip) {
                return true;
            }
        }
        false
    }
}

/// Parse "socks5://host:port" or "http://host:port"
pub fn parse_proxy_url(url: &str) -> Result<(UpstreamProxyType, String, u16), String> {
    if let Some(rest) = url.strip_prefix("socks5://") {
        let (host, port) = parse_host_port(rest, 1080)?;
        Ok((UpstreamProxyType::Socks5, host, port))
    } else if let Some(rest) = url.strip_prefix("http://") {
        let (host, port) = parse_host_port(rest, 8080)?;
        Ok((UpstreamProxyType::Http, host, port))
    } else {
        Err(format!("unsupported proxy scheme: {} (use socks5:// or http://)", url))
    }
}

pub fn parse_host_port(s: &str, default_port: u16) -> Result<(String, u16), String> {
    if let Some(pos) = s.rfind(':') {
        let host = &s[..pos];
        let port: u16 = s[pos + 1..]
            .parse()
            .map_err(|_| format!("invalid port in: {}", s))?;
        Ok((host.to_string(), port))
    } else {
        Ok((s.to_string(), default_port))
    }
}
