use crate::config::{Policy, UpstreamProxy, UpstreamProxyType};
use crate::resolver;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{info, warn};

pub async fn handle_client(client: TcpStream, policy: &Policy) -> Result<(), String> {
    let mut reader = BufReader::new(client);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await.map_err(|e| format!("read: {e}"))?;
    let request_line = request_line.trim().to_string();
    if request_line.is_empty() { return Err("empty request".into()); }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 { return Err(format!("malformed: {}", request_line)); }

    if parts[0].eq_ignore_ascii_case("CONNECT") {
        handle_connect(reader, parts[1], policy).await
    } else {
        handle_http(reader, &request_line, policy).await
    }
}

async fn handle_connect(mut client: BufReader<TcpStream>, target: &str, policy: &Policy) -> Result<(), String> {
    let (host, port) = parse_host_port(target, 443)?;
    if !policy.is_allowed(&host) {
        warn!("BLOCKED CONNECT {}:{}", host, port);
        drain_headers(&mut client).await;
        let mut inner = client.into_inner();
        inner.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n").await.ok();
        return Err(format!("blocked: {}:{}", host, port));
    }
    drain_headers(&mut client).await;

    let remote = if policy.needs_upstream(&host) {
        let up = policy.upstream.as_ref().unwrap();
        info!("CONNECT {}:{} via {}:{}", host, port, up.host, up.port);
        connect_via_upstream(up, &host, port).await?
    } else {
        info!("CONNECT {}:{} direct", host, port);
        connect_direct(&host, port).await?
    };

    let mut client_stream = client.into_inner();
    client_stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await.map_err(|e| format!("write: {e}"))?;
    relay(client_stream, remote).await
}

async fn handle_http(mut client: BufReader<TcpStream>, request_line: &str, policy: &Policy) -> Result<(), String> {
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 3 { return Err("malformed HTTP".into()); }
    let (host, port, path) = parse_url(parts[1])?;

    if !policy.is_allowed(&host) {
        warn!("BLOCKED HTTP {}:{}", host, port);
        drain_headers(&mut client).await;
        let mut inner = client.into_inner();
        inner.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n").await.ok();
        return Err(format!("blocked: {}:{}", host, port));
    }

    let mut headers = String::new();
    loop {
        let mut line = String::new();
        client.read_line(&mut line).await.map_err(|e| format!("read: {e}"))?;
        if line.trim().is_empty() { break; }
        if !line.to_lowercase().starts_with("proxy-") { headers.push_str(&line); }
    }

    let mut remote = if policy.needs_upstream(&host) {
        let up = policy.upstream.as_ref().unwrap();
        info!("HTTP {} {}:{}{} via upstream", parts[0], host, port, path);
        connect_via_upstream(up, &host, port).await?
    } else {
        info!("HTTP {} {}:{}{} direct", parts[0], host, port, path);
        connect_direct(&host, port).await?
    };

    let rewritten = format!("{} {} {}\r\n{}\r\n", parts[0], path, parts[2], headers);
    remote.write_all(rewritten.as_bytes()).await.map_err(|e| format!("write: {e}"))?;
    let client_stream = client.into_inner();
    relay(client_stream, remote).await
}

async fn connect_direct(host: &str, port: u16) -> Result<TcpStream, String> {
    let addrs = tokio::task::spawn_blocking({
        let host = host.to_string();
        move || resolver::resolve(&host, port)
    }).await.map_err(|e| e.to_string())?.map_err(|e| e)?;
    TcpStream::connect(addrs.as_slice()).await.map_err(|e| format!("connect {}:{}: {e}", host, port))
}

async fn connect_via_upstream(upstream: &UpstreamProxy, target_host: &str, target_port: u16) -> Result<TcpStream, String> {
    let proxy_addrs = tokio::task::spawn_blocking({
        let h = upstream.host.clone(); let p = upstream.port;
        move || resolver::resolve(&h, p)
    }).await.map_err(|e| e.to_string())?.map_err(|e| e)?;

    let mut stream = TcpStream::connect(proxy_addrs.as_slice()).await
        .map_err(|e| format!("connect upstream {}:{}: {e}", upstream.host, upstream.port))?;

    match upstream.proxy_type {
        UpstreamProxyType::Http => {
            let req = format!("CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n\r\n", target_host, target_port, target_host, target_port);
            stream.write_all(req.as_bytes()).await.map_err(|e| format!("write CONNECT: {e}"))?;
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.map_err(|e| format!("read: {e}"))?;
            if !String::from_utf8_lossy(&buf[..n]).contains("200") {
                return Err(format!("upstream rejected CONNECT to {}:{}", target_host, target_port));
            }
        }
        UpstreamProxyType::Socks5 => {
            // Greeting
            stream.write_all(&[0x05, 0x01, 0x00]).await.map_err(|e| format!("socks5: {e}"))?;
            let mut resp = [0u8; 2];
            stream.read_exact(&mut resp).await.map_err(|e| format!("socks5: {e}"))?;
            if resp[0] != 0x05 || resp[1] != 0x00 { return Err("socks5 auth failed".into()); }

            // Connect request
            let hb = target_host.as_bytes();
            let mut req = Vec::with_capacity(7 + hb.len());
            req.extend_from_slice(&[0x05, 0x01, 0x00, 0x03, hb.len() as u8]);
            req.extend_from_slice(hb);
            req.push((target_port >> 8) as u8);
            req.push((target_port & 0xff) as u8);
            stream.write_all(&req).await.map_err(|e| format!("socks5: {e}"))?;

            // Response
            let mut rh = [0u8; 4];
            stream.read_exact(&mut rh).await.map_err(|e| format!("socks5: {e}"))?;
            if rh[1] != 0x00 { return Err(format!("socks5 connect failed: 0x{:02x}", rh[1])); }
            // Drain bound address
            match rh[3] {
                0x01 => { let mut d = [0u8; 6]; stream.read_exact(&mut d).await.ok(); }
                0x03 => { let mut l = [0u8; 1]; stream.read_exact(&mut l).await.ok(); let mut d = vec![0u8; l[0] as usize + 2]; stream.read_exact(&mut d).await.ok(); }
                0x04 => { let mut d = [0u8; 18]; stream.read_exact(&mut d).await.ok(); }
                _ => {}
            }
        }
    }
    Ok(stream)
}

async fn relay(client: TcpStream, remote: TcpStream) -> Result<(), String> {
    let (mut cr, mut cw) = client.into_split();
    let (mut rr, mut rw) = remote.into_split();
    tokio::select! {
        r = tokio::io::copy(&mut cr, &mut rw) => { r.map_err(|e| format!("c→r: {e}"))?; }
        r = tokio::io::copy(&mut rr, &mut cw) => { r.map_err(|e| format!("r→c: {e}"))?; }
    }
    Ok(())
}

fn parse_host_port(target: &str, default_port: u16) -> Result<(String, u16), String> {
    if let Some(pos) = target.rfind(':') {
        let host = &target[..pos];
        let port: u16 = target[pos + 1..].parse().map_err(|_| format!("bad port: {}", target))?;
        Ok((host.to_string(), port))
    } else {
        Ok((target.to_string(), default_port))
    }
}

fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let without_scheme = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")).unwrap_or(url);
    let (host_port, path) = match without_scheme.find('/') {
        Some(pos) => (&without_scheme[..pos], &without_scheme[pos..]),
        None => (without_scheme, "/"),
    };
    let (host, port) = parse_host_port(host_port, 80)?;
    Ok((host, port, path.to_string()))
}

async fn drain_headers(reader: &mut BufReader<TcpStream>) {
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => { if line.trim().is_empty() { break; } }
        }
    }
}
