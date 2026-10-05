use super::model::ChatMessage;

/// Deliver one normalized chat model to both sinks only while its connection
/// generation is still active. Callers keep their connection-state guard held
/// for this function so stop/replacement cannot split the two deliveries.
pub(super) fn dispatch_chat_message(
    message: &ChatMessage,
    active_generation: Option<u64>,
    ui_delivery: impl FnOnce(&ChatMessage),
    speech_enqueue: impl FnOnce(&ChatMessage),
) -> bool {
    let Some(active_generation) = active_generation else {
        return false;
    };
    if !message.belongs_to_connection_generation(active_generation) {
        return false;
    }

    ui_delivery(message);
    speech_enqueue(message);
    true
}

#[cfg(test)]
mod tests {
    use super::dispatch_chat_message;
    use crate::twitch::model::{ChatMessage, Platform};
    use chrono::Utc;
    use std::cell::RefCell;

    fn message(generation: Option<u64>) -> ChatMessage {
        ChatMessage {
            id: "message".into(),
            platform: Platform::Twitch,
            channel_id: "channel".into(),
            channel_login: "channel".into(),
            user_id: "user".into(),
            user_login: "user".into(),
            user_display_name: "User".into(),
            text: "hello".into(),
            fragments: Vec::new(),
            badges: Vec::new(),
            received_at: Utc::now(),
            connection_generation: generation,
        }
    }

    #[test]
    fn shared_boundary_delivers_identical_generation_or_rejects_both_sinks() {
        let accepted = message(Some(7));
        let ui = RefCell::new(Vec::new());
        let speech = RefCell::new(Vec::new());
        assert!(dispatch_chat_message(
            &accepted,
            Some(7),
            |message| ui.borrow_mut().push(message.connection_generation),
            |message| speech.borrow_mut().push(message.connection_generation),
        ));
        assert_eq!(*ui.borrow(), [Some(7)]);
        assert_eq!(*speech.borrow(), [Some(7)]);

        for (stale_message, active_generation) in
            [(message(Some(6)), Some(7)), (message(Some(7)), None)]
        {
            let ui = RefCell::new(Vec::new());
            let speech = RefCell::new(Vec::new());
            assert!(!dispatch_chat_message(
                &stale_message,
                active_generation,
                |_| ui.borrow_mut().push(()),
                |_| speech.borrow_mut().push(()),
            ));
            assert!(ui.borrow().is_empty());
            assert!(speech.borrow().is_empty());
        }
    }
}
