use netsand_proxy::resolver;

#[test]
fn test_resolve_localhost() {
    let addrs = resolver::resolve("localhost", 80).unwrap();
    assert!(!addrs.is_empty());
    // Should contain 127.0.0.1 or ::1
    assert!(addrs.iter().any(|a| a.ip().is_loopback()));
}

#[test]
fn test_resolve_ip_address() {
    let addrs = resolver::resolve("127.0.0.1", 8080).unwrap();
    assert_eq!(addrs.len(), 1);
    assert_eq!(addrs[0].port(), 8080);
    assert_eq!(addrs[0].ip().to_string(), "127.0.0.1");
}

#[test]
fn test_resolve_invalid_host() {
    let result = resolver::resolve("this.domain.definitely.does.not.exist.invalid", 80);
    assert!(result.is_err());
}

#[test]
fn test_resolve_port_preserved() {
    let addrs = resolver::resolve("127.0.0.1", 12345).unwrap();
    assert_eq!(addrs[0].port(), 12345);
}
