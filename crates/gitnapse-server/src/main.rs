mod convert;
mod routes;
mod service;
mod webui;

use anyhow::{Context, Result};
use axum::Router;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;

use routes::router;
use service::ApiService;

#[derive(Debug, Parser)]
#[command(
    name = "gitnapse-server",
    version,
    about = "GitNapse protocol server (HTTP)"
)]
struct Args {
    /// Interface to bind. Keep loopback unless you know what you are doing.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 8787)]
    port: u16,
    /// Require `Authorization: Bearer <token>` on /api/* routes.
    /// Also read from the GITNAPSE_SERVER_TOKEN environment variable.
    #[arg(long, env = "GITNAPSE_SERVER_TOKEN")]
    api_token: Option<String>,
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    log::info!("shutdown signal received, draining connections…");
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let service = Arc::new(ApiService::from_env()?);
    service.warn_if_anonymous();

    let app: Router = router(service.clone(), args.api_token.clone());
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .context("invalid bind address")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;

    let exposed = if matches!(
        args.host.as_str(),
        "127.0.0.1" | "localhost" | "::1" | "[::1]"
    ) {
        String::new()
    } else {
        format!(
            "\nWARNING: binding to a non-loopback interface ({}) — the server proxies the GitHub \
             token of the local user. Prefer 127.0.0.1.",
            args.host
        )
    };
    log::info!("GitNapse protocol server listening on http://{addr}{exposed}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")
}
