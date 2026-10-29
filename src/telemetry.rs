use serde::{Deserialize, Serialize};
use thiserror::Error;

/// All errors that can originate from the GQL / PubSub layer.
#[derive(Debug, Error)]
pub enum TwitchError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("GQL error: {0}")]
    Gql(String),

    #[error("invalid / unauthenticated token")]
    Unauthenticated,

    #[error("channel '{0}' not found")]
    ChannelNotFound(String),

    #[error("empty claim response")]
    EmptyClaimResponse,
}

// ── GQL response types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GqlError {
    pub message: String,
}

// GetCurrentUser
#[derive(Debug, Deserialize)]
pub struct CurrentUserResponse {
    pub data: CurrentUserData,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUserData {
    pub current_user: Option<CurrentUser>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUser {
    pub id: String,
    pub login: String,
    pub display_name: String,
}

// GetChannelInfo
#[derive(Debug, Deserialize)]
pub struct UserChannelResponse {
    pub data: UserChannelData,
}
#[derive(Debug, Deserialize)]
pub struct UserChannelData {
    pub user: Option<UserNode>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserNode {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub stream: Option<StreamNode>,
    pub channel: Option<ChannelNode>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamNode {
    pub id: String,
    pub title: String,
    pub viewers_count: i64,
    #[serde(rename = "type")]
    pub stream_type: String,
}
#[derive(Debug, Deserialize)]
pub struct ChannelNode {
    pub id: String,
    #[serde(rename = "self")]
    pub channel_self: Option<ChannelSelfNode>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSelfNode {
    pub community_points: Option<CommunityPoints>,
}
#[derive(Debug, Deserialize)]
pub struct CommunityPoints {
    pub balance: i64,
}

/// Parsed channel info (domain object).
#[derive(Debug, Clone, Default)]
pub struct ChannelInfo {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub is_live: bool,
    pub stream_title: String,
    pub viewer_count: i64,
    pub points: i64,
    pub claim_id: String,
}

// ClaimBonusPoints
#[derive(Debug, Deserialize)]
pub struct ClaimResponse {
    pub data: ClaimData,
    #[serde(default)]
    pub errors: Vec<GqlError>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimData {
    pub claim_community_points: Option<ClaimResult>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimResult {
    pub current_points: i64,
    pub points_earned: i64,
}

// JoinRaid
#[derive(Debug, Deserialize)]
pub struct JoinRaidResponse {
    pub data: JoinRaidData,
    #[serde(default)]
    pub errors: Vec<GqlError>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRaidData {
    pub join_raid: Option<JoinRaidResult>,
}
#[derive(Debug, Deserialize)]
pub struct JoinRaidResult {
    #[serde(rename = "__typename")]
    pub typename: String,
}

// ── PubSub event payload types ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaidEventData {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub target_login: String,
    pub target_display_name: String,
}

// ── API status types (for HTTP server) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamerStatus {
    pub login: String,
    pub display_name: String,
    pub is_live: bool,
    pub title: String,
    pub viewers: i64,
    pub points: i64,
    pub watching: bool,
    pub last_claim_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub username: String,
    pub user_id: String,
    pub streamers: std::collections::HashMap<String, StreamerStatus>,
}
