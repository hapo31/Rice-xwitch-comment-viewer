use super::*;
use crate::twitch::auth_state::{EventSubConnectionParams, PendingDeviceAuth};
use crate::twitch::error::{SubscriptionRequestError, TwitchApiError, TwitchAuthFailure};
use crate::twitch::oauth::{DeviceOAuthTransport, OAuthTransport, PollAuthError};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

struct Reply(u16, &'static str);
struct Server {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    connections: Arc<std::sync::atomic::AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start(replies: Vec<Reply>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let replies = Arc::new(Mutex::new(VecDeque::from(replies)));
        let seen = requests.clone();
        let accepted = connections.clone();
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let (socket, _) = result.unwrap();
                        accepted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let replies = replies.clone();
                        let seen = seen.clone();
                        clients.spawn(async move {
                            let mut socket = BufReader::new(socket);
                            loop {
                                let mut request = String::new();
                                if socket.read_line(&mut request).await.unwrap_or(0) == 0 { break; }
                                let mut length = 0;
                                loop {
                                    let mut line = String::new();
                                    if socket.read_line(&mut line).await.unwrap_or(0) == 0 { return; }
                                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                                        length = value.trim().parse::<usize>().unwrap();
                                    }
                                    request.push_str(&line);
                                    if line == "\r\n" { break; }
                                }
                                assert!(length < 16384);
                                let mut body = vec![0; length];
                                socket.read_exact(&mut body).await.unwrap();
                                request.push_str(std::str::from_utf8(&body).unwrap());
                                seen.lock().unwrap().push(request);
                                let reply = replies.lock().unwrap().pop_front();
                                // Missing reply deliberately stalls until the client's deadline.
                                let Some(Reply(status, body)) = reply else {
                                    std::future::pending::<()>().await;
                                    unreachable!();
                                };
                                let response = format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
                                if socket.write_all(response.as_bytes()).await.is_err() { break; }
                                socket.flush().await.unwrap();
                            }
                        });
                    }
                    result = clients.join_next(), if !clients.is_empty() => {
                        result.unwrap().unwrap();
                    }
                }
            }
        });
        Self {
            base,
            requests,
            connections,
            task,
        }
    }
    fn transport(&self, deadline: Duration) -> TwitchHttp {
        TwitchHttp {
            client: client_builder(deadline).no_proxy().build().unwrap(),
            endpoints: HttpEndpoints {
                device: format!("{}/device", self.base),
                token: format!("{}/token", self.base),
                validate: format!("{}/validate", self.base),
                users: format!("{}/users", self.base),
                subscriptions: format!("{}/subscriptions", self.base),
            },
        }
    }
}
fn pending() -> PendingDeviceAuth {
    PendingDeviceAuth {
        generation: 1,
        poll_in_flight: false,
        client_id: "device-client".into(),
        device_code: "device-code".into(),
        interval: 1,
    }
}
fn params() -> EventSubConnectionParams {
    EventSubConnectionParams {
        generation: 1,
        auth_generation: 1,
        broadcaster_user_id: "channel-id".into(),
        broadcaster_login: "channel".into(),
        client_id: "initial-client".into(),
        user_id: "user-id".into(),
    }
}
const DEVICE: &str = r#"{"device_code":"device-code","user_code":"code","verification_uri":"https://www.twitch.tv/activate","expires_in":600,"interval":1}"#;
const TOKEN: &str = r#"{"access_token":"access","refresh_token":"refresh","scope":["user:read:chat"],"expires_in":600}"#;
const PROFILE: &str = r#"{"client_id":"client","login":"user","user_id":"user-id","scopes":["user:read:chat"],"expires_in":600}"#;

#[tokio::test]
async fn cloned_transport_reuses_one_connection_and_sets_credentials_per_request() {
    let server = Server::start(vec![
        Reply(200, DEVICE),
        Reply(200, TOKEN),
        Reply(200, TOKEN),
        Reply(200, PROFILE),
        Reply(200, r#"{"data":[{"id":"channel-id","login":"channel"}]}"#),
        Reply(202, "{}"),
        Reply(200, DEVICE),
    ])
    .await;
    let http = server.transport(Duration::from_secs(5));
    let clone = http.clone();
    http.device_code("device-client").await.unwrap();
    assert!(clone.poll_token(&pending()).await.is_ok());
    http.refresh("refresh-client", "old-refresh").await.unwrap();
    clone.validate("old-access").await.unwrap();
    http.fetch_twitch_user("next-client", "next-access", "channel")
        .await
        .unwrap();
    clone
        .send_chat_message_subscription(&params(), "session", "rotated-client", "rotated-access")
        .await
        .unwrap();
    http.device_code("last-client").await.unwrap();
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 7);
    assert_eq!(
        server.connections.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "every request and clone must share the same pool"
    );
    for index in [0, 1, 2, 6] {
        assert!(!requests[index]
            .to_ascii_lowercase()
            .contains("authorization:"));
    }
    assert!(requests[1].contains("device_code=device-code"));
    assert!(requests[2].contains("refresh_token=old-refresh"));
    for (index, client, token) in [
        (4, "next-client", "next-access"),
        (5, "rotated-client", "rotated-access"),
    ] {
        let headers = requests[index].to_ascii_lowercase();
        assert!(headers.contains(&format!("client-id: {client}\r\n")));
        assert!(headers.contains(&format!("authorization: bearer {token}\r\n")));
        assert!(!headers.contains("old-access"));
    }
    assert!(requests[3]
        .to_ascii_lowercase()
        .contains("authorization: bearer old-access\r\n"));
    assert!(requests[5].contains("\"session_id\":\"session\""));
}

#[tokio::test]
async fn every_http_operation_retains_the_shared_deadline() {
    let server = Server::start(vec![]).await;
    let http = server.transport(Duration::from_millis(500));
    fn timed_out(error: anyhow::Error) {
        assert!(
            error
                .downcast_ref::<reqwest::Error>()
                .is_some_and(reqwest::Error::is_timeout),
            "{error}"
        );
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        timed_out(http.device_code("client").await.unwrap_err());
        match http.poll_token(&pending()).await { Err(PollAuthError::Other(error)) => timed_out(error), _ => panic!("poll must time out") }
        timed_out(http.refresh("client", "refresh").await.unwrap_err());
        timed_out(http.validate("access").await.unwrap_err());
        timed_out(http.fetch_twitch_user("client", "access", "channel").await.unwrap_err());
        match http.send_chat_message_subscription(&params(), "session", "client", "access").await {
            Err(SubscriptionRequestError::Retryable(error)) => assert!(matches!(error.downcast_ref::<TwitchApiError>(), Some(TwitchApiError::Transport(error)) if error.is_timeout())),
            other => panic!("subscription timeout must be retryable: {other:?}"),
        }
    }).await.expect("all six operations must have a deadline");
    assert_eq!(server.requests.lock().unwrap().len(), 6);
}

#[tokio::test]
async fn shared_transport_preserves_auth_poll_and_subscription_error_policy() {
    let server = Server::start(vec![
        Reply(401, r#"{"message":"invalid"}"#),
        Reply(400, r#"{"error":"invalid_grant"}"#),
        Reply(400, r#"{"error":"authorization_pending"}"#),
        Reply(400, r#"{"error":"slow_down"}"#),
        Reply(400, r#"{"error":"access_denied"}"#),
        Reply(400, r#"{"error":"expired_token"}"#),
        Reply(401, "{}"),
        Reply(503, "{}"),
        Reply(403, "{}"),
    ])
    .await;
    let http = server.transport(Duration::from_secs(5));
    for (error, expected) in [
        (
            http.validate("expired").await.unwrap_err(),
            TwitchAuthFailure::InvalidAccessToken,
        ),
        (
            http.refresh("client", "revoked").await.unwrap_err(),
            TwitchAuthFailure::InvalidGrant,
        ),
    ] {
        assert_eq!(
            error
                .downcast_ref::<TwitchApiError>()
                .unwrap()
                .auth_failure(),
            Some(expected)
        );
    }
    assert!(matches!(
        http.poll_token(&pending()).await,
        Err(PollAuthError::Pending)
    ));
    assert!(matches!(
        http.poll_token(&pending()).await,
        Err(PollAuthError::SlowDown)
    ));
    assert!(matches!(
        http.poll_token(&pending()).await,
        Err(PollAuthError::Denied)
    ));
    assert!(matches!(
        http.poll_token(&pending()).await,
        Err(PollAuthError::Expired)
    ));
    assert!(matches!(
        http.send_chat_message_subscription(&params(), "session", "client", "access")
            .await,
        Err(SubscriptionRequestError::Unauthorized)
    ));
    assert!(matches!(
        http.send_chat_message_subscription(&params(), "session", "client", "access")
            .await,
        Err(SubscriptionRequestError::Retryable(_))
    ));
    assert!(matches!(
        http.send_chat_message_subscription(&params(), "session", "client", "access")
            .await,
        Err(SubscriptionRequestError::Permanent(TwitchApiError::Http {
            status: 403,
            ..
        }))
    ));
}
