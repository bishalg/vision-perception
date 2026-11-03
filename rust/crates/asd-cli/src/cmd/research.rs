//! owner: WP-X.10
//!
//! `asd-cli research reprocess|evaluate` (RPX-5): the researcher toolkit core on files. All
//! logic is `asd_core::analysis::evaluate`; this module parses arguments, reads files, calls
//! the core and prints the `Evaluation` as pretty JSON.
//!
//! Exit codes: 0 ok, 1 usage or input error (unreadable or malformed file, invalid profile),
//! 2 the input cannot be processed at the requested level (`--level frames` on a session
//! without a frame log or on a log without clips; `--level score` on a frame log).

use std::fs;

use asd_contract::params::{self, DocumentKind};
use asd_core::analysis::{
    evaluate, AnalysisProfile, Evaluation, LevelRequest, ResearchInput, SessionInput,
};
use asd_core::gaze::raw_log;
use asd_core::gaze::reprocess::ReprocessOptions;
use asd_core::types::tracking::CalModelKind;
use asd_core::types::{NormativeBaselineSet, StimulusCatalog};
use serde_json::Value;

const USAGE: &str = "\
usage:
  asd-cli research reprocess LOG --catalog FILE --baselines FILE --age-months N
      [--profile FILE] [--level auto|frames] [--trace-hz N] [--scored-at TS]
      [--model KIND] [--glasses true|false]
  asd-cli research evaluate SESSION.json --baselines FILE
      [--profile FILE] [--age-months N] [--level auto|score] [--scored-at TS]

reprocess re-runs a research frame log (JSONL) through the gaze pipeline with the profile
and scores it; evaluate re-scores a stored session document at score level. Both print the
Evaluation as JSON. Without --profile the default scoring profile is used.
Exit codes: 0 ok, 1 usage or input error, 2 cannot be processed at the requested level.";

/// Runs `research <sub> ...`; `args` are the arguments after `research`.
pub fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some(sub @ ("reprocess" | "evaluate")) => match Opts::parse(&args[1..]) {
            Ok(o) => match go(sub == "reprocess", &o) {
                Ok(e) => print(&e),
                Err(Fail::Input(m)) => fail(1, &m),
                Err(Fail::Level(m)) => fail(2, &m),
            },
            Err(m) => fail(1, &m),
        },
        Some("-h" | "--help") => {
            println!("{USAGE}");
            0
        }
        _ => fail(1, "missing or unknown research subcommand"),
    }
}

fn fail(code: i32, message: &str) -> i32 {
    eprintln!("error: {message}");
    if code == 1 {
        eprintln!("\n{USAGE}");
    }
    code
}

enum Fail {
    Input(String),
    Level(String),
}

fn input<T, E: std::fmt::Display>(r: Result<T, E>, what: &str) -> Result<T, Fail> {
    r.map_err(|e| Fail::Input(format!("{what}: {e}")))
}

#[derive(Default)]
struct Opts {
    file: Option<String>,
    catalog: Option<String>,
    baselines: Option<String>,
    profile: Option<String>,
    age_months: Option<i64>,
    level: Option<String>,
    trace_hz: Option<u32>,
    scored_at: String,
    model: Option<CalModelKind>,
    glasses: Option<bool>,
}

impl Opts {
    fn parse(args: &[String]) -> Result<Opts, String> {
        let mut o = Opts::default();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            if !arg.starts_with("--") {
                if o.file.replace(arg.clone()).is_some() {
                    return Err(format!("unexpected argument {arg}"));
                }
                continue;
            }
            let value = it
                .next()
                .ok_or_else(|| format!("{arg} needs a value"))?
                .clone();
            let bad = |what: &str| format!("{arg}: {value} is not {what}");
            match arg.as_str() {
                "--catalog" => o.catalog = Some(value),
                "--baselines" => o.baselines = Some(value),
                "--profile" => o.profile = Some(value),
                "--scored-at" => o.scored_at = value,
                "--age-months" => {
                    o.age_months = Some(value.parse().map_err(|_| bad("a whole number"))?)
                }
                "--trace-hz" => {
                    o.trace_hz = Some(value.parse().map_err(|_| bad("a whole number"))?)
                }
                "--glasses" => o.glasses = Some(value.parse().map_err(|_| bad("true or false"))?),
                "--model" => {
                    o.model = Some(
                        serde_json::from_value(Value::String(value.clone()))
                            .map_err(|_| bad("a calibration model"))?,
                    )
                }
                "--level" => {
                    if !matches!(value.as_str(), "auto" | "score" | "frames") {
                        return Err(bad("auto, score or frames"));
                    }
                    o.level = Some(value);
                }
                other => return Err(format!("unexpected argument {other}")),
            }
        }
        Ok(o)
    }
}

fn read(path: &str) -> Result<String, Fail> {
    input(fs::read_to_string(path), path)
}

fn read_json(path: &str) -> Result<Value, Fail> {
    input(serde_json::from_str(&read(path)?), path)
}

/// The profile file (validated against the parameter registry, PRM-4), or the defaults.
fn load_profile(path: Option<&str>) -> Result<AnalysisProfile, Fail> {
    let Some(path) = path else {
        return Ok(AnalysisProfile::default());
    };
    let body = read_json(path)?;
    let problems = params::validate_messages(DocumentKind::ScoringProfile, &body);
    if !problems.is_empty() {
        return Err(Fail::Input(format!(
            "{path}: invalid profile: {}",
            problems.join("; ")
        )));
    }
    input(serde_json::from_value(body), path)
}

fn level(o: &Opts, allowed: &str) -> Result<LevelRequest, Fail> {
    match o.level.as_deref() {
        None | Some("auto") => Ok(LevelRequest::Auto),
        Some("score") if allowed == "score" => Ok(LevelRequest::Score),
        Some("frames") if allowed == "frames" => Ok(LevelRequest::Frames),
        Some("score") => Err(Fail::Level(
            "--level score: a frame log has no score rows; use `research evaluate`".to_owned(),
        )),
        Some(_) => Err(Fail::Level(
            "--level frames: a session document carries no frame log; use `research reprocess`"
                .to_owned(),
        )),
    }
}

fn go(is_reprocess: bool, o: &Opts) -> Result<Evaluation, Fail> {
    let Some(file) = o.file.as_deref() else {
        return Err(Fail::Input(if is_reprocess {
            "research reprocess needs a LOG file".to_owned()
        } else {
            "research evaluate needs a SESSION.json file".to_owned()
        }));
    };
    let Some(baselines) = o.baselines.as_deref() else {
        return Err(Fail::Input("--baselines FILE is required".to_owned()));
    };
    let profile = load_profile(o.profile.as_deref())?;
    let baselines: NormativeBaselineSet = input(
        serde_json::from_value(read_json(baselines)?),
        o.baselines.as_deref().unwrap_or_default(),
    )?;

    if is_reprocess {
        let Some(catalog) = o.catalog.as_deref() else {
            return Err(Fail::Input("--catalog FILE is required".to_owned()));
        };
        let Some(age) = o.age_months else {
            return Err(Fail::Input("--age-months N is required".to_owned()));
        };
        let request = level(o, "frames")?;
        let catalog: StimulusCatalog = input(serde_json::from_value(read_json(catalog)?), catalog)?;
        let log = input(raw_log::parse(&read(file)?), file)?;
        if log.header.clips.is_empty() {
            return Err(Fail::Level(format!(
                "{file}: the log header has no clips, so it cannot be reprocessed at frames level"
            )));
        }
        let session = SessionInput {
            age_months: Some(age),
            scored_at_utc: o.scored_at.clone(),
            research: Some(ResearchInput {
                log,
                catalog,
                options: ReprocessOptions {
                    model: o.model,
                    glasses: o.glasses,
                    trace_hz: o.trace_hz,
                },
            }),
            ..SessionInput::default()
        };
        Ok(evaluate(&session, &profile, &baselines, request))
    } else {
        let request = level(o, "score")?;
        let doc = read_json(file)?;
        let raw = doc.get("rawPayload").unwrap_or(&doc);
        let reports = match raw.get("perVideo") {
            Some(v) => input(serde_json::from_value(v.clone()), "rawPayload.perVideo")?,
            None => Vec::new(),
        };
        let age = o
            .age_months
            .or_else(|| doc.pointer("/summary/ageMonths").and_then(Value::as_i64));
        let session = SessionInput {
            session_id: raw["sessionId"].as_str().unwrap_or("").to_owned(),
            age_months: age,
            reports,
            has_reports: raw.get("perVideo").is_some(),
            valid_frame_ratio: raw["validFrameRatio"].as_f64(),
            tracking: serde_json::from_value(raw["trackingQuality"].clone()).ok(),
            scored_at_utc: o.scored_at.clone(),
            research: None,
        };
        Ok(evaluate(&session, &profile, &baselines, request))
    }
}

fn print(e: &Evaluation) -> i32 {
    match serde_json::to_string_pretty(e) {
        Ok(s) => {
            println!("{s}");
            0
        }
        Err(err) => fail(1, &err.to_string()),
    }
}
