use std::net::SocketAddr;

use anyhow::Context;

use jsonrpsee::server::{BatchRequestConfig, ServerConfig};

use oracle::api::{OracleApiServer, OracleImpl};
use oracle::config::AppConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("oracle=debug".parse().unwrap()),
        )
        .init();

    let config = AppConfig::from_env()?;
    let addr: SocketAddr = config
        .bind_addr
        .as_deref()
        .unwrap_or("127.0.0.1:3000")
        .parse()
        .context("invalid bind_addr")?;

    // Batch is not supported: one JSON-RPC object per request, so a caller
    // cannot smuggle a burst past per-request work limits.
    let server_cfg = ServerConfig::builder()
        .set_batch_request_config(BatchRequestConfig::Disabled)
        .build();
    let server = jsonrpsee::server::Server::builder()
        .set_config(server_cfg)
        .build(addr)
        .await?;
    let module = OracleImpl::new(config).into_rpc();

    tracing::info!("JSON-RPC listening on http://{}", server.local_addr()?);
    server.start(module).stopped().await;
    Ok(())
}
