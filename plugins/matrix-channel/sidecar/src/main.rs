//! Matrix channel sidecar — polls homeserver sync and forwards room messages to Akasha daemon.
//!
//! Required env:
//! - `MATRIX_HOMESERVER_URL` — e.g. https://matrix.example.org
//! - `MATRIX_ACCESS_TOKEN` — user access token (or vault export)
//!
//! Optional:
//! - `AKASHA_DAEMON_URL` — default http://127.0.0.1:3876
//! - `MATRIX_ROOM_ID` — single room filter (default: all joined rooms)
//! - `MATRIX_SYNC_TIMEOUT_MS` — long-poll timeout (default 30000)

use reqwest::blocking::Client;
use reqwest::Url;
use serde_json::{json, Value};
use std::env;
use std::thread;
use std::time::Duration;

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MatrixMessage {
    room_id: String,
    sender: String,
    event_id: String,
    text: String,
}

fn build_sync_url(base: &str, sync_timeout_ms: u64, since: Option<&str>) -> Result<String, String> {
    let mut url = Url::parse(&format!(
        "{}/_matrix/client/v3/sync",
        base.trim_end_matches('/')
    ))
    .map_err(|e| format!("invalid MATRIX_HOMESERVER_URL: {e}"))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("timeout", &sync_timeout_ms.to_string());
        if let Some(s) = since {
            query.append_pair("since", s);
        }
    }
    Ok(url.to_string())
}

fn extract_messages(body: &Value, room_filter: Option<&str>) -> Vec<MatrixMessage> {
    let rooms = body
        .pointer("/rooms/join")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for (room_id, room_data) in rooms {
        if let Some(filter) = room_filter {
            if room_id != filter {
                continue;
            }
        }
        let events = room_data
            .pointer("/timeline/events")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for ev in events {
            if ev.get("type").and_then(Value::as_str) != Some("m.room.message") {
                continue;
            }
            let Some(text) = ev.pointer("/content/body").and_then(Value::as_str) else {
                continue;
            };
            if text.trim().is_empty() {
                continue;
            }
            out.push(MatrixMessage {
                room_id: room_id.clone(),
                sender: ev
                    .get("sender")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                event_id: ev
                    .get("event_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                text: text.to_string(),
            });
        }
    }
    out
}

fn main() {
    if env::var("MATRIX_HOMESERVER_URL").is_err() {
        eprintln!("akasha-matrix-sidecar: set MATRIX_HOMESERVER_URL and MATRIX_ACCESS_TOKEN.");
        std::process::exit(1);
    }

    let homeserver = env_or("MATRIX_HOMESERVER_URL", "");
    let token = env_or("MATRIX_ACCESS_TOKEN", "");
    if homeserver.is_empty() || token.is_empty() {
        eprintln!("akasha-matrix-sidecar: MATRIX_HOMESERVER_URL and MATRIX_ACCESS_TOKEN are required.");
        std::process::exit(1);
    }

    let daemon_url = env_or("AKASHA_DAEMON_URL", "http://127.0.0.1:3876");
    let room_filter = env::var("MATRIX_ROOM_ID").ok().filter(|s| !s.trim().is_empty());
    let sync_timeout_ms: u64 = env::var("MATRIX_SYNC_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30_000);

    let client = Client::builder()
        .timeout(Duration::from_millis(sync_timeout_ms + 10_000))
        .build()
        .expect("http client");

    let base = homeserver.trim_end_matches('/');
    let mut since: Option<String> = None;

    eprintln!(
        "akasha-matrix-sidecar: listening on {base} → {daemon_url}/api/message"
    );

    loop {
        let initial_sync = since.is_none();
        let url = match build_sync_url(base, sync_timeout_ms, since.as_deref()) {
            Ok(url) => url,
            Err(e) => {
                eprintln!("matrix sync URL build failed: {e}");
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        let resp = match client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("matrix sync request failed: {e}");
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        if !resp.status().is_success() {
            eprintln!("matrix sync HTTP {}", resp.status());
            thread::sleep(Duration::from_secs(5));
            continue;
        }

        let body: Value = match resp.json() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("matrix sync JSON parse failed: {e}");
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        let next_batch = body
            .get("next_batch")
            .and_then(Value::as_str)
            .map(|next| next.to_string());

        if initial_sync {
            if let Some(next) = next_batch {
                since = Some(next);
            }
            continue;
        }

        let mut delivered = true;
        for message_data in extract_messages(&body, room_filter.as_deref()) {
            let room_id = &message_data.room_id;
            let sender = &message_data.sender;
            let event_id = &message_data.event_id;
            let text = &message_data.text;
            let message = format!("[Matrix {room_id} from {sender}]\n{text}");
            let payload = json!({
                "message": message,
                "session_id": format!("matrix-{room_id}"),
                "channel": "matrix",
                "matrix_event_id": event_id,
                "matrix_room_id": room_id,
                "matrix_sender": sender,
            });
            match client
                .post(format!("{daemon_url}/api/message"))
                .json(&payload)
                .send()
            {
                Ok(r) if r.status().is_success() => {
                    eprintln!("forwarded matrix message {event_id} → akasha");
                }
                Ok(r) => {
                    eprintln!("akasha POST /api/message HTTP {}", r.status());
                    delivered = false;
                    break;
                }
                Err(e) => {
                    eprintln!("akasha POST /api/message failed: {e}");
                    delivered = false;
                    break;
                }
            }
        }

        if delivered {
            if let Some(next) = next_batch {
                since = Some(next);
            }
        } else {
            eprintln!("delivery failed; retaining since token to replay events");
            thread::sleep(Duration::from_secs(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{build_sync_url, extract_messages};
    use serde_json::json;

    #[test]
    fn build_sync_url_encodes_since_token() {
        let url = build_sync_url(
            "https://matrix.example.org",
            30_000,
            Some("s123+/=?&token"),
        )
        .expect("url");
        assert_eq!(
            url,
            "https://matrix.example.org/_matrix/client/v3/sync?timeout=30000&since=s123%2B%2F%3D%3F%26token"
        );
    }

    #[test]
    fn extract_messages_filters_room_and_type() {
        let body = json!({
            "rooms": {
                "join": {
                    "!keep:example.org": {
                        "timeline": {
                            "events": [
                                {
                                    "type": "m.room.message",
                                    "event_id": "$1",
                                    "sender": "@alice:example.org",
                                    "content": { "body": "hello" }
                                },
                                {
                                    "type": "m.room.member",
                                    "event_id": "$2",
                                    "sender": "@alice:example.org",
                                    "content": { "body": "ignored" }
                                }
                            ]
                        }
                    },
                    "!drop:example.org": {
                        "timeline": {
                            "events": [
                                {
                                    "type": "m.room.message",
                                    "event_id": "$3",
                                    "sender": "@bob:example.org",
                                    "content": { "body": "ignored by room filter" }
                                }
                            ]
                        }
                    }
                }
            }
        });

        let messages = extract_messages(&body, Some("!keep:example.org"));
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].event_id, "$1");
        assert_eq!(messages[0].text, "hello");
    }
}
