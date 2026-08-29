#[cfg(feature = "app")]
use crate::app_events::{
    emit_app_log, emit_speech_queue_updated, emit_speech_status, AppLogLevel, SpeechQueuePhase,
    SpeechStatus,
};
#[cfg(feature = "app")]
use crate::settings::AppState;
#[cfg(feature = "app")]
use crate::speech::{clear_speech_queue, pause_queue, resume_queue, skip_current_queue_item};
use crate::speech::{SpeechAdapter, SpeechHealth, SpeechRequest, SpeechResult};
use serde::Serialize;
use std::net::IpAddr;
use std::time::Duration;
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    time::{timeout, Instant},
};

pub const DEFAULT_CONNECTION_SUCCESS_MESSAGE: &str = "棒読みちゃんと接続しました";

#[derive(Debug, Clone)]
pub struct BouyomiAddress {
    host: String,
    port: u16,
}

impl BouyomiAddress {
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
    let host = host.trim();
    let invalid = || {
        "棒読みちゃんのホストが無効です。IPv4、DNS名、または角括弧なしのIPv6アドレスを入力してください。"
            .to_string()
    };

    if host.is_empty() || host.contains(char::is_whitespace) || host.contains(['[', ']']) {
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
    pub fn new(
        host: impl AsRef<str>,
        port: u16,
        defaults: BouyomiTalkConfig,
    ) -> Result<Self, String> {
        Ok(Self {
            address: BouyomiAddress::new(host, port)?,
            defaults,
            timeout: Duration::from_secs(2),
        })
    }

    pub async fn health_check(
        &self,
        speak_on_success: bool,
        success_message: &str,
    ) -> anyhow::Result<Duration> {
        let started_at = Instant::now();
        if speak_on_success {
            self.speak(normalize_connection_success_message(success_message))
                .await?;
        } else {
            self.send_query(BouyomiQueryCommand::IsNowPlaying).await?;
        }
        Ok(started_at.elapsed())
    }

    pub async fn speak(&self, text: &str) -> anyhow::Result<()> {
        let packet = build_talk_packet(&self.defaults, text);
        self.send_packet(&packet).await
    }

    pub async fn control(&self, command: BouyomiControlCommand) -> anyhow::Result<()> {
        self.send_packet(&command.packet()).await
    }

    async fn send_query(&self, command: BouyomiQueryCommand) -> anyhow::Result<()> {
        self.send_packet(&command.packet()).await
    }

    async fn send_packet(&self, packet: &[u8]) -> anyhow::Result<()> {
        let mut stream = self.connect().await?;
        timeout(self.timeout, stream.write_all(packet)).await??;
        Ok(())
    }

    async fn connect(&self) -> anyhow::Result<TcpStream> {
        self.connect_to_address().await
    }

    async fn connect_to_address(&self) -> anyhow::Result<TcpStream> {
        Ok(timeout(
            self.timeout,
            TcpStream::connect((self.address.host.as_str(), self.address.port)),
        )
        .await??)
    }

    pub async fn diagnose(&self) -> BouyomiConnectionDiagnostics {
        let mut attempted = Vec::new();

        let addr = self.address.display();
        let started_at = Instant::now();
        let result = self.connect_to_address().await;
        let elapsed_ms = started_at.elapsed().as_millis();

        match result {
            Ok(stream) => {
                drop(stream);
                attempted.push(BouyomiConnectionAttempt {
                    addr,
                    status: BouyomiConnectionStatus::Connected,
                    message: "接続できました。".to_string(),
                    elapsed_ms,
                });
            }
            Err(error) => attempted.push(BouyomiConnectionAttempt {
                addr,
                status: BouyomiConnectionStatus::Failed,
                message: error.to_string(),
                elapsed_ms,
            }),
        }

        let recommendation = build_diagnostic_recommendation(&attempted);
        BouyomiConnectionDiagnostics {
            configured_addr: self.address.display(),
            attempted,
            recommendation,
        }
    }
}

impl SpeechAdapter for BouyomiAdapter {
    async fn health_check(&self) -> anyhow::Result<SpeechHealth> {
        BouyomiAdapter::health_check(self, true, DEFAULT_CONNECTION_SUCCESS_MESSAGE).await?;
        Ok(SpeechHealth::Connected)
    }

    async fn speak(&self, request: SpeechRequest) -> anyhow::Result<SpeechResult> {
        BouyomiAdapter::speak(self, &request.text).await?;
        Ok(SpeechResult::Accepted)
    }

    async fn pause(&self) -> anyhow::Result<()> {
        self.control(BouyomiControlCommand::Pause).await
    }

    async fn resume(&self) -> anyhow::Result<()> {
        self.control(BouyomiControlCommand::Resume).await
    }

    async fn skip(&self) -> anyhow::Result<()> {
        self.control(BouyomiControlCommand::Skip).await
    }

    async fn clear(&self) -> anyhow::Result<()> {
        self.control(BouyomiControlCommand::Clear).await
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BouyomiControlCommand {
    Pause,
    Resume,
    Skip,
    Clear,
}

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
}

impl BouyomiQueryCommand {
    fn packet(self) -> [u8; 2] {
        let command: i16 = match self {
            Self::IsNowPlaying => 0x120,
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
pub async fn speech_health_check(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<String, String> {
    let adapter = adapter_from_settings(&state)?;
    let (speak_on_success, success_message) = connection_success_settings(&state)?;
    let result = adapter
        .health_check(speak_on_success, &success_message)
        .await
        .map(|elapsed| {
            format!(
                "棒読みちゃんに接続できました。応答時間 {}ms",
                elapsed.as_millis()
            )
        })
        .map_err(to_user_message);
    match &result {
        Ok(message) => {
            emit_speech_status(&app, SpeechStatus::Idle, Some(message.clone()));
            emit_app_log(&app, AppLogLevel::Info, message.clone());
        }
        Err(message) => {
            emit_speech_status(&app, SpeechStatus::Disconnected, Some(message.clone()));
            emit_app_log(&app, AppLogLevel::Warning, message.clone());
        }
    }
    result
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_health_probe(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<String, String> {
    let adapter = adapter_from_settings(&state)?;
    let (speak_on_success, success_message) = connection_success_settings(&state)?;
    let result = adapter
        .health_check(speak_on_success, &success_message)
        .await
        .map(|elapsed| {
            format!(
                "棒読みちゃんに接続できました。応答時間 {}ms",
                elapsed.as_millis()
            )
        })
        .map_err(to_user_message);
    match &result {
        Ok(_) => {
            let paused = state
                .speech_queue
                .lock()
                .map(|queue| queue.paused)
                .unwrap_or(false);
            emit_speech_status(
                &app,
                if paused {
                    SpeechStatus::Paused
                } else {
                    SpeechStatus::Idle
                },
                Some("棒読みちゃんの接続を確認しました。".to_string()),
            );
        }
        Err(message) => emit_speech_status(&app, SpeechStatus::Disconnected, Some(message.clone())),
    }
    result
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_connection_diagnostics(
    state: tauri::State<'_, AppState>,
) -> Result<BouyomiConnectionDiagnostics, String> {
    let adapter = adapter_from_settings(&state)?;
    Ok(adapter.diagnose().await)
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_test(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
    text: String,
) -> Result<(), String> {
    let adapter = adapter_from_settings(&state)?;
    let text = normalize_test_text(&text);
    emit_speech_status(
        &app,
        SpeechStatus::Speaking,
        Some("テスト読み上げを送信しています。".to_string()),
    );
    let result = adapter.speak(&text).await.map_err(to_user_message);
    match &result {
        Ok(()) => {
            emit_speech_status(
                &app,
                SpeechStatus::Idle,
                Some("テスト読み上げを送信しました。".to_string()),
            );
            emit_app_log(&app, AppLogLevel::Info, "テスト読み上げを送信しました。");
        }
        Err(message) => {
            emit_speech_status(&app, SpeechStatus::Error, Some(message.clone()));
            emit_app_log(&app, AppLogLevel::Error, message.clone());
        }
    }
    result
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_pause(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    let result = control_from_settings(&state, BouyomiControlCommand::Pause).await;
    if result.is_ok() {
        let _ = pause_queue(&app);
        emit_speech_status(
            &app,
            SpeechStatus::Paused,
            Some("読み上げを一時停止しました。".to_string()),
        );
        emit_app_log(&app, AppLogLevel::Info, "読み上げを一時停止しました。");
    }
    result
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_resume(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    let result = control_from_settings(&state, BouyomiControlCommand::Resume).await;
    if result.is_ok() {
        let _ = resume_queue(app.clone());
        emit_speech_status(
            &app,
            SpeechStatus::Idle,
            Some("読み上げを再開しました。".to_string()),
        );
        emit_app_log(&app, AppLogLevel::Info, "読み上げを再開しました。");
    }
    result
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_skip(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    let result = control_from_settings(&state, BouyomiControlCommand::Skip).await;
    if result.is_ok() {
        let _ = skip_current_queue_item(&app);
        emit_speech_status(
            &app,
            SpeechStatus::Idle,
            Some("現在の読み上げをスキップしました。".to_string()),
        );
        emit_app_log(
            &app,
            AppLogLevel::Info,
            "現在の読み上げをスキップしました。",
        );
    }
    result
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_clear(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    let result = control_from_settings(&state, BouyomiControlCommand::Clear).await;
    if result.is_ok() {
        let _ = clear_speech_queue(&app);
        emit_speech_status(
            &app,
            SpeechStatus::Idle,
            Some("読み上げキューをクリアしました。".to_string()),
        );
        emit_speech_queue_updated(&app, 0, Vec::new(), SpeechQueuePhase::Idle, None);
        emit_app_log(&app, AppLogLevel::Info, "読み上げキューをクリアしました。");
    }
    result
}

#[cfg(feature = "app")]
async fn control_from_settings(
    state: &tauri::State<'_, AppState>,
    command: BouyomiControlCommand,
) -> Result<(), String> {
    let adapter = adapter_from_settings(state)?;
    adapter.control(command).await.map_err(to_user_message)
}

#[cfg(feature = "app")]
pub(crate) fn adapter_from_settings(
    state: &tauri::State<'_, AppState>,
) -> Result<BouyomiAdapter, String> {
    let settings = state.settings.lock().map_err(|error| error.to_string())?;
    let defaults = BouyomiTalkConfig {
        speed: settings.speech.bouyomi_speed,
        tone: settings.speech.bouyomi_tone,
        volume: settings.speech.bouyomi_volume,
        voice: settings.speech.bouyomi_voice,
        code: 0,
    };

    BouyomiAdapter::new(
        &settings.speech.bouyomi_host,
        settings.speech.bouyomi_port,
        defaults,
    )
}

#[cfg(feature = "app")]
fn connection_success_settings(
    state: &tauri::State<'_, AppState>,
) -> Result<(bool, String), String> {
    let settings = state.settings.lock().map_err(|error| error.to_string())?;
    Ok((
        settings.speech.connection_success_speech_enabled,
        settings.speech.connection_success_speech_text.clone(),
    ))
}

fn normalize_test_text(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        "テスト読み上げです。".to_string()
    } else {
        text.chars().take(120).collect()
    }
}

pub(crate) fn to_user_message(error: anyhow::Error) -> String {
    let message = error.to_string();
    if message.contains("Connection refused")
        || message.contains("os error 111")
        || message.contains("os error 10061")
    {
        "棒読みちゃんに接続できません。棒読みちゃんが起動中で、アプリ連携/TCP受付が有効か確認してください。続けて［診断］を実行すると接続先と原因を確認できます。"
            .to_string()
    } else if message.contains("timed out") || message.contains("elapsed") {
        "棒読みちゃんへの接続がタイムアウトしました。ポート番号とセキュリティソフトの設定を確認してください。続けて［診断］を実行すると接続先と原因を確認できます。".to_string()
    } else {
        format!("棒読みちゃん連携でエラーが発生しました: {message}")
    }
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
        let message = to_user_message(anyhow::anyhow!("Connection refused (os error 111)"));

        assert!(message.contains("［診断］"));
        assert!(!message.contains("Voices"));
        assert!(!message.contains("Settings"));
    }

    #[test]
    fn timeout_recovery_mentions_the_diagnostic_action_without_a_route_name() {
        let message = to_user_message(anyhow::anyhow!("operation timed out"));

        assert!(message.contains("［診断］"));
        assert!(!message.contains("Voices"));
        assert!(!message.contains("Settings"));
    }
}
