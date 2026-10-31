//! owner: WP-G.5
//!
//! `asd-cli gaze`: synthetic raw tracking logs, replay through the Rust pipeline, and the
//! bench report (technical.md TV-1, REP-1, BEN-5).
//!
//! ```text
//! gaze synth --scenario NAME|all --seed N --out FILE|DIR|-   [--model ID] [--condition C] [--ambient LUX]
//! gaze synth --list
//! gaze replay FILE [--check] [--model KIND] [--glasses true|false]
//! gaze bench-report --logs DIR [--models all|KIND,KIND]
//! ```
//!
//! `replay` prints the replay report (target results, advice, `trackingQuality`) as JSON;
//! with `--check` it only validates the log (header, format version, strictly increasing
//! `t_sec`, target schedule). `bench-report` replays every `*.jsonl` log of a directory with
//! each calibration model and prints accuracy, precision and loss per device x condition x
//! model, ranks the models, and reports how strongly ambient light tracks the error (CNF-2).
//! Bench logs are biometric data and never committed (BEN-8): the report lists each log by
//! its SHA-256 only.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use asd_core::gaze::raw_log::{self, RawLog};
use asd_core::gaze::replay::{check_schedule, replay, ReplayOptions};
use asd_core::synth_tracking::{generate_with, Scenario, SCENARIOS};
use asd_core::types::tracking::CalModelKind;
use serde_json::Value;
use sha2::{Digest, Sha256};

const USAGE: &str = "\
usage:
  asd-cli gaze synth --scenario NAME|all --seed N --out FILE|DIR|- [--model ID] [--condition C] [--ambient LUX]
  asd-cli gaze synth --list
  asd-cli gaze replay FILE [--check] [--model KIND] [--glasses true|false]
  asd-cli gaze bench-report --logs DIR [--models all|KIND,KIND,...]

KIND is one of screenAffine, angularAffine, screenHomography, screenPoly2.";

const ALL_MODELS: [CalModelKind; 4] = [
    CalModelKind::ScreenAffine,
    CalModelKind::AngularAffine,
    CalModelKind::ScreenHomography,
    CalModelKind::ScreenPoly2,
];

/// Runs the `gaze` subcommand; `args` are the arguments after `gaze`. Returns the exit code
/// (0 success, 1 check failure, 2 usage or I/O error).
pub fn run(args: &[String]) -> i32 {
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        Some("synth") => synth(rest),
        Some("replay") => replay_cmd(rest),
        Some("bench-report") => bench_cmd(rest),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            0
        }
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}

fn usage_error(message: &str) -> i32 {
    eprintln!("error: {message}");
    2
}

fn model_name(kind: CalModelKind) -> String {
    match serde_json::to_value(kind) {
        Ok(Value::String(s)) => s,
        _ => format!("{kind:?}"),
    }
}

fn parse_model(s: &str) -> Result<CalModelKind, String> {
    serde_json::from_value(Value::String(s.to_string())).map_err(|_| {
        let all: Vec<String> = ALL_MODELS.iter().map(|m| model_name(*m)).collect();
        format!(
            "unknown calibration model `{s}` (one of: {})",
            all.join(", ")
        )
    })
}

// ------------------------------------------------------------------------------------------
// gaze synth
// ------------------------------------------------------------------------------------------

fn synth(args: &[String]) -> i32 {
    let (mut scenario, mut seed, mut out) = (None::<String>, 0u64, None::<String>);
    let (mut model, mut condition, mut ambient) = (None::<String>, None::<String>, None::<f64>);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--list" {
            for s in SCENARIOS {
                println!("{}", s.name());
            }
            return 0;
        }
        let Some(value) = it.next() else {
            return usage_error(&format!("{flag} needs a value"));
        };
        let ok = match flag.as_str() {
            "--scenario" => {
                scenario = Some(value.clone());
                true
            }
            "--seed" => value.parse().map(|v| seed = v).is_ok(),
            "--out" => {
                out = Some(value.clone());
                true
            }
            "--model" => {
                model = Some(value.clone());
                true
            }
            "--condition" => {
                condition = Some(value.clone());
                true
            }
            "--ambient" => value.parse().map(|v| ambient = Some(v)).is_ok(),
            _ => return usage_error(&format!("unexpected argument {flag}")),
        };
        if !ok {
            return usage_error(&format!("invalid value {value:?} for {flag}"));
        }
    }
    let (Some(scenario), Some(out)) = (scenario, out) else {
        return usage_error("gaze synth needs --scenario and --out (or --list)");
    };
    let scenarios: Vec<Scenario> = if scenario == "all" {
        SCENARIOS.to_vec()
    } else {
        match scenario.parse() {
            Ok(s) => vec![s],
            Err(e) => return usage_error(&e),
        }
    };

    let render = |s: Scenario| -> Result<String, String> {
        let mut spec = s.spec();
        if let Some(m) = &model {
            spec.model_identifier = m.clone();
        }
        if let Some(c) = &condition {
            spec.condition = c.clone();
        }
        if let Some(a) = ambient {
            spec.ambient_lux = a;
        }
        raw_log::to_jsonl(&generate_with(&spec, s.name(), seed)).map_err(|e| e.to_string())
    };

    let to_stdout = out == "-";
    let out_path = PathBuf::from(&out);
    let dir_mode = !to_stdout && (scenarios.len() > 1 || out_path.is_dir());
    if to_stdout && scenarios.len() > 1 {
        return usage_error("--scenario all needs --out DIR");
    }
    if dir_mode {
        if let Err(e) = fs::create_dir_all(&out_path) {
            return usage_error(&format!("cannot create {out}: {e}"));
        }
    }
    for s in scenarios {
        let text = match render(s) {
            Ok(t) => t,
            Err(e) => return usage_error(&e),
        };
        if to_stdout {
            print!("{text}");
            continue;
        }
        let path = if dir_mode {
            out_path.join(format!("{}.jsonl", s.name()))
        } else {
            out_path.clone()
        };
        if let Err(e) = fs::write(&path, text) {
            return usage_error(&format!("cannot write {}: {e}", path.display()));
        }
        eprintln!("Wrote SYNTHETIC log {}", path.display());
    }
    0
}

// ------------------------------------------------------------------------------------------
// gaze replay
// ------------------------------------------------------------------------------------------

fn replay_cmd(args: &[String]) -> i32 {
    let (mut file, mut check) = (None::<String>, false);
    let mut options = ReplayOptions::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--model" | "--glasses" => {
                let Some(value) = it.next() else {
                    return usage_error(&format!("{arg} needs a value"));
                };
                if arg == "--model" {
                    match parse_model(value) {
                        Ok(m) => options.model = Some(m),
                        Err(e) => return usage_error(&e),
                    }
                } else {
                    match value.parse() {
                        Ok(b) => options.glasses = Some(b),
                        Err(_) => return usage_error("--glasses takes true or false"),
                    }
                }
            }
            a if a.starts_with("--") => return usage_error(&format!("unexpected argument {a}")),
            a if file.is_none() => file = Some(a.to_string()),
            a => return usage_error(&format!("unexpected argument {a}")),
        }
    }
    let Some(file) = file else {
        return usage_error("gaze replay needs a log FILE");
    };
    let text = match fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) => return usage_error(&format!("cannot read {file}: {e}")),
    };
    let log = match raw_log::parse(&text) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {file}: {e}");
            return 1;
        }
    };
    if check {
        if let Err(e) = check_schedule(&log.header.targets) {
            eprintln!("error: {file}: {e}");
            return 1;
        }
        println!(
            "{file}: ok ({} samples, {} targets, {})",
            log.samples.len(),
            log.header.targets.len(),
            log.header.device_model
        );
        return 0;
    }
    match replay(&log, &options) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(s) => {
                println!("{s}");
                0
            }
            Err(e) => usage_error(&e.to_string()),
        },
        Err(e) => {
            eprintln!("error: {file}: {e}");
            1
        }
    }
}

// ------------------------------------------------------------------------------------------
// gaze bench-report
// ------------------------------------------------------------------------------------------

fn bench_cmd(args: &[String]) -> i32 {
    let (mut logs, mut models) = (None::<String>, "all".to_string());
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let Some(value) = it.next() else {
            return usage_error(&format!("{flag} needs a value"));
        };
        match flag.as_str() {
            "--logs" => logs = Some(value.clone()),
            "--models" => models = value.clone(),
            _ => return usage_error(&format!("unexpected argument {flag}")),
        }
    }
    let Some(logs) = logs else {
        return usage_error("gaze bench-report needs --logs DIR");
    };
    let kinds: Vec<CalModelKind> = if models == "all" {
        ALL_MODELS.to_vec()
    } else {
        match models.split(',').map(|m| parse_model(m.trim())).collect() {
            Ok(k) => k,
            Err(e) => return usage_error(&e),
        }
    };
    match bench_report(Path::new(&logs), &kinds) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

/// One (log, model) replay.
struct Row {
    device: String,
    condition: String,
    model: CalModelKind,
    accuracy: Option<f64>,
    precision: Option<f64>,
    data_loss: Option<f64>,
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// Ranks with ties sharing their average rank.
fn ranks(values: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..values.len()).collect();
    idx.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut out = vec![0.0; values.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && values[idx[j + 1]] == values[idx[i]] {
            j += 1;
        }
        let rank = (i + j) as f64 / 2.0 + 1.0;
        for &k in &idx[i..=j] {
            out[k] = rank;
        }
        i = j + 1;
    }
    out
}

/// Spearman rank correlation; `None` for fewer than 3 pairs or no variation in either series.
fn spearman(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() < 3 || x.len() != y.len() {
        return None;
    }
    let (rx, ry) = (ranks(x), ranks(y));
    let (mx, my) = (mean(&rx)?, mean(&ry)?);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in rx.iter().zip(&ry) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    (sxx > 0.0 && syy > 0.0).then(|| sxy / (sxx * syy).sqrt())
}

fn fmt_opt(v: Option<f64>, digits: usize) -> String {
    v.map_or("-".to_string(), |x| format!("{x:.digits$}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn mean_ambient(log: &RawLog) -> Option<f64> {
    let v: Vec<f64> = log
        .samples
        .iter()
        .filter(|s| s.face_tracked)
        .filter_map(|s| s.ambient_intensity)
        .collect();
    mean(&v)
}

/// Builds the BEN-5 report text for every `*.jsonl` log in `dir`, with each of `models`.
pub fn bench_report(dir: &Path, models: &[CalModelKind]) -> Result<String, String> {
    if models.is_empty() {
        return Err("no calibration models selected".into());
    }
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("no *.jsonl logs in {}", dir.display()));
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut listing = String::new();
    // Per log: mean ambient light and the validation accuracy of the first model.
    let mut light: Vec<(f64, f64)> = Vec::new();
    for path in &files {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let bytes = fs::read(path).map_err(|e| format!("cannot read {name}: {e}"))?;
        let text = String::from_utf8(bytes.clone()).map_err(|e| format!("{name}: {e}"))?;
        let log = raw_log::parse(&text).map_err(|e| format!("{name}: {e}"))?;
        let device = log.header.device_model.clone();
        let condition = log
            .header
            .condition
            .clone()
            .unwrap_or_else(|| "unknown".into());
        let _ = writeln!(
            listing,
            "  {}  {name}  ({device}, {condition}, {} samples)",
            sha256_hex(&bytes),
            log.samples.len()
        );
        for (i, kind) in models.iter().enumerate() {
            let rep = replay(
                &log,
                &ReplayOptions {
                    model: Some(*kind),
                    ..Default::default()
                },
            )
            .map_err(|e| format!("{name} ({}): {e}", model_name(*kind)))?;
            let val = rep.summary.validation.as_ref();
            let accuracy = val.and_then(|v| v.accuracy_deg);
            if i == 0 {
                if let (Some(a), Some(l)) = (accuracy, mean_ambient(&log)) {
                    light.push((l, a));
                }
            }
            rows.push(Row {
                device: device.clone(),
                condition: condition.clone(),
                model: *kind,
                accuracy,
                precision: val.and_then(|v| v.precision_rms_deg),
                data_loss: val.map(|v| v.data_loss),
            });
        }
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "Gaze bench report: {} logs, {} models",
        files.len(),
        models.len()
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{:<18} {:<14} {:<17} {:>4} {:>13} {:>14} {:>10} {:>7}",
        "device",
        "condition",
        "model",
        "logs",
        "accuracy deg",
        "precision deg",
        "data loss",
        "no val"
    );
    let mut groups: BTreeMap<(String, String, String), Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        groups
            .entry((r.device.clone(), r.condition.clone(), model_name(r.model)))
            .or_default()
            .push(r);
    }
    for ((device, condition, model), g) in &groups {
        let col = |f: fn(&Row) -> Option<f64>| g.iter().filter_map(|r| f(r)).collect::<Vec<f64>>();
        let _ = writeln!(
            out,
            "{:<18} {:<14} {:<17} {:>4} {:>13} {:>14} {:>10} {:>7}",
            device,
            condition,
            model,
            g.len(),
            fmt_opt(median(&col(|r| r.accuracy)), 3),
            fmt_opt(median(&col(|r| r.precision)), 3),
            fmt_opt(mean(&col(|r| r.data_loss)), 3),
            g.iter().filter(|r| r.accuracy.is_none()).count(),
        );
    }

    // Ranking: lowest median accuracy first; fewer logs without a validation breaks ties.
    let mut ranking: Vec<(String, Option<f64>, usize)> = models
        .iter()
        .map(|m| {
            let mine: Vec<&Row> = rows.iter().filter(|r| r.model == *m).collect();
            let acc: Vec<f64> = mine.iter().filter_map(|r| r.accuracy).collect();
            (
                model_name(*m),
                median(&acc),
                mine.iter().filter(|r| r.accuracy.is_none()).count(),
            )
        })
        .collect();
    ranking.sort_by(|a, b| {
        let key = |x: &(String, Option<f64>, usize)| x.1.unwrap_or(f64::INFINITY);
        key(a)
            .total_cmp(&key(b))
            .then(a.2.cmp(&b.2))
            .then(a.0.cmp(&b.0))
    });
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Model ranking (median validation accuracy over all logs, lower is better):"
    );
    for (i, (name, acc, missing)) in ranking.iter().enumerate() {
        let _ = writeln!(
            out,
            "  {}. {:<17} {:>8} deg   {missing} of {} logs without validation",
            i + 1,
            name,
            fmt_opt(*acc, 3),
            files.len()
        );
    }

    let _ = writeln!(out);
    let (lx, ay): (Vec<f64>, Vec<f64>) = light.iter().copied().unzip();
    match spearman(&lx, &ay) {
        Some(rho) => {
            let _ = writeln!(
                out,
                "Light vs error (CNF-2, model {}): Spearman rho = {rho:+.2} over {} logs with ambient light and validation.",
                model_name(models[0]),
                lx.len()
            );
        }
        None => {
            let _ = writeln!(
                out,
                "Light vs error (CNF-2): not computable (needs at least 3 logs with validation and varying ambient light)."
            );
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "Logs (SHA-256, BEN-8):");
    out.push_str(&listing);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spearman_of_monotone_series() {
        assert_eq!(
            spearman(&[1.0, 2.0, 3.0, 4.0], &[2.0, 4.0, 6.0, 9.0]),
            Some(1.0)
        );
        assert_eq!(
            spearman(&[1.0, 2.0, 3.0, 4.0], &[9.0, 6.0, 4.0, 2.0]),
            Some(-1.0)
        );
        assert_eq!(spearman(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]), None);
        assert_eq!(spearman(&[1.0, 2.0], &[1.0, 2.0]), None);
    }

    #[test]
    fn ranks_share_ties() {
        assert_eq!(ranks(&[3.0, 1.0, 3.0, 2.0]), vec![3.5, 1.0, 3.5, 2.0]);
    }

    #[test]
    fn median_and_mean() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
        assert_eq!(mean(&[1.0, 2.0]), Some(1.5));
    }

    #[test]
    fn parses_model_names() {
        assert_eq!(
            parse_model("angularAffine"),
            Ok(CalModelKind::AngularAffine)
        );
        assert!(parse_model("nope").unwrap_err().contains("screenPoly2"));
    }
}
