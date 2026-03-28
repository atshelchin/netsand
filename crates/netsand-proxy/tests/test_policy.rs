use netsand_proxy::config::{Policy, UpstreamProxy, UpstreamProxyType, parse_proxy_url, parse_host_port};
use std::collections::HashSet;
use std::net::IpAddr;

fn make_policy() -> Policy {
    Policy {
        allowed_domains: HashSet::from([
            "gamma-api.polymarket.com".into(),
            "stream.binance.com".into(),
            "data-api.polymarket.com".into(),
        ]),
        allowed_ips: HashSet::new(),
        upstream: Some(UpstreamProxy {
            proxy_type: UpstreamProxyType::Socks5,
            host: "127.0.0.1".into(),
            port: 1080,
            domains: HashSet::from([
                "gamma-api.polymarket.com".into(),
                "data-api.polymarket.com".into(),
            ]),
            ips: HashSet::new(),
        }),
    }
}

// ── is_allowed ──

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
    assert!(!p.is_allowed("polymarket.com")); // parent domain not in list
    assert!(!p.is_allowed("")); // empty string
}

#[test]
fn test_case_insensitive() {
    let p = make_policy();
    assert!(p.is_allowed("Stream.Binance.COM"));
    assert!(p.is_allowed("GAMMA-API.POLYMARKET.COM"));
    assert!(p.is_allowed("Data-Api.Polymarket.Com"));
}

#[test]
fn test_subdomain_match() {
    let p = make_policy();
    assert!(p.is_allowed("foo.stream.binance.com"));
    assert!(p.is_allowed("sub.gamma-api.polymarket.com"));
    assert!(p.is_allowed("deep.sub.data-api.polymarket.com"));
}

#[test]
fn test_partial_domain_not_allowed() {
    let p = make_policy();
    // "evilstream.binance.com" should NOT match "stream.binance.com"
    // because it's not a proper subdomain (no dot prefix)
    assert!(!p.is_allowed("evilstream.binance.com"));
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
    p.allowed_ips.insert("2001:db8::1".parse().unwrap());
    assert!(p.is_allowed("52.84.123.45"));
    assert!(p.is_allowed("2001:db8::1")); // IPv6
    assert!(!p.is_allowed("1.2.3.4"));
    assert!(!p.is_allowed("52.84.123.46")); // off by one
}

#[test]
fn test_empty_policy_blocks_everything() {
    let p = Policy {
        allowed_domains: HashSet::new(),
        allowed_ips: HashSet::new(),
        upstream: None,
    };
    assert!(!p.is_allowed("anything.com"));
    assert!(p.is_allowed("localhost")); // localhost always allowed
}

// ── needs_upstream ──

#[test]
fn test_needs_upstream() {
    let p = make_policy();
    assert!(p.needs_upstream("gamma-api.polymarket.com"));
    assert!(p.needs_upstream("data-api.polymarket.com"));
    assert!(!p.needs_upstream("stream.binance.com")); // allowed but direct
    assert!(!p.needs_upstream("evil.com"));
}

#[test]
fn test_needs_upstream_subdomain() {
    let p = make_policy();
    assert!(p.needs_upstream("sub.gamma-api.polymarket.com"));
}

#[test]
fn test_needs_upstream_case_insensitive() {
    let p = make_policy();
    assert!(p.needs_upstream("GAMMA-API.POLYMARKET.COM"));
}

#[test]
fn test_needs_upstream_ip() {
    let mut p = make_policy();
    let up = p.upstream.as_mut().unwrap();
    up.ips.insert("10.0.0.1".parse().unwrap());
    assert!(p.needs_upstream("10.0.0.1"));
    assert!(!p.needs_upstream("10.0.0.2"));
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

// ── parse_proxy_url ──

#[test]
fn test_parse_socks5() {
    let (t, h, p) = parse_proxy_url("socks5://127.0.0.1:1080").unwrap();
    assert_eq!(t, UpstreamProxyType::Socks5);
    assert_eq!(h, "127.0.0.1");
    assert_eq!(p, 1080);
}

#[test]
fn test_parse_http() {
    let (t, h, p) = parse_proxy_url("http://proxy.example.com:8080").unwrap();
    assert_eq!(t, UpstreamProxyType::Http);
    assert_eq!(h, "proxy.example.com");
    assert_eq!(p, 8080);
}

#[test]
fn test_parse_default_port() {
    let (_, _, p) = parse_proxy_url("socks5://localhost").unwrap();
    assert_eq!(p, 1080);
    let (_, _, p) = parse_proxy_url("http://localhost").unwrap();
    assert_eq!(p, 8080);
}

#[test]
fn test_parse_invalid_scheme() {
    assert!(parse_proxy_url("ftp://localhost").is_err());
    assert!(parse_proxy_url("https://localhost").is_err());
    assert!(parse_proxy_url("garbage").is_err());
    assert!(parse_proxy_url("").is_err());
}

// ── parse_host_port ──

#[test]
fn test_parse_host_port_with_port() {
    let (h, p) = parse_host_port("example.com:443", 80).unwrap();
    assert_eq!(h, "example.com");
    assert_eq!(p, 443);
}

#[test]
fn test_parse_host_port_default() {
    let (h, p) = parse_host_port("example.com", 80).unwrap();
    assert_eq!(h, "example.com");
    assert_eq!(p, 80);
}

#[test]
fn test_parse_host_port_invalid_port() {
    assert!(parse_host_port("example.com:99999", 80).is_err());
    assert!(parse_host_port("example.com:abc", 80).is_err());
}
