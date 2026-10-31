//! owner: WP-G.5
//!
//! `asd-cli gaze synth | replay | bench-report` through the real binary: deterministic
//! synthetic logs, `replay --check` rejections (missing header, non-monotonic `t_sec`, wrong
//! format version), the replay report, and the BEN-5 table over three synthetic devices.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_asd-cli"))
        .args(args)
        .output()
        .expect("run asd-cli")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// A fresh scratch directory per test.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("asd-cli-gaze-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn p(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn synth(scenario: &str, seed: &str, out: &Path, extra: &[&str]) {
    let mut args = vec![
        "gaze",
        "synth",
        "--scenario",
        scenario,
        "--seed",
        seed,
        "--out",
        p(out),
    ];
    args.extend_from_slice(extra);
    let o = cli(&args);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
}

#[test]
fn synth_is_deterministic_per_seed() {
    let d = scratch("determinism");
    let (a, b, c) = (d.join("a.jsonl"), d.join("b.jsonl"), d.join("c.jsonl"));
    synth("bias_only", "5", &a, &[]);
    synth("bias_only", "5", &b, &[]);
    synth("bias_only", "6", &c, &[]);
    let (ta, tb, tc) = (
        fs::read(&a).unwrap(),
        fs::read(&b).unwrap(),
        fs::read(&c).unwrap(),
    );
    assert_eq!(ta, tb, "same seed, same bytes");
    assert_ne!(ta, tc, "another seed, other bytes");
    let first = String::from_utf8(ta).unwrap();
    assert!(first.starts_with("{\"format\":\"asd-raw-tracking/1\""));
    assert!(first.ends_with('\n'));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn synth_stdout_matches_the_file() {
    let d = scratch("stdout");
    let f = d.join("x.jsonl");
    synth("ideal", "1", &f, &[]);
    let o = cli(&[
        "gaze",
        "synth",
        "--scenario",
        "ideal",
        "--seed",
        "1",
        "--out",
        "-",
    ]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(o.stdout, fs::read(&f).unwrap());
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn synth_lists_and_writes_every_scenario() {
    let o = cli(&["gaze", "synth", "--list"]);
    assert_eq!(o.status.code(), Some(0));
    let names: Vec<String> = stdout(&o).lines().map(str::to_string).collect();
    assert!(names.len() >= 12);
    for want in [
        "ideal",
        "bias_only",
        "bias_gain",
        "distance_shift_after_cal",
        "head_yaw_15",
        "drift_3deg_linear",
        "blinks_and_loss",
        "gap_60ms_vs_200ms",
        "artifact_spike",
        "orientation_landscape_ipad",
        "fps_30",
        "missing_target",
    ] {
        assert!(names.iter().any(|n| n == want), "{want}");
    }
    let d = scratch("all");
    synth("all", "2", &d.join("logs"), &[]);
    let written = fs::read_dir(d.join("logs")).unwrap().count();
    assert_eq!(written, names.len());
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn synth_rejects_bad_arguments() {
    for args in [
        &["gaze", "synth", "--scenario", "nope", "--out", "-"][..],
        &["gaze", "synth", "--scenario", "ideal"][..],
        &["gaze", "synth", "--scenario", "all", "--out", "-"][..],
        &[
            "gaze",
            "synth",
            "--scenario",
            "ideal",
            "--seed",
            "x",
            "--out",
            "-",
        ][..],
        &["gaze", "bogus"][..],
    ] {
        assert_eq!(cli(args).status.code(), Some(2), "{args:?}");
    }
}

// ------------------------------------------------------------------------------------------
// replay --check
// ------------------------------------------------------------------------------------------

fn good_log(dir: &Path) -> (PathBuf, String) {
    let f = dir.join("good.jsonl");
    synth("ideal", "3", &f, &[]);
    let text = fs::read_to_string(&f).unwrap();
    (f, text)
}

#[test]
fn check_accepts_a_generated_log_and_the_committed_sample() {
    let d = scratch("check-ok");
    let (f, _) = good_log(&d);
    let o = cli(&["gaze", "replay", "--check", p(&f)]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("ok"), "{}", stdout(&o));
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../asd-core/tests/fixtures/raw_tracking_sample.jsonl");
    let o = cli(&["gaze", "replay", "--check", p(&sample)]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn check_rejects_a_missing_header() {
    let d = scratch("check-header");
    let (_, text) = good_log(&d);
    let without: String = text.lines().skip(1).map(|l| format!("{l}\n")).collect();
    let f = d.join("no_header.jsonl");
    fs::write(&f, without).unwrap();
    let o = cli(&["gaze", "replay", "--check", p(&f)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("header"), "{}", stderr(&o));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn check_rejects_non_monotonic_time() {
    let d = scratch("check-time");
    let (_, text) = good_log(&d);
    let mut lines: Vec<&str> = text.lines().collect();
    lines.swap(10, 11);
    let f = d.join("time.jsonl");
    fs::write(&f, lines.join("\n") + "\n").unwrap();
    let o = cli(&["gaze", "replay", "--check", p(&f)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("strictly increasing"), "{}", stderr(&o));
    assert!(stderr(&o).contains("line 12"), "{}", stderr(&o));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn check_rejects_a_wrong_format_version() {
    let d = scratch("check-version");
    let (_, text) = good_log(&d);
    let f = d.join("v2.jsonl");
    fs::write(
        &f,
        text.replacen("asd-raw-tracking/1", "asd-raw-tracking/2", 1),
    )
    .unwrap();
    let o = cli(&["gaze", "replay", "--check", p(&f)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("asd-raw-tracking/2"), "{}", stderr(&o));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn check_rejects_a_bad_target_schedule_and_reports_io_errors() {
    let d = scratch("check-schedule");
    let (_, text) = good_log(&d);
    // Header target 0 ends before it starts.
    let mut header: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    let start = header["targets"][0]["startSec"].as_f64().unwrap();
    header["targets"][0]["endSec"] = Value::from(start - 1.0);
    let rest: String = text.lines().skip(1).map(|l| format!("{l}\n")).collect();
    let f = d.join("schedule.jsonl");
    fs::write(&f, format!("{header}\n{rest}")).unwrap();
    let o = cli(&["gaze", "replay", "--check", p(&f)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("target 0"), "{}", stderr(&o));
    let o = cli(&["gaze", "replay", "--check", p(&d.join("absent.jsonl"))]);
    assert_eq!(o.status.code(), Some(2));
    let _ = fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------------------------------
// replay
// ------------------------------------------------------------------------------------------

#[test]
fn replay_prints_the_report() {
    let d = scratch("replay");
    let f = d.join("bias.jsonl");
    synth("bias_only", "4", &f, &[]);
    let o = cli(&["gaze", "replay", p(&f)]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let report: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(report["summary"]["calibration"]["quality"], "good");
    assert!(
        report["summary"]["validation"]["accuracyDeg"]
            .as_f64()
            .unwrap()
            <= 0.2
    );
    assert_eq!(report["advice"]["action"], "none");
    assert_eq!(report["targets"].as_array().unwrap().len(), 11);
    // Same input, same bytes.
    assert_eq!(o.stdout, cli(&["gaze", "replay", p(&f)]).stdout);
    // A forced model is honoured.
    let o = cli(&["gaze", "replay", "--model", "angularAffine", p(&f)]);
    let report: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(report["summary"]["calibration"]["model"], "angularAffine");
    assert_eq!(
        cli(&["gaze", "replay", "--model", "nope", p(&f)])
            .status
            .code(),
        Some(2)
    );
    let _ = fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------------------------------
// bench-report
// ------------------------------------------------------------------------------------------

#[test]
fn bench_report_tables_three_devices_and_ranks_models() {
    let d = scratch("bench");
    let logs = d.join("logs");
    fs::create_dir_all(&logs).unwrap();
    let devices = ["iPhone17,3", "iPhone15,2", "iPad16,3"];
    let conditions = [("baseline", "400"), ("lowLight", "50")];
    let mut n = 0;
    for (i, dev) in devices.iter().enumerate() {
        for (j, (cond, lux)) in conditions.iter().enumerate() {
            let file = logs.join(format!("{i}_{j}.jsonl"));
            let seed = (10 * i + j).to_string();
            synth(
                "bias_gain",
                &seed,
                &file,
                &["--model", dev, "--condition", cond, "--ambient", lux],
            );
            n += 1;
        }
    }
    let o = cli(&[
        "gaze",
        "bench-report",
        "--logs",
        p(&logs),
        "--models",
        "all",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let text = stdout(&o);
    assert!(text.contains(&format!("{n} logs, 4 models")), "{text}");
    // One table row per device x condition x model.
    for dev in devices {
        for (cond, _) in conditions {
            for model in [
                "screenAffine",
                "angularAffine",
                "screenHomography",
                "screenPoly2",
            ] {
                let hit = text.lines().any(|l| {
                    let f: Vec<&str> = l.split_whitespace().collect();
                    f.len() >= 4 && f[0] == dev && f[1] == cond && f[2] == model && f[3] == "1"
                });
                assert!(hit, "missing row {dev} {cond} {model}\n{text}");
            }
        }
    }
    assert_eq!(
        text.lines()
            .filter(|l| l
                .split_whitespace()
                .nth(2)
                .is_some_and(|m| m == "screenAffine")
                && l.split_whitespace().nth(3) == Some("1"))
            .count(),
        6
    );
    // The ranking lists every model once, best first, with non-decreasing accuracy.
    let ranking: Vec<&str> = text
        .lines()
        .skip_while(|l| !l.starts_with("Model ranking"))
        .skip(1)
        .take(4)
        .collect();
    assert_eq!(ranking.len(), 4, "{text}");
    let acc: Vec<f64> = ranking
        .iter()
        .map(|l| {
            l.split_whitespace()
                .nth(2)
                .unwrap()
                .parse::<f64>()
                .unwrap_or(f64::INFINITY)
        })
        .collect();
    assert!(acc.windows(2).all(|w| w[0] <= w[1]), "{ranking:?}");
    assert!(acc[0].is_finite() && acc[0] < 0.5, "{ranking:?}");
    for model in [
        "screenAffine",
        "angularAffine",
        "screenHomography",
        "screenPoly2",
    ] {
        assert_eq!(ranking.iter().filter(|l| l.contains(model)).count(), 1);
    }
    assert!(text.contains("Light vs error"), "{text}");
    // Logs are referenced by SHA-256 (BEN-8).
    assert_eq!(
        text.lines()
            .filter(|l| l
                .split_whitespace()
                .next()
                .is_some_and(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())))
            .count(),
        n
    );
    // Deterministic.
    assert_eq!(
        o.stdout,
        cli(&["gaze", "bench-report", "--logs", p(&logs)]).stdout
    );
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn bench_report_errors() {
    let d = scratch("bench-err");
    assert_eq!(
        cli(&["gaze", "bench-report", "--logs", p(&d)])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(cli(&["gaze", "bench-report"]).status.code(), Some(2));
    assert_eq!(
        cli(&["gaze", "bench-report", "--logs", p(&d), "--models", "x"])
            .status
            .code(),
        Some(2)
    );
    fs::write(d.join("bad.jsonl"), "not json\n").unwrap();
    let o = cli(&["gaze", "bench-report", "--logs", p(&d)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("bad.jsonl"), "{}", stderr(&o));
    let _ = fs::remove_dir_all(&d);
}
