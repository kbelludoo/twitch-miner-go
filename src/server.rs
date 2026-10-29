use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::manager::SharedState;

const DASHBOARD_HTML: &str = include_str!("../src/dashboard.html");

#[derive(Clone)]
pub struct AppState {
    pub shared: Arc<RwLock<SharedState>>,
    pub started_at: std::time::Instant,
}

pub async fn start_server(port: u16, state: AppState) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(handler_root))
        .route("/api/status", get(handler_status))
        .route("/metrics", get(handler_metrics))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    info!("Web dashboard online at http://localhost:{port}");
    println!("[Web Server] Dashboard Twitch Miner Rust online em http://localhost:{port}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handler_root() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(DASHBOARD_HTML.to_owned())
        .expect("building static HTML response is infallible")
}

async fn handler_status(State(state): State<AppState>) -> impl IntoResponse {
    let st = state.shared.read().await;
    let uptime = state.started_at.elapsed();
    let h = uptime.as_secs() / 3600;
    let m = (uptime.as_secs() % 3600) / 60;
    let s = uptime.as_secs() % 60;
    let uptime_str = format!("{h:02}:{m:02}:{s:02}");

    // RAM via /proc/self/status is Linux-only; approximate with allocation info
    let ram_mb = get_ram_mb();

    let body = json!({
        "statuses": st.statuses,
        "logs": st.logs,
        "ram_mb": ram_mb,
        "uptime": uptime_str,
    });
    Json(body)
}

async fn handler_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let st = state.shared.read().await;
    let mut lines = Vec::new();
    lines.push("# HELP twitch_miner_watching Channels currently being watched".to_owned());
    lines.push("# TYPE twitch_miner_watching gauge".to_owned());

    for (username, acc) in &st.statuses {
        for (login, info) in &acc.streamers {
            lines.push(format!(
                "twitch_miner_watching{{account=\"{}\",channel=\"{}\"}} {}",
                username,
                login,
                if info.watching { 1 } else { 0 }
            ));
            lines.push(format!(
                "twitch_miner_points{{account=\"{}\",channel=\"{}\"}} {}",
                username, login, info.points
            ));
        }
    }

    lines.push(String::new());
    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(lines.join("\n"))
        .expect("building metrics response is infallible")
}

fn get_ram_mb() -> f64 {
    // Cross-platform approximation: read from /proc/self/status on Linux,
    // fall back to 0 on other platforms.
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<f64>() {
                            return kb / 1024.0;
                        }
                    }
                }
            }
        }
        0.0
    }
    #[cfg(not(target_os = "linux"))]
    {
        0.0
    }
}
