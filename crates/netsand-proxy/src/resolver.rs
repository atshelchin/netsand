use std::net::ToSocketAddrs;

pub fn resolve(host: &str, port: u16) -> Result<Vec<std::net::SocketAddr>, String> {
    let addr_str = format!("{}:{}", host, port);
    addr_str
        .to_socket_addrs()
        .map(|iter| iter.collect())
        .map_err(|e| format!("DNS resolve failed for {}: {}", host, e))
}
