//! One app-owned HTTP pool; credentials belong to individual requests.
#[cfg(feature = "app")]
use super::TWITCH_EVENTSUB_SUBSCRIPTIONS_URL;
use super::{
    TWITCH_DEVICE_URL, TWITCH_HTTP_TIMEOUT, TWITCH_TOKEN_URL, TWITCH_USERS_URL, TWITCH_VALIDATE_URL,
};

#[derive(Clone)]
pub(crate) struct TwitchHttp {
    pub(super) client: reqwest::Client,
    pub(super) endpoints: HttpEndpoints,
}

#[derive(Clone)]
pub(super) struct HttpEndpoints {
    pub(super) device: String,
    pub(super) token: String,
    pub(super) validate: String,
    pub(super) users: String,
    #[cfg(feature = "app")]
    pub(super) subscriptions: String,
}

impl TwitchHttp {
    pub(crate) fn new() -> anyhow::Result<Self> {
        Ok(Self {
            client: client_builder(TWITCH_HTTP_TIMEOUT).build()?,
            endpoints: HttpEndpoints {
                device: TWITCH_DEVICE_URL.into(),
                token: TWITCH_TOKEN_URL.into(),
                validate: TWITCH_VALIDATE_URL.into(),
                users: TWITCH_USERS_URL.into(),
                #[cfg(feature = "app")]
                subscriptions: TWITCH_EVENTSUB_SUBSCRIPTIONS_URL.into(),
            },
        })
    }
}

// Client clones share reqwest's pool. No default authentication headers: login
// changes and token rotation must take effect on the very next request.
fn client_builder(timeout: std::time::Duration) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .use_rustls_tls()
        .connect_timeout(timeout)
        .timeout(timeout)
}

#[cfg(all(test, feature = "app"))]
#[path = "http_tests.rs"]
mod tests;
