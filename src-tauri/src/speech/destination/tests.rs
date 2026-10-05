use super::*;
use crate::speech::bouyomi::{BouyomiAdapter, BouyomiControlCommand, BouyomiTalkConfig};
use crate::speech::FailureCode;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Resolver(Mutex<Vec<IpAddr>>);
impl Resolver {
    fn new(ips: &[&str]) -> Arc<Self> {
        Arc::new(Self(Mutex::new(
            ips.iter().map(|ip| ip.parse().unwrap()).collect(),
        )))
    }
    fn change(&self, ips: &[&str]) {
        *self.0.lock().unwrap() = ips.iter().map(|ip| ip.parse().unwrap()).collect();
    }
}
impl EndpointResolver for Resolver {
    fn resolve<'a>(
        &'a self,
        endpoint: &'a BouyomiAddress,
    ) -> SpeechFuture<'a, Result<Vec<SocketAddr>, ValidationError>> {
        Box::pin(async move {
            Ok(self
                .0
                .lock()
                .unwrap()
                .iter()
                .map(|ip| SocketAddr::new(*ip, endpoint.port()))
                .collect())
        })
    }
}
struct Consent {
    answer: bool,
    calls: AtomicUsize,
    message: Mutex<String>,
    rebind: Option<Arc<Resolver>>,
}
impl Consent {
    fn new(answer: bool) -> Self {
        Self {
            answer,
            calls: AtomicUsize::new(0),
            message: Mutex::new(String::new()),
            rebind: None,
        }
    }
}
impl EndpointConsent for Consent {
    fn ask(&self, message: String) -> SpeechFuture<'_, Result<bool, ValidationError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.message.lock().unwrap() = message;
            if let Some(resolver) = &self.rebind {
                resolver.change(&["10.0.0.2"]);
            }
            Ok(self.answer)
        })
    }
}
fn endpoint() -> BouyomiAddress {
    BouyomiAddress::new("speech.test", 50001).unwrap()
}

#[tokio::test]
async fn normal_mode_checks_every_dns_address_and_all_defined_loopbacks() {
    let resolver = Resolver::new(&["127.0.0.1", "127.255.255.254", "::1", "::ffff:127.0.0.1"]);
    let policy = DestinationPolicy::with_resolver(resolver.clone());
    assert_eq!(
        policy
            .connection_addresses(&endpoint(), false)
            .await
            .unwrap()
            .len(),
        4
    );
    resolver.change(&["127.0.0.1", "10.0.0.1"]);
    assert_eq!(
        policy
            .connection_addresses(&endpoint(), false)
            .await
            .unwrap_err()
            .code,
        "loopbackOnly"
    );
    assert_eq!(
        policy
            .connection_addresses(&endpoint(), true)
            .await
            .unwrap_err()
            .code,
        "consentRequired"
    );
    resolver.change(&["10.0.0.1"]);
    assert_eq!(
        policy
            .connection_addresses(&endpoint(), true)
            .await
            .unwrap_err()
            .code,
        "consentRequired"
    );
}

#[tokio::test]
async fn only_one_native_prompt_can_be_pending_and_declining_reconfirmation_revokes() {
    struct GatedConsent {
        started: tokio::sync::Notify,
        released: tokio::sync::Notify,
    }
    impl EndpointConsent for GatedConsent {
        fn ask(&self, _: String) -> SpeechFuture<'_, Result<bool, ValidationError>> {
            Box::pin(async move {
                self.started.notify_one();
                self.released.notified().await;
                Ok(true)
            })
        }
    }
    let policy = Arc::new(DestinationPolicy::with_resolver(Resolver::new(&[
        "10.0.0.1",
    ])));
    let consent = Arc::new(GatedConsent {
        started: tokio::sync::Notify::new(),
        released: tokio::sync::Notify::new(),
    });
    let pending = {
        let policy = policy.clone();
        let consent = consent.clone();
        tokio::spawn(async move {
            policy
                .prepare_approval(&endpoint(), true, consent.as_ref())
                .await
        })
    };
    consent.started.notified().await;
    assert_eq!(
        policy
            .prepare_approval(&endpoint(), true, &Consent::new(true))
            .await
            .unwrap_err()
            .code,
        "consentBusy"
    );
    consent.released.notify_one();
    policy.install(pending.await.unwrap().unwrap()).unwrap();
    assert!(policy.connection_addresses(&endpoint(), true).await.is_ok());
    // Deterministic clock seam, not a 30s wall-clock sleep or a production bypass.
    *policy.last_prompt.lock().unwrap() = None;
    assert_eq!(
        policy
            .prepare_approval(&endpoint(), true, &Consent::new(false))
            .await
            .unwrap_err()
            .code,
        "consentDeclined"
    );
    assert!(policy
        .connection_addresses(&endpoint(), true)
        .await
        .is_err());
}

#[tokio::test]
async fn native_consent_is_bound_to_endpoint_all_addresses_and_this_process_only() {
    let resolver = Resolver::new(&["10.0.0.1", "fd00::1"]);
    let policy = DestinationPolicy::with_resolver(resolver.clone());
    let consent = Consent::new(true);
    let approval = policy
        .prepare_approval(&endpoint(), true, &consent)
        .await
        .unwrap();
    let message = consent.message.lock().unwrap().clone();
    for required in [
        "speech.test:50001",
        "10.0.0.1:50001",
        "[fd00::1]:50001",
        "Twitchユーザー名",
        "チャット本文",
        "平文TCP",
        "相手認証",
        "VPN",
        "この起動中",
    ] {
        assert!(message.contains(required), "{required}");
    }
    policy.install(approval).unwrap();
    assert_eq!(
        policy
            .connection_addresses(&endpoint(), true)
            .await
            .unwrap()
            .len(),
        2
    );
    // Inspect identity differences without an unrelated request revoking the
    // grant used below to verify an observed DNS change.
    assert_ne!(
        policy.identity(&endpoint()).await.unwrap(),
        policy
            .identity(&BouyomiAddress::new("other.test", 50001).unwrap())
            .await
            .unwrap()
    );
    assert_ne!(
        policy.identity(&endpoint()).await.unwrap(),
        policy
            .identity(&BouyomiAddress::new("speech.test", 50002).unwrap())
            .await
            .unwrap()
    );
    assert!(DestinationPolicy::with_resolver(resolver.clone())
        .connection_addresses(&endpoint(), true)
        .await
        .is_err());
    resolver.change(&["10.0.0.2", "fd00::1"]);
    assert!(policy
        .connection_addresses(&endpoint(), true)
        .await
        .is_err());
    resolver.change(&["10.0.0.1", "fd00::1"]);
    assert!(policy
        .connection_addresses(&endpoint(), true)
        .await
        .is_err());
    *policy.last_prompt.lock().unwrap() = None;
    policy
        .install(
            policy
                .prepare_approval(&endpoint(), true, &consent)
                .await
                .unwrap(),
        )
        .unwrap();
    assert_eq!(consent.calls.load(Ordering::SeqCst), 2);
    assert!(policy.connection_addresses(&endpoint(), true).await.is_ok());
    resolver.change(&["127.0.0.1"]);
    assert!(policy.connection_addresses(&endpoint(), true).await.is_ok());
    resolver.change(&["10.0.0.1", "fd00::1"]);
    assert!(policy
        .connection_addresses(&endpoint(), true)
        .await
        .is_err());
    policy.revoke();
    assert!(policy
        .connection_addresses(&endpoint(), true)
        .await
        .is_err());
}

#[tokio::test]
async fn another_host_or_port_revokes_the_grant_instead_of_reusing_it() {
    for other in [
        BouyomiAddress::new("other.test", 50001).unwrap(),
        BouyomiAddress::new("speech.test", 50002).unwrap(),
    ] {
        let policy = DestinationPolicy::with_resolver(Resolver::new(&["10.0.0.1"]));
        let consent = Consent::new(true);
        policy
            .install(
                policy
                    .prepare_approval(&endpoint(), true, &consent)
                    .await
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            policy
                .connection_addresses(&other, true)
                .await
                .unwrap_err()
                .code,
            "consentRequired"
        );
        assert_eq!(
            policy
                .connection_addresses(&endpoint(), true)
                .await
                .unwrap_err()
                .code,
            "consentRequired"
        );
        assert_eq!(consent.calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn decline_renderer_flags_and_changed_dns_never_install_approval() {
    let resolver = Resolver::new(&["10.0.0.1"]);
    let policy = DestinationPolicy::with_resolver(resolver.clone());
    let mut consent = Consent::new(false);
    assert_eq!(
        policy
            .prepare_approval(&endpoint(), false, &consent)
            .await
            .unwrap_err()
            .code,
        "remoteModeRequired"
    );
    assert_eq!(consent.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        policy
            .prepare_approval(&endpoint(), true, &consent)
            .await
            .unwrap_err()
            .code,
        "consentDeclined"
    );
    assert_eq!(
        policy
            .prepare_approval(&endpoint(), true, &consent)
            .await
            .unwrap_err()
            .code,
        "consentRateLimited"
    );
    assert_eq!(consent.calls.load(Ordering::SeqCst), 1);
    assert!(policy
        .connection_addresses(&endpoint(), true)
        .await
        .is_err());
    let changed = DestinationPolicy::with_resolver(resolver.clone());
    consent.answer = true;
    consent.rebind = Some(resolver);
    assert_eq!(
        changed
            .prepare_approval(&endpoint(), true, &consent)
            .await
            .unwrap_err()
            .code,
        "endpointChanged"
    );
    assert!(changed
        .connection_addresses(&endpoint(), true)
        .await
        .is_err());
}

#[tokio::test]
async fn public_link_local_multicast_unspecified_and_unbounded_dns_fail_closed() {
    for ip in [
        "0.0.0.0",
        "8.8.8.8",
        "169.254.169.254",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "fe80::1",
        "ff02::1",
        "2001:4860:4860::8888",
    ] {
        let policy = DestinationPolicy::with_resolver(Resolver::new(&[ip]));
        let consent = Consent::new(true);
        assert_eq!(
            policy
                .prepare_approval(&endpoint(), true, &consent)
                .await
                .unwrap_err()
                .code,
            "endpointNotAllowed"
        );
        assert_eq!(consent.calls.load(Ordering::SeqCst), 0);
        assert!(policy
            .connection_addresses(&endpoint(), true)
            .await
            .is_err());
    }
    for ips in [vec![], vec!["127.0.0.1"; 17]] {
        let policy = DestinationPolicy::with_resolver(Resolver::new(&ips));
        assert_eq!(
            policy
                .connection_addresses(&endpoint(), false)
                .await
                .unwrap_err()
                .code,
            "invalidResolution"
        );
    }
}

#[tokio::test]
async fn all_bouyomi_send_paths_block_without_native_consent_not_as_network_retry() {
    let policy = Arc::new(DestinationPolicy::with_resolver(Resolver::new(&[
        "10.0.0.1",
    ])));
    let adapter = BouyomiAdapter::new("speech.test", 50001, BouyomiTalkConfig::default())
        .unwrap()
        .with_destination_policy(policy, true);
    let results = [
        adapter.health_probe().await.map(|_| ()),
        adapter.speak("private chat").await,
        adapter.control(BouyomiControlCommand::Pause).await,
        adapter.control(BouyomiControlCommand::Skip).await,
        adapter.control(BouyomiControlCommand::Clear).await,
    ];
    for result in results {
        let error = crate::speech::bouyomi::classify_error(result.unwrap_err());
        assert_eq!(error.code, FailureCode::Configuration);
        assert!(!error.retryable);
        assert!(error.user_message.contains("外部へは送信していません"));
    }
    let diagnostics = adapter.diagnose().await;
    assert!(diagnostics.attempted[0]
        .message
        .contains("外部へは送信していません"));
}
