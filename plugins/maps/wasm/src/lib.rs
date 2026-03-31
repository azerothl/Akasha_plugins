mod host_http;

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

#[derive(Debug, Clone, Copy, PartialEq)]
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
struct RouteOption {
    id: &'static str,
    label: String,
    mode: String,
    distance_m: f64,
    duration_s: f64,
    geometry: Value,
    steps: Vec<Value>,
}

#[derive(Serialize)]
struct PluginResponse {
    ok: bool,
    view: &'static str,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    /// `road_network` (OSRM) or `great_circle_estimate` (gazetteer / fallback).
    geometry_kind: String,
    mode: String,
    distance_m: f64,
    duration_s: f64,
    geometry: Value,
    steps: Vec<Value>,
    routes: Vec<RouteOption>,
    bbox: Value,
    osm_embed_url: String,
    osm_browse_url: String,
    map_attribution: String,
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

fn polyline_length_m(points: &[Point]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    points
        .windows(2)
        .map(|w| haversine_m(w[0], w[1]))
        .sum()
}

/// Linear interpolation in lat/lon (rarely needed — tends to read as a meaningless diagonal in UI).
#[allow(dead_code)]
fn interpolate_line(from: Point, to: Point, n_segments: usize) -> Vec<Point> {
    let n = n_segments.max(1);
    let mut out = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        out.push(Point {
            lat: from.lat + (to.lat - from.lat) * t,
            lon: from.lon + (to.lon - from.lon) * t,
        });
    }
    out
}

/// Densify the geographic shortest path on the sphere so a lon/lat schematic curves like a map trajectory.
fn interpolate_great_circle(from: Point, to: Point, n_segments: usize) -> Vec<Point> {
    let n = n_segments.max(1);
    let lat1 = from.lat.to_radians();
    let lon1 = from.lon.to_radians();
    let lat2 = to.lat.to_radians();
    let lon2 = to.lon.to_radians();

    let sin_lat1 = lat1.sin();
    let cos_lat1 = lat1.cos();
    let sin_lat2 = lat2.sin();
    let cos_lat2 = lat2.cos();
    let dlon = lon2 - lon1;

    let cos_d = (sin_lat1 * sin_lat2 + cos_lat1 * cos_lat2 * dlon.cos()).clamp(-1.0, 1.0);
    let d = cos_d.acos();

    if !d.is_finite() || d < 1e-9 {
        return vec![from, to];
    }

    let sin_d = d.sin();
    if sin_d.abs() < 1e-10 {
        return vec![from, to];
    }

    let mut out = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let a = ((1.0 - t) * d).sin() / sin_d;
        let b = (t * d).sin() / sin_d;
        let x = a * cos_lat1 * lon1.cos() + b * cos_lat2 * lon2.cos();
        let y = a * cos_lat1 * lon1.sin() + b * cos_lat2 * lon2.sin();
        let z = a * sin_lat1 + b * sin_lat2;
        let lat = z.atan2((x * x + y * y).sqrt()).to_degrees();
        let lon = y.atan2(x).to_degrees();
        out.push(Point { lat, lon });
    }
    out
}

/// Second car option: bent path via a lateral offset at the midpoint (illustrative, not OSM-routed).
fn detour_polyline(from: Point, to: Point, n_seg: usize, offset_deg: f64) -> Vec<Point> {
    let n = n_seg.max(4);
    let mid_lat = (from.lat + to.lat) / 2.0;
    let mid_lon = (from.lon + to.lon) / 2.0;
    let dlat = to.lat - from.lat;
    let dlon = to.lon - from.lon;
    let len = (dlat * dlat + dlon * dlon).sqrt().max(1e-9);
    let ox = (-dlon / len) * offset_deg;
    let oy = (dlat / len) * offset_deg;
    let via = Point {
        lat: mid_lat + oy,
        lon: mid_lon + ox,
    };
    let half = n / 2;
    let mut a = interpolate_great_circle(from, via, half.max(1));
    a.pop();
    let mut b = interpolate_great_circle(via, to, (n - half).max(1));
    a.append(&mut b);
    a
}

fn linestring_geojson(points_v: &[Point]) -> Value {
    let coords: Vec<Vec<f64>> = points_v
        .iter()
        .map(|p| vec![p.lon, p.lat])
        .collect();
    json!({
        "type": "LineString",
        "coordinates": coords
    })
}

fn bbox_for_points(points: &[Point], pad_deg: f64) -> Value {
    if points.is_empty() {
        return json!({});
    }
    let mut min_lat = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    let mut min_lon = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;
    for p in points {
        min_lat = min_lat.min(p.lat);
        max_lat = max_lat.max(p.lat);
        min_lon = min_lon.min(p.lon);
        max_lon = max_lon.max(p.lon);
    }
    json!({
        "min_lat": min_lat - pad_deg,
        "max_lat": max_lat + pad_deg,
        "min_lon": min_lon - pad_deg,
        "max_lon": max_lon + pad_deg
    })
}

fn osm_embed_from_bbox(bbox: &Value) -> String {
    let Some(obj) = bbox.as_object() else {
        return String::new();
    };
    let gl = |k: &str| obj.get(k).and_then(Value::as_f64);
    match (gl("min_lon"), gl("min_lat"), gl("max_lon"), gl("max_lat")) {
        (Some(a), Some(b), Some(c), Some(d)) if c > a && d > b => format!(
            "https://www.openstreetmap.org/export/embed.html?bbox={},{},{},{}&layer=mapnik",
            a, b, c, d
        ),
        _ => String::new(),
    }
}

fn osm_browse_url_from_bbox(bbox: &Value) -> String {
    let Some(obj) = bbox.as_object() else {
        return "https://www.openstreetmap.org".into();
    };
    let gl = |k: &str| obj.get(k).and_then(Value::as_f64);
    match (gl("min_lon"), gl("min_lat"), gl("max_lon"), gl("max_lat")) {
        (Some(a), Some(b), Some(c), Some(d)) if c > a && d > b => {
            let lat = (b + d) / 2.0;
            let lon = (a + c) / 2.0;
            let span = (d - b).max(c - a);
            let z = if span > 8.0 {
                5
            } else if span > 4.0 {
                6
            } else if span > 1.5 {
                7
            } else if span > 0.6 {
                8
            } else if span > 0.25 {
                9
            } else {
                10
            };
            format!("https://www.openstreetmap.org/#map={}/{}/{}", z, lat, lon)
        }
        _ => "https://www.openstreetmap.org".into(),
    }
}

fn ordinal_labels_for_polyline(points: &[Point], from_l: &str, to_l: &str) -> Vec<Value> {
    if points.len() < 2 {
        return vec![json!({
            "instruction": format!("Point unique — {}", to_l),
            "distance_m": 0
        })];
    }
    let mut steps = Vec::new();
    for w in points.windows(2) {
        let d = haversine_m(w[0], w[1]);
        let instr = if steps.is_empty() {
            format!(
                "Départ : {} — orientation générale vers {} (~{:.1} km)",
                from_l,
                to_l,
                d / 1000.0
            )
        } else if w[1] == *points.last().unwrap() {
            format!(
                "Arrivée : approche de {} (~{:.1} km)",
                to_l,
                d / 1000.0
            )
        } else {
            format!(
                "Poursuivre sur l’itinéraire estimé (~{:.1} km) — axes principaux",
                d / 1000.0
            )
        };
        steps.push(json!({"instruction": instr, "distance_m": d}));
    }
    steps
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

fn osrm_route_steps(route: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    let Some(legs) = route.get("legs").and_then(|x| x.as_array()) else {
        return out;
    };
    for leg in legs {
        let Some(steps) = leg.get("steps").and_then(|x| x.as_array()) else {
            continue;
        };
        for s in steps {
            let dist = s.get("distance").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let name = s.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let instr = if !name.is_empty() {
                format!("{} — {:.2} km", name, dist / 1000.0)
            } else {
                let typ = s
                    .get("maneuver")
                    .and_then(|m| m.get("type"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("segment");
                format!("{} — {:.2} km", typ, dist / 1000.0)
            };
            out.push(json!({"instruction": instr, "distance_m": dist}));
        }
    }
    out
}

/// Returns road geometry from OSRM public demo (rate-limited; configure host allowlist).
fn try_osrm_route(
    from: Point,
    to: Point,
    profile: &str,
) -> Option<(Vec<Point>, f64, f64, Vec<Value>)> {
    let url = format!(
        "https://router.project-osrm.org/route/v1/{}/{:.6},{:.6};{:.6},{:.6}?overview=full&geometries=geojson&steps=true",
        profile, from.lon, from.lat, to.lon, to.lat
    );
    let req = json!({"url": url, "method": "GET"});
    let txt = host_http::http_fetch_json(&req).ok()?;
    let body = host_http::decode_fetch_response(&txt).ok()?;
    let j: Value = serde_json::from_slice(&body).ok()?;
    let route = j.get("routes")?.as_array()?.first()?;
    let coords = route.get("geometry")?.get("coordinates")?.as_array()?;
    let mut pts = Vec::new();
    for c in coords {
        let arr = c.as_array()?;
        let lon = arr.get(0)?.as_f64()?;
        let lat = arr.get(1)?.as_f64()?;
        pts.push(Point { lat, lon });
    }
    if pts.len() < 2 {
        return None;
    }
    let distance_m = route.get("distance")?.as_f64()?;
    let duration_s = route.get("duration")?.as_f64()?;
    let steps = osrm_route_steps(route);
    Some((pts, distance_m, duration_s, steps))
}

#[allow(clippy::too_many_arguments)]
fn build_response_osrm(
    mode: TravelMode,
    from: Point,
    to: Point,
    from_l: &str,
    to_l: &str,
    resolved_from: Option<String>,
    resolved_to: Option<String>,
    chord_m: f64,
    road_pts: Vec<Point>,
    road_dist: f64,
    road_dur: f64,
    road_steps: Vec<Value>,
) -> PluginResponse {
    let chord_km = chord_m / 1000.0;
    let offset_deg = (chord_km / 900.0).clamp(0.05, 0.35);
    let n_seg = 24_usize;
    let alt_pts = detour_polyline(from, to, n_seg, offset_deg);
    let alt_dist = polyline_length_m(&alt_pts);
    let alt_duration = (alt_dist / 1000.0) / mode.average_speed_kmh() * 3600.0;

    let train_mode = TravelMode::Train;
    let train_duration_s = (chord_m / 1000.0) / train_mode.average_speed_kmh() * 3600.0;

    let primary_gc = interpolate_great_circle(from, to, n_seg);

    let primary_label = match mode {
        TravelMode::Walking => format!("À pied — route (~{:.0} min)", road_dur / 60.0),
        _ => format!("Voiture — route (~{:.0} min)", road_dur / 60.0),
    };

    let summary = format!(
        "Itinéraire routier (OSRM, données OpenStreetMap) : {:.1} km, ~{:.0} min. Une variante « corridor » et une option train restent indicatives.",
        road_dist / 1000.0,
        road_dur / 60.0
    );

    let detail = Some(
        "Tracé principal calculé par le moteur OSRM (Project-OSRM). Soumis aux limites du service public ; pour la production, hébergez votre propre instance.".to_string(),
    );

    let routes = vec![
        RouteOption {
            id: "primary_road",
            label: primary_label,
            mode: mode.as_str().to_string(),
            distance_m: road_dist,
            duration_s: road_dur,
            geometry: linestring_geojson(&road_pts),
            steps: if road_steps.is_empty() {
                ordinal_labels_for_polyline(&road_pts, from_l, to_l)
            } else {
                road_steps
            },
        },
        RouteOption {
            id: "alt_corridor",
            label: format!(
                "Variante corridor (estimation, +{:.1} km vs route)",
                (alt_dist - road_dist).max(0.0) / 1000.0
            ),
            mode: mode.as_str().to_string(),
            distance_m: alt_dist,
            duration_s: alt_duration,
            geometry: linestring_geojson(&alt_pts),
            steps: ordinal_labels_for_polyline(&alt_pts, from_l, to_l),
        },
        RouteOption {
            id: "train_indicator",
            label: format!(
                "Train — temps indicatif (~{:.0} min, tracé ≠ voies réelles)",
                train_duration_s / 60.0
            ),
            mode: train_mode.as_str().to_string(),
            distance_m: chord_m,
            duration_s: train_duration_s,
            geometry: linestring_geojson(&primary_gc),
            steps: vec![json!({
                "instruction": format!(
                    "Temps de parcours train approximatif entre {} et {} — vérifier les gares.",
                    from_l, to_l
                ),
                "distance_m": chord_m
            })],
        },
    ];

    let mut all_pts = road_pts.clone();
    all_pts.extend(alt_pts.iter().copied());

    let bbox = bbox_for_points(&all_pts, 0.08);
    let osm_embed_url = osm_embed_from_bbox(&bbox);
    let osm_browse_url = osm_browse_url_from_bbox(&bbox);

    let primary_geom = routes[0].geometry.clone();
    let primary_steps = routes[0].steps.clone();

    PluginResponse {
        ok: true,
        view: "map",
        summary,
        detail,
        geometry_kind: "road_network".to_string(),
        mode: mode.as_str().to_string(),
        distance_m: road_dist,
        duration_s: road_dur,
        geometry: primary_geom,
        steps: primary_steps,
        routes,
        bbox,
        osm_embed_url,
        osm_browse_url,
        map_attribution: "© OpenStreetMap contributors — routage OSRM (ODbL).".to_string(),
        resolved_from,
        resolved_to,
    }
}

fn build_response(
    action: &str,
    mode: TravelMode,
    from: Point,
    to: Point,
    resolved_from: Option<String>,
    resolved_to: Option<String>,
) -> PluginResponse {
    let from_l = resolved_from
        .as_deref()
        .unwrap_or("origine")
        .to_string();
    let to_l = resolved_to.as_deref().unwrap_or("destination").to_string();

    let chord_m = haversine_m(from, to);
    let is_distance_only = action.contains("distance");

    if !is_distance_only {
        let profile = match mode {
            TravelMode::Walking => "walking",
            TravelMode::Car => "driving",
            TravelMode::Train => "",
        };
        if !profile.is_empty() {
            if let Some((road_pts, road_dist, road_dur, road_steps)) =
                try_osrm_route(from, to, profile)
            {
                return build_response_osrm(
                    mode,
                    from,
                    to,
                    &from_l,
                    &to_l,
                    resolved_from.clone(),
                    resolved_to.clone(),
                    chord_m,
                    road_pts,
                    road_dist,
                    road_dur,
                    road_steps,
                );
            }
        }
    }

    let n_seg = 24_usize;
    let primary_pts = interpolate_great_circle(from, to, n_seg);
    let chord_km = chord_m / 1000.0;
    let offset_deg = (chord_km / 900.0).clamp(0.05, 0.35);
    let alt_pts = detour_polyline(from, to, n_seg, offset_deg);

    let train_mode = TravelMode::Train;
    let train_duration_s = (chord_m / 1000.0) / train_mode.average_speed_kmh() * 3600.0;

    let primary_dist = polyline_length_m(&primary_pts);
    let alt_dist = polyline_length_m(&alt_pts);

    let primary_duration = (primary_dist / 1000.0) / mode.average_speed_kmh() * 3600.0;
    let alt_duration = (alt_dist / 1000.0) / mode.average_speed_kmh() * 3600.0;

    let summary = if is_distance_only {
        format!(
            "Distance estimée : {:.1} km ({}), durée indicative ~{:.0} min. Tracé sur carte = ligne simplifiée entre les centres résolus.",
            chord_m / 1000.0,
            mode.as_str(),
            primary_duration / 60.0
        )
    } else {
        format!(
            "Itinéraire estimé : {:.1} km, ~{:.0} min ({}). Voir la carte, les variantes et les étapes ; pour le détail routier réel, ouvrir OpenStreetMap ou une appli GPS.",
            chord_m / 1000.0,
            primary_duration / 60.0,
            mode.as_str()
        )
    };

    let detail = Some(
        "Les tracés sont des estimations entre coordonnées du gazetteer (pas de calcul routier OSRM/ORS). Les alternatives illustrent un autre corridor et un mode train indicatif.".to_string(),
    );

    let routes = vec![
        RouteOption {
            id: "primary_car",
            label: format!("Voiture — direct (~{:.0} min)", primary_duration / 60.0),
            mode: mode.as_str().to_string(),
            distance_m: primary_dist,
            duration_s: primary_duration,
            geometry: linestring_geojson(&primary_pts),
            steps: ordinal_labels_for_polyline(&primary_pts, &from_l, &to_l),
        },
        RouteOption {
            id: "alt_corridor",
            label: format!(
                "Voiture — variante corridor (+{:.1} km vs direct)",
                (alt_dist - primary_dist) / 1000.0
            ),
            mode: mode.as_str().to_string(),
            distance_m: alt_dist,
            duration_s: alt_duration,
            geometry: linestring_geojson(&alt_pts),
            steps: ordinal_labels_for_polyline(&alt_pts, &from_l, &to_l),
        },
        RouteOption {
            id: "train_indicator",
            label: format!(
                "Train — temps indicatif (~{:.0} min, tracé ≠ voies réelles)",
                train_duration_s / 60.0
            ),
            mode: train_mode.as_str().to_string(),
            distance_m: chord_m,
            duration_s: train_duration_s,
            geometry: linestring_geojson(&primary_pts),
            steps: vec![json!({
                "instruction": format!(
                    "Temps de parcours train approximatif (grande vitesse moyenne indicative) entre {} et {} — vérifier les gares et correspondances.",
                    from_l, to_l
                ),
                "distance_m": chord_m
            })],
        },
    ];

    let mut all_pts = primary_pts.clone();
    all_pts.extend(alt_pts.iter().copied());

    let bbox = bbox_for_points(&all_pts, 0.08);
    let osm_embed_url = osm_embed_from_bbox(&bbox);
    let osm_browse_url = osm_browse_url_from_bbox(&bbox);

    let primary_geom = routes[0].geometry.clone();
    let primary_steps = routes[0].steps.clone();

    PluginResponse {
        ok: true,
        view: "map",
        summary,
        detail,
        geometry_kind: "great_circle_estimate".to_string(),
        mode: mode.as_str().to_string(),
        distance_m: chord_m,
        duration_s: primary_duration,
        geometry: primary_geom,
        steps: primary_steps,
        routes,
        bbox,
        osm_embed_url,
        osm_browse_url,
        map_attribution: "© OpenStreetMap contributors — embarqué sous licence ODbL.".to_string(),
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
    
    // Debug: log input stats to stderr in debug builds only (WASM safe, no raw args)
    if cfg!(debug_assertions) {
        let has_first_arg = args.first().is_some();
        eprintln!(
            "[maps-plugin] input_len={} action={} args_count={} has_first_arg={}",
            input.len(),
            action,
            args.len(),
            has_first_arg
        );
    }

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
        if cfg!(debug_assertions) { eprintln!("[maps-plugin] Resolved from explicit from/to in payload"); }
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
        if cfg!(debug_assertions) { eprintln!("[maps-plugin] Resolved from explicit from_text/to_text in payload"); }
        resolved_from = Some(fs.1);
        resolved_to = Some(ts.1);
        (fs.0, ts.0)
    } else if let Some((f, t)) = parse_args_points(&args) {
        if cfg!(debug_assertions) { eprintln!("[maps-plugin] Resolved from numeric args"); }
        (f, t)
    } else if let Some((fs, ts)) = parse_args_text_points(&args) {
        if cfg!(debug_assertions) { eprintln!("[maps-plugin] Resolved from text args"); }
        resolved_from = Some(fs.1);
        resolved_to = Some(ts.1);
        (fs.0, ts.0)
    } else {
        if cfg!(debug_assertions) { eprintln!("[maps-plugin] ERROR: Could not resolve any points (args_count={})", args.len()); }
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

/// Size of the shared I/O buffer. The host must not read/write more than this
/// many bytes relative to the pointer returned by `buffer_ptr()`.
const BUFFER_SIZE: usize = 65536;

/// Shared memory buffer for input/output (WASM linear memory ABI).
/// The host uses buffer_ptr() to find where to write input and read output.
static mut BUFFER: [u8; BUFFER_SIZE] = [0u8; BUFFER_SIZE];

/// Export the address of the I/O buffer so the host can write input here
/// and read output from here, instead of using the hardcoded offset 0
/// (which points into the shadow-stack area, not the BSS data segment).
/// The returned value is an unsigned linear-memory byte offset (cast to i32
/// for WASM ABI compatibility; the host should treat it as u32).
#[no_mangle]
pub extern "C" fn buffer_ptr() -> i32 {
    std::ptr::addr_of_mut!(BUFFER) as *mut u8 as i32
}

/// Export the size of the I/O buffer so the host knows the maximum number of
/// bytes it may write to (or read from) the address returned by `buffer_ptr()`.
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
        return 0;  // Input too large
    }

    // Copy input bytes into an owned String before we ever touch BUFFER again,
    // so that the immutable borrow does not overlap the later mutable write.
    let input: String = unsafe {
        let bytes = std::slice::from_raw_parts(std::ptr::addr_of!(BUFFER) as *const u8, len);
        std::str::from_utf8(bytes).unwrap_or("{}").to_owned()
    };

    let output = handle(&input);
    let out_bytes = output.as_bytes();

    if out_bytes.len() > BUFFER_SIZE {
        return 0;  // Output too large
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
