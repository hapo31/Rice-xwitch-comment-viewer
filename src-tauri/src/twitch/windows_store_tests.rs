use super::{AuthSecretStore, KeyringAuthStore};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn native_credentials_roundtrip_missing_overwrite_failure_and_cleanup() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let service = format!("rice.ci.native-store.{}.{nonce}", std::process::id());
    let store = KeyringAuthStore {
        service: &service,
        account: "isolated-test",
    };
    struct Cleanup<'a>(&'a KeyringAuthStore<'a>);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.clear_secret();
        }
    }
    let _cleanup = Cleanup(&store);
    // Never access the application's real service/account or real OAuth data.
    assert!(store.load_secret().unwrap().is_none());
    store.save_secret("fake-ci-credential-one").unwrap();
    assert_eq!(
        store.load_secret().unwrap().as_deref(),
        Some("fake-ci-credential-one")
    );
    store.save_secret("fake-ci-credential-two").unwrap();
    assert_eq!(
        store.load_secret().unwrap().as_deref(),
        Some("fake-ci-credential-two")
    );
    // Windows native credential blobs are bounded. A real write failure must
    // not silently succeed or replace the last valid credential.
    let error = store.save_secret(&"x".repeat(16 * 1024)).unwrap_err();
    assert!(error.downcast_ref::<keyring::Error>().is_some());
    assert_eq!(
        store.load_secret().unwrap().as_deref(),
        Some("fake-ci-credential-two")
    );
    store.clear_secret().unwrap();
    assert!(store.load_secret().unwrap().is_none());
    store.clear_secret().unwrap();
}
