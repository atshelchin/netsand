use netsand_proxy::config::{Policy, UpstreamProxy, UpstreamProxyType, parse_proxy_url};
use std::collections::HashSet;
use std::net::IpAddr;

fn make_policy() -> Policy {
    let mut allowed_domains = HashSet::new();
    allowed_domains.insert("gamma-api.polymarket.com".into());
    allowed_domains.insert("stream.binance.com".into());
    allowed_domains.insert("data-api.polymarket.com".into());

    let mut upstream_domains = HashSet::new();
    upstream_domains.insert("gamma-api.polymarket.com".into());
    upstream_domains.insert("data-api.polymarket.com".into());

    Policy {
        allowed_domains,
        allowed_ips: HashSet::new(),
        upstream: Some(UpstreamProxy {
            proxy_type: UpstreamProxyType::Socks5,
            host: "127.0.0.1".into(),
            port: 1080,
            domains: upstream_domains,
            ips: HashSet::new(),
        }),
    }
}

#[test]
fn test_allowed_domain() {
    let p = make_policy();
    assert!(p.is_allowed("gamma-api.polymarket.com"));
    assert!(p.is_allowed("stream.binance.com"));
    assert!(p.is_allowed("data-api.polymarket.com"));
}

#[test]
fn test_blocked_domain() {
    let p = make_policy();
    assert!(!p.is_allowed("evil.com"));
    assert!(!p.is_allowed("httpbin.org"));
    assert!(!p.is_allowed("polymarket.com")); // parent not in list
}

#[test]
fn test_case_insensitive() {
    let p = make_policy();
    assert!(p.is_allowed("Stream.Binance.COM"));
    assert!(p.is_allowed("GAMMA-API.POLYMARKET.COM"));
}

#[test]
fn test_subdomain_match() {
    let p = make_policy();
    assert!(p.is_allowed("foo.stream.binance.com"));
    assert!(p.is_allowed("sub.gamma-api.polymarket.com"));
}

#[test]
fn test_localhost_always_allowed() {
    let p = make_policy();
    assert!(p.is_allowed("localhost"));
    assert!(p.is_allowed("127.0.0.1"));
    assert!(p.is_allowed("::1"));
}

#[test]
fn test_ip_whitelist() {
    let mut p = make_policy();
    p.allowed_ips.insert("52.84.123.45".parse().unwrap());
    assert!(p.is_allowed("52.84.123.45"));
    assert!(!p.is_allowed("1.2.3.4"));
}

#[test]
fn test_needs_upstream() {
    let p = make_policy();
    assert!(p.needs_upstream("gamma-api.polymarket.com"));
    assert!(p.needs_upstream("data-api.polymarket.com"));
    assert!(!p.needs_upstream("stream.binance.com")); // allowed but direct
    assert!(!p.needs_upstream("evil.com")); // not in upstream list
}

#[test]
fn test_needs_upstream_subdomain() {
    let p = make_policy();
    assert!(p.needs_upstream("sub.gamma-api.polymarket.com"));
}

#[test]
fn test_no_upstream() {
    let p = Policy {
        allowed_domains: HashSet::from(["example.com".into()]),
        allowed_ips: HashSet::new(),
        upstream: None,
    };
    assert!(!p.needs_upstream("example.com"));
}

#[test]
fn test_parse_proxy_url_socks5() {
    let (t, h, p) = parse_proxy_url("socks5://127.0.0.1:1080").unwrap();
    assert_eq!(t, UpstreamProxyType::Socks5);
    assert_eq!(h, "127.0.0.1");
    assert_eq!(p, 1080);
}

#[test]
fn test_parse_proxy_url_http() {
    let (t, h, p) = parse_proxy_url("http://proxy.example.com:8080").unwrap();
    assert_eq!(t, UpstreamProxyType::Http);
    assert_eq!(h, "proxy.example.com");
    assert_eq!(p, 8080);
}

#[test]
fn test_parse_proxy_url_default_port() {
    let (_, _, p) = parse_proxy_url("socks5://localhost").unwrap();
    assert_eq!(p, 1080);
    let (_, _, p) = parse_proxy_url("http://localhost").unwrap();
    assert_eq!(p, 8080);
}

#[test]
fn test_parse_proxy_url_invalid() {
    assert!(parse_proxy_url("ftp://localhost").is_err());
    assert!(parse_proxy_url("garbage").is_err());
}
