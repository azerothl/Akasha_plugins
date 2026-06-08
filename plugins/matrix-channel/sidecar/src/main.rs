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

fn truthy(key: &str) -> bool {
    env::var(key)
        .ok()
        .as_deref()
        .map(|s| matches!(s, "1" | "true" | "yes" | "on" | "TRUE" | "YES" | "ON"))
        .unwrap_or(false)
}

fn main() {
    if !truthy("MATRIX_SIDECAR_ENABLED") && env::var("MATRIX_HOMESERVER_URL").is_err() {
        eprintln!(
            "akasha-matrix-sidecar: set MATRIX_HOMESERVER_URL and MATRIX_ACCESS_TOKEN (or MATRIX_SIDECAR_ENABLED=1)."
        );
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
        let mut url = format!(
            "{base}/_matrix/client/v3/sync?timeout={sync_timeout_ms}",
        );
        if let Some(ref s) = since {
            url.push_str("&since=");
            url.push_str(s);
        }

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

        if let Some(next) = body.get("next_batch").and_then(Value::as_str) {
            since = Some(next.to_string());
        }

        let rooms = body
            .pointer("/rooms/join")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        for (room_id, room_data) in rooms {
            if let Some(ref filter) = room_filter {
                if room_id != *filter {
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
                let sender = ev
                    .get("sender")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let content = ev.pointer("/content/body").and_then(Value::as_str);
                let Some(text) = content else { continue };
                if text.trim().is_empty() {
                    continue;
                }
                let event_id = ev
                    .get("event_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
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
                    }
                    Err(e) => {
                        eprintln!("akasha POST /api/message failed: {e}");
                    }
                }
            }
        }
    }
}
