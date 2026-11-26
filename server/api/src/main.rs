use std::net::SocketAddr;
use std::sync::Arc;

use community_api::{build_router, AppState, RateLimits};
use community_application::{AuthService, StarService};
use community_infrastructure::{
    Config, DiscordHttp, OsTokenGenerator, Sha256Hasher, SqliteStore, SystemClock,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    if let Err(error) = run().await {
        tracing::error!("fatal: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let store = SqliteStore::connect(&config.database_url).await?;
    store.migrate().await?;

    let clock = Arc::new(SystemClock);
    let discord = DiscordHttp::new(
        &config.discord_api_base,
        &config.discord_client_id,
        &config.discord_client_secret,
        &config.public_base_url,
    )?;
    let store = Arc::new(store);
    let auth = AuthService {
        clock: clock.clone(),
        tokens: Arc::new(OsTokenGenerator),
        hasher: Arc::new(Sha256Hasher),
        users: store.clone(),
        sessions: store.clone(),
        flows: store.clone(),
        codes: store.clone(),
        discord: Arc::new(discord),
    };
    let state = AppState {
        clock: clock.clone(),
        health: store.clone(),
        auth: Arc::new(auth),
        stars: Arc::new(StarService::new(store.clone())),
        limits: RateLimits::new(clock),
        trust_proxy_headers: config.trust_proxy_headers,
    };

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "community server listening");
    axum::serve(
        listener,
        build_router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("community server stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
