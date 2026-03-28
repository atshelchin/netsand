pub mod config;
pub mod proxy;
pub mod resolver;

use config::Policy;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

/// Run a proxy listener on the given address with the given policy.
/// Returns a JoinHandle that runs until cancelled.
pub async fn run_listener(
    addr: &str,
    policy: Arc<Policy>,
    name: &str,
) -> Result<tokio::task::JoinHandle<()>, String> {
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("bind {}: {e}", addr))?;

    let name = name.to_string();
    info!("[{}] listening on {}", name, addr);

    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let policy = policy.clone();
                    tokio::spawn(async move {
                        if let Err(e) = proxy::handle_client(stream, &policy).await {
                            tracing::debug!("proxy error: {e}");
                        }
                    });
                }
                Err(e) => {
                    error!("[{}] accept error: {e}", name);
                }
            }
        }
    });

    Ok(handle)
}
