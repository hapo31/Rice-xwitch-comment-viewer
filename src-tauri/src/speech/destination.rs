//! Every Bouyomi TCP connection passes this backend-only privacy boundary.
//! Approval is process-local, bound to the exact endpoint and resolved addresses.
use super::bouyomi::BouyomiAddress;
use super::SpeechFuture;
use crate::settings::validation::ValidationError;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub trait EndpointResolver: Send + Sync {
    fn resolve<'a>(
        &'a self,
        endpoint: &'a BouyomiAddress,
    ) -> SpeechFuture<'a, Result<Vec<SocketAddr>, ValidationError>>;
}
pub trait EndpointConsent: Send + Sync {
    fn ask(&self, message: String) -> SpeechFuture<'_, Result<bool, ValidationError>>;
}
struct SystemResolver;
impl EndpointResolver for SystemResolver {
    fn resolve<'a>(
        &'a self,
        endpoint: &'a BouyomiAddress,
    ) -> SpeechFuture<'a, Result<Vec<SocketAddr>, ValidationError>> {
        Box::pin(async move {
            if let Ok(ip) = endpoint.host().parse::<IpAddr>() {
                return Ok(vec![SocketAddr::new(ip, endpoint.port())]);
            }
            let addresses = tokio::time::timeout(
                Duration::from_secs(2),
                tokio::net::lookup_host((endpoint.host(), endpoint.port())),
            )
            .await
            .map_err(|_| {
                error(
                    "resolutionTimeout",
                    "接続先のDNS確認がタイムアウトしました。ホストを確認してください。",
                )
            })?
            .map_err(|_| {
                error(
                    "resolutionFailed",
                    "接続先のDNS解決に失敗しました。ホストを確認してください。",
                )
            })?;
            let addresses: Vec<_> = addresses.take(17).collect();
            if addresses.is_empty() || addresses.len() > 16 {
                return Err(error(
                    "tooManyAddresses",
                    "接続先の解決結果は1〜16アドレスにしてください。",
                ));
            }
            Ok(addresses)
        })
    }
}
fn error(code: &'static str, message: &str) -> ValidationError {
    let mut error = ValidationError::new("speech.bouyomiHost", code, message);
    error.recovery = "同じPCでは127.0.0.1/::1を使ってください。外部接続は接続先を保存し、ネイティブ確認で再許可してください。";
    error
}
fn normalized_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ip)),
        _ => ip,
    }
}
fn loopback(ip: IpAddr) -> bool {
    normalized_ip(ip).is_loopback()
}
fn private_unicast(ip: IpAddr) -> bool {
    match normalized_ip(ip) {
        IpAddr::V4(ip) => ip.is_private(),
        IpAddr::V6(ip) => ip.is_unique_local(),
    }
}
fn private_addresses(addresses: &[SocketAddr]) -> Result<(), ValidationError> {
    if addresses
        .iter()
        .any(|address| !loopback(address.ip()) && !private_unicast(address.ip()))
    {
        return Err(error("endpointNotAllowed", "外部接続はprivate LAN/VPNのunicast宛先だけです。public・link-local・multicast・未指定宛先へは接続しません。"));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EndpointIdentity {
    host: String,
    port: u16,
    addresses: Vec<SocketAddr>,
}
/// Not deserializable and fields are private: only native consent creates this.
#[derive(Debug)]
pub(crate) struct PreparedApproval(EndpointIdentity);
pub struct DestinationPolicy {
    resolver: Arc<dyn EndpointResolver>,
    approved: Mutex<Option<EndpointIdentity>>,
    pending: Arc<tokio::sync::Mutex<()>>,
    last_prompt: Mutex<Option<Instant>>,
}
impl std::fmt::Debug for DestinationPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DestinationPolicy { backend_only_approval }")
    }
}
impl Default for DestinationPolicy {
    fn default() -> Self {
        Self::with_resolver(Arc::new(SystemResolver))
    }
}
impl DestinationPolicy {
    fn with_resolver(resolver: Arc<dyn EndpointResolver>) -> Self {
        Self {
            resolver,
            approved: Mutex::new(None),
            pending: Arc::default(),
            last_prompt: Mutex::new(None),
        }
    }
    async fn identity(
        &self,
        endpoint: &BouyomiAddress,
    ) -> Result<EndpointIdentity, ValidationError> {
        let mut addresses = self.resolver.resolve(endpoint).await?;
        if addresses.is_empty()
            || addresses.len() > 16
            || addresses
                .iter()
                .any(|address| address.port() != endpoint.port())
        {
            return Err(error("invalidResolution", "接続先の解決結果が無効です。"));
        }
        addresses.sort_unstable();
        addresses.dedup();
        Ok(EndpointIdentity {
            host: endpoint.host().to_ascii_lowercase(),
            port: endpoint.port(),
            addresses,
        })
    }
    pub(crate) async fn connection_addresses(
        &self,
        endpoint: &BouyomiAddress,
        remote_mode: bool,
    ) -> Result<Vec<SocketAddr>, ValidationError> {
        let identity = self.identity(endpoint).await?;
        if identity
            .addresses
            .iter()
            .all(|address| loopback(address.ip()))
        {
            return Ok(identity.addresses);
        }
        if !remote_mode {
            return Err(error(
                "loopbackOnly",
                "通常モードはループバック宛先だけです。外部へは送信していません。",
            ));
        }
        private_addresses(&identity.addresses)?;
        if self
            .approved
            .lock()
            .map_err(|_| error("policyUnavailable", "外部接続の許可状態を確認できません。"))?
            .as_ref()
            != Some(&identity)
        {
            return Err(error(
                "consentRequired",
                "外部接続は未許可か、接続先/DNS結果が変更されています。外部へは送信していません。",
            ));
        }
        // Callers connect to these SocketAddr values, never resolve a second time.
        Ok(identity.addresses)
    }
    pub(crate) async fn prepare_approval(
        &self,
        endpoint: &BouyomiAddress,
        remote_mode: bool,
        consent: &dyn EndpointConsent,
    ) -> Result<PreparedApproval, ValidationError> {
        if !remote_mode {
            return Err(error(
                "remoteModeRequired",
                "外部接続モードを明示的に選択・保存してから許可してください。",
            ));
        }
        let _pending = self.pending.clone().try_lock_owned().map_err(|_| {
            error(
                "consentBusy",
                "接続先の確認中です。完了までお待ちください。",
            )
        })?;
        let identity = self.identity(endpoint).await?;
        private_addresses(&identity.addresses)?;
        {
            let mut last = self
                .last_prompt
                .lock()
                .map_err(|_| error("policyUnavailable", "許可状態を確認できません。"))?;
            if last.is_some_and(|time| time.elapsed() < Duration::from_secs(30)) {
                return Err(error(
                    "consentRateLimited",
                    "ネイティブ確認は30秒に1回です。しばらく待って再操作してください。",
                ));
            }
            *last = Some(Instant::now());
        }
        let addresses = identity
            .addresses
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        // A native re-confirmation that is declined must not leave an older
        // approval active. Pending transport cannot retroactively unsend bytes.
        self.revoke();
        let message = format!("接続先: {}:{}\nDNS解決後の宛先:\n{addresses}\n\nTwitchユーザー名・チャット本文・テスト文・操作要求を、TLS暗号化も相手認証もない平文TCPで送信します。private LAN/VPN内でも盗聴・なりすましの危険があります。信頼する相手だけを許可し、暗号化トンネル/VPNを使用してください。\n\nこの起動中、上記の接続先とアドレスだけを許可します。変更・DNS結果変更・再起動後は再確認が必要です。許可しますか？", identity.host, identity.port);
        if !consent.ask(message).await? {
            return Err(error(
                "consentDeclined",
                "外部接続を許可しませんでした。送信していません。",
            ));
        }
        if self.identity(endpoint).await? != identity {
            return Err(error(
                "endpointChanged",
                "確認中にDNS結果が変わりました。送信していません。再確認してください。",
            ));
        }
        Ok(PreparedApproval(identity))
    }
    pub(crate) fn install(&self, approval: PreparedApproval) -> Result<(), ValidationError> {
        *self
            .approved
            .lock()
            .map_err(|_| error("policyUnavailable", "許可状態を更新できません。"))? =
            Some(approval.0);
        Ok(())
    }
    pub(crate) fn revoke(&self) {
        // A poisoned policy also fails closed at every connection.
        if let Ok(mut approved) = self.approved.lock() {
            *approved = None;
        }
    }
}

#[cfg(feature = "app")]
struct NativeConsent(tauri::AppHandle<tauri::Wry>);
#[cfg(feature = "app")]
impl EndpointConsent for NativeConsent {
    fn ask(&self, message: String) -> SpeechFuture<'_, Result<bool, ValidationError>> {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        Box::pin(async move {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            self.0
                .dialog()
                .message(message)
                .title("Rice: 外部への平文送信を許可しますか？")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancel)
                .show(move |approved| {
                    let _ = sender.send(approved);
                });
            receiver.await.map_err(|_| {
                error(
                    "consentUnavailable",
                    "ネイティブ確認を完了できませんでした。外部接続は許可していません。",
                )
            })
        })
    }
}
#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_authorize_endpoint(
    state: tauri::State<'_, crate::settings::AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), ValidationError> {
    let snapshot = state
        .settings
        .lock()
        .map_err(|error| error.to_string())?
        .speech
        .clone();
    crate::settings::validation::validate_speech(&snapshot)?;
    let endpoint = BouyomiAddress::new(&snapshot.bouyomi_host, snapshot.bouyomi_port)?;
    let policy = state.speech_runtime.destination_policy();
    let approval = policy
        .prepare_approval(
            &endpoint,
            snapshot.bouyomi_remote_mode,
            &NativeConsent(app.clone()),
        )
        .await?;
    // No settings lock was held during DNS/dialog. Compare and install atomically
    // with respect to endpoint changes; no persistent/renderer approval flag exists.
    let current = state.settings.lock().map_err(|error| error.to_string())?;
    if current.speech.bouyomi_host != snapshot.bouyomi_host
        || current.speech.bouyomi_port != snapshot.bouyomi_port
        || current.speech.bouyomi_remote_mode != snapshot.bouyomi_remote_mode
    {
        return Err(error(
            "endpointChanged",
            "確認中に接続設定が変わりました。外部接続は許可していません。",
        ));
    }
    policy.install(approval)?;
    drop(current);
    crate::app_events::emit_app_log(
        &app,
        crate::app_events::AppLogLevel::Info,
        "棒読みちゃんの外部接続先を、この起動中だけ許可しました。",
    );
    Ok(())
}

#[cfg(test)]
mod tests;
