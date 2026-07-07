use sema_pkg::{build_router, AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Install the tracing subscriber with OpenTelemetry span export when
    // configured; the guard flushes on drop at the end of `main`. Every exporter
    // is a no-op until its env var is set.
    let _otel_guard = sema_pkg::telemetry::init();

    // Prometheus recorder + `/metrics`, when `METRICS_ENABLED=true`.
    let metrics_render = sema_pkg::telemetry::init_metrics();

    let config = sema_pkg::config::Config::from_env();

    // Fail closed on insecure production secrets before accepting any traffic.
    if let Err(e) = config.check_production_secrets() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    // Ensure data directories exist before connecting
    std::fs::create_dir_all(&config.blob_dir).expect("Failed to create blob dir");
    std::fs::create_dir_all("data").ok();

    let db = sema_pkg::db::connect(&config.database_url).await;
    let blobs =
        sema_pkg::blob::BlobStore::from_config(&config).expect("Failed to initialize blob store");
    tracing::info!("Blob storage: {}", blobs.describe());

    // Publish process + application gauges alongside the per-request metrics.
    if metrics_render.is_some() {
        sema_pkg::telemetry::spawn_collectors(db.clone());
    }

    let state = Arc::new(AppState {
        db,
        config,
        blobs,
        metrics_render,
    });
    let addr = format!("{}:{}", state.config.host, state.config.port);

    let app = build_router(state.clone());

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!(
                "Port {} is already in use, finding an available port...",
                state.config.port
            );
            let fallback = format!("{}:0", state.config.host);
            match tokio::net::TcpListener::bind(&fallback).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Error: failed to bind to a fallback port: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: failed to bind to {addr}: {e}");
            std::process::exit(1);
        }
    };

    let local_addr = listener.local_addr().expect("listener has local addr");
    println!("sema-pkg listening on http://{local_addr}");
    tracing::info!("sema-pkg listening on http://{}", local_addr);

    // `into_make_service_with_connect_info` surfaces the peer address so the
    // rate limiter's IP key extractor has a fallback when no forwarded header
    // is present. `with_graceful_shutdown` drains in-flight requests on
    // SIGINT/SIGTERM instead of dropping them.
    let service = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
    if let Err(e) = axum::serve(listener, service)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("Error: server exited unexpectedly: {e}");
        std::process::exit(1);
    }
    tracing::info!("sema-pkg shut down cleanly");
}

/// Resolve when the process is asked to stop, so the server can drain in-flight
/// requests before exiting. Handles Ctrl-C everywhere and SIGTERM on Unix (the
/// signal Docker/Kubernetes/systemd send on stop).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
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
    tracing::info!("shutdown signal received — draining in-flight requests");
}
