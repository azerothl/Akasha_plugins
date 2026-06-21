mod host_http;

use host_http::{decode_fetch_response, http_fetch_json};
use serde_json::{json, Value};

const BUFFER_SIZE: usize = 65536;
static mut BUFFER: [u8; BUFFER_SIZE] = [0u8; BUFFER_SIZE];

fn extract_payload(input: &str) -> Value {
    let v: Value = serde_json::from_str(input).unwrap_or(json!({}));
    if let Some(args) = v.get("args").and_then(|a| a.as_array()) {
        if let Some(first) = args.first().and_then(|x| x.as_str()) {
            if let Ok(inner) = serde_json::from_str::<Value>(first) {
                let mut merged = v.clone();
                if let Some(obj) = inner.as_object() {
                    for (k, val) in obj {
                        merged[k] = val.clone();
                    }
                }
                return merged;
            }
        }
    }
    v
}

fn resolve_action(payload: &Value) -> &str {
    if let Some(a) = payload.get("action").and_then(|x| x.as_str()) {
        if !a.is_empty() {
            return a;
        }
    }
    let tool = payload
        .get("tool")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if tool.contains("list") {
        "list_entities"
    } else if tool.contains("call") || tool.contains("service") {
        "call_service"
    } else if tool.contains("script") {
        "run_script"
    } else if tool.contains("state") || tool.contains("get") {
        "get_state"
    } else {
        "get_state"
    }
}

fn creds(payload: &Value) -> Result<(String, String), String> {
    let base = payload
        .get("ha_base_url")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "ha_base_url missing (enable Home Assistant connector in Akasha settings)".to_string())?;
    let token = payload
        .get("ha_access_token")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "ha_access_token missing (set token in Akasha vault via connectors settings)".to_string())?;
    Ok((base, token))
}

fn auth_headers(token: &str) -> Value {
    json!({
        "Authorization": format!("Bearer {}", token),
        "Content-Type": "application/json"
    })
}

fn ha_fetch(base: &str, token: &str, method: &str, path: &str, body: Option<&str>) -> Result<Value, String> {
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let mut req = json!({
        "url": url,
        "method": method,
        "headers": auth_headers(token),
    });
    if let Some(b) = body {
        req["body"] = json!(b);
    }
    let raw = http_fetch_json(&req)?;
    let bytes = decode_fetch_response(&raw)?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("invalid JSON from HA: {e}"))
}

fn sensitive_domain(domain: &str) -> bool {
    matches!(domain, "lock" | "alarm_control_panel" | "cover" | "valve")
}

fn require_confirm(payload: &Value, domain: &str) -> Result<(), String> {
    if sensitive_domain(domain)
        && payload.get("confirm").and_then(|x| x.as_bool()) != Some(true)
    {
        return Err(format!(
            "action on domain '{domain}' requires confirm:true (human approval)"
        ));
    }
    Ok(())
}

fn handle_get_state(payload: &Value) -> String {
    let (base, token) = match creds(payload) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "error": e, "view": "summary"}).to_string(),
    };
    let entity_id = payload
        .get("entity_id")
        .and_then(|x| x.as_str())
        .or_else(|| payload.get("args").and_then(|a| a.get(0)).and_then(|x| x.as_str()))
        .unwrap_or("")
        .trim();
    if entity_id.is_empty() {
        return json!({"ok": false, "error": "entity_id required", "view": "summary"}).to_string();
    }
    match ha_fetch(&base, &token, "GET", &format!("/api/states/{entity_id}"), None) {
        Ok(state) => {
            let st = state.get("state").and_then(|x| x.as_str()).unwrap_or("?");
            let name = state
                .pointer("/attributes/friendly_name")
                .and_then(|x| x.as_str())
                .unwrap_or(entity_id);
            json!({
                "ok": true,
                "view": "table",
                "summary": format!("{name} ({entity_id}) = {st}"),
                "entity_id": entity_id,
                "state": state,
            })
            .to_string()
        }
        Err(e) => json!({"ok": false, "error": e, "view": "summary"}).to_string(),
    }
}

fn handle_list_entities(payload: &Value) -> String {
    let (base, token) = match creds(payload) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "error": e, "view": "summary"}).to_string(),
    };
    let domain_filter = payload
        .get("domain")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    match ha_fetch(&base, &token, "GET", "/api/states", None) {
        Ok(states) => {
            let arr = states.as_array().cloned().unwrap_or_default();
            let mut rows = Vec::new();
            for s in arr {
                let eid = s.get("entity_id").and_then(|x| x.as_str()).unwrap_or("");
                if let Some(ref dom) = domain_filter {
                    if !eid.starts_with(&format!("{dom}.")) {
                        continue;
                    }
                }
                rows.push(json!({
                    "entity_id": eid,
                    "state": s.get("state"),
                    "friendly_name": s.pointer("/attributes/friendly_name"),
                }));
            }
            let summary = format!("{} entité(s) Home Assistant", rows.len());
            json!({
                "ok": true,
                "view": "table",
                "summary": summary,
                "entities": rows,
            })
            .to_string()
        }
        Err(e) => json!({"ok": false, "error": e, "view": "summary"}).to_string(),
    }
}

fn handle_call_service(payload: &Value) -> String {
    let (base, token) = match creds(payload) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "error": e, "view": "summary"}).to_string(),
    };
    let args = payload.get("args").and_then(|a| a.as_array());
    let domain = payload
        .get("domain")
        .and_then(|x| x.as_str())
        .or_else(|| args.and_then(|a| a.get(0)).and_then(|x| x.as_str()))
        .unwrap_or("")
        .trim();
    let service = payload
        .get("service")
        .and_then(|x| x.as_str())
        .or_else(|| args.and_then(|a| a.get(1)).and_then(|x| x.as_str()))
        .unwrap_or("")
        .trim();
    if domain.is_empty() || service.is_empty() {
        return json!({"ok": false, "error": "domain and service required", "view": "summary"}).to_string();
    }
    if let Err(e) = require_confirm(payload, domain) {
        return json!({"ok": false, "error": e, "view": "summary"}).to_string();
    }
    let entity_id = payload
        .get("entity_id")
        .and_then(|x| x.as_str())
        .or_else(|| args.and_then(|a| a.get(2)).and_then(|x| x.as_str()));
    let mut data = payload.get("data").cloned().unwrap_or(json!({}));
    if data.is_null() {
        data = json!({});
    }
    if let Some(eid) = entity_id {
        if !eid.is_empty() {
            if let Some(obj) = data.as_object_mut() {
                obj.entry("entity_id".to_string())
                    .or_insert(json!(eid));
            }
        }
    }
    let body = data.to_string();
    let path = format!("/api/services/{domain}/{service}");
    match ha_fetch(&base, &token, "POST", &path, Some(&body)) {
        Ok(result) => json!({
            "ok": true,
            "view": "summary",
            "summary": format!("Service {domain}.{service} invoked"),
            "service_result": result,
        })
        .to_string(),
        Err(e) => json!({"ok": false, "error": e, "view": "summary"}).to_string(),
    }
}

fn handle_run_script(payload: &Value) -> String {
    let mut p = payload.clone();
    if p.get("domain").is_none() {
        p["domain"] = json!("script");
    }
    if p.get("service").is_none() {
        p["service"] = json!("turn_on");
    }
    if p.get("entity_id").is_none() {
        if let Some(script) = payload.get("script_id").and_then(|x| x.as_str()) {
            let eid = if script.contains('.') {
                script.to_string()
            } else {
                format!("script.{script}")
            };
            p["entity_id"] = json!(eid);
        }
    }
    handle_call_service(&p)
}

fn handle(input: &str) -> String {
    let payload = extract_payload(input);
    match resolve_action(&payload) {
        "list_entities" | "list" => handle_list_entities(&payload),
        "call_service" | "call" => handle_call_service(&payload),
        "run_script" | "script" => handle_run_script(&payload),
        _ => handle_get_state(&payload),
    }
}

#[no_mangle]
pub extern "C" fn buffer_ptr() -> i32 {
    std::ptr::addr_of_mut!(BUFFER) as *mut u8 as i32
}

#[no_mangle]
pub extern "C" fn buffer_len() -> i32 {
    BUFFER_SIZE as i32
}

#[no_mangle]
pub extern "C" fn run(input_len: i32) -> i32 {
    if input_len < 0 {
        return 0;
    }
    let len = input_len as usize;
    if len > BUFFER_SIZE {
        return 0;
    }
    let input: String = unsafe {
        let bytes = std::slice::from_raw_parts(std::ptr::addr_of!(BUFFER) as *const u8, len);
        std::str::from_utf8(bytes).unwrap_or("{}").to_owned()
    };
    let output = handle(&input);
    let out_bytes = output.as_bytes();
    if out_bytes.len() > BUFFER_SIZE {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(
            out_bytes.as_ptr(),
            std::ptr::addr_of_mut!(BUFFER) as *mut u8,
            out_bytes.len(),
        );
    }
    out_bytes.len() as i32
}
