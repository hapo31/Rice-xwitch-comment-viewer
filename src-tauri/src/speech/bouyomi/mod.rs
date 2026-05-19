#[cfg(feature = "app")]
use crate::settings::AppState;
use crate::speech::{SpeechAdapter, SpeechHealth, SpeechRequest, SpeechResult};
use std::time::Duration;
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    time::{timeout, Instant},
};

#[derive(Debug, Clone)]
pub struct BouyomiAdapter {
    pub addr: String,
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
    pub fn new(addr: impl Into<String>, defaults: BouyomiTalkConfig) -> Self {
        Self {
            addr: addr.into(),
            defaults,
            timeout: Duration::from_secs(2),
        }
    }

    pub async fn health_check(&self) -> anyhow::Result<Duration> {
        let started_at = Instant::now();
        let stream = timeout(self.timeout, TcpStream::connect(self.addr.as_str())).await??;
        drop(stream);
        Ok(started_at.elapsed())
    }

    pub async fn speak(&self, text: &str) -> anyhow::Result<()> {
        let packet = build_talk_packet(&self.defaults, text);
        self.send_packet(&packet).await
    }

    pub async fn control(&self, command: BouyomiControlCommand) -> anyhow::Result<()> {
        self.send_packet(&command.packet()).await
    }

    async fn send_packet(&self, packet: &[u8]) -> anyhow::Result<()> {
        let mut stream = timeout(self.timeout, TcpStream::connect(self.addr.as_str())).await??;
        timeout(self.timeout, stream.write_all(packet)).await??;
        Ok(())
    }
}

impl SpeechAdapter for BouyomiAdapter {
    async fn health_check(&self) -> anyhow::Result<SpeechHealth> {
        BouyomiAdapter::health_check(self).await?;
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

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_health_check(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let adapter = adapter_from_settings(&state)?;
    adapter
        .health_check()
        .await
        .map(|elapsed| {
            format!(
                "棒読みちゃんに接続できました。応答時間 {}ms",
                elapsed.as_millis()
            )
        })
        .map_err(to_user_message)
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_test(state: tauri::State<'_, AppState>, text: String) -> Result<(), String> {
    let adapter = adapter_from_settings(&state)?;
    let text = normalize_test_text(&text);
    adapter.speak(&text).await.map_err(to_user_message)
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_pause(state: tauri::State<'_, AppState>) -> Result<(), String> {
    control_from_settings(&state, BouyomiControlCommand::Pause).await
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_resume(state: tauri::State<'_, AppState>) -> Result<(), String> {
    control_from_settings(&state, BouyomiControlCommand::Resume).await
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_skip(state: tauri::State<'_, AppState>) -> Result<(), String> {
    control_from_settings(&state, BouyomiControlCommand::Skip).await
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn speech_clear(state: tauri::State<'_, AppState>) -> Result<(), String> {
    control_from_settings(&state, BouyomiControlCommand::Clear).await
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
fn adapter_from_settings(state: &tauri::State<'_, AppState>) -> Result<BouyomiAdapter, String> {
    let settings = state.settings.lock().map_err(|error| error.to_string())?;
    let addr = format!(
        "{}:{}",
        settings.speech.bouyomi_host, settings.speech.bouyomi_port
    );

    let defaults = BouyomiTalkConfig {
        speed: settings.speech.bouyomi_speed,
        tone: settings.speech.bouyomi_tone,
        volume: settings.speech.bouyomi_volume,
        voice: settings.speech.bouyomi_voice,
        code: 0,
    };

    Ok(BouyomiAdapter::new(addr, defaults))
}

fn normalize_test_text(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        "テスト発話です。".to_string()
    } else {
        text.chars().take(120).collect()
    }
}

fn to_user_message(error: anyhow::Error) -> String {
    let message = error.to_string();
    if message.contains("Connection refused")
        || message.contains("os error 111")
        || message.contains("os error 10061")
    {
        "棒読みちゃんに接続できません。棒読みちゃんが起動中で、アプリ連携が有効か確認してください。"
            .to_string()
    } else if message.contains("timed out") || message.contains("elapsed") {
        "棒読みちゃんへの接続がタイムアウトしました。ポート番号とセキュリティソフトの設定を確認してください。".to_string()
    } else {
        format!("棒読みちゃん連携でエラーが発生しました: {message}")
    }
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
    }
}
