use crate::telemetry::RaidEventData;
use futures_util::{SinkExt, StreamExt};
use rand::RngCore;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

const PUBSUB_URL: &str = "wss://pubsub-edge.twitch.tv/v1";
const PING_INTERVAL: Duration = Duration::from_secs(4 * 60);
const PONG_TIMEOUT: Duration = Duration::from_secs(10);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

fn random_nonce() -> String {
    let mut buf = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Shared state between the connect loop and callers of `listen_topic`.
struct Inner {
    topics: HashSet<String>,
    new_topic_notify: Arc<Notify>,
}

/// PubSub WebSocket manager — mirrors Go's `PubSubManager`.
pub struct PubSubManager {
    token: String,
    user_id: String,
    inner: Arc<Mutex<Inner>>,
    notify: Arc<Notify>,
    on_claim: Arc<dyn Fn(String, String) + Send + Sync + 'static>,
    on_raid: Arc<dyn Fn(String, RaidEventData) + Send + Sync + 'static>,
}

impl PubSubManager {
    pub fn new(
        token: impl Into<String>,
        user_id: impl Into<String>,
        on_claim: impl Fn(String, String) + Send + Sync + 'static,
        on_raid: impl Fn(String, RaidEventData) + Send + Sync + 'static,
    ) -> Self {
        let user_id = user_id.into();
        let mut topics = HashSet::new();
        topics.insert(format!("community-points-user-v1.{user_id}"));

        let notify = Arc::new(Notify::new());
        let inner = Arc::new(Mutex::new(Inner {
            topics,
            new_topic_notify: Arc::clone(&notify),
        }));

        Self {
            token: token.into(),
            user_id,
            inner,
            notify,
            on_claim: Arc::new(on_claim),
            on_raid: Arc::new(on_raid),
        }
    }

    /// Add a topic to subscribe to; sends LISTEN on the live connection if open.
    pub async fn listen_topic(&self, topic: String) {
        let mut guard = self.inner.lock().await;
        if guard.topics.contains(&topic) {
            return;
        }
        guard.topics.insert(topic);
        guard.new_topic_notify.notify_one();
    }

    /// Spawn the reconnect loop; runs until `ctx` is cancelled.
    pub fn start(self: Arc<Self>, ctx: tokio_util::sync::CancellationToken) {
        tokio::spawn(async move {
            loop {
                if ctx.is_cancelled() {
                    break;
                }
                if let Err(e) = self.connect_and_listen(ctx.clone()).await {
                    warn!("PubSub[{}] disconnected: {e}; reconnecting in 5s", self.user_id);
                }
                tokio::select! {
                    _ = ctx.cancelled() => break,
                    _ = tokio::time::sleep(RECONNECT_DELAY) => {}
                }
            }
            info!("PubSub[{}] loop stopped", self.user_id);
        });
    }

    async fn connect_and_listen(
        &self,
        ctx: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<()> {
        let (ws, _) = connect_async(PUBSUB_URL).await?;
        let (mut sink, mut stream) = ws.split();

        // Send initial LISTEN for all current topics
        {
            let guard = self.inner.lock().await;
            let topics: Vec<String> = guard.topics.iter().cloned().collect();
            let clean_token = self.token.trim_start_matches("oauth:");
            let msg = json!({
                "type": "LISTEN",
                "nonce": random_nonce(),
                "data": { "topics": topics, "auth_token": clean_token }
            });
            sink.send(Message::Text(msg.to_string())).await?;
        }

        info!("PubSub[{}] connected", self.user_id);

        // Ping task
        let (ping_tx, mut ping_rx) = tokio::sync::mpsc::channel::<()>(1);
        let ctx2 = ctx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(PING_INTERVAL);
            interval.tick().await; // skip the immediate first tick
            loop {
                tokio::select! {
                    _ = ctx2.cancelled() => break,
                    _ = interval.tick() => {
                        if ping_tx.send(()).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let mut last_ping: Option<Instant> = None;
        let mut pong_pending = false;

        loop {
            // Build deadline: if we're waiting for a PONG, apply tight timeout
            let read_timeout = if pong_pending {
                Duration::from_secs(1)
            } else {
                Duration::from_secs(5 * 60)
            };

            tokio::select! {
                _ = ctx.cancelled() => return Ok(()),

                // Ping trigger
                _ = ping_rx.recv() => {
                    let ping = json!({"type": "PING"});
                    sink.send(Message::Text(ping.to_string())).await?;
                    last_ping = Some(Instant::now());
                    pong_pending = true;
                }

                // New topic added while connected
                _ = self.notify.notified() => {
                    let guard = self.inner.lock().await;
                    let clean_token = self.token.trim_start_matches("oauth:");
                    // Send LISTEN for every topic (idempotent on Twitch's side)
                    let topics: Vec<String> = guard.topics.iter().cloned().collect();
                    drop(guard);
                    let msg = json!({
                        "type": "LISTEN",
                        "nonce": random_nonce(),
                        "data": { "topics": topics, "auth_token": clean_token }
                    });
                    sink.send(Message::Text(msg.to_string())).await?;
                }

                // Incoming message with timeout
                result = tokio::time::timeout(read_timeout, stream.next()) => {
                    match result {
                        Err(_elapsed) => {
                            if pong_pending {
                                if let Some(t) = last_ping {
                                    if t.elapsed() > PONG_TIMEOUT {
                                        return Err(anyhow::anyhow!("PONG timeout"));
                                    }
                                }
                            }
                        }
                        Ok(None) => return Err(anyhow::anyhow!("stream closed")),
                        Ok(Some(Err(e))) => return Err(e.into()),
                        Ok(Some(Ok(msg))) => {
                            if let Message::Text(text) = msg {
                                self.handle_message(&text);
                            }
                            if pong_pending {
                                // Any message (incl. PONG) resets the timeout
                                pong_pending = false;
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_message(&self, raw: &str) {
        let v: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => return,
        };

        let msg_type = v["type"].as_str().unwrap_or_default();

        match msg_type {
            "PONG" | "RECONNECT" => {} // handled implicitly by the loop
            "RESPONSE" => {
                if let Some(err) = v["error"].as_str() {
                    if !err.is_empty() {
                        error!("PubSub LISTEN error: {err}");
                    }
                }
            }
            "MESSAGE" => {
                let topic = v["data"]["topic"].as_str().unwrap_or_default();
                let message_str = v["data"]["message"].as_str().unwrap_or_default();

                if topic.starts_with("community-points-user-v1") {
                    self.handle_community_points(message_str);
                } else if topic.starts_with("raid.") {
                    let source_channel_id =
                        topic.trim_start_matches("raid.").to_owned();
                    self.handle_raid(source_channel_id, message_str);
                }
            }
            _ => {}
        }
    }

    fn handle_community_points(&self, message_str: &str) {
        let v: Value = match serde_json::from_str(message_str) {
            Ok(v) => v,
            Err(_) => return,
        };
        if v["type"].as_str() != Some("claim-available") {
            return;
        }
        let claim_id = match v["data"]["claim"]["id"].as_str() {
            Some(id) if !id.is_empty() => id.to_owned(),
            _ => return,
        };
        let channel_id = v["data"]["claim"]["channel_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let on_claim = Arc::clone(&self.on_claim);
        tokio::spawn(async move { on_claim(channel_id, claim_id) });
    }

    fn handle_raid(&self, source_channel_id: String, message_str: &str) {
        let v: Value = match serde_json::from_str(message_str) {
            Ok(v) => v,
            Err(_) => return,
        };
        let raid_type = v["type"].as_str().unwrap_or_default();
        if raid_type != "raid_update_v2" && raid_type != "raid_start" {
            return;
        }
        let raid = &v["raid"];
        let id = raid["id"].as_str().unwrap_or_default();
        if id.is_empty() {
            return;
        }
        let event = RaidEventData {
            id: id.to_owned(),
            source_id: raid["source_id"].as_str().unwrap_or_default().to_owned(),
            target_id: raid["target_id"].as_str().unwrap_or_default().to_owned(),
            target_login: raid["target_login"].as_str().unwrap_or_default().to_owned(),
            target_display_name: raid["target_display_name"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        };
        let on_raid = Arc::clone(&self.on_raid);
        tokio::spawn(async move { on_raid(source_channel_id, event) });
    }
}
