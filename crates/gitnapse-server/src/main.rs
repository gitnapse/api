mod convert;
mod routes;
mod service;
mod webui;

use anyhow::{Context, Result};
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use service::ApiService;

#[derive(Debug, Parser)]
#[command(
    name = "gitnapse-server",
    version,
    about = "GitNapse protocol server (HTTP)"
)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 8787)]
    port: u16,
}

/// Build the HTTP router implementing the protocol (v1) + the embedded web UI.
pub fn build_router(service: Arc<ApiService>) -> Router {
    routes::router(service)
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args = Args::parse();
    gitnapse::runtime::ensure_crypto_provider();

    let service = ApiService::from_env()?;
    let app = build_router(Arc::new(service));
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .context("invalid bind address")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;
    println!("GitNapse protocol server listening on http://{addr}");
    axum::serve(listener, app).await.context("server error")
}
