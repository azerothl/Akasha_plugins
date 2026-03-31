//! Host import `akasha::http_fetch` — only available when the Akasha plugin host grants network permission.

use base64::Engine;
use serde_json::Value;

#[link(wasm_import_module = "akasha")]
unsafe extern "C" {
    fn http_fetch(req_ptr: i32, req_len: i32, out_ptr: i32, out_cap: i32) -> i32;
}

static mut HTTP_REQ_BUF: [u8; 16384] = [0u8; 16384];
static mut HTTP_OUT_BUF: [u8; 1048576] = [0u8; 1048576];

pub fn http_fetch_json(req: &Value) -> Result<String, String> {
    let s = req.to_string();
    let b = s.as_bytes();
    unsafe {
        if b.len() > HTTP_REQ_BUF.len() {
            return Err("request JSON too large".into());
        }
        HTTP_REQ_BUF[..b.len()].copy_from_slice(b);
        let n = http_fetch(
            HTTP_REQ_BUF.as_ptr() as i32,
            b.len() as i32,
            HTTP_OUT_BUF.as_ptr() as i32,
            HTTP_OUT_BUF.len() as i32,
        );
        if n <= 0 {
            return Err(format!("http_fetch returned {n}"));
        }
        let slice = &HTTP_OUT_BUF[..n as usize];
        core::str::from_utf8(slice)
            .map(|s| s.to_string())
            .map_err(|_| "host response is not UTF-8".into())
    }
}

pub fn decode_fetch_response(txt: &str) -> Result<Vec<u8>, String> {
    let v: Value = serde_json::from_str(txt).map_err(|e| e.to_string())?;
    if v.get("ok").and_then(|x| x.as_bool()) == Some(false) {
        let err = v
            .get("error")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown error");
        return Err(err.to_string());
    }
    let b64 = v
        .get("body_b64")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "missing body_b64".to_string())?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| e.to_string())
}
