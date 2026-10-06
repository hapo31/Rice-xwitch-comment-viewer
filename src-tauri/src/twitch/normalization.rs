//! EventSub wire decoding and chat normalization with an explicit receive clock.
use super::model::{ChatBadge, ChatMessage, MessageFragment, Platform};
use super::CHANNEL_CHAT_MESSAGE_TYPE;
use chrono::{DateTime, Timelike, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct EventSubEnvelope {
    pub(super) metadata: EventSubMetadata,
    #[serde(default)]
    pub(super) payload: EventSubPayload,
}

#[derive(Debug, Deserialize)]
pub(super) struct EventSubMetadata {
    pub(super) message_id: String,
    pub(super) message_type: String,
    #[serde(default)]
    pub(super) message_timestamp: Option<serde_json::Value>,
    pub(super) subscription_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct EventSubPayload {
    pub(super) session: Option<EventSubSession>,
    pub(super) subscription: Option<EventSubSubscription>,
    pub(super) event: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct EventSubSession {
    pub(super) id: String,
    pub(super) keepalive_timeout_seconds: Option<u64>,
    pub(super) reconnect_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct EventSubSubscription {
    pub(super) status: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
}

#[derive(Debug, Deserialize)]
struct EventSubChatMessageEvent {
    broadcaster_user_id: String,
    broadcaster_user_login: String,
    chatter_user_id: String,
    chatter_user_login: String,
    chatter_user_name: String,
    message_id: String,
    message: EventSubChatMessageBody,
    #[serde(default)]
    badges: Vec<ChatBadge>,
}

#[derive(Debug, Deserialize)]
struct EventSubChatMessageBody {
    text: String,
    #[serde(default)]
    fragments: Vec<MessageFragment>,
}

pub(super) struct NormalizedChatMessage {
    pub(super) message: ChatMessage,
    pub(super) timestamp_warning: Option<String>,
}

pub(super) fn normalize_chat_message(
    envelope: EventSubEnvelope,
    fallback_received_at: DateTime<Utc>,
    connection_generation: u64,
) -> anyhow::Result<Option<NormalizedChatMessage>> {
    if envelope.metadata.subscription_type.as_deref() != Some(CHANNEL_CHAT_MESSAGE_TYPE) {
        return Ok(None);
    }

    let event = match envelope.payload.event {
        Some(event) => event,
        None => return Ok(None),
    };
    let event = serde_json::from_value::<EventSubChatMessageEvent>(event)?;
    let (received_at, used_timestamp_fallback) = match envelope
        .metadata
        .message_timestamp
        .as_ref()
        .and_then(serde_json::Value::as_str)
        .and_then(parse_chat_timestamp)
    {
        Some(timestamp) => (timestamp, false),
        None => (fallback_received_at, true),
    };
    let id = if event.message_id.is_empty() {
        envelope.metadata.message_id
    } else {
        event.message_id
    };

    let timestamp_warning = used_timestamp_fallback.then(|| {
        format!("Twitch チャット {id} の受信時刻が不正なため、WebSocket 受信時刻を使用しました。")
    });

    Ok(Some(NormalizedChatMessage {
        message: ChatMessage {
            id,
            platform: Platform::Twitch,
            channel_id: event.broadcaster_user_id,
            channel_login: event.broadcaster_user_login,
            user_id: event.chatter_user_id,
            user_login: event.chatter_user_login,
            user_display_name: event.chatter_user_name,
            text: event.message.text,
            fragments: event.message.fragments,
            badges: event.badges,
            received_at,
            connection_generation: Some(connection_generation),
        },
        timestamp_warning,
    }))
}

fn parse_chat_timestamp(timestamp: &str) -> Option<DateTime<Utc>> {
    let timestamp = DateTime::parse_from_rfc3339(timestamp).ok()?;

    // JavaScript Date/Intl cannot represent RFC 3339 leap seconds. Reject them
    // at the backend boundary so Rust and the renderer use the same fallback.
    if timestamp.nanosecond() >= 1_000_000_000 {
        return None;
    }

    Some(timestamp.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("fixtures/channel_chat_message.json")).unwrap()
    }

    #[test]
    fn metadata_id_and_explicit_receive_time_are_fallbacks_not_global_clock_reads() {
        let mut value = fixture();
        value["payload"]["event"]["message_id"] = "".into();
        value["metadata"]["message_timestamp"] = serde_json::Value::Null;
        let expected_id = value["metadata"]["message_id"].as_str().unwrap().to_owned();
        let received_at = DateTime::parse_from_rfc3339("2026-08-15T12:34:56.789Z")
            .unwrap()
            .with_timezone(&Utc);
        let normalized =
            normalize_chat_message(serde_json::from_value(value).unwrap(), received_at, 0)
                .unwrap()
                .unwrap();
        assert_eq!(normalized.message.id, expected_id);
        assert_eq!(normalized.message.received_at, received_at);
        assert!(normalized.timestamp_warning.is_some());
    }

    #[test]
    fn unrelated_or_empty_notifications_do_not_produce_chat_messages() {
        let received_at = Utc::now();
        let mut unrelated = fixture();
        unrelated["metadata"]["subscription_type"] = "channel.follow".into();
        assert!(
            normalize_chat_message(serde_json::from_value(unrelated).unwrap(), received_at, 0)
                .unwrap()
                .is_none()
        );
        let mut empty = fixture();
        empty["payload"].as_object_mut().unwrap().remove("event");
        assert!(
            normalize_chat_message(serde_json::from_value(empty).unwrap(), received_at, 0)
                .unwrap()
                .is_none()
        );
    }
}
