//! Home Assistant WebSocket → Akasha signed automation webhook.

use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use std::collections::HashMap;
use std::env;
use std::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

type HmacSha256 = Hmac<Sha256>;

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn ws_url(base: &str) -> Result<String, String> {
    let trimmed = base.trim_end_matches('/');
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let scheme = if trimmed.starts_with("https://") {
        "wss"
    } else {
        "ws"
    };
    Ok(format!("{scheme}://{rest}/api/websocket"))
}

fn domain_allowed(entity_id: &str, domains: &[String]) -> bool {
    let domain = entity_id.split('.').next().unwrap_or("");
    domains.iter().any(|d| d == domain)
}

fn sign_body(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

async fn post_webhook(
    client: &reqwest::Client,
    daemon_url: &str,
    secret: &str,
    payload: Value,
    idem_key: &str,
) -> Result<(), String> {
    let body = payload.to_string();
    let sig = sign_body(secret, body.as_bytes());
    let resp = client
        .post(format!("{}/api/automation/webhook", daemon_url.trim_end_matches('/')))
        .header("Content-Type", "application/json")
        .header("X-Signature", sig)
        .header("Idempotency-Key", idem_key)
        .body(body)
        .send()
        .await
        .map_err(|e| format!("webhook POST failed: {e}"))?;
    if !resp.status().is_success() && resp.status().as_u16() != 202 {
        return Err(format!("webhook HTTP {}", resp.status()));
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let ha_base = env::var("HA_BASE_URL").expect("HA_BASE_URL required");
    let token = env::var("HA_ACCESS_TOKEN").expect("HA_ACCESS_TOKEN required");
    let daemon_url = env_or("AKASHA_DAEMON_URL", "http://127.0.0.1:3876");
    let secret = env::var("AKASHA_AUTOMATION_WEBHOOK_SECRET")
        .expect("AKASHA_AUTOMATION_WEBHOOK_SECRET required");
    let domains: Vec<String> = env_or(
        "HA_EVENT_DOMAINS",
        "binary_sensor,sensor,device_tracker,lock",
    )
    .split(',')
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .collect();
    let debounce_ms: u64 = env_or("HA_EVENT_DEBOUNCE_MS", "2000").parse().unwrap_or(2000);

    let ws = match ws_url(&ha_base) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    eprintln!("connecting to {ws}");
    let client = reqwest::Client::new();
    let mut debounce: HashMap<String, Instant> = HashMap::new();

    loop {
        let (ws_stream, _) = match connect_async(&ws).await {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("websocket connect failed: {e}; retry in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        let (mut write, mut read) = ws_stream.split();
        let mut authed = false;
        let mut subscribed = false;

        while let Some(msg) = read.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("websocket read error: {e}");
                    break;
                }
            };
            if msg.is_close() {
                break;
            }
            let text = match msg.into_text() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let v: Value = match serde_json::from_str(&text) {
                Ok(j) => j,
                Err(_) => continue,
            };
            let msg_type = v.get("type").and_then(|x| x.as_str()).unwrap_or("");

            if msg_type == "auth_required" && !authed {
                let auth = json!({"type": "auth", "access_token": token});
                if write.send(Message::Text(auth.to_string())).await.is_err() {
                    break;
                }
                continue;
            }
            if msg_type == "auth_ok" {
                authed = true;
                if !subscribed {
                    let sub = json!({"type": "subscribe_events", "event_type": "state_changed"});
                    if write.send(Message::Text(sub.to_string())).await.is_err() {
                        break;
                    }
                    subscribed = true;
                    eprintln!("subscribed to state_changed");
                }
                continue;
            }
            if msg_type == "auth_invalid" {
                eprintln!("HA auth failed");
                std::process::exit(1);
            }
            if msg_type != "event" {
                continue;
            }
            let event = v.get("event").cloned().unwrap_or(json!({}));
            if event.get("event_type").and_then(|x| x.as_str()) != Some("state_changed") {
                continue;
            }
            let data = event.get("data").cloned().unwrap_or(json!({}));
            let entity_id = data
                .get("entity_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if entity_id.is_empty() || !domain_allowed(&entity_id, &domains) {
                continue;
            }
            let now = Instant::now();
            if let Some(last) = debounce.get(&entity_id) {
                if now.duration_since(*last) < Duration::from_millis(debounce_ms) {
                    continue;
                }
            }
            debounce.insert(entity_id.clone(), now);

            let old_state = data
                .pointer("/old_state/state")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let new_state = data
                .pointer("/new_state/state")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if old_state == new_state {
                continue;
            }
            let attributes = data
                .pointer("/new_state/attributes")
                .cloned()
                .unwrap_or(json!({}));
            let payload = json!({
                "source": "homeassistant",
                "event": "state_changed",
                "entity_id": entity_id,
                "old_state": old_state,
                "new_state": new_state,
                "attributes": attributes,
            });
            let idem = format!("ha-{}-{}-{}", entity_id, new_state, now.elapsed().as_nanos());
            if let Err(e) = post_webhook(&client, &daemon_url, &secret, payload, &idem).await {
                eprintln!("{e}");
            } else {
                eprintln!("webhook → akasha: {entity_id} {old_state} → {new_state}");
            }
        }
        eprintln!("websocket closed; reconnect in 5s");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
