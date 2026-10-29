use crate::{
    config::{AccountConfig, Config},
    pubsub::PubSubManager,
    telemetry::{AccountStatus, RaidEventData, StreamerStatus},
    twitch_gql::TwitchClient,
};
use chrono::Utc;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use tokio::time;
use tracing::info;

/// Shared mutable state exposed to both manager and HTTP server.
#[derive(Debug, Default)]
pub struct SharedState {
    pub statuses: HashMap<String, AccountStatus>,
    pub logs: Vec<String>,
}

impl SharedState {
    pub fn add_log(&mut self, msg: impl Into<String>) {
        let entry = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), msg.into());
        println!("{entry}");
        self.logs.push(entry);
        if self.logs.len() > 200 {
            let drain_to = self.logs.len() - 200;
            self.logs.drain(..drain_to);
        }
    }
}

pub struct Manager {
    cfg: Config,
    client: TwitchClient,
    started_at: Instant,
    state: Arc<RwLock<SharedState>>,
    /// username → pubsub manager
    pubsubs: Arc<Mutex<HashMap<String, Arc<PubSubManager>>>>,
    /// username → streamer_login → watching
    active_map: Arc<RwLock<HashMap<String, HashMap<String, bool>>>>,
}

impl Manager {
    pub fn new(cfg: Config, state: Arc<RwLock<SharedState>>) -> Self {
        Self {
            cfg,
            client: TwitchClient::new(),
            started_at: Instant::now(),
            state,
            pubsubs: Arc::new(Mutex::new(HashMap::new())),
            active_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Entry point — initialises accounts then drives the check loop.
    pub async fn start(self: Arc<Self>, ctx: tokio_util::sync::CancellationToken) {
        {
            let mut st = self.state.write().await;
            st.add_log(format!(
                "Iniciando Twitch Miner Rust para {} conta(s)...",
                self.cfg.accounts.len()
            ));
        }

        for acc in &self.cfg.accounts {
            self.init_account(acc, ctx.clone()).await;
        }

        // First check immediately
        self.check_all().await;

        // Console ticker every 5s
        let ticker_self = Arc::clone(&self);
        let ticker_ctx = ctx.clone();
        tokio::spawn(async move {
            ticker_self.ticker_loop(ticker_ctx).await;
        });

        // Periodic check loop
        let mut interval = time::interval(Duration::from_secs(
            self.cfg.settings.check_interval_seconds,
        ));
        interval.tick().await; // skip immediate

        loop {
            tokio::select! {
                _ = ctx.cancelled() => break,
                _ = interval.tick() => {
                    self.check_all().await;
                }
            }
        }

        info!("Manager shutting down");
    }

    async fn init_account(&self, acc: &AccountConfig, ctx: tokio_util::sync::CancellationToken) {
        match self.client.get_current_user(&acc.auth_token).await {
            Err(e) => {
                let mut st = self.state.write().await;
                st.add_log(format!("[{}] ❌ Erro autenticando conta: {e}", acc.username));
            }
            Ok((uid, login, _display)) => {
                // Initialise status map
                {
                    let mut st = self.state.write().await;
                    let mut streamers = HashMap::new();
                    for s in &acc.streamers {
                        let clean = s.trim().to_lowercase();
                        if !clean.is_empty() {
                            streamers.insert(
                                clean.clone(),
                                StreamerStatus {
                                    login: clean,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                    st.statuses.insert(
                        acc.username.clone(),
                        AccountStatus {
                            username: acc.username.clone(),
                            user_id: uid.clone(),
                            streamers,
                        },
                    );
                    st.add_log(format!(
                        "[{}] ✅ Autenticado com sucesso na Twitch! (ID: {uid}, Login: {login})",
                        acc.username
                    ));
                }

                {
                    let mut am = self.active_map.write().await;
                    am.insert(acc.username.clone(), HashMap::new());
                }

                // Start PubSub
                let state_claim = Arc::clone(&self.state);
                let client_claim = self.client.clone();
                let token_claim = acc.auth_token.clone();
                let username_claim = acc.username.clone();

                let state_raid = Arc::clone(&self.state);
                let client_raid = self.client.clone();
                let token_raid = acc.auth_token.clone();
                let username_raid = acc.username.clone();
                let active_map_raid = Arc::clone(&self.active_map);
                let auto_join_raid = self.cfg.settings.auto_join_raid;
                let follow_raids = self.cfg.settings.follow_raids;

                let ps = Arc::new(PubSubManager::new(
                    acc.auth_token.clone(),
                    uid.clone(),
                    // on_claim callback
                    move |channel_id, claim_id| {
                        let state = Arc::clone(&state_claim);
                        let client = client_claim.clone();
                        let token = token_claim.clone();
                        let username = username_claim.clone();
                        tokio::spawn(async move {
                            handle_claim_bonus(state, client, token, username, channel_id, claim_id).await;
                        });
                    },
                    // on_raid callback
                    move |source_channel_id, raid| {
                        let state = Arc::clone(&state_raid);
                        let client = client_raid.clone();
                        let token = token_raid.clone();
                        let username = username_raid.clone();
                        let active_map = Arc::clone(&active_map_raid);
                        tokio::spawn(async move {
                            handle_raid(
                                state,
                                client,
                                token,
                                username,
                                active_map,
                                source_channel_id,
                                raid,
                                auto_join_raid,
                                follow_raids,
                            )
                            .await;
                        });
                    },
                ));

                ps.clone().start(ctx.clone());

                let mut psmap = self.pubsubs.lock().await;
                psmap.insert(acc.username.clone(), ps);
            }
        }
    }

    async fn check_all(&self) {
        let accs = self.cfg.accounts.clone();
        for acc in accs {
            let client = self.client.clone();
            let state = Arc::clone(&self.state);
            let active_map = Arc::clone(&self.active_map);
            let pubsubs = Arc::clone(&self.pubsubs);
            let auto_claim = self.cfg.settings.auto_claim_bonus;
            tokio::spawn(async move {
                check_account(client, state, active_map, pubsubs, acc, auto_claim).await;
            });
        }
    }

    async fn ticker_loop(&self, ctx: tokio_util::sync::CancellationToken) {
        let mut interval = time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = ctx.cancelled() => break,
                _ = interval.tick() => {
                    let uptime = self.started_at.elapsed();
                    let h = uptime.as_secs() / 3600;
                    let m = (uptime.as_secs() % 3600) / 60;
                    let s = uptime.as_secs() % 60;

                    let st = self.state.read().await;
                    let am = self.active_map.read().await;

                    let mut active_list: Vec<String> = Vec::new();
                    let mut total_points: i64 = 0;

                    for (username, streamer_map) in &*am {
                        if let Some(acc_st) = st.statuses.get(username) {
                            for (streamer, watching) in streamer_map {
                                if *watching {
                                    let pts = acc_st
                                        .streamers
                                        .get(streamer)
                                        .map(|s| s.points)
                                        .unwrap_or(0);
                                    total_points += pts;
                                    active_list.push(format!("{streamer} ({pts} pts)"));
                                }
                            }
                        }
                    }

                    println!(
                        "[Ticker] Uptime: {h:02}:{m:02}:{s:02} | Canais: {} | Total Pontos: {total_points} | Assistindo: {:?}",
                        active_list.len(),
                        active_list
                    );
                }
            }
        }
    }
}

// ── Free functions (handlers) ─────────────────────────────────────────────────

async fn check_account(
    client: TwitchClient,
    state: Arc<RwLock<SharedState>>,
    active_map: Arc<RwLock<HashMap<String, HashMap<String, bool>>>>,
    pubsubs: Arc<Mutex<HashMap<String, Arc<PubSubManager>>>>,
    acc: AccountConfig,
    auto_claim: bool,
) {
    let max_slots = if acc.max_concurrent == 0 {
        5
    } else {
        acc.max_concurrent
    };

    // Count current active slots
    let active_count = {
        let am = active_map.read().await;
        am.get(&acc.username)
            .map(|m| m.values().filter(|&&v| v).count())
            .unwrap_or(0)
    };

    let mut current_active = active_count;

    for streamer in &acc.streamers {
        let clean = streamer.trim().to_lowercase();
        if clean.is_empty() {
            continue;
        }

        let info = match client.get_channel_info(&acc.auth_token, &clean).await {
            Ok(i) => i,
            Err(_) => continue,
        };

        let was_watching = {
            let am = active_map.read().await;
            am.get(&acc.username)
                .and_then(|m| m.get(&clean))
                .copied()
                .unwrap_or(false)
        };

        let now_watching;

        if info.is_live {
            if !was_watching && current_active < max_slots {
                now_watching = true;
                current_active += 1;
                {
                    let mut am = active_map.write().await;
                    am.entry(acc.username.clone())
                        .or_default()
                        .insert(clean.clone(), true);
                }
                let mut st = state.write().await;
                st.add_log(format!(
                    "[{}] ⭐ Assistindo live de '{}'! Pontos: {}",
                    acc.username, info.display_name, info.points
                ));
            } else {
                now_watching = was_watching;
            }
        } else {
            if was_watching {
                current_active = current_active.saturating_sub(1);
                {
                    let mut am = active_map.write().await;
                    am.entry(acc.username.clone())
                        .or_default()
                        .insert(clean.clone(), false);
                }
                let mut st = state.write().await;
                st.add_log(format!(
                    "[{}] 📴 Live de '{}' finalizou; slot liberado.",
                    acc.username, info.display_name
                ));
            }
            now_watching = false;
        }

        // Subscribe to raid topic if watching
        if now_watching && !info.id.is_empty() {
            let psmap = pubsubs.lock().await;
            if let Some(ps) = psmap.get(&acc.username) {
                ps.listen_topic(format!("raid.{}", info.id)).await;
            }
        }

        // Claim if available
        if !info.claim_id.is_empty() && auto_claim {
            let client2 = client.clone();
            let state2 = Arc::clone(&state);
            let token = acc.auth_token.clone();
            let username = acc.username.clone();
            let channel_id = info.id.clone();
            let claim_id = info.claim_id.clone();
            tokio::spawn(async move {
                handle_claim_bonus(state2, client2, token, username, channel_id, claim_id).await;
            });
        }

        // Update status
        {
            let mut st = state.write().await;
            let status = st
                .statuses
                .entry(acc.username.clone())
                .or_insert_with(|| AccountStatus {
                    username: acc.username.clone(),
                    user_id: String::new(),
                    streamers: HashMap::new(),
                });
            let entry = status.streamers.entry(clean.clone()).or_insert_with(|| StreamerStatus {
                login: clean.clone(),
                ..Default::default()
            });
            entry.display_name = info.display_name;
            entry.is_live = info.is_live;
            entry.title = info.stream_title;
            entry.viewers = info.viewer_count;
            entry.points = info.points;
            entry.watching = now_watching;
        }
    }
}

async fn handle_claim_bonus(
    state: Arc<RwLock<SharedState>>,
    client: TwitchClient,
    token: String,
    username: String,
    channel_id: String,
    claim_id: String,
) {
    match client
        .claim_bonus_points(&token, &channel_id, &claim_id)
        .await
    {
        Err(_) => {}
        Ok(new_points) => {
            let mut st = state.write().await;
            st.add_log(format!(
                "[{username}] 🎁 Baú de bônus resgatado (+50 pts)! Novo saldo: {new_points} pontos."
            ));
        }
    }
}

async fn handle_raid(
    state: Arc<RwLock<SharedState>>,
    client: TwitchClient,
    token: String,
    username: String,
    active_map: Arc<RwLock<HashMap<String, HashMap<String, bool>>>>,
    _source_channel_id: String,
    raid: RaidEventData,
    auto_join_raid: bool,
    follow_raids: bool,
) {
    {
        let mut st = state.write().await;
        st.add_log(format!(
            "[{username}] 🚀 RAID DETECTADA! O streamer iniciou raid para '{}' (ID: {})! Participando (+250 pts)...",
            raid.target_display_name, raid.id
        ));
    }

    if auto_join_raid {
        match client.join_raid(&token, &raid.id).await {
            Err(e) => {
                let mut st = state.write().await;
                st.add_log(format!(
                    "[{username}] ℹ Presença na raid registrada via WebSocket ({e})"
                ));
            }
            Ok(()) => {
                let mut st = state.write().await;
                st.add_log(format!(
                    "[{username}] ✅ Confirmação de JoinRaid enviada com sucesso (+250 pontos)!"
                ));
            }
        }
    }

    if follow_raids && !raid.target_login.is_empty() {
        let target = raid.target_login.to_lowercase();
        {
            let mut st = state.write().await;
            st.add_log(format!(
                "[{username}] 🔀 Seguindo raid para o canal '{}' para manter o farm 24/7!",
                raid.target_display_name
            ));
            if let Some(acc_st) = st.statuses.get_mut(&username) {
                acc_st.streamers.entry(target.clone()).or_insert_with(|| StreamerStatus {
                    login: target.clone(),
                    display_name: raid.target_display_name.clone(),
                    is_live: true,
                    watching: true,
                    last_claim_at: Utc::now().to_rfc3339(),
                    ..Default::default()
                });
            }
        }
        {
            let mut am = active_map.write().await;
            am.entry(username.clone()).or_default().insert(target, true);
        }
    }
}
