mod config;
mod manager;
mod pubsub;
mod server;
mod telemetry;
mod twitch_gql;

use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("================================================================================");
    println!("     🟣 TWITCH CHANNEL POINTS MINER RUST (ULTRA-LEVE & ALTA PERFORMANCE) 🟣");
    println!("             Zero Navegadores • Auto-Baú Instantâneo • ~2MB RAM");
    println!("================================================================================");

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.json".to_string());

    let cfg = match config::load_config(&config_path) {
        Ok(c) => c,
        Err(_) => config::load_config("config.example.json")
            .unwrap_or_else(|_| panic!("Failed to load config from {config_path}")),
    };

    let started_at = std::time::Instant::now();
    let shared_state = Arc::new(RwLock::new(manager::SharedState::default()));

    // Start HTTP Web Dashboard
    let app_state = server::AppState {
        shared: shared_state.clone(),
        started_at,
    };
    let port = cfg.settings.web_dashboard_port;
    tokio::spawn(async move {
        if let Err(e) = server::start_server(port, app_state).await {
            eprintln!("[Web Server] Erro no servidor web: {e}");
        }
    });

    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let token_clone = cancellation_token.clone();

    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        println!("\nEncerrando Twitch Miner...");
        token_clone.cancel();
    });

    let mgr = Arc::new(manager::Manager::new(cfg, shared_state));
    mgr.start(cancellation_token).await;

    Ok(())
}
