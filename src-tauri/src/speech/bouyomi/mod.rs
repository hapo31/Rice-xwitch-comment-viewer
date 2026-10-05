#[cfg(feature = "app")]
use crate::settings::AppState;
#[cfg(feature = "app")]
use crate::speech::commands::report_failure;
use crate::speech::{SpeechAdapter, SpeechHealth, SpeechRequest, SpeechResult};

mod error;
use crate::speech::{SpeechFailure, SpeechFuture, SpeechPlaybackCompletion};
pub(crate) use error::{classify_error, BouyomiError};
use serde::Serialize;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Instant},
};

pub type BouyomiDispatcher = crate::speech::runtime::SpeechDispatcher;

pub const DEFAULT_CONNECTION_SUCCESS_MESSAGE: &str = "棒読みちゃんと接続しました";
const PLAYBACK_SETTLE_DELAY: Duration = Duration::from_millis(50);
const PLAYBACK_POLL_INTERVAL: Duration = Duration::from_millis(100);
const PLAYBACK_TRACKING_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(crate) use crate::speech::SpeechPlaybackCompletion as BouyomiPlaybackCompletion;

#[derive(Debug, Clone)]
pub struct BouyomiAddress {
    host: String,
    port: u16,
}

impl BouyomiAddress {
    pub(crate) fn host(&self) -> &str {
        &self.host
    }
    pub(crate) fn port(&self) -> u16 {
        self.port
    }
    pub fn new(host: impl AsRef<str>, port: u16) -> Result<Self, String> {
        if port == 0 {
            return Err("棒読みちゃんのポート番号が無効です。".to_string());
        }

        Ok(Self {
            host: validate_bouyomi_host(host.as_ref())?,
            port,
        })
    }

    fn display(&self) -> String {
        if matches!(self.host.parse::<IpAddr>(), Ok(IpAddr::V6(_))) {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

pub fn validate_bouyomi_host(host: &str) -> Result<String, String> {
    let raw_bytes = host.len();
    let raw_controls = host.chars().any(char::is_control);
    let host = host.trim();
    let invalid = || {
        "棒読みちゃんのホストが無効です。IPv4、DNS名、または角括弧なしのIPv6アドレスを入力してください。"
            .to_string()
    };

    if host.is_empty()
        || raw_bytes > 253
        || raw_controls
        || host.contains(char::is_whitespace)
        || host.contains(['[', ']'])
    {
        return Err(invalid());
    }

    if host.contains(':') {
        return host
            .parse::<IpAddr>()
            .ok()
            .filter(|address| address.is_ipv6())
            .map(|_| host.to_string())
            .ok_or_else(invalid);
    }

    if host.parse::<IpAddr>().is_ok()
        || host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
    {
        Ok(host.to_string())
    } else {
        Err(invalid())
    }
}

#[derive(Debug, Clone)]
pub struct BouyomiAdapter {
    address: BouyomiAddress,
    pub defaults: BouyomiTalkConfig,
    pub timeout: Duration,
    dispatcher: BouyomiDispatcher,
    destination_policy: Arc<super::destination::DestinationPolicy>,
    remote_mode: bool,
}

#[derive(Debug, Clone)]
pub struct BouyomiTalkConfig {
    pub speed: i16,
    pub tone: i16,
    pub volume: i16,
    pub voice: i16,
    pub code: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BouyomiConnectionDiagnostics {
    pub configured_addr: String,
    pub attempted: Vec<BouyomiConnectionAttempt>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BouyomiConnectionAttempt {
    pub addr: String,
    pub status: BouyomiConnectionStatus,
    pub message: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BouyomiConnectionStatus {
    Connected,
    Failed,
}

impl Default for BouyomiTalkConfig {
    fn default() -> Self {
        Self {
            speed: -1,
            tone: -1,
            volume: -1,
            voice: 0,
            code: 0,
        }
    }
}

impl BouyomiAdapter {
    #[cfg(test)]
    pub fn new(
        host: impl AsRef<str>,
        port: u16,
        defaults: BouyomiTalkConfig,
    ) -> Result<Self, String> {
        Self::with_dispatcher(host, port, defaults, BouyomiDispatcher::default())
    }

    pub fn with_dispatcher(
        host: impl AsRef<str>,
        port: u16,
        defaults: BouyomiTalkConfig,
        dispatcher: BouyomiDispatcher,
    ) -> Result<Self, String> {
        Ok(Self {
            address: BouyomiAddress::new(host, port)?,
            defaults,
            timeout: Duration::from_secs(2),
            dispatcher,
            destination_policy: Arc::default(),
            remote_mode: false,
        })
    }
    pub(crate) fn with_destination_policy(
        mut self,
        policy: Arc<super::destination::DestinationPolicy>,
        remote_mode: bool,
    ) -> Self {
        self.destination_policy = policy;
        self.remote_mode = remote_mode;
        self
    }

    pub async fn health_check(
        &self,
        speak_on_success: bool,
        success_message: &str,
    ) -> anyhow::Result<Duration> {
        let started_at = Instant::now();
        let _dispatch_guard = self.dispatcher.lock().await;
        self.send_query_unordered(BouyomiQueryCommand::IsNowPlaying)
            .await?;
        if speak_on_success {
            self.send_talk_unordered(normalize_connection_success_message(success_message))
                .await?;
        }
        Ok(started_at.elapsed())
    }

    pub async fn health_probe(&self) -> anyhow::Result<Duration> {
        self.health_check(false, "").await
    }

    #[cfg(test)]
    pub async fn speak(&self, text: &str) -> anyhow::Result<()> {
        let _dispatch_guard = self.dispatcher.lock().await;
        self.send_talk_after_dispatch_lock(text).await
    }

    #[cfg(test)]
    pub(crate) async fn acquire_dispatcher(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.dispatcher.clone().lock_owned().await
    }

    pub(crate) async fn send_talk_after_dispatch_lock(&self, text: &str) -> anyhow::Result<()> {
        self.send_talk_unordered(text).await
    }

    #[cfg(test)]
    /// Submit exactly one talk request and keep it locally in-flight until
    /// Bouyomi reports neither playback nor queued tasks. A tracking failure is
    /// distinct from a submission failure: callers must not resend an already
    /// accepted talk automatically because that could duplicate speech.
    pub(crate) async fn speak_and_wait(
        &self,
        text: &str,
    ) -> anyhow::Result<BouyomiPlaybackCompletion> {
        self.speak(text).await?;
        Ok(self.wait_for_playback_completion().await)
    }

    pub(crate) async fn wait_for_playback_completion(&self) -> BouyomiPlaybackCompletion {
        tokio::time::sleep(PLAYBACK_SETTLE_DELAY).await;
        let started_at = Instant::now();

        loop {
            let state = self.playback_state().await;
            match state {
                Ok((false, 0)) => return BouyomiPlaybackCompletion::Completed,
                Ok(_) if started_at.elapsed() < PLAYBACK_TRACKING_TIMEOUT => {
                    tokio::time::sleep(PLAYBACK_POLL_INTERVAL).await;
                }
                Ok(_) => {
                    let mut failure = SpeechFailure::unknown(
                        "playback tracking deadline exceeded after 5 minutes".to_string(),
                    );
                    failure.user_message = "読み上げは受付済みですが、5分以内に再生完了を確認できませんでした。重複を避けるため、自動再送しません。［診断］を実行してください。".to_string();
                    return BouyomiPlaybackCompletion::Unconfirmed(failure);
                }
                Err(error) => {
                    let mut failure = classify_error(error);
                    failure.user_message = format!(
                        "再生完了を確認できません。受付済みのため自動再送しません。 {}",
                        failure.user_message
                    );
                    return BouyomiPlaybackCompletion::Unconfirmed(failure);
                }
            }
        }
    }

    async fn playback_state(&self) -> anyhow::Result<(bool, u8)> {
        let _dispatch_guard = self.dispatcher.lock().await;
        let remaining = self
            .send_query_unordered(BouyomiQueryCommand::RemainingTasks)
            .await?;
        let is_playing = self
            .send_query_unordered(BouyomiQueryCommand::IsNowPlaying)
            .await?
            != 0;
        Ok((is_playing, remaining))
    }

    async fn send_talk_unordered(&self, text: &str) -> anyhow::Result<()> {
        let packet = build_talk_packet(&self.defaults, text);
        self.send_packet_unordered(&packet).await
    }

    #[cfg(test)]
    pub async fn control(&self, command: BouyomiControlCommand) -> anyhow::Result<()> {
        let _dispatch_guard = self.dispatcher.lock().await;
        self.send_control_after_dispatch_lock(command).await
    }

    pub(crate) async fn send_control_after_dispatch_lock(
        &self,
        command: BouyomiControlCommand,
    ) -> anyhow::Result<()> {
        self.send_packet_unordered(&command.packet()).await
    }

    #[cfg(test)]
    pub(crate) async fn send_control_and_apply<T>(
        &self,
        command: BouyomiControlCommand,
        apply_local: impl FnOnce() -> T,
    ) -> anyhow::Result<T> {
        let dispatch_guard = self.acquire_dispatcher().await;
        self.send_control_after_dispatch_lock(command).await?;
        let result = apply_local();
        drop(dispatch_guard);
        Ok(result)
    }

    async fn send_query_unordered(&self, command: BouyomiQueryCommand) -> anyhow::Result<u8> {
        let mut stream = self.connect().await?;
        write_packet(&mut stream, &command.packet(), self.timeout).await?;
        let mut response = [0_u8; 1];
        timeout(self.timeout, stream.read_exact(&mut response))
            .await
            .map_err(|_| BouyomiError::ResponseTimeout)?
            .map_err(BouyomiError::ResponseIo)?;
        match (command, response[0]) {
            (BouyomiQueryCommand::IsNowPlaying, value @ (0 | 1)) => Ok(value),
            (BouyomiQueryCommand::RemainingTasks, value) => Ok(value),
            (_, value) => Err(BouyomiError::InvalidResponse(value).into()),
        }
    }

    async fn send_packet_unordered(&self, packet: &[u8]) -> anyhow::Result<()> {
        let mut stream = self.connect().await?;
        write_packet(&mut stream, packet, self.timeout).await?;
        Ok(())
    }

    async fn connect(&self) -> anyhow::Result<TcpStream> {
        self.connect_to_address().await
    }

    async fn connect_to_address(&self) -> anyhow::Result<TcpStream> {
        let addresses = self
            .destination_policy
            .connection_addresses(&self.address, self.remote_mode)
            .await
            .map_err(BouyomiError::Destination)?;
        Ok(
            timeout(self.timeout, TcpStream::connect(addresses.as_slice()))
                .await
                .map_err(|_| BouyomiError::ConnectTimeout)?
                .map_err(BouyomiError::ConnectIo)?,
        )
    }

    #[cfg(test)]
    pub async fn diagnose(&self) -> BouyomiConnectionDiagnostics {
        self.diagnose_with_failure().await.0
    }

    async fn diagnose_with_failure(&self) -> (BouyomiConnectionDiagnostics, Option<SpeechFailure>) {
        let mut attempted = Vec::new();
        let mut failure = None;

        let addr = self.address.display();
        let started_at = Instant::now();
        let result = self.health_probe().await;
        let elapsed_ms = started_at.elapsed().as_millis();

        match result {
            Ok(_) => {
                attempted.push(BouyomiConnectionAttempt {
                    addr,
                    status: BouyomiConnectionStatus::Connected,
                    message: "棒読みちゃん互換の状態応答を確認しました。".to_string(),
                    elapsed_ms,
                });
            }
            Err(error) => {
                let classified = classify_error(error);
                attempted.push(BouyomiConnectionAttempt {
                    addr,
                    status: BouyomiConnectionStatus::Failed,
                    message: classified.user_message.clone(),
                    elapsed_ms,
                });
                failure = Some(classified);
            }
        }

        let recommendation = build_diagnostic_recommendation(&attempted);
        (
            BouyomiConnectionDiagnostics {
                configured_addr: self.address.display(),
                attempted,
                recommendation,
            },
            failure,
        )
    }
}

impl SpeechAdapter for BouyomiAdapter {
    fn health_check(&self) -> SpeechFuture<'_, Result<SpeechHealth, SpeechFailure>> {
        Box::pin(async move {
            match self
                .send_query_unordered(BouyomiQueryCommand::IsNowPlaying)
                .await
            {
                Ok(_) => Ok(SpeechHealth::Connected),
                Err(error) => {
                    let failure = classify_error(error);
                    if failure.status == crate::app_events::SpeechStatus::Disconnected {
                        Ok(SpeechHealth::Disconnected { failure })
                    } else {
                        Err(failure)
                    }
                }
            }
        })
    }
    fn speak(
        &self,
        request: SpeechRequest,
    ) -> SpeechFuture<'_, Result<SpeechResult, SpeechFailure>> {
        Box::pin(async move {
            self.send_talk_after_dispatch_lock(&request.text)
                .await
                .map_err(classify_error)?;
            Ok(SpeechResult::Accepted)
        })
    }
    fn pause(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.control_future(BouyomiControlCommand::Pause)
    }
    fn resume(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.control_future(BouyomiControlCommand::Resume)
    }
    fn skip(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.control_future(BouyomiControlCommand::Skip)
    }
    fn clear(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.control_future(BouyomiControlCommand::Clear)
    }
    fn wait_for_completion(&self) -> SpeechFuture<'_, SpeechPlaybackCompletion> {
        Box::pin(self.wait_for_playback_completion())
    }
}

impl BouyomiAdapter {
    fn control_future(
        &self,
        command: BouyomiControlCommand,
    ) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        Box::pin(async move {
            self.send_control_after_dispatch_lock(command)
                .await
                .map_err(classify_error)
        })
    }
}

pub use crate::speech::SpeechControl as BouyomiControlCommand;

impl BouyomiControlCommand {
    fn packet(self) -> [u8; 2] {
        let command: i16 = match self {
            Self::Pause => 0x10,
            Self::Resume => 0x20,
            Self::Skip => 0x30,
            Self::Clear => 0x40,
        };

        command.to_le_bytes()
    }
}

#[derive(Debug, Clone, Copy)]
enum BouyomiQueryCommand {
    IsNowPlaying,
    RemainingTasks,
}

impl BouyomiQueryCommand {
    fn packet(self) -> [u8; 2] {
        let command: i16 = match self {
            Self::IsNowPlaying => 0x120,
            Self::RemainingTasks => 0x130,
        };

        command.to_le_bytes()
    }
}

pub fn build_talk_packet(config: &BouyomiTalkConfig, text: &str) -> Vec<u8> {
    let message = text.as_bytes();
    let mut bytes = Vec::with_capacity(15 + message.len());
    bytes.extend_from_slice(&1_i16.to_le_bytes());
    bytes.extend_from_slice(&config.speed.to_le_bytes());
    bytes.extend_from_slice(&config.tone.to_le_bytes());
    bytes.extend_from_slice(&config.volume.to_le_bytes());
    bytes.extend_from_slice(&config.voice.to_le_bytes());
    bytes.push(config.code);
    bytes.extend_from_slice(&(message.len() as u32).to_le_bytes());
    bytes.extend_from_slice(message);
    bytes
}

pub fn normalize_connection_success_message(text: &str) -> &str {
    let text = text.trim();
    if text.is_empty() {
        DEFAULT_CONNECTION_SUCCESS_MESSAGE
    } else {
        text
    }
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_connection_diagnostics(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<BouyomiConnectionDiagnostics, String> {
    let adapter = adapter_from_settings(&state).map_err(|failure| report_failure(&app, failure))?;
    let (diagnostics, failure) = adapter.diagnose_with_failure().await;
    if let Some(failure) = failure {
        report_failure(&app, failure);
    }
    Ok(diagnostics)
}

#[cfg(feature = "app")]
fn adapter_from_settings(
    state: &tauri::State<'_, AppState>,
) -> Result<BouyomiAdapter, SpeechFailure> {
    let settings = state
        .settings
        .lock()
        .map_err(|error| SpeechFailure::unknown(error.to_string()))?;
    super::factory::bouyomi_from_settings(
        &settings.speech,
        state.speech_runtime.dispatcher(),
        state.speech_runtime.destination_policy(),
    )
}

#[cfg(test)]
fn to_user_message(error: anyhow::Error) -> String {
    classify_error(error).user_message
}

async fn write_packet<W: tokio::io::AsyncWrite + Unpin>(
    stream: &mut W,
    packet: &[u8],
    deadline: Duration,
) -> Result<(), BouyomiError> {
    timeout(deadline, stream.write_all(packet))
        .await
        .map_err(|_| BouyomiError::WriteTimeout)?
        .map_err(BouyomiError::WriteIo)
}

fn build_diagnostic_recommendation(attempted: &[BouyomiConnectionAttempt]) -> String {
    if let Some(attempt) = attempted
        .iter()
        .find(|attempt| attempt.status == BouyomiConnectionStatus::Connected)
    {
        return format!(
            "{} に接続できました。この宛先でテスト読み上げできます。",
            attempt.addr
        );
    }

    let tried = attempted
        .iter()
        .map(|attempt| attempt.addr.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "接続できませんでした。試行先: {tried}。棒読みちゃんが起動しているか、アプリ連携/TCP受付が有効か、ホストとポート番号が設定と一致しているかを確認してください。Windows Defender Firewall やセキュリティソフトが通信を遮断していないかも確認してください。"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_events::SpeechQueueItemStatus;
    use crate::speech::{SpeechQueueDeliveryState, SpeechQueueItem, SpeechQueueState};
    use std::sync::{Arc, Mutex};
    use tokio::{io::AsyncReadExt, net::TcpListener};

    fn queued_item(id: &str) -> SpeechQueueItem {
        SpeechQueueItem {
            id: id.to_string(),
            source_message_id: None,
            user_display_name: "viewer".to_string(),
            text: "こんにちは".to_string(),
            status: SpeechQueueItemStatus::Queued,
            retry_count: 0,
            delivery_state: SpeechQueueDeliveryState::Ready,
            outcome: None,
        }
    }

    #[test]
    fn builds_bouyomi_talk_packet() {
        let packet = build_talk_packet(&BouyomiTalkConfig::default(), "test");

        assert_eq!(&packet[0..2], &1_i16.to_le_bytes());
        assert_eq!(&packet[2..4], &(-1_i16).to_le_bytes());
        assert_eq!(packet[10], 0);
        assert_eq!(&packet[11..15], &4_u32.to_le_bytes());
        assert_eq!(&packet[15..], b"test");
    }

    #[test]
    fn accepts_and_formats_ipv4_dns_and_ipv6_addresses() {
        let ipv4 = BouyomiAddress::new("127.0.0.1", 50001).unwrap();
        let dns = BouyomiAddress::new("localhost", 50001).unwrap();
        let ipv6 = BouyomiAddress::new("::1", 50001).unwrap();

        assert_eq!(ipv4.display(), "127.0.0.1:50001");
        assert_eq!(dns.display(), "localhost:50001");
        assert_eq!(ipv6.display(), "[::1]:50001");
    }

    #[test]
    fn rejects_invalid_bouyomi_addresses_with_a_japanese_message() {
        for host in ["", "[::1]", "::1:50001", "invalid host"] {
            let error = BouyomiAddress::new(host, 50001).unwrap_err();
            assert!(error.contains("棒読みちゃんのホストが無効"));
        }
    }

    #[test]
    fn health_check_message_is_not_empty() {
        assert_eq!(
            DEFAULT_CONNECTION_SUCCESS_MESSAGE,
            "棒読みちゃんと接続しました"
        );
    }

    #[test]
    fn normalizes_empty_health_check_message_to_default() {
        assert_eq!(
            normalize_connection_success_message("  "),
            DEFAULT_CONNECTION_SUCCESS_MESSAGE
        );
        assert_eq!(
            normalize_connection_success_message("接続しました"),
            "接続しました"
        );
    }

    #[test]
    fn builds_talk_packet_with_configured_voice_values() {
        let config = BouyomiTalkConfig {
            speed: 120,
            tone: 110,
            volume: 80,
            voice: 10001,
            code: 0,
        };
        let packet = build_talk_packet(&config, "あ");

        assert_eq!(&packet[2..4], &120_i16.to_le_bytes());
        assert_eq!(&packet[4..6], &110_i16.to_le_bytes());
        assert_eq!(&packet[6..8], &80_i16.to_le_bytes());
        assert_eq!(&packet[8..10], &10001_i16.to_le_bytes());
        assert_eq!(&packet[11..15], &3_u32.to_le_bytes());
        assert_eq!(&packet[15..], "あ".as_bytes());
    }

    #[test]
    fn builds_control_packets() {
        assert_eq!(
            BouyomiControlCommand::Pause.packet(),
            0x10_i16.to_le_bytes()
        );
        assert_eq!(
            BouyomiControlCommand::Resume.packet(),
            0x20_i16.to_le_bytes()
        );
        assert_eq!(BouyomiControlCommand::Skip.packet(), 0x30_i16.to_le_bytes());
        assert_eq!(
            BouyomiControlCommand::Clear.packet(),
            0x40_i16.to_le_bytes()
        );
        assert_eq!(
            BouyomiQueryCommand::IsNowPlaying.packet(),
            0x120_i16.to_le_bytes()
        );
        assert_eq!(
            BouyomiQueryCommand::RemainingTasks.packet(),
            0x130_i16.to_le_bytes()
        );
    }

    #[tokio::test]
    async fn automatic_health_probe_sends_only_the_silent_status_query() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let received = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut packet = [0_u8; 2];
            stream.read_exact(&mut packet).await.unwrap();
            stream.write_all(&[0]).await.unwrap();
            packet
        });
        let adapter = BouyomiAdapter::new("127.0.0.1", port, BouyomiTalkConfig::default()).unwrap();

        adapter.health_probe().await.unwrap();

        assert_eq!(received.await.unwrap(), 0x120_i16.to_le_bytes());
    }

    #[tokio::test]
    async fn domain_request_sends_all_configured_voice_values_without_hidden_overrides() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let received = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut packet = Vec::new();
            stream.read_to_end(&mut packet).await.unwrap();
            packet
        });
        let config = BouyomiTalkConfig {
            speed: 300,
            tone: 50,
            volume: 0,
            voice: 10001,
            code: 0,
        };
        let mut settings = crate::settings::AppSettings::default().speech;
        settings.bouyomi_host = "127.0.0.1".to_string();
        settings.bouyomi_port = port;
        settings.bouyomi_speed = config.speed;
        settings.bouyomi_tone = config.tone;
        settings.bouyomi_volume = config.volume;
        settings.bouyomi_voice = config.voice;
        let runtime = crate::speech::runtime::SpeechRuntime::default();
        let selected = runtime.select(&settings).unwrap();
        let session = selected.lock().await;
        let request = SpeechRequest {
            id: "1".into(),
            source_message_id: None,
            text: "こんにちは".into(),
        };
        assert!(matches!(
            session.speak(request).await.unwrap(),
            SpeechResult::Accepted
        ));
        assert_eq!(
            received.await.unwrap(),
            build_talk_packet(&config, "こんにちは")
        );
    }

    async fn shared_dispatcher_keeps_control_behind_an_in_flight_talk(
        command: BouyomiControlCommand,
    ) {
        use crate::speech::{runtime::SpeechRuntime, SpeechControl};
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let runtime = SpeechRuntime::default();
        let mut settings = crate::settings::AppSettings::default().speech;
        settings.bouyomi_host = "127.0.0.1".into();
        settings.bouyomi_port = port;
        let talk = runtime.select(&settings).unwrap();
        let control = runtime.select(&settings).unwrap();
        // Hold the actual production SpeechSession permit through submission
        // and the caller's local application step. No assumption about the OS
        // socket send buffer, large writes or a wall-clock negative timeout.
        let talk_session = talk.lock().await;
        talk_session
            .speak(SpeechRequest {
                id: "ordered-talk".into(),
                source_message_id: None,
                text: "順序検証".into(),
            })
            .await
            .unwrap();
        let (mut talk_stream, _) = listener.accept().await.unwrap();
        let mut talk_packet = Vec::new();
        talk_stream.read_to_end(&mut talk_packet).await.unwrap();
        assert_eq!(
            talk_packet,
            build_talk_packet(&BouyomiTalkConfig::default(), "順序検証")
        );
        let control_command = match command {
            BouyomiControlCommand::Pause => SpeechControl::Pause,
            BouyomiControlCommand::Skip => SpeechControl::Skip,
            BouyomiControlCommand::Clear => SpeechControl::Clear,
            BouyomiControlCommand::Resume => SpeechControl::Resume,
        };
        let pending_control = async {
            let session = control.lock().await;
            session.control(control_command).await
        };
        tokio::pin!(pending_control);
        assert!(futures_util::poll!(pending_control.as_mut()).is_pending());
        assert!(runtime.dispatcher().try_lock().is_err());
        // Releasing the same real permit must wake the control path. Its exact
        // packet is checked after the talk packet, not replaced by a fake gate.
        drop(talk_session);
        pending_control.await.unwrap();
        let (mut control_stream, _) = listener.accept().await.unwrap();
        let mut control_packet = [0_u8; 2];
        control_stream
            .read_exact(&mut control_packet)
            .await
            .unwrap();
        assert_eq!(control_packet, command.packet());
    }

    #[tokio::test]
    async fn shared_dispatcher_keeps_pause_behind_an_in_flight_talk() {
        shared_dispatcher_keeps_control_behind_an_in_flight_talk(BouyomiControlCommand::Pause)
            .await;
    }

    #[tokio::test]
    async fn shared_dispatcher_keeps_skip_behind_an_in_flight_talk() {
        shared_dispatcher_keeps_control_behind_an_in_flight_talk(BouyomiControlCommand::Skip).await;
    }

    #[tokio::test]
    async fn shared_dispatcher_keeps_clear_behind_an_in_flight_talk() {
        shared_dispatcher_keeps_control_behind_an_in_flight_talk(BouyomiControlCommand::Clear)
            .await;
    }

    #[tokio::test]
    async fn control_before_talk_keeps_the_reserved_talk_from_opening_a_connection() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let dispatcher = BouyomiDispatcher::default();
        let control_adapter = BouyomiAdapter::with_dispatcher(
            "127.0.0.1",
            port,
            BouyomiTalkConfig::default(),
            dispatcher.clone(),
        )
        .unwrap();
        let talk_adapter = BouyomiAdapter::with_dispatcher(
            "127.0.0.1",
            port,
            BouyomiTalkConfig::default(),
            dispatcher,
        )
        .unwrap();
        let queue = Arc::new(Mutex::new(SpeechQueueState {
            controls_in_progress: 1,
            ..SpeechQueueState::default()
        }));
        {
            let mut queue = queue.lock().unwrap();
            queue.pending.push_back(queued_item("pending"));
        }

        let queue_for_control = queue.clone();
        let control = tokio::spawn(async move {
            control_adapter
                .send_control_and_apply(BouyomiControlCommand::Pause, || {
                    let mut queue = queue_for_control.lock().unwrap();
                    queue.controls_in_progress = queue.controls_in_progress.saturating_sub(1);
                    queue.paused = true;
                })
                .await
        });
        let (mut control_stream, _) = listener.accept().await.unwrap();
        let mut control_packet = [0_u8; 2];
        control_stream
            .read_exact(&mut control_packet)
            .await
            .unwrap();
        control.await.unwrap().unwrap();
        assert_eq!(control_packet, BouyomiControlCommand::Pause.packet());

        let talk = tokio::spawn(async move {
            let dispatch_guard = talk_adapter.acquire_dispatcher().await;
            let request = queue
                .lock()
                .unwrap()
                .reserve_next_request_after_dispatch_lock();
            drop(dispatch_guard);
            request.is_some()
        });

        assert!(!talk.await.unwrap());
        assert!(timeout(Duration::from_millis(75), listener.accept())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn dispatcher_applies_pause_and_resume_in_wire_order() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let dispatcher = BouyomiDispatcher::default();
        let pause_adapter = BouyomiAdapter::with_dispatcher(
            "127.0.0.1",
            port,
            BouyomiTalkConfig::default(),
            dispatcher.clone(),
        )
        .unwrap();
        let resume_adapter = BouyomiAdapter::with_dispatcher(
            "127.0.0.1",
            port,
            BouyomiTalkConfig::default(),
            dispatcher.clone(),
        )
        .unwrap();
        let local_applies = Arc::new(Mutex::new(Vec::new()));
        let first_guard = dispatcher.lock().await;

        let local_applies_for_pause = local_applies.clone();
        let pause = tokio::spawn(async move {
            pause_adapter
                .send_control_and_apply(BouyomiControlCommand::Pause, || {
                    local_applies_for_pause.lock().unwrap().push("pause");
                })
                .await
        });
        tokio::task::yield_now().await;
        let local_applies_for_resume = local_applies.clone();
        let resume = tokio::spawn(async move {
            resume_adapter
                .send_control_and_apply(BouyomiControlCommand::Resume, || {
                    local_applies_for_resume.lock().unwrap().push("resume");
                })
                .await
        });
        tokio::task::yield_now().await;
        drop(first_guard);

        let mut packets = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut packet = [0_u8; 2];
            stream.read_exact(&mut packet).await.unwrap();
            packets.push(packet);
        }
        pause.await.unwrap().unwrap();
        resume.await.unwrap().unwrap();

        let wire_order = packets
            .iter()
            .map(|packet| match *packet {
                packet if packet == BouyomiControlCommand::Pause.packet() => "pause",
                packet if packet == BouyomiControlCommand::Resume.packet() => "resume",
                _ => unreachable!("unexpected control packet"),
            })
            .collect::<Vec<_>>();
        assert_eq!(*local_applies.lock().unwrap(), wire_order);
    }

    #[tokio::test]
    async fn submitted_talk_stays_in_flight_until_remote_playback_is_idle() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let adapter = BouyomiAdapter::new(
            "127.0.0.1",
            listener.local_addr().unwrap().port(),
            BouyomiTalkConfig::default(),
        )
        .unwrap();
        let server = tokio::spawn(async move {
            let (mut talk, _) = listener.accept().await.unwrap();
            let mut header = [0_u8; 15];
            talk.read_exact(&mut header).await.unwrap();
            assert_eq!(&header[0..2], &1_i16.to_le_bytes());

            for (expected, response) in [
                (0x130_i16, 1_u8),
                (0x120_i16, 1_u8),
                (0x130_i16, 0_u8),
                (0x120_i16, 0_u8),
            ] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut command = [0_u8; 2];
                stream.read_exact(&mut command).await.unwrap();
                assert_eq!(command, expected.to_le_bytes());
                stream.write_all(&[response]).await.unwrap();
            }
        });

        assert_eq!(
            adapter.speak_and_wait("slow").await.unwrap(),
            BouyomiPlaybackCompletion::Completed
        );
        server.await.unwrap();
    }

    async fn probe_fixture(response: Option<Vec<u8>>, diagnose: bool) -> Result<(), String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let mut adapter = BouyomiAdapter::new(
            "127.0.0.1",
            listener.local_addr().unwrap().port(),
            BouyomiTalkConfig::default(),
        )
        .unwrap();
        adapter.timeout = Duration::from_millis(50);
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut command = [0; 2];
            stream.read_exact(&mut command).await.unwrap();
            assert_eq!(command, 0x120_i16.to_le_bytes());
            if let Some(bytes) = response {
                stream.write_all(&bytes).await.unwrap();
            } else {
                std::future::pending::<()>().await;
            }
        });
        let result = if diagnose {
            let diagnostics = adapter.diagnose().await;
            let attempt = &diagnostics.attempted[0];
            if attempt.status == BouyomiConnectionStatus::Connected {
                Ok(())
            } else {
                Err(attempt.message.clone())
            }
        } else {
            adapter
                .health_probe()
                .await
                .map(|_| ())
                .map_err(to_user_message)
        };
        server.abort();
        let _ = server.await;
        result
    }

    #[tokio::test]
    async fn health_and_diagnostics_require_valid_protocol_responses() {
        for diagnose in [false, true] {
            for value in [0, 1] {
                probe_fixture(Some(vec![value]), diagnose).await.unwrap();
            }
            assert!(probe_fixture(Some(vec![2]), diagnose)
                .await
                .unwrap_err()
                .contains("互換性"));
            assert!(probe_fixture(Some(b"HTTP/1.1".to_vec()), diagnose)
                .await
                .unwrap_err()
                .contains("ポート競合"));
            assert!(probe_fixture(Some(vec![]), diagnose)
                .await
                .unwrap_err()
                .contains("切断"));
            assert!(probe_fixture(None, diagnose)
                .await
                .unwrap_err()
                .contains("タイムアウト"));
        }
    }

    #[tokio::test]
    async fn refused_endpoint_is_not_connected() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let adapter = BouyomiAdapter::new(
            "127.0.0.1",
            listener.local_addr().unwrap().port(),
            BouyomiTalkConfig::default(),
        )
        .unwrap();
        drop(listener);

        let errors = [
            adapter.health_probe().await.unwrap_err(),
            adapter.health_check(true, "probe").await.unwrap_err(),
            adapter.speak("test").await.unwrap_err(),
            adapter
                .control(BouyomiControlCommand::Pause)
                .await
                .unwrap_err(),
        ];
        let failures: Vec<_> = errors.into_iter().map(classify_error).collect();
        for failure in &failures {
            assert_eq!(
                failure.status,
                crate::app_events::SpeechStatus::Disconnected
            );
            // Windows can retry a closed loopback endpoint beyond our deadline.
            // A domain deadline must stay ConnectTimeout, not be relabeled by
            // guessing that an eventual native error would have been refused.
            assert!(matches!(
                failure.code,
                error::FailureCode::ConnectionRefused | error::FailureCode::ConnectTimeout
            ));
            assert!(failure.retryable);
            for same_cause in failures.iter().filter(|other| other.code == failure.code) {
                assert_eq!(failure.user_message, same_cause.user_message);
            }
            assert!(failure.log_message().contains("connect"));
        }
        match SpeechAdapter::health_check(&adapter).await.unwrap() {
            SpeechHealth::Disconnected { failure } => {
                assert!(failure.user_message.contains("［診断］"))
            }
            SpeechHealth::Connected => panic!("refused endpoint must not be connected"),
        }
        assert!(adapter.health_probe().await.is_err());
        let result = adapter.diagnose().await;
        assert_eq!(result.attempted[0].status, BouyomiConnectionStatus::Failed);
        assert!(result.attempted[0].message.contains("［診断］"));
    }

    #[tokio::test]
    async fn confirmation_speech_is_not_sent_to_an_incompatible_endpoint() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let adapter = BouyomiAdapter::new(
            "127.0.0.1",
            listener.local_addr().unwrap().port(),
            BouyomiTalkConfig::default(),
        )
        .unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut command = [0; 2];
            stream.read_exact(&mut command).await.unwrap();
            assert_eq!(command, 0x120_i16.to_le_bytes());
            stream.write_all(&[5]).await.unwrap();
            assert!(timeout(Duration::from_millis(50), listener.accept())
                .await
                .is_err());
        });
        assert!(adapter.health_check(true, "test").await.is_err());
        server.await.unwrap();
    }

    #[test]
    fn builds_diagnostic_recommendation_for_connected_attempt() {
        let recommendation = build_diagnostic_recommendation(&[BouyomiConnectionAttempt {
            addr: "127.0.0.1:50001".to_string(),
            status: BouyomiConnectionStatus::Connected,
            message: "接続できました。".to_string(),
            elapsed_ms: 1,
        }]);

        assert!(recommendation.contains("127.0.0.1:50001"));
        assert!(recommendation.contains("テスト読み上げ"));
    }

    #[test]
    fn builds_diagnostic_recommendation_for_failed_attempt() {
        let recommendation = build_diagnostic_recommendation(&[BouyomiConnectionAttempt {
            addr: "127.0.0.1:50001".to_string(),
            status: BouyomiConnectionStatus::Failed,
            message: "connection refused".to_string(),
            elapsed_ms: 1,
        }]);

        assert!(recommendation.contains("棒読みちゃんが起動"));
        assert!(recommendation.contains("ホストとポート番号"));
    }

    #[test]
    fn connection_refused_recovery_mentions_the_diagnostic_action_without_a_route_name() {
        let message = to_user_message(
            BouyomiError::ConnectIo(std::io::Error::from(std::io::ErrorKind::ConnectionRefused))
                .into(),
        );

        assert!(message.contains("［診断］"));
        assert!(!message.contains("Voices"));
        assert!(!message.contains("Settings"));
    }

    #[test]
    fn timeout_recovery_mentions_the_diagnostic_action_without_a_route_name() {
        let message = to_user_message(BouyomiError::ConnectTimeout.into());

        assert!(message.contains("［診断］"));
        assert!(!message.contains("Voices"));
        assert!(!message.contains("Settings"));
    }

    #[tokio::test]
    async fn write_timeout_and_io_failure_keep_the_write_phase() {
        let (mut blocked, _peer) = tokio::io::duplex(1);
        let error = write_packet(&mut blocked, &[0; 256], Duration::from_millis(10))
            .await
            .unwrap_err();
        let failure = classify_error(error.into());
        assert_eq!(failure.code, error::FailureCode::WriteTimeout);
        assert!(!failure.retryable);
        let (mut disconnected, peer) = tokio::io::duplex(1);
        drop(peer);
        let error = write_packet(&mut disconnected, &[1], Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(matches!(error, BouyomiError::WriteIo(_)));
        let failure = classify_error(error.into());
        assert_eq!(failure.code, error::FailureCode::ConnectionLost);
        assert_eq!(
            failure.status,
            crate::app_events::SpeechStatus::Disconnected
        );
        assert!(!failure.retryable);
    }
}
