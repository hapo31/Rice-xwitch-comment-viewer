//! Twitch auth_state responsibility boundary.
use super::REQUIRED_TWITCH_SCOPES;
#[cfg(feature = "app")]
use crate::settings::default_twitch_client_id;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct TwitchAuthState {
    pub(super) generation: u64,
    pub(super) pending: Option<PendingDeviceAuth>,
    pub(super) token: Option<TwitchToken>,
    pub(super) profile: Option<TwitchUserProfile>,
}

#[derive(Debug, Clone)]
pub(super) struct PendingDeviceAuth {
    pub(super) generation: u64,
    pub(super) poll_in_flight: bool,
    pub(super) client_id: String,
    pub(super) device_code: String,
    pub(super) interval: u64,
}

#[derive(Debug, Clone)]
pub(super) struct TwitchToken {
    pub(super) access_token: String,
    pub(super) refresh_token: String,
    pub(super) scopes: Vec<String>,
    pub(super) expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StoredTwitchAuth {
    pub(super) client_id: String,
    pub(super) access_token: String,
    pub(super) refresh_token: String,
    pub(super) scopes: Vec<String>,
    pub(super) expires_in: u64,
    pub(super) profile: TwitchUserProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct TwitchDeviceAuthStart {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub expires_at_ms: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct TwitchUserProfile {
    pub user_id: String,
    pub login: String,
    #[serde(default, skip_serializing)]
    #[cfg_attr(test, ts(skip))]
    pub client_id: String,
    pub scopes: Vec<String>,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum TwitchAuthPollResult {
    Pending {
        message: String,
        interval: u64,
    },
    SlowDown {
        message: String,
        interval: u64,
    },
    Authorized {
        profile: TwitchUserProfile,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(test, ts(optional))]
        storage_warning: Option<String>,
    },
    Denied {
        message: String,
    },
    Expired {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct TwitchAuthValidationResult {
    pub profile: TwitchUserProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_warning: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DeviceCodeResponse {
    pub(super) device_code: String,
    pub(super) user_code: String,
    pub(super) verification_uri: String,
    pub(super) expires_in: u64,
    pub(super) interval: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct TokenResponse {
    pub(super) access_token: String,
    pub(super) refresh_token: String,
    #[serde(default)]
    pub(super) scope: Vec<String>,
    #[serde(default)]
    pub(super) expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ValidateResponse {
    pub(super) client_id: String,
    pub(super) login: String,
    pub(super) user_id: String,
    pub(super) scopes: Vec<String>,
    pub(super) expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct HelixUsersResponse {
    pub(super) data: Vec<HelixUser>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct HelixUser {
    pub(super) id: String,
    pub(super) login: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OAuthErrorResponse {
    pub(super) message: Option<String>,
    pub(super) error: Option<String>,
}

#[cfg(feature = "app")]
#[derive(Debug, Clone)]
pub(super) struct EventSubConnectionParams {
    pub(super) generation: u64,
    pub(super) broadcaster_user_id: String,
    pub(super) broadcaster_login: String,
    pub(super) user_id: String,
}

#[cfg(feature = "app")]
#[derive(Debug, Clone)]
pub(super) struct EventSubAuthCredentials {
    pub(super) client_id: String,
    pub(super) access_token: String,
    pub(super) refresh_token: String,
}

impl From<ValidateResponse> for TwitchUserProfile {
    fn from(value: ValidateResponse) -> Self {
        Self {
            user_id: value.user_id,
            login: value.login,
            client_id: value.client_id,
            scopes: value.scopes,
            expires_in: value.expires_in,
        }
    }
}

impl TwitchAuthState {
    pub(super) fn invalidate_operations(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
        self.generation
    }

    pub(super) fn pending_is_current(&self, generation: u64) -> bool {
        self.generation == generation && self.pending.is_some()
    }
    pub(super) fn profile(&self) -> Option<TwitchUserProfile> {
        self.profile.clone()
    }

    pub(super) fn restore(stored: StoredTwitchAuth) -> anyhow::Result<Self> {
        let mut profile = stored.profile;
        if profile.client_id.trim().is_empty() {
            profile.client_id = stored.client_id.clone();
        }
        let scopes = token_scopes(stored.scopes, &profile);
        ensure_required_twitch_scopes(&scopes)?;
        Ok(Self {
            generation: 0,
            pending: None,
            token: Some(TwitchToken {
                access_token: stored.access_token,
                refresh_token: stored.refresh_token,
                scopes,
                expires_in: stored.expires_in,
            }),
            profile: Some(profile),
        })
    }

    pub(super) fn stored_auth(&self) -> Option<StoredTwitchAuth> {
        let token = self.token.as_ref()?;
        let profile = self.profile.clone()?;
        Some(StoredTwitchAuth {
            client_id: profile.client_id.clone(),
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            scopes: token.scopes.clone(),
            expires_in: token.expires_in,
            profile,
        })
    }

    #[cfg(feature = "app")]
    pub(super) fn eventsub_credentials(&self) -> anyhow::Result<EventSubAuthCredentials> {
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Twitch にログインしていません。"))?;
        let profile = self.profile.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Twitch のユーザー情報がありません。認証を確認してください。")
        })?;
        let client_id = if profile.client_id.trim().is_empty() {
            default_twitch_client_id()
        } else {
            profile.client_id.clone()
        };

        if client_id.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "Twitch Client ID が見つかりません。再ログインしてください。"
            ));
        }

        ensure_required_twitch_scopes(&profile.scopes)?;

        Ok(EventSubAuthCredentials {
            client_id,
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
        })
    }

    pub(super) fn replace_token(
        &mut self,
        token: TokenResponse,
        profile: TwitchUserProfile,
    ) -> anyhow::Result<String> {
        // A refresh response can omit `scope`.  The profile obtained by validating the
        // newly-issued access token is therefore the authoritative source here; do
        // not retain the scopes of the token that was just rejected by EventSub.
        ensure_required_twitch_scopes(&profile.scopes)?;
        let scopes = token_scopes(token.scope, &profile);
        let access_token = token.access_token.clone();
        self.token = Some(TwitchToken {
            access_token,
            refresh_token: token.refresh_token,
            scopes,
            expires_in: token.expires_in,
        });
        self.profile = Some(profile);
        Ok(token.access_token)
    }
}

pub(super) fn token_scopes(scopes: Vec<String>, profile: &TwitchUserProfile) -> Vec<String> {
    if scopes.is_empty() {
        profile.scopes.clone()
    } else {
        scopes
    }
}

pub(super) fn ensure_required_twitch_scopes(scopes: &[String]) -> anyhow::Result<()> {
    let missing_scopes = REQUIRED_TWITCH_SCOPES
        .iter()
        .filter(|required_scope| !scopes.iter().any(|scope| scope == **required_scope))
        .copied()
        .collect::<Vec<_>>();

    if missing_scopes.is_empty() {
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "Twitch 認証に必要な権限がありません: {}。Login から再ログインし、{} を許可してください。",
        missing_scopes.join(", "),
        missing_scopes.join(", "),
    ))
}
