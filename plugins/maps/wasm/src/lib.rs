use serde::Serialize;
use serde_json::{json, Value};

const CITY_GAZETTEER: &[(&str, f64, f64, &[&str])] = &[
    (
        "chateaubernard",
        45.6663,
        -0.3341,
        &[
            "chateaubernard",
            "chateaubernad",
            "chateaubernard 16",
            "chateaubernard charente",
        ],
    ),
    (
        "meulan-en-yvelines",
        49.0079,
        1.9082,
        &["meulan", "meulan en yvelines", "meulan-en-yvelines"],
    ),
    (
        "roissy charles de gaulle",
        49.0097,
        2.5479,
        &[
            "roissy",
            "roissy charles de gaulle",
            "cdg",
            "gare de roissy charles de gaulle",
            "aeroport charles de gaulle",
        ],
    ),
    ("cognac", 45.6940, -0.3290, &["cognac"]),
    ("paris", 48.8566, 2.3522, &["paris", "paris france"]),
    ("lyon", 45.7640, 4.8357, &["lyon", "lyon france"]),
    (
        "marseille",
        43.2965,
        5.3698,
        &["marseille", "marseille france"],
    ),
    (
        "bordeaux",
        44.8378,
        -0.5792,
        &["bordeaux", "bordeaux france"],
    ),
    ("lille", 50.6292, 3.0573, &["lille", "lille france"]),
    (
        "toulouse",
        43.6047,
        1.4442,
        &["toulouse", "toulouse france"],
    ),
    ("nantes", 47.2184, -1.5536, &["nantes", "nantes france"]),
    (
        "strasbourg",
        48.5734,
        7.7521,
        &["strasbourg", "strasbourg france"],
    ),
    ("nice", 43.7102, 7.2620, &["nice", "nice france"]),
    (
        "rennes",
        48.1173,
        -1.6778,
        &["rennes", "rennes france"],
    ),
    (
        "montpellier",
        43.6108,
        3.8767,
        &["montpellier", "montpellier france"],
    ),
    (
        "grenoble",
        45.1885,
        5.7245,
        &["grenoble", "grenoble france"],
    ),
];

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
    resolved_from: Option<String>,
    resolved_to: Option<String>,
}

#[derive(Serialize)]
struct GeocodeResponse {
    ok: bool,
    view: &'static str,
    query: String,
    resolved_name: String,
    lat: f64,
    lon: f64,
    confidence: f64,
}

#[derive(Serialize)]
struct GeocodeBatchResponse {
    ok: bool,
    view: &'static str,
    results: Vec<Value>,
}

fn normalize_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let mapped = match ch {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => {
                'a'
            }
            'ç' | 'Ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
            'ñ' | 'Ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
            'ý' | 'ÿ' | 'Ý' => 'y',
            '-' | '_' | '/' | '\\' | ',' | ';' | ':' | '|' | '(' | ')' | '[' | ']' | '{'
            | '}' | '\'' | '"' | '`' => ' ',
            c => c.to_ascii_lowercase(),
        };
        out.push(mapped);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr = vec![0; b_chars.len() + 1];

    for (i, ca) in a_chars.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b_chars.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_chars.len()]
}

fn resolve_point_from_text(input: &str) -> Option<(Point, String)> {
    let q = normalize_text(input);
    if q.is_empty() {
        return None;
    }

    let mut best: Option<(usize, Point, String)> = None;

    for (city_name, lat, lon, aliases) in CITY_GAZETTEER.iter().copied() {
        for alias in aliases {
            let alias_n = normalize_text(alias);
            if alias_n == q {
                return Some((
                    Point { lat, lon },
                    city_name.to_string(),
                ));
            }

            let score = if q.contains(&alias_n) || alias_n.contains(&q) {
                q.len().abs_diff(alias_n.len())
            } else {
                levenshtein(&q, &alias_n)
            };

            match &best {
                Some((best_score, _, _)) if score >= *best_score => {}
                _ => {
                    best = Some((
                        score,
                        Point { lat, lon },
                        city_name.to_string(),
                    ));
                }
            }
        }
    }

    if let Some((score, point, name)) = best {
        if score <= 3 {
            return Some((point, name));
        }
    }

    None
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
            let lat = arr.first().and_then(Value::as_f64)?;
            let lon = arr.get(1).and_then(Value::as_f64)?;
            return Some(Point { lat, lon });
        }
    }
    if let Some(s) = v.as_str() {
        return resolve_point_from_text(s).map(|(p, _)| p);
    }
    None
}

fn parse_args_points(args: &[String]) -> Option<(Point, Point)> {
    if args.len() < 4 {
        return None;
    }
    let lat1 = args.first()?.parse::<f64>().ok()?;
    let lon1 = args.get(1)?.parse::<f64>().ok()?;
    let lat2 = args.get(2)?.parse::<f64>().ok()?;
    let lon2 = args.get(3)?.parse::<f64>().ok()?;
    Some((
        Point { lat: lat1, lon: lon1 },
        Point { lat: lat2, lon: lon2 },
    ))
}

fn parse_args_text_points(args: &[String]) -> Option<((Point, String), (Point, String))> {
    if args.is_empty() {
        return None;
    }

    // 1) Direct two-args mode: <from_city> <to_city>
    if args.len() >= 2 {
        if let (Some(from), Some(to)) = (
            resolve_point_from_text(args.first()?),
            resolve_point_from_text(args.get(1)?),
        ) {
            return Some((from, to));
        }
    }

    // 2) Free-text mode: join all args and try to split by common connectors.
    let joined = normalize_text(&args.join(" "));
    let separators = [" et ", " vers ", " to ", " -> ", " jusqu a ", " jusqu'au "];
    for sep in separators {
        if let Some(idx) = joined.find(sep) {
            let left = joined[..idx].trim();
            let right = joined[idx + sep.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                if let (Some(from), Some(to)) = (
                    resolve_point_from_text(left),
                    resolve_point_from_text(right),
                ) {
                    return Some((from, to));
                }
            }
        }
    }

    // 3) Fallback: detect two known city aliases in the full text.
    let mut matches: Vec<(usize, usize, Point, String)> = Vec::new();
    for (city_name, lat, lon, aliases) in CITY_GAZETTEER.iter().copied() {
        for alias in aliases {
            let alias_n = normalize_text(alias);
            if alias_n.len() < 3 {
                continue;
            }
            if let Some(start) = joined.find(&alias_n) {
                let end = start + alias_n.len();
                matches.push((start, end, Point { lat, lon }, city_name.to_string()));
            }
        }
    }

    if matches.is_empty() {
        return None;
    }

    // Keep longest match first when starts are equal.
    matches.sort_by(|a, b| {
        if a.0 == b.0 {
            (b.1 - b.0).cmp(&(a.1 - a.0))
        } else {
            a.0.cmp(&b.0)
        }
    });

    let first = matches.first().cloned()?;
    let second = matches
        .iter()
        .find(|m| m.3 != first.3 && m.0 >= first.1)
        .cloned()
        .or_else(|| matches.iter().find(|m| m.3 != first.3).cloned())?;

    Some(((first.2, first.3), (second.2, second.3)))
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

fn build_response(
    action: &str,
    mode: TravelMode,
    from: Point,
    to: Point,
    resolved_from: Option<String>,
    resolved_to: Option<String>,
) -> PluginResponse {
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
        resolved_from,
        resolved_to,
    }
}

fn build_geocode_response(query: &str, resolved_name: &str, point: Point, confidence: f64) -> GeocodeResponse {
    GeocodeResponse {
        ok: true,
        view: "table",
        query: query.to_string(),
        resolved_name: resolved_name.to_string(),
        lat: point.lat,
        lon: point.lon,
        confidence,
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
        if tool_name.contains("geocode") {
            action = "geocode".to_string();
        } else if tool_name.contains("distance") {
            action = "distance".to_string();
        } else {
            action = "route".to_string();
        }
    }
    
    // Debug: log input length to stderr (WASM safe)
    eprintln!("[maps-plugin] input_len={} action={} args_count={} first_arg={:?}", 
        input.len(), action, args.len(), args.first());

    if action.eq_ignore_ascii_case("geocode") {
        if let Some(locations) = payload.get("locations").and_then(Value::as_array) {
            let mut rows: Vec<Value> = Vec::new();
            for loc in locations {
                let query = loc.as_str().unwrap_or("");
                if query.trim().is_empty() {
                    continue;
                }
                if let Some((point, resolved_name)) = resolve_point_from_text(query) {
                    rows.push(json!({
                        "ok": true,
                        "query": query,
                        "resolved_name": resolved_name,
                        "lat": point.lat,
                        "lon": point.lon,
                        "confidence": 0.9
                    }));
                } else {
                    rows.push(json!({
                        "ok": false,
                        "query": query,
                        "error": "not_found"
                    }));
                }
            }

            return serde_json::to_string(&GeocodeBatchResponse {
                ok: rows.iter().any(|r| r.get("ok").and_then(Value::as_bool).unwrap_or(false)),
                view: "table",
                results: rows,
            })
            .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization\"}".to_string());
        }

        let query = payload
            .get("query")
            .and_then(Value::as_str)
            .or_else(|| payload.get("city").and_then(Value::as_str))
            .or_else(|| payload.get("from_text").and_then(Value::as_str))
            .or_else(|| args.first().map(|s| s.as_str()));

        let Some(query_str) = query else {
            return json!({
                "ok": false,
                "error": "invalid_input",
                "message": "Provide a city name using query/city/from_text or first arg.",
                "hint": "Example: {\"action\":\"geocode\",\"query\":\"Chateaubernard\"}"
            })
            .to_string();
        };

        let Some((point, resolved_name)) = resolve_point_from_text(query_str) else {
            return json!({
                "ok": false,
                "error": "not_found",
                "message": format!("No known city match for '{}'.", query_str),
                "hint": "Try a nearby major city name or add this location to the plugin gazetteer."
            })
            .to_string();
        };

        return serde_json::to_string(&build_geocode_response(query_str, &resolved_name, point, 0.9))
            .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization\"}".to_string());
    }

    let mode = TravelMode::from_value(
        payload
            .get("mode")
            .and_then(Value::as_str)
            .or_else(|| args.get(4).map(|s| s.as_str()))
            .or_else(|| args.get(2).map(|s| s.as_str())),
    );

    let mut resolved_from: Option<String> = None;
    let mut resolved_to: Option<String> = None;

    // Try extracting from explicit payload fields first
    let (from, to) = if let (Some(f), Some(t)) = (
        payload.get("from").and_then(point_from_value),
        payload.get("to").and_then(point_from_value),
    ) {
        eprintln!("[maps-plugin] Resolved from explicit from/to in payload");
        (f, t)
    } else if let Some(locations) = payload.get("locations").and_then(Value::as_array) {
        if locations.len() >= 2 {
            if let (Some(fs), Some(ts)) = (
                locations
                    .first()
                    .and_then(Value::as_str)
                    .and_then(resolve_point_from_text),
                locations
                    .get(1)
                    .and_then(Value::as_str)
                    .and_then(resolve_point_from_text),
            ) {
                resolved_from = Some(fs.1);
                resolved_to = Some(ts.1);
                (fs.0, ts.0)
            } else {
                return json!({
                    "ok": false,
                    "error": "invalid_locations",
                    "message": "Could not resolve locations[0]/locations[1] to known cities.",
                    "hint": "Try explicit from_text/to_text or extend plugin gazetteer aliases."
                })
                .to_string();
            }
        } else {
            return json!({
                "ok": false,
                "error": "invalid_locations",
                "message": "locations must contain at least two city names."
            })
            .to_string();
        }
    } else if let (Some(fs), Some(ts)) = (
        payload
            .get("from_text")
            .and_then(Value::as_str)
            .and_then(resolve_point_from_text),
        payload
            .get("to_text")
            .and_then(Value::as_str)
            .and_then(resolve_point_from_text),
    ) {
        eprintln!("[maps-plugin] Resolved from explicit from_text/to_text in payload");
        resolved_from = Some(fs.1);
        resolved_to = Some(ts.1);
        (fs.0, ts.0)
    } else if let Some((f, t)) = parse_args_points(&args) {
        eprintln!("[maps-plugin] Resolved from numeric args");
        (f, t)
    } else if let Some((fs, ts)) = parse_args_text_points(&args) {
        eprintln!("[maps-plugin] Resolved from text args: {} -> {}", fs.1, ts.1);
        resolved_from = Some(fs.1);
        resolved_to = Some(ts.1);
        (fs.0, ts.0)
    } else {
        eprintln!("[maps-plugin] ERROR: Could not resolve any points from args: {:?}", args);
        return json!({
            "ok": false,
            "error": "invalid_input",
            "message": "Provide from/to as coordinates ({lat,lon}) OR city names via from_text/to_text OR args: <from_city> <to_city> [mode].",
            "hint": "Example: {\"action\":\"route\",\"from_text\":\"Chateaubernard\",\"to_text\":\"Meulan-en-Yvelines\",\"mode\":\"car\"}"
        })
        .to_string();
    };

    serde_json::to_string(&build_response(
        &action,
        mode,
        from,
        to,
        resolved_from,
        resolved_to,
    ))
    .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization\"}".to_string())
}

/// Shared memory buffer for input/output (WASM linear memory ABI).
/// The host uses buffer_ptr() to find where to write input and read output.
static mut BUFFER: [u8; 65536] = [0u8; 65536];

/// Export the address of the I/O buffer so the host can write input here
/// and read output from here, instead of using the hardcoded offset 0
/// (which points into the shadow-stack area, not the BSS data segment).
#[no_mangle]
pub extern "C" fn buffer_ptr() -> i32 {
    unsafe { BUFFER.as_ptr() as i32 }
}

#[no_mangle]
pub extern "C" fn run(input_len: i32) -> i32 {
    if input_len < 0 {
        return 0;
    }
    let len = input_len as usize;
    const BUFFER_SIZE: usize = 65536;
    if len > BUFFER_SIZE {
        return 0;  // Input too large
    }

    let input = unsafe {
        let bytes = std::slice::from_raw_parts(BUFFER.as_ptr(), len);
        std::str::from_utf8(bytes).unwrap_or("{}")
    };

    let output = handle(input);
    let out_bytes = output.as_bytes();

    if out_bytes.len() > BUFFER_SIZE {
        return 0;  // Output too large
    }

    unsafe {
        std::ptr::copy_nonoverlapping(out_bytes.as_ptr(), BUFFER.as_mut_ptr(), out_bytes.len());
    }

    out_bytes.len() as i32
}
