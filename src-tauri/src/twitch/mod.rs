//! Stable Twitch facade. Production modules import explicit dependencies.
#[cfg(test)]
pub(crate) use auth_state::{
    TwitchAuthPollResult, TwitchAuthValidationResult, TwitchDeviceAuthStart,
};
#[cfg(feature = "app")]
mod auth_service;
mod auth_state;
#[cfg(feature = "app")]
mod auth_store;
#[cfg(any(feature = "app", test))]
mod chat_delivery;
#[cfg(feature = "app")]
mod chat_service;
#[cfg(feature = "app")]
pub(crate) mod commands;
mod dedupe;
mod error;
mod eventsub;
mod model;
mod normalization;
mod oauth;
#[cfg(feature = "app")]
mod runtime;
#[cfg(feature = "app")]
mod subscription;

pub use auth_state::TwitchAuthState;
#[cfg(all(test, feature = "app"))]
pub(crate) use auth_store::{AuthCredentialStore, AuthLoadResult};
#[cfg(feature = "app")]
pub(crate) use auth_store::{AuthLoadNotice, AuthLoadReason, TwitchAuthStore};
#[cfg(all(feature = "app", test))]
pub(crate) use chat_service::TwitchConnectionHandle;
#[cfg(feature = "app")]
#[cfg(feature = "app")]
pub(crate) use chat_service::TwitchConnectionOwner;
pub use model::*;
use std::time::Duration;

const TWITCH_DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
const TWITCH_TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const TWITCH_VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";
const TWITCH_USERS_URL: &str = "https://api.twitch.tv/helix/users";
#[cfg(feature = "app")]
const TWITCH_EVENTSUB_SUBSCRIPTIONS_URL: &str =
    "https://api.twitch.tv/helix/eventsub/subscriptions";
#[cfg(feature = "app")]
const TWITCH_EVENTSUB_WS_URL: &str = "wss://eventsub.wss.twitch.tv/ws?keepalive_timeout_seconds=30";
#[cfg(feature = "app")]
const EVENTSUB_BACKOFF_RESET_STABLE_DURATION: Duration = Duration::from_secs(30);
#[cfg(feature = "app")]
const EVENTSUB_RECONNECT_HANDOVER_TIMEOUT: Duration = Duration::from_secs(25);
const CHAT_READ_SCOPE: &str = "user:read:chat";
const REQUIRED_TWITCH_SCOPES: &[&str] = &[CHAT_READ_SCOPE];
const KEYRING_SERVICE: &str = "rice.twitch.oauth";
const KEYRING_ACCOUNT: &str = "default";
const CHANNEL_CHAT_MESSAGE_TYPE: &str = "channel.chat.message";
const CHANNEL_CHAT_MESSAGE_VERSION: &str = "1";
const DEDUPE_CACHE_LIMIT: usize = 5_000;
const DEDUPE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const TWITCH_HTTP_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(feature = "app")]
const TWITCH_WS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(all(feature = "app", target_os = "linux"))]
const LEGACY_AUTH_DIR: &str = ".rice";
#[cfg(all(feature = "app", target_os = "linux"))]
const LEGACY_AUTH_FILE: &str = "twitch-auth.json";

// Compatibility imports for existing regression/native suites, not production wiring.
#[cfg(all(test, feature = "app"))]
use crate::app_events::{AppLogLevel, TwitchActiveConnection, TwitchStatus, TwitchStatusDomain};
#[cfg(all(test, feature = "app"))]
use auth_service::*;
#[cfg(test)]
use auth_state::*;
#[cfg(all(test, feature = "app"))]
use auth_store::*;
#[cfg(all(test, feature = "app"))]
use chrono::Utc;
#[cfg(test)]
use dedupe::MessageDedupe;
#[cfg(test)]
use error::*;
#[cfg(test)]
use eventsub::*;
#[cfg(all(test, feature = "app"))]
use futures_util::StreamExt;
#[cfg(test)]
use normalization::{normalize_chat_message, EventSubEnvelope};
#[cfg(test)]
use oauth::*;
#[cfg(all(test, feature = "app"))]
use std::collections::VecDeque;
#[cfg(all(test, feature = "app"))]
use subscription::*;
#[cfg(all(test, feature = "app"))]
use tokio_tungstenite::tungstenite::Message;

#[cfg(all(test, feature = "app"))]
mod service_tests;
#[cfg(all(test, feature = "app"))]
mod test_harness;
#[cfg(test)]
mod tests;
#[cfg(all(test, feature = "app", target_os = "windows"))]
mod windows_store_tests;
