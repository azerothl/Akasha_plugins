//! Matrix channel plugin stub — registers channel lifecycle hooks for the host.
//! Matrix sync/send bridge is intentionally deferred for Wave 6 hardening.

use serde::Serialize;
use serde_json::{json, Value};

const REGISTERED_HOOKS: &[&str] = &[
    "on_channel_connect",
    "on_channel_message",
    "on_channel_disconnect",
];

#[derive(Serialize)]
struct HookResponse {
    ok: bool,
    plugin_id: &'static str,
    kind: &'static str,
    registered_hooks: &'static [&'static str],
    message: String,
}

fn handle(input: &str) -> String {
    let value = serde_json::from_str::<Value>(input).unwrap_or_else(|_| json!({}));
    let action = value
        .get("action")
        .or_else(|| value.get("hook"))
        .and_then(Value::as_str)
        .unwrap_or("register_channel_hooks");

    let message = match action {
        "register_channel_hooks" | "on_channel_connect" => {
            "Matrix channel stub: hooks registered. Configure MATRIX_HOMESERVER_URL and vault matrix_access_token before full sync.".to_string()
        }
        "on_channel_message" => "Matrix channel: message hook acknowledged. Run akasha-matrix-sidecar (MATRIX_HOMESERVER_URL + MATRIX_ACCESS_TOKEN) to sync and POST /api/message.".to_string(),
        "on_channel_disconnect" => {
            "Matrix channel stub: disconnect hook acknowledged.".to_string()
        }
        other => format!("Matrix channel stub: unknown action '{other}'"),
    };

    let out = HookResponse {
        ok: true,
        plugin_id: "matrix-channel",
        kind: "channel",
        registered_hooks: REGISTERED_HOOKS,
        message,
    };

    serde_json::to_string(&out)
        .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization\"}".to_string())
}

#[no_mangle]
pub extern "C" fn buffer_ptr() -> i32 {
    std::ptr::addr_of_mut!(BUFFER) as *mut u8 as i32
}

#[no_mangle]
pub extern "C" fn buffer_len() -> i32 {
    BUFFER_SIZE as i32
}

const BUFFER_SIZE: usize = 65536;
static mut BUFFER: [u8; BUFFER_SIZE] = [0u8; BUFFER_SIZE];

#[no_mangle]
pub extern "C" fn run(input_len: i32) -> i32 {
    if input_len < 0 {
        return 0;
    }
    let len = input_len as usize;
    if len > BUFFER_SIZE {
        return 0;
    }
    let input = unsafe {
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
