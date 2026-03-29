use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy)]
struct SimParams {
    initial: f64,
    growth_rate: f64,
    noise: f64,
    horizon: usize,
}

#[derive(Serialize)]
struct SimRunResponse {
    ok: bool,
    view: &'static str,
    summary: String,
    model: String,
    series: Vec<Value>,
    metrics: Value,
}

#[derive(Serialize)]
struct SimCompareResponse {
    ok: bool,
    view: &'static str,
    summary: String,
    scenarios: Vec<Value>,
    delta: Value,
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

fn parse_params(payload: &Value, args: &[String]) -> SimParams {
    // Args form: sim_run <initial> <growth_rate> <noise> <horizon>
    let from_args = || -> Option<SimParams> {
        if args.len() < 4 {
            return None;
        }
        Some(SimParams {
            initial: args.get(0)?.parse::<f64>().ok()?,
            growth_rate: args.get(1)?.parse::<f64>().ok()?,
            noise: args.get(2)?.parse::<f64>().ok()?,
            horizon: args.get(3)?.parse::<usize>().ok()?,
        })
    };

    if let Some(p) = payload.get("params") {
        let initial = p.get("initial").and_then(Value::as_f64).unwrap_or(100.0);
        let growth_rate = p
            .get("growth_rate")
            .and_then(Value::as_f64)
            .unwrap_or(0.02);
        let noise = p.get("noise").and_then(Value::as_f64).unwrap_or(0.03);
        let horizon = p
            .get("horizon")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(60);
        return SimParams {
            initial,
            growth_rate,
            noise,
            horizon,
        };
    }

    from_args().unwrap_or(SimParams {
        initial: 100.0,
        growth_rate: 0.02,
        noise: 0.03,
        horizon: 60,
    })
}

fn deterministic_noise(seed: u64, t: usize) -> f64 {
    let x = ((seed as f64) * 0.0001 + (t as f64) * 12.9898).sin() * 43758.5453;
    let frac = x - x.floor();
    (frac * 2.0) - 1.0
}

fn build_series(params: SimParams, seed: u64) -> Vec<(usize, f64)> {
    let mut out = Vec::with_capacity(params.horizon + 1);
    let mut y = params.initial.max(0.0);
    out.push((0, y));
    for t in 1..=params.horizon {
        let eps = deterministic_noise(seed, t) * params.noise;
        y = (y * (1.0 + params.growth_rate + eps)).max(0.0);
        out.push((t, y));
    }
    out
}

fn compute_metrics(series: &[(usize, f64)]) -> Value {
    let ys: Vec<f64> = series.iter().map(|(_, y)| *y).collect();
    if ys.is_empty() {
        return json!({"count": 0});
    }
    let count = ys.len();
    let min = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let max = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let avg = ys.iter().sum::<f64>() / count as f64;
    let final_value = *ys.last().unwrap_or(&0.0);

    let mut sorted = ys.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_idx = ((count as f64) * 0.95).floor() as usize;
    let p95 = sorted.get(p95_idx.min(count - 1)).copied().unwrap_or(final_value);

    json!({
        "count": count,
        "min": min,
        "max": max,
        "avg": avg,
        "p95": p95,
        "final": final_value
    })
}

fn sim_run(payload: &Value, args: &[String], model: &str) -> String {
    let seed = payload
        .get("seed")
        .and_then(Value::as_u64)
        .unwrap_or(42);
    let params = parse_params(payload, args);
    let points = build_series(params, seed);
    let metrics = compute_metrics(&points);

    let xy: Vec<Value> = points
        .iter()
        .map(|(x, y)| json!([x, y]))
        .collect();

    let out = SimRunResponse {
        ok: true,
        view: "timeseries",
        summary: format!(
            "Simulation '{}' finished (horizon={}, final={:.2})",
            model,
            params.horizon,
            metrics.get("final").and_then(Value::as_f64).unwrap_or(0.0)
        ),
        model: model.to_string(),
        series: vec![json!({
            "name": "value",
            "points": xy
        })],
        metrics,
    };

    serde_json::to_string(&out)
        .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization\"}".to_string())
}

fn sim_compare(payload: &Value, args: &[String], model: &str) -> String {
    let base_seed = payload
        .get("seed")
        .and_then(Value::as_u64)
        .unwrap_or(42);

    let base_params = parse_params(payload, args);
    let alt_params = SimParams {
        initial: payload
            .get("compare")
            .and_then(|c| c.get("initial"))
            .and_then(Value::as_f64)
            .unwrap_or(base_params.initial),
        growth_rate: payload
            .get("compare")
            .and_then(|c| c.get("growth_rate"))
            .and_then(Value::as_f64)
            .unwrap_or(base_params.growth_rate + 0.01),
        noise: payload
            .get("compare")
            .and_then(|c| c.get("noise"))
            .and_then(Value::as_f64)
            .unwrap_or(base_params.noise),
        horizon: payload
            .get("compare")
            .and_then(|c| c.get("horizon"))
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(base_params.horizon),
    };

    let base_series = build_series(base_params, base_seed);
    let alt_series = build_series(alt_params, base_seed + 1);

    let base_final = base_series.last().map(|(_, y)| *y).unwrap_or(0.0);
    let alt_final = alt_series.last().map(|(_, y)| *y).unwrap_or(0.0);

    let out = SimCompareResponse {
        ok: true,
        view: "table",
        summary: format!(
            "Simulation comparison '{}' (base={:.2}, alternative={:.2})",
            model, base_final, alt_final
        ),
        scenarios: vec![
            json!({"name": "base", "final": base_final, "metrics": compute_metrics(&base_series)}),
            json!({"name": "alternative", "final": alt_final, "metrics": compute_metrics(&alt_series)}),
        ],
        delta: json!({
            "final_delta": alt_final - base_final,
            "final_delta_pct": if base_final.abs() > f64::EPSILON { ((alt_final - base_final) / base_final) * 100.0 } else { 0.0 }
        }),
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

    let (mut action, payload, args) = extract_payload(&value);
    if action.is_empty() {
        if tool_name.contains("compare") {
            action = "compare".to_string();
        } else {
            action = "run".to_string();
        }
    }

    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("generic_growth");

    match action.to_ascii_lowercase().as_str() {
        "compare" | "sim_compare" => sim_compare(&payload, &args, model),
        _ => sim_run(&payload, &args, model),
    }
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
