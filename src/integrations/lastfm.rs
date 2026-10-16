//! Last.fm scrobbling.
//!
//! Needs an API account (<https://www.last.fm/api/account/create>) and a session key for your
//! user, which `symphony lastfm-login` fetches through Last.fm's browser authorization.

use std::collections::BTreeMap;

use reqwest::Client;
use serde_json::Value;

/// Last.fm does not count tracks shorter than this
const MIN_TRACK_SECS: f64 = 30.0;
/// A track counts after this long even if it is longer than twice as much
const MAX_THRESHOLD_SECS: f64 = 240.0;

pub const DEFAULT_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";

/// Last.fm's rule: tracks of at least 30 s count once `threshold_percent` of them (at most
/// 4 minutes) has been played
pub fn should_scrobble(duration_secs: f64, played_secs: f64, threshold_percent: u8) -> bool {
    if duration_secs < MIN_TRACK_SECS {
        return false;
    }
    let needed = (duration_secs * f64::from(threshold_percent.clamp(1, 100)) / 100.0).min(MAX_THRESHOLD_SECS);
    played_secs >= needed
}

/// A track as Last.fm wants it
#[derive(Debug, Clone, PartialEq)]
pub struct ScrobbleTrack {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub duration_secs: u64,
}

/// Client for one user's account
#[derive(Debug, Clone)]
pub struct LastFm {
    api_key: String,
    api_secret: String,
    session_key: String,
    api_url: String,
    client: Client,
}

impl LastFm {
    /// Client for `session_key` (empty until the user has logged in)
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        session_key: impl Into<String>,
        api_url: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            session_key: session_key.into(),
            api_url: api_url.into(),
            client: Client::new(),
        }
    }

    /// `api_sig`: md5 of every parameter (except `format`) as `name` + `value`, sorted by name,
    /// followed by the shared secret
    pub fn sign(&self, params: &BTreeMap<String, String>) -> String {
        let mut text = String::new();
        for (name, value) in params.iter().filter(|(name, _)| name.as_str() != "format") {
            text.push_str(name);
            text.push_str(value);
        }
        text.push_str(&self.api_secret);
        format!("{:x}", md5::compute(text.as_bytes()))
    }

    /// Call a signed API method
    async fn call(&self, method: &str, mut params: BTreeMap<String, String>, with_session: bool) -> Result<Value, String> {
        params.insert("method".into(), method.into());
        params.insert("api_key".into(), self.api_key.clone());
        if with_session {
            params.insert("sk".into(), self.session_key.clone());
        }
        let signature = self.sign(&params);
        params.insert("api_sig".into(), signature);
        params.insert("format".into(), "json".into());

        let response = self
            .client
            .post(&self.api_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let body: Value = response.json().await.map_err(|e| e.to_string())?;
        if let Some(message) = body["message"].as_str() {
            return Err(format!("Last.fm: {message}"));
        }
        if !status.is_success() {
            return Err(format!("Last.fm answered {status}"));
        }
        Ok(body)
    }

    /// Show `track` as "now playing"
    pub async fn now_playing(&self, track: &ScrobbleTrack) -> Result<(), String> {
        let mut params = track_params(track, "");
        if track.duration_secs > 0 {
            params.insert("duration".into(), track.duration_secs.to_string());
        }
        self.call("track.updateNowPlaying", params, true).await.map(drop)
    }

    /// Record that `track` was played, starting at unix time `started_at`
    pub async fn scrobble(&self, track: &ScrobbleTrack, started_at: i64) -> Result<(), String> {
        let mut params = track_params(track, "[0]");
        params.insert("timestamp[0]".into(), started_at.to_string());
        self.call("track.scrobble", params, true).await.map(drop)
    }

    /// Step 1 of the login: a token the user authorizes in the browser at
    /// `https://www.last.fm/api/auth/?api_key=<key>&token=<token>`
    pub async fn request_token(&self) -> Result<String, String> {
        let body = self.call("auth.getToken", BTreeMap::new(), false).await?;
        body["token"].as_str().map(str::to_string).ok_or_else(|| "Last.fm sent no token".to_string())
    }

    /// Step 2: trade the authorized token for `(username, session key)`
    pub async fn request_session(&self, token: &str) -> Result<(String, String), String> {
        let params = BTreeMap::from([("token".to_string(), token.to_string())]);
        let body = self.call("auth.getSession", params, false).await?;
        match (body["session"]["name"].as_str(), body["session"]["key"].as_str()) {
            (Some(name), Some(key)) => Ok((name.to_string(), key.to_string())),
            _ => Err("Last.fm sent no session".to_string()),
        }
    }

    /// Where the user authorizes `token`
    pub fn auth_url(&self, token: &str) -> String {
        format!("https://www.last.fm/api/auth/?api_key={}&token={token}", self.api_key)
    }
}

/// `artist`, `track`, `album` parameters; `suffix` is `[0]` for the scrobble batch form
fn track_params(track: &ScrobbleTrack, suffix: &str) -> BTreeMap<String, String> {
    let mut params = BTreeMap::from([
        (format!("artist{suffix}"), track.artist.clone()),
        (format!("track{suffix}"), track.title.clone()),
    ]);
    if !track.album.is_empty() {
        params.insert(format!("album{suffix}"), track.album.clone());
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_an_independently_computed_md5() {
        let client = LastFm::new("xxx", "zzz", "", DEFAULT_API_URL);
        let params = BTreeMap::from([
            ("token".to_string(), "yyy".to_string()),
            ("method".to_string(), "auth.getSession".to_string()),
            ("api_key".to_string(), "xxx".to_string()),
            ("format".to_string(), "json".to_string()), // never signed
        ]);
        // python: md5("api_keyxxxmethodauth.getSessiontokenyyyzzz")
        assert_eq!(client.sign(&params), "75df1fdb6b738160924a52b1732fdde7");
    }

    #[test]
    fn scrobble_rule_follows_last_fm() {
        // under 30 s never counts
        assert!(!should_scrobble(25.0, 25.0, 50));
        // half of a 3 minute track
        assert!(!should_scrobble(180.0, 89.0, 50));
        assert!(should_scrobble(180.0, 90.0, 50));
        // long tracks count after 4 minutes
        assert!(!should_scrobble(1200.0, 239.0, 50));
        assert!(should_scrobble(1200.0, 240.0, 50));
        // a configured 80% threshold
        assert!(!should_scrobble(100.0, 79.0, 80));
        assert!(should_scrobble(100.0, 80.0, 80));
    }
}
