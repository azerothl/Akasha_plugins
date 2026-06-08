//! CalDAV sidecar — pull sync + outbox push toward Akasha daemon.

use base64::Engine;
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use reqwest::{blocking::Client, Url};
use serde_json::{json, Value};
use std::env;
use std::thread;
use std::time::Duration as StdDuration;

#[derive(Default)]
struct ParsedEvent {
    uid: String,
    summary: String,
    description: String,
    location: String,
    dtstart: Option<DateTime<Utc>>,
    dtend: Option<DateTime<Utc>>,
    rrule: Option<String>,
}

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
    let path = path.trim();
    if path.is_empty() {
        return base.trim_end_matches('/').to_string();
    }
    if let Ok(url) = Url::parse(path) {
        return url.to_string();
    }
    Url::parse(base)
        .ok()
        .and_then(|url| url.join(path).ok())
        .map(|url| url.to_string())
        .unwrap_or_else(|| {
            let base = base.trim_end_matches('/');
            let path = path.trim_start_matches('/');
            format!("{base}/{path}")
        })
}

fn extract_xml_text<'a>(text: &'a str, names: &[&str]) -> Option<&'a str> {
    for name in names {
        let open = format!("<{name}");
        let Some(start) = text.find(&open) else {
            continue;
        };
        let Some(open_end) = text[start..].find('>') else {
            continue;
        };
        let content_start = start + open_end + 1;
        let close = format!("</{name}>");
        if let Some(end) = text[content_start..].find(&close) {
            return Some(text[content_start..content_start + end].trim());
        }
    }
    None
}

fn extract_nested_href(text: &str, names: &[&str]) -> Option<String> {
    for name in names {
        let open = format!("<{name}");
        let Some(start) = text.find(&open) else {
            continue;
        };
        let Some(open_end) = text[start..].find('>') else {
            continue;
        };
        let content_start = start + open_end + 1;
        let close = format!("</{name}>");
        let Some(end) = text[content_start..].find(&close) else {
            continue;
        };
        if let Some(href) = extract_xml_text(
            &text[content_start..content_start + end],
            &["D:href", "d:href", "href"],
        ) {
            return Some(href.to_string());
        }
    }
    None
}

fn extract_xml_blocks<'a>(text: &'a str, names: &[&str]) -> Vec<&'a str> {
    let mut blocks = Vec::new();
    let mut offset = 0;
    while offset < text.len() {
        let next = names
            .iter()
            .filter_map(|name| {
                text[offset..]
                    .find(&format!("<{name}"))
                    .map(|pos| (offset + pos, *name))
            })
            .min_by_key(|(pos, _)| *pos);
        let Some((start, name)) = next else { break };
        let Some(open_end) = text[start..].find('>') else {
            break;
        };
        let content_start = start + open_end + 1;
        let close = format!("</{name}>");
        let Some(end) = text[content_start..].find(&close) else {
            break;
        };
        blocks.push(&text[content_start..content_start + end]);
        offset = content_start + end + close.len();
    }
    blocks
}

fn unfold_ical_lines(ics: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in ics.replace("\r\n", "\n").replace('\r', "\n").lines() {
        if raw.starts_with(' ') || raw.starts_with('\t') {
            if let Some(last) = lines.last_mut() {
                last.push_str(raw.trim_start());
            }
            continue;
        }
        lines.push(raw.to_string());
    }
    lines
}

fn parse_ical_datetime(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = DateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ") {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S") {
        return Some(Utc.from_utc_datetime(&dt));
    }
    NaiveDate::parse_from_str(value, "%Y%m%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|dt| Utc.from_utc_datetime(&dt))
}

fn parse_ics(ics: &str) -> Result<Vec<ParsedEvent>, String> {
    let mut events = Vec::new();
    let mut current: Option<ParsedEvent> = None;
    for line in unfold_ical_lines(ics) {
        let line = line.trim();
        if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            current = Some(ParsedEvent::default());
            continue;
        }
        if line.eq_ignore_ascii_case("END:VEVENT") {
            if let Some(event) = current.take() {
                if event.dtstart.is_some() {
                    events.push(event);
                }
            }
            continue;
        }
        let Some(event) = current.as_mut() else {
            continue;
        };
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let field = name.split(';').next().unwrap_or(name);
        match field {
            "UID" => event.uid = value.trim().to_string(),
            "SUMMARY" => event.summary = value.trim().to_string(),
            "DESCRIPTION" => event.description = value.trim().to_string(),
            "LOCATION" => event.location = value.trim().to_string(),
            "DTSTART" => event.dtstart = parse_ical_datetime(value),
            "DTEND" => event.dtend = parse_ical_datetime(value),
            "RRULE" => event.rrule = Some(value.trim().to_string()),
            _ => {}
        }
    }
    if events.is_empty() {
        Err("no VEVENT entries found".to_string())
    } else {
        Ok(events)
    }
}

fn is_ical_datetime(value: &str) -> bool {
    let bytes = value.as_bytes();
    matches!(bytes.len(), 8 | 15 | 16)
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, byte)| match (bytes.len(), idx, byte) {
                (8, _, b'0'..=b'9') => true,
                (15 | 16, 8, b'T') => true,
                (15, 9..=14, b'0'..=b'9') => true,
                (16, 9..=14, b'0'..=b'9') => true,
                (16, 15, b'Z') => true,
                _ => false,
            })
}

fn format_ical_datetime(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() || is_ical_datetime(value) {
        return value.to_string();
    }
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc).format("%Y%m%dT%H%M%SZ").to_string())
        .unwrap_or_else(|_| value.to_string())
}

fn propfind_calendar_home(client: &Client, base_url: &str, auth: &str) -> Result<String, String> {
    let principal_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
  <D:prop><D:current-user-principal/></D:prop>
</D:propfind>"#;
    let resp = client
        .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), base_url)
        .header("Authorization", auth)
        .header("Depth", "0")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(principal_body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("PROPFIND status {}", resp.status()));
    }
    let text = resp.text().map_err(|e| e.to_string())?;
    let principal_href = extract_nested_href(
        &text,
        &["D:current-user-principal", "d:current-user-principal"],
    )
    .ok_or_else(|| "missing current-user-principal href".to_string())?;
    let principal_url = normalize_url(base_url, &principal_href);

    let calendar_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop><C:calendar-home-set/></D:prop>
</D:propfind>"#;
    let resp = client
        .request(
            reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
            &principal_url,
        )
        .header("Authorization", auth)
        .header("Depth", "0")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(calendar_body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "PROPFIND calendar-home-set status {}",
            resp.status()
        ));
    }
    let text = resp.text().map_err(|e| e.to_string())?;
    let calendar_home = extract_nested_href(&text, &["C:calendar-home-set", "c:calendar-home-set"])
        .ok_or_else(|| "missing calendar-home-set href".to_string())?;
    Ok(normalize_url(base_url, &calendar_home))
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
    for chunk in extract_xml_blocks(&text, &["D:response", "d:response"]) {
        let Some(ics) = extract_xml_text(chunk, &["C:calendar-data", "c:calendar-data"]) else {
            continue;
        };
        let href = extract_xml_text(chunk, &["D:href", "d:href"])
            .unwrap_or("")
            .to_string();
        let etag = extract_xml_text(chunk, &["D:getetag", "d:getetag"])
            .map(|s| s.trim().trim_matches('"').to_string());
        if let Ok(parsed) = parse_ics(ics) {
            for ev in parsed {
                let Some(dtstart) = ev.dtstart else { continue };
                events.push(json!({
                    "uid": ev.uid,
                    "href": href,
                    "etag": etag,
                    "summary": ev.summary,
                    "description": ev.description,
                    "location": ev.location,
                    "dtstart": dtstart.to_rfc3339(),
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
    let mut req = client.post(&url).json(&json!({
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
    let url = format!("{daemon_url}/api/calendar/external/outbox/pending?account_id={account_id}");
    let mut req = client.get(&url);
    if !token.is_empty() {
        req = req.header("X-Akasha-Calendar-Token", token);
    }
    let Ok(resp) = req.send() else { return };
    let Ok(body) = resp.json::<Value>() else {
        return;
    };
    let Some(items) = body.get("outbox").and_then(|v| v.as_array()) else {
        return;
    };
    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let op = item.get("operation").and_then(|v| v.as_str()).unwrap_or("");
        let payload = item
            .get("payload_json")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
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
                    let uid = p
                        .get("uid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("akasha-event");
                    let summary = p.get("summary").and_then(|v| v.as_str()).unwrap_or("Event");
                    let dtstart = format_ical_datetime(
                        p.get("dtstart").and_then(|v| v.as_str()).unwrap_or(""),
                    );
                    let dtend =
                        format_ical_datetime(p.get("dtend").and_then(|v| v.as_str()).unwrap_or(""));
                    let dtend_line = if dtend.is_empty() {
                        String::new()
                    } else {
                        format!("DTEND:{dtend}\r\n")
                    };
                    let ics = format!(
                        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nSUMMARY:{summary}\r\nDTSTART:{dtstart}\r\n{dtend_line}END:VEVENT\r\nEND:VCALENDAR\r\n"
                    );
                    let href = p
                        .get("href")
                        .and_then(|v| v.as_str())
                        .map(|h| normalize_url(cal_url, h))
                        .unwrap_or_else(|| normalize_url(cal_url, &format!("{uid}.ics")));
                    let mut req = client
                        .request(reqwest::Method::PUT, &href)
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
        let mut ack = client
            .post(&ack_url)
            .json(&json!({ "id": id, "error": err }));
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
            let _ = push_sync_to_daemon(
                client,
                daemon_url,
                token,
                account_id,
                vec![],
                None,
                Some(&e),
            );
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
            let _ = push_sync_to_daemon(
                client,
                daemon_url,
                token,
                account_id,
                vec![],
                None,
                Some(&e),
            );
        }
    }
}

fn main() {
    if !truthy("CALDAV_SIDECAR_ENABLED") && env::var("CALDAV_URL").is_err() {
        eprintln!(
            "akasha-caldav-sidecar: set CALDAV_URL + credentials or CALDAV_SIDECAR_ENABLED=1"
        );
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

    eprintln!("akasha-caldav-sidecar: syncing {caldav_url} every {interval}s → {daemon_url}");
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

#[cfg(test)]
mod tests {
    use super::{extract_xml_blocks, format_ical_datetime, normalize_url};

    #[test]
    fn normalize_url_resolves_absolute_paths() {
        assert_eq!(
            normalize_url("https://example.com/dav/root/", "/principals/user/"),
            "https://example.com/principals/user/"
        );
    }

    #[test]
    fn calendar_query_response_blocks_preserve_href_context() {
        let xml = r#"
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response><D:href>/a.ics</D:href><D:getetag>"1"</D:getetag><C:calendar-data>BEGIN:VCALENDAR
BEGIN:VEVENT
UID:a
DTSTART:20260608T120000Z
END:VEVENT
END:VCALENDAR</C:calendar-data></D:response>
</D:multistatus>
"#;
        let blocks = extract_xml_blocks(xml, &["D:response"]);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("/a.ics"));
        assert!(blocks[0].contains("calendar-data"));
    }

    #[test]
    fn format_ical_datetime_converts_rfc3339() {
        assert_eq!(
            format_ical_datetime("2026-06-08T12:00:00+02:00"),
            "20260608T100000Z"
        );
        assert_eq!(format_ical_datetime("20260608T100000Z"), "20260608T100000Z");
    }
}
