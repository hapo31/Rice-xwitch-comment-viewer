pub mod bouyomi;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechRequest {
    pub id: String,
    pub source_message_id: Option<String>,
    pub text: String,
    pub voice: Option<String>,
    pub speed: Option<i16>,
    pub tone: Option<i16>,
    pub volume: Option<i16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeechHealth {
    Connected,
    Disconnected { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeechResult {
    Accepted,
}

#[allow(dead_code)]
pub trait SpeechAdapter: Send + Sync {
    fn health_check(&self) -> impl std::future::Future<Output = anyhow::Result<SpeechHealth>> + Send;
    fn speak(&self, request: SpeechRequest) -> impl std::future::Future<Output = anyhow::Result<SpeechResult>> + Send;
}
