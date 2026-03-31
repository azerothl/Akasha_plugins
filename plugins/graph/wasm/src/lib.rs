use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize)]
struct GraphResponse {
    ok: bool,
    view: &'static str,
    library: &'static str,
    figure: Value,
    summary: String,
}

#[derive(Serialize)]
struct StatsResponse {
    ok: bool,
    view: &'static str,
    summary: String,
    stats: Value,
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

fn default_payload_from_args(args: &[String]) -> Value {
    // graph_plot line 1,2,3,4
    if args.len() >= 2 {
        let chart = args.first().cloned().unwrap_or_else(|| "line".to_string());
        let y: Vec<f64> = args
            .iter()
            .skip(1)
            .filter_map(|v| v.parse::<f64>().ok())
            .collect();
        if !y.is_empty() {
            let x: Vec<usize> = (0..y.len()).collect();
            return json!({
                "chart": chart,
                "x": x,
                "series": [{"name": "series_1", "y": y}],
                "title": "Generated graph"
            });
        }
    }
    json!({})
}

fn chart_to_plotly_type(chart: &str) -> (&'static str, Option<&'static str>) {
    match chart.to_ascii_lowercase().as_str() {
        "bar" => ("bar", None),
        "scatter" => ("scatter", Some("markers")),
        "histogram" => ("histogram", None),
        _ => ("scatter", Some("lines")),
    }
}

fn compute_stats(series: &Value) -> Value {
    let mut rows = Vec::new();
    if let Some(arr) = series.as_array() {
        for s in arr {
            let name = s
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("series")
                .to_string();
            let ys: Vec<f64> = s
                .get("y")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            if ys.is_empty() {
                rows.push(json!({
                    "name": name,
                    "count": 0,
                    "min": Value::Null,
                    "max": Value::Null,
                    "avg": Value::Null
                }));
                continue;
            }
            let min = ys.iter().copied().fold(f64::INFINITY, f64::min);
            let max = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let sum: f64 = ys.iter().sum();
            let avg = sum / ys.len() as f64;
            rows.push(json!({
                "name": name,
                "count": ys.len(),
                "min": min,
                "max": max,
                "avg": avg
            }));
        }
    }
    json!(rows)
}

fn handle_graph_plot(payload: &Value) -> String {
    let chart = payload
        .get("chart")
        .and_then(Value::as_str)
        .unwrap_or("line");
    let (plotly_type, mode) = chart_to_plotly_type(chart);

    let x = payload.get("x").cloned().unwrap_or_else(|| json!([]));
    let series = payload
        .get("series")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if series.is_empty() {
        return json!({
            "ok": false,
            "error": "invalid_input",
            "message": "Provide series: [{name, y:[...]}] and optional x/chart/title"
        })
        .to_string();
    }

    let data: Vec<Value> = series
        .iter()
        .map(|s| {
            let name = s.get("name").and_then(Value::as_str).unwrap_or("series");
            let y = s.get("y").cloned().unwrap_or_else(|| json!([]));
            let mut trace = json!({
                "type": plotly_type,
                "name": name,
                "x": x,
                "y": y
            });
            if let Some(m) = mode {
                trace["mode"] = json!(m);
            }
            trace
        })
        .collect();

    let title = payload
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Graph");

    let out = GraphResponse {
        ok: true,
        view: "graph",
        library: "plotly",
        figure: json!({
            "data": data,
            "layout": {
                "title": title,
                "xaxis": { "title": payload.get("x_label").and_then(Value::as_str).unwrap_or("x") },
                "yaxis": { "title": payload.get("y_label").and_then(Value::as_str).unwrap_or("y") }
            }
        }),
        summary: format!("Graph generated ({} series, chart={})", series.len(), chart),
    };

    serde_json::to_string(&out)
        .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization\"}".to_string())
}

fn handle_graph_stats(payload: &Value) -> String {
    let stats = compute_stats(payload.get("series").unwrap_or(&json!([])));
    let out = StatsResponse {
        ok: true,
        view: "table",
        summary: "Series statistics computed".to_string(),
        stats,
    };
    serde_json::to_string(&out)
        .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization\"}".to_string())
}

fn handle(input: &str) -> String {
    let value = serde_json::from_str::<Value>(input).unwrap_or_else(|_| json!({}));
    let tool_name = value
        .get("tool")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();

    let (mut action, mut payload, args) = extract_payload(&value);
    if payload == json!({}) {
        payload = default_payload_from_args(&args);
    }

    if action.is_empty() {
        if tool_name.contains("stats") {
            action = "stats".to_string();
        } else {
            action = "plot".to_string();
        }
    }

    match action.to_ascii_lowercase().as_str() {
        "stats" | "graph_stats" => handle_graph_stats(&payload),
        _ => handle_graph_plot(&payload),
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
