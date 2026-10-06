//! Cross-module visibility used by the native Windows application fixture.
use crate::twitch::{AuthCredentialStore, AuthLoadResult, TwitchAuthState, TwitchAuthStore};
use std::sync::Arc;

struct NoCredentials;
impl AuthCredentialStore for NoCredentials {
    fn load(&self) -> AuthLoadResult {
        AuthLoadResult {
            auth: None,
            notice: None,
        }
    }
    fn save(&self, _: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        panic!("boundary test must not save credentials")
    }
    fn clear(&self) -> anyhow::Result<()> {
        panic!("boundary test must not clear credentials")
    }
}

#[tokio::test]
async fn sibling_modules_can_inject_credentials_through_the_stable_facade() {
    let store = TwitchAuthStore::with_backend(Arc::new(NoCredentials));
    let loaded = store.load().await.unwrap();
    assert!(loaded.auth.is_none());
    assert!(loaded.notice.is_none());
}
