//! CalDAV sidecar — pull sync + outbox push toward Akasha daemon.

use akasha_calendar::parse_ics;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::env;
use std::thread;
use std::time::Duration as StdDuration;

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

fn basic_auth_header(user: &str, pass: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
    format!("Basic {token}")
}

fn normalize_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{path}")
    }
}

fn propfind_calendar_home(client: &Client, base_url: &str, auth: &str) -> Result<String, String> {
    let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
  <D:prop><D:current-user-principal/></D:prop>
</D:propfind>"#;
    let resp = client
        .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), base_url)
        .header("Authorization", auth)
        .header("Depth", "0")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("PROPFIND status {}", resp.status()));
    }
    let text = resp.text().map_err(|e| e.to_string())?;
    if let Some(start) = text.find("<D:href>") {
        if let Some(end) = text[start + 8..].find("</D:href>") {
            let href = &text[start + 8..start + 8 + end];
            return Ok(normalize_url(base_url, href));
        }
    }
    Ok(base_url.to_string())
}

fn calendar_query(
    client: &Client,
    calendar_url: &str,
    auth: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Value>, String> {
    let start = from.format("%Y%m%dT%H%M%SZ").to_string();
    let end = to.format("%Y%m%dT%H%M%SZ").to_string();
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8" ?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT">
        <C:time-range start="{start}" end="{end}"/>
      </C:comp-filter>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#
    );
    let resp = client
        .request(
            reqwest::Method::from_bytes(b"REPORT").unwrap(),
            calendar_url,
        )
        .header("Authorization", auth)
        .header("Depth", "1")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("REPORT status {}", resp.status()));
    }
    let text = resp.text().map_err(|e| e.to_string())?;
    let mut events = Vec::new();
    for chunk in text.split("<C:calendar-data").skip(1) {
        let Some(data_start) = chunk.find('>') else { continue };
        let Some(data_end) = chunk[data_start + 1..].find("</C:calendar-data>") else {
            continue;
        };
        let ics = &chunk[data_start + 1..data_start + 1 + data_end];
        let href = chunk
            .split("<D:href>")
            .nth(1)
            .and_then(|s| s.split("</D:href>").next())
            .unwrap_or("")
            .to_string();
        let etag = chunk
            .split("<D:getetag>")
            .nth(1)
            .and_then(|s| s.split("</D:getetag>").next())
            .map(|s| s.trim().trim_matches('"').to_string());
        if let Ok(parsed) = parse_ics(ics) {
            for ev in parsed {
                events.push(json!({
                    "uid": ev.uid,
                    "href": href,
                    "etag": etag,
                    "summary": ev.summary,
                    "description": ev.description,
                    "location": ev.location,
                    "dtstart": ev.dtstart.to_rfc3339(),
                    "dtend": ev.dtend.map(|t| t.to_rfc3339()),
                    "rrule": ev.rrule,
                }));
            }
        }
    }
    Ok(events)
}

fn push_sync_to_daemon(
    client: &Client,
    daemon_url: &str,
    token: &str,
    account_id: &str,
    events: Vec<Value>,
    sync_token: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let url = format!("{daemon_url}/api/calendar/external/sync");
    let mut req = client
        .post(&url)
        .json(&json!({
            "account_id": account_id,
            "events": events,
            "deleted_hrefs": [],
            "sync_token": sync_token,
            "error": error,
        }));
    if !token.is_empty() {
        req = req.header("X-Akasha-Calendar-Token", token);
    }
    let resp = req.send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("daemon sync status {}", resp.status()));
    }
    Ok(())
}

fn process_outbox(
    client: &Client,
    daemon_url: &str,
    token: &str,
    cal_url: &str,
    auth: &str,
    account_id: &str,
) {
    let url = format!(
        "{daemon_url}/api/calendar/external/outbox/pending?account_id={account_id}"
    );
    let mut req = client.get(&url);
    if !token.is_empty() {
        req = req.header("X-Akasha-Calendar-Token", token);
    }
    let Ok(resp) = req.send() else { return };
    let Ok(body) = resp.json::<Value>() else { return };
    let Some(items) = body.get("outbox").and_then(|v| v.as_array()) else {
        return;
    };
    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let op = item.get("operation").and_then(|v| v.as_str()).unwrap_or("");
        let payload = item.get("payload_json").and_then(|v| v.as_str()).unwrap_or("{}");
        let mut err: Option<String> = None;
        match op {
            "delete" => {
                if let Ok(p) = serde_json::from_str::<Value>(payload) {
                    if let Some(href) = p.get("href").and_then(|v| v.as_str()) {
                        let target = normalize_url(cal_url, href);
                        let r = client
                            .request(reqwest::Method::DELETE, &target)
                            .header("Authorization", auth)
                            .send();
                        if let Ok(r) = r {
                            if !r.status().is_success() && r.status().as_u16() != 404 {
                                err = Some(format!("DELETE {}", r.status()));
                            }
                        } else {
                            err = Some("DELETE failed".into());
                        }
                    }
                }
            }
            "create" | "update" => {
                if let Ok(p) = serde_json::from_str::<Value>(payload) {
                    let uid = p.get("uid").and_then(|v| v.as_str()).unwrap_or("akasha-event");
                    let summary = p
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Event");
                    let dtstart = p.get("dtstart").and_then(|v| v.as_str()).unwrap_or("");
                    let dtend = p.get("dtend").and_then(|v| v.as_str()).unwrap_or("");
                    let ics = format!(
                        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nSUMMARY:{summary}\r\nDTSTART:{dtstart}\r\nDTEND:{dtend}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
                    );
                    let href = p
                        .get("href")
                        .and_then(|v| v.as_str())
                        .map(|h| normalize_url(cal_url, h))
                        .unwrap_or_else(|| normalize_url(cal_url, &format!("{uid}.ics")));
                    let method = if op == "create" {
                        reqwest::Method::PUT
                    } else {
                        reqwest::Method::PUT
                    };
                    let mut req = client
                        .request(method, &href)
                        .header("Authorization", auth)
                        .header("Content-Type", "text/calendar; charset=utf-8")
                        .body(ics);
                    if let Some(etag) = p.get("etag").and_then(|v| v.as_str()) {
                        req = req.header("If-Match", etag);
                    }
                    match req.send() {
                        Ok(r) if r.status().is_success() || r.status().as_u16() == 201 => {}
                        Ok(r) => err = Some(format!("PUT {}", r.status())),
                        Err(e) => err = Some(e.to_string()),
                    }
                }
            }
            _ => {}
        }
        let ack_url = format!("{daemon_url}/api/calendar/external/outbox/ack");
        let mut ack = client.post(&ack_url).json(&json!({ "id": id, "error": err }));
        if !token.is_empty() {
            ack = ack.header("X-Akasha-Calendar-Token", token);
        }
        let _ = ack.send();
    }
}

fn sync_once(
    client: &Client,
    caldav_url: &str,
    cal_path: &str,
    user: &str,
    pass: &str,
    daemon_url: &str,
    token: &str,
    account_id: &str,
) {
    let auth = basic_auth_header(user, pass);
    let home = match propfind_calendar_home(client, caldav_url, &auth) {
        Ok(h) => h,
        Err(e) => {
            let _ = push_sync_to_daemon(client, daemon_url, token, account_id, vec![], None, Some(&e));
            return;
        }
    };
    let calendar_url = if cal_path.is_empty() {
        home
    } else {
        normalize_url(&home, cal_path)
    };
    let now = Utc::now();
    let from = now - Duration::days(30);
    let to = now + Duration::days(90);
    match calendar_query(client, &calendar_url, &auth, from, to) {
        Ok(events) => {
            let token_str = format!("sync-{}", now.timestamp());
            let _ = push_sync_to_daemon(
                client,
                daemon_url,
                token,
                account_id,
                events,
                Some(&token_str),
                None,
            );
            process_outbox(client, daemon_url, token, &calendar_url, &auth, account_id);
        }
        Err(e) => {
            let _ = push_sync_to_daemon(client, daemon_url, token, account_id, vec![], None, Some(&e));
        }
    }
}

fn main() {
    if !truthy("CALDAV_SIDECAR_ENABLED") && env::var("CALDAV_URL").is_err() {
        eprintln!("akasha-caldav-sidecar: set CALDAV_URL + credentials or CALDAV_SIDECAR_ENABLED=1");
        std::process::exit(1);
    }
    let caldav_url = env_or("CALDAV_URL", "");
    let user = env_or("CALDAV_USERNAME", "");
    let pass = env_or("CALDAV_PASSWORD", "");
    if caldav_url.is_empty() || user.is_empty() || pass.is_empty() {
        eprintln!("CALDAV_URL, CALDAV_USERNAME, CALDAV_PASSWORD required");
        std::process::exit(1);
    }
    let daemon_url = env_or("AKASHA_DAEMON_URL", "http://127.0.0.1:3876");
    let cal_path = env_or("CALDAV_CALENDAR_PATH", "");
    let account_id = env_or("CALDAV_ACCOUNT_ID", "00000000-0000-4000-8000-000000000001");
    let sync_token = env_or("AKASHA_CALENDAR_SYNC_TOKEN", "");
    let interval: u64 = env::var("CALDAV_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);

    let client = Client::builder()
        .timeout(StdDuration::from_secs(120))
        .build()
        .expect("http client");

    eprintln!(
        "akasha-caldav-sidecar: syncing {caldav_url} every {interval}s → {daemon_url}"
    );
    loop {
        sync_once(
            &client,
            &caldav_url,
            &cal_path,
            &user,
            &pass,
            &daemon_url,
            &sync_token,
            &account_id,
        );
        thread::sleep(StdDuration::from_secs(interval));
    }
}
