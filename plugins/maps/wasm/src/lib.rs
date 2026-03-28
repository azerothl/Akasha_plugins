use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy)]
struct Point {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Clone, Copy)]
enum TravelMode {
    Car,
    Train,
    Walking,
}

impl TravelMode {
    fn from_value(v: Option<&str>) -> Self {
        match v.unwrap_or("car").to_ascii_lowercase().as_str() {
            "train" | "rail" => Self::Train,
            "walk" | "walking" | "foot" => Self::Walking,
            _ => Self::Car,
        }
    }

    fn average_speed_kmh(self) -> f64 {
        match self {
            Self::Car => 80.0,
            Self::Train => 140.0,
            Self::Walking => 5.0,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Car => "car",
            Self::Train => "train",
            Self::Walking => "walking",
        }
    }
}

#[derive(Serialize)]
struct PluginResponse {
    ok: bool,
    view: &'static str,
    summary: String,
    mode: String,
    distance_m: f64,
    duration_s: f64,
    geometry: Value,
    steps: Vec<Value>,
}

fn haversine_m(from: Point, to: Point) -> f64 {
    let r = 6_371_000.0_f64;
    let d_lat = (to.lat - from.lat).to_radians();
    let d_lon = (to.lon - from.lon).to_radians();
    let lat1 = from.lat.to_radians();
    let lat2 = to.lat.to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

fn point_from_value(v: &Value) -> Option<Point> {
    if let Some(obj) = v.as_object() {
        let lat = obj.get("lat").and_then(Value::as_f64)?;
        let lon = obj
            .get("lon")
            .and_then(Value::as_f64)
            .or_else(|| obj.get("lng").and_then(Value::as_f64))?;
        return Some(Point { lat, lon });
    }
    if let Some(arr) = v.as_array() {
        if arr.len() >= 2 {
            let lat = arr.get(0).and_then(Value::as_f64)?;
            let lon = arr.get(1).and_then(Value::as_f64)?;
            return Some(Point { lat, lon });
        }
    }
    None
}

fn parse_args_points(args: &[String]) -> Option<(Point, Point)> {
    if args.len() < 4 {
        return None;
    }
    let lat1 = args.get(0)?.parse::<f64>().ok()?;
    let lon1 = args.get(1)?.parse::<f64>().ok()?;
    let lat2 = args.get(2)?.parse::<f64>().ok()?;
    let lon2 = args.get(3)?.parse::<f64>().ok()?;
    Some((
        Point { lat: lat1, lon: lon1 },
        Point { lat: lat2, lon: lon2 },
    ))
}

fn extract_payload(input: &Value) -> (String, Value, Vec<String>) {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let mut payload = input.clone();
    let args: Vec<String> = input
        .get("args")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if let Some(first) = args.first() {
        if let Ok(v) = serde_json::from_str::<Value>(first) {
            payload = v;
        }
    }

    (action, payload, args)
}

fn build_response(action: &str, mode: TravelMode, from: Point, to: Point) -> PluginResponse {
    let distance_m = haversine_m(from, to);
    let duration_s = (distance_m / 1000.0) / mode.average_speed_kmh() * 3600.0;
    let is_distance_only = action.contains("distance");

    let summary = if is_distance_only {
        format!(
            "Estimated distance: {:.1} km ({})",
            distance_m / 1000.0,
            mode.as_str()
        )
    } else {
        format!(
            "Estimated route: {:.1} km, {:.0} min ({})",
            distance_m / 1000.0,
            duration_s / 60.0,
            mode.as_str()
        )
    };

    PluginResponse {
        ok: true,
        view: "map",
        summary,
        mode: mode.as_str().to_string(),
        distance_m,
        duration_s,
        geometry: json!({
            "type": "LineString",
            "coordinates": [[from.lon, from.lat], [to.lon, to.lat]]
        }),
        steps: vec![json!({
            "instruction": "Go to destination",
            "distance_m": distance_m
        })],
    }
}

fn handle(input: &str) -> String {
    let value = serde_json::from_str::<Value>(input).unwrap_or_else(|_| json!({}));
    let tool_name = value
        .get("tool")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();

    let (mut action, payload, args) = extract_payload(&value);
    if action.is_empty() {
        if tool_name.contains("distance") {
            action = "distance".to_string();
        } else {
            action = "route".to_string();
        }
    }

    let mode = TravelMode::from_value(
        payload
            .get("mode")
            .and_then(Value::as_str)
            .or_else(|| args.get(4).map(|s| s.as_str())),
    );

    let (from, to) = if let (Some(f), Some(t)) = (
        payload.get("from").and_then(point_from_value),
        payload.get("to").and_then(point_from_value),
    ) {
        (f, t)
    } else if let Some((f, t)) = parse_args_points(&args) {
        (f, t)
    } else {
        return json!({
            "ok": false,
            "error": "invalid_input",
            "message": "Provide from/to points as objects ({lat,lon}) or args: <from_lat> <from_lon> <to_lat> <to_lon> [mode]"
        })
        .to_string();
    };

    serde_json::to_string(&build_response(&action, mode, from, to))
        .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization\"}".to_string())
}

#[no_mangle]
#[allow(invalid_null_arguments)]
pub extern "C" fn run(input_len: i32) -> i32 {
    if input_len < 0 {
        return 0;
    }
    let len = input_len as usize;
    let input = unsafe {
        let ptr = 0 as *const u8;
        let bytes = std::slice::from_raw_parts(ptr, len);
        std::str::from_utf8(bytes).unwrap_or("{}")
    };

    let output = handle(input);
    let out_bytes = output.as_bytes();

    unsafe {
        std::ptr::copy_nonoverlapping(out_bytes.as_ptr(), 0 as *mut u8, out_bytes.len());
    }

    out_bytes.len() as i32
}
