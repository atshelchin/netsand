use netsand_proxy::config::{Policy, UpstreamProxy, UpstreamProxyType};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn make_test_policy() -> Policy {
    Policy {
        allowed_domains: HashSet::from([
            "allowed.example.com".into(),
            "another.allowed.com".into(),
        ]),
        allowed_ips: HashSet::new(),
        upstream: None,
    }
}

async fn start_proxy(policy: Policy) -> String {
    let policy = Arc::new(policy);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let policy = policy.clone();
                tokio::spawn(async move {
                    let _ = netsand_proxy::proxy::handle_client(stream, &policy).await;
                });
            }
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    addr
}

#[tokio::test]
async fn test_connect_blocked_domain() {
    let addr = start_proxy(make_test_policy()).await;
    let mut s = TcpStream::connect(&addr).await.unwrap();
    s.write_all(b"CONNECT evil.com:443 HTTP/1.1\r\nHost: evil.com\r\n\r\n").await.unwrap();

    let mut buf = vec![0u8; 1024];
    let n = s.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.contains("403"), "expected 403, got: {}", resp);
}

#[tokio::test]
async fn test_connect_allowed_domain() {
    let addr = start_proxy(make_test_policy()).await;
    let mut s = TcpStream::connect(&addr).await.unwrap();
    // localhost is always allowed
    s.write_all(b"CONNECT localhost:1 HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();

    let mut buf = vec![0u8; 1024];
    let n = s.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(!resp.contains("403"), "localhost should not be blocked: {}", resp);
}

#[tokio::test]
async fn test_http_blocked_domain() {
    let addr = start_proxy(make_test_policy()).await;
    let mut s = TcpStream::connect(&addr).await.unwrap();
    s.write_all(b"GET http://evil.com/steal HTTP/1.1\r\nHost: evil.com\r\n\r\n").await.unwrap();

    let mut buf = vec![0u8; 1024];
    let n = s.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.contains("403"), "expected 403 for blocked HTTP: {}", resp);
}

#[tokio::test]
async fn test_empty_request_no_crash() {
    let addr = start_proxy(make_test_policy()).await;

    // Send empty and close
    let mut s = TcpStream::connect(&addr).await.unwrap();
    s.shutdown().await.ok();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Proxy should still work
    let mut s2 = TcpStream::connect(&addr).await.unwrap();
    s2.write_all(b"CONNECT evil.com:443 HTTP/1.1\r\n\r\n").await.unwrap();
    let mut buf = vec![0u8; 1024];
    let n = s2.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("403"));
}

#[tokio::test]
async fn test_malformed_request_no_crash() {
    let addr = start_proxy(make_test_policy()).await;
    let mut s = TcpStream::connect(&addr).await.unwrap();
    s.write_all(b"GARBAGE\r\n\r\n").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Still works
    let result = TcpStream::connect(&addr).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_multiple_profiles_different_policies() {
    // Simulate two profiles with different whitelists
    let policy_a = Policy {
        allowed_domains: HashSet::from(["a.example.com".into()]),
        allowed_ips: HashSet::new(),
        upstream: None,
    };
    let policy_b = Policy {
        allowed_domains: HashSet::from(["b.example.com".into()]),
        allowed_ips: HashSet::new(),
        upstream: None,
    };

    let addr_a = start_proxy(policy_a).await;
    let addr_b = start_proxy(policy_b).await;

    // Profile A blocks b.example.com
    let mut s = TcpStream::connect(&addr_a).await.unwrap();
    s.write_all(b"CONNECT b.example.com:443 HTTP/1.1\r\n\r\n").await.unwrap();
    let mut buf = vec![0u8; 1024];
    let n = s.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("403"));

    // Profile B blocks a.example.com
    let mut s = TcpStream::connect(&addr_b).await.unwrap();
    s.write_all(b"CONNECT a.example.com:443 HTTP/1.1\r\n\r\n").await.unwrap();
    let mut buf = vec![0u8; 1024];
    let n = s.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("403"));
}
