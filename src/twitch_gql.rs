use crate::telemetry::{
    ChannelInfo, ClaimResponse, CurrentUserResponse, JoinRaidResponse, TwitchError,
    UserChannelResponse,
};
use reqwest::Client as ReqwestClient;
use serde_json::{json, Value};
use std::time::Duration;

const GQL_URL: &str = "https://gql.twitch.tv/gql";
const CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Twitch GQL HTTP client.
#[derive(Debug, Clone)]
pub struct TwitchClient {
    http: ReqwestClient,
}

impl TwitchClient {
    pub fn new() -> Self {
        let http = ReqwestClient::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest client builder is infallible with valid config");
        Self { http }
    }

    /// POST a GQL payload and deserialize the response.
    async fn post_gql<T>(&self, token: &str, payload: Value) -> Result<T, TwitchError>
    where
        T: serde::de::DeserializeOwned,
    {
        let clean_token = token.trim_start_matches("oauth:");
        let resp = self
            .http
            .post(GQL_URL)
            .header("Client-ID", CLIENT_ID)
            .header("Authorization", format!("OAuth {clean_token}"))
            .header("Content-Type", "application/json")
            .header("User-Agent", USER_AGENT)
            .json(&payload)
            .send()
            .await?;

        let result = resp.json::<T>().await?;
        Ok(result)
    }

    /// `query { currentUser { id login displayName } }`
    pub async fn get_current_user(
        &self,
        token: &str,
    ) -> Result<(String, String, String), TwitchError> {
        let payload = json!({
            "query": "query { currentUser { id login displayName } }"
        });
        let res: CurrentUserResponse = self.post_gql(token, payload).await?;
        let u = res.data.current_user.ok_or(TwitchError::Unauthenticated)?;
        Ok((u.id, u.login, u.display_name))
    }

    /// GetChannelInfo query.
    pub async fn get_channel_info(
        &self,
        token: &str,
        streamer_login: &str,
    ) -> Result<ChannelInfo, TwitchError> {
        let payload = json!({
            "query": r#"query($login: String!) {
              user(login: $login) {
                id login displayName
                stream { id title viewersCount type }
                channel {
                  id
                  self { communityPoints { balance } }
                }
              }
            }"#,
            "variables": { "login": streamer_login.to_lowercase() }
        });

        let res: UserChannelResponse = self.post_gql(token, payload).await?;
        let u = res
            .data
            .user
            .ok_or_else(|| TwitchError::ChannelNotFound(streamer_login.to_owned()))?;

        let is_live = u
            .stream
            .as_ref()
            .map(|s| s.stream_type == "live")
            .unwrap_or(false);

        let stream_title = u
            .stream
            .as_ref()
            .map(|s| s.title.clone())
            .unwrap_or_default();

        let viewer_count = u
            .stream
            .as_ref()
            .map(|s| s.viewers_count)
            .unwrap_or_default();

        let points = u
            .channel
            .as_ref()
            .and_then(|c| c.channel_self.as_ref())
            .and_then(|s| s.community_points.as_ref())
            .map(|p| p.balance)
            .unwrap_or_default();

        Ok(ChannelInfo {
            id: u.id,
            login: u.login,
            display_name: u.display_name,
            is_live,
            stream_title,
            viewer_count,
            points,
            claim_id: String::new(), // populated via PubSub
        })
    }

    /// ClaimBonusPoints mutation.
    pub async fn claim_bonus_points(
        &self,
        token: &str,
        channel_id: &str,
        claim_id: &str,
    ) -> Result<i64, TwitchError> {
        let payload = json!({
            "query": r#"mutation($input: ClaimCommunityPointsInput!) {
              claimCommunityPoints(input: $input) {
                currentPoints pointsEarned
              }
            }"#,
            "variables": {
                "input": { "channelID": channel_id, "claimID": claim_id }
            }
        });

        let res: ClaimResponse = self.post_gql(token, payload).await?;
        if let Some(e) = res.errors.first() {
            return Err(TwitchError::Gql(e.message.clone()));
        }
        let data = res
            .data
            .claim_community_points
            .ok_or(TwitchError::EmptyClaimResponse)?;
        Ok(data.current_points)
    }

    /// JoinRaid mutation.
    pub async fn join_raid(&self, token: &str, raid_id: &str) -> Result<(), TwitchError> {
        let payload = json!({
            "query": r#"mutation($input: JoinRaidInput!) {
              joinRaid(input: $input) { __typename }
            }"#,
            "variables": { "input": { "raidID": raid_id } }
        });

        let res: JoinRaidResponse = self.post_gql(token, payload).await?;
        if let Some(e) = res.errors.first() {
            return Err(TwitchError::Gql(e.message.clone()));
        }
        Ok(())
    }
}

impl Default for TwitchClient {
    fn default() -> Self {
        Self::new()
    }
}
