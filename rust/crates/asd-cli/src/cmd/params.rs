//! owner: WP-X.0
//!
//! `asd-cli params list [--stage S] [--reprocess R]` and `asd-cli params check FILE` (PRM-4):
//! the parameter registry as a table, and the registry validation (`params::validate`) of a
//! scoring profile JSON file. Exit codes: 0 valid, 1 problems, 2 usage or I/O error.

use std::path::Path;

use asd_contract::params::{self, DocumentKind, ParamKind, ParamSpec};
use serde_json::Value;

const USAGE: &str = "\
usage:
  asd-cli params list [--stage STAGE] [--reprocess score|metrics|frames|capture_only]
  asd-cli params check FILE

STAGE: capture, preprocess, fixation, metrics, scoring, flag, norms, display.";

/// Runs `params <sub> ...`; `args` are the arguments after `params`.
pub fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("list") => match filters(&args[1..]) {
            Ok((stage, reprocess)) => {
                print!("{}", list(stage.as_deref(), reprocess.as_deref()));
                0
            }
            Err(e) => usage_error(&e),
        },
        Some("check") if args.len() == 2 => check(Path::new(&args[1])),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            0
        }
        _ => usage_error("missing or unknown params subcommand"),
    }
}

fn usage_error(message: &str) -> i32 {
    eprintln!("error: {message}\n\n{USAGE}");
    2
}

/// The wire name of a serde enum value (`Stage::Preprocess` -> `preprocess`).
fn wire(v: Result<Value, serde_json::Error>) -> String {
    match v {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

fn filters(args: &[String]) -> Result<(Option<String>, Option<String>), String> {
    let (mut stage, mut reprocess) = (None, None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} expects a value"))?
            .clone();
        match flag.as_str() {
            "--stage" => stage = Some(value),
            "--reprocess" => reprocess = Some(value),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    let known = |want: &Option<String>, of: &dyn Fn(&ParamSpec) -> String| {
        want.as_ref()
            .is_none_or(|w| params::registry().iter().any(|s| &of(s) == w))
    };
    if !known(&stage, &|s| wire(serde_json::to_value(s.stage))) {
        return Err(format!("unknown stage {}", stage.unwrap_or_default()));
    }
    if !known(&reprocess, &|s| wire(serde_json::to_value(s.reprocess))) {
        return Err(format!(
            "unknown reprocess level {}",
            reprocess.unwrap_or_default()
        ));
    }
    Ok((stage, reprocess))
}

fn range(kind: &ParamKind) -> String {
    match kind {
        ParamKind::Number {
            min,
            max,
            step,
            unit,
        } => format!(
            "number [{min}, {max}] step {step} {}",
            wire(serde_json::to_value(unit))
        ),
        ParamKind::Integer { min, max, unit } => format!(
            "integer [{min}, {max}] {}",
            wire(serde_json::to_value(unit))
        ),
        ParamKind::Bool => "bool".into(),
        ParamKind::Enum { values } => format!("enum {}", values.join("|")),
        ParamKind::PointList {
            min_points,
            max_points,
        } => format!("points {min_points}..{max_points}"),
        ParamKind::AgeBands {
            min_bands,
            max_bands,
            min_months,
            max_months,
        } => format!("age bands {min_bands}..{max_bands}, months {min_months}..{max_months}"),
    }
}

/// The registry rows as a tab-separated table (id, stage, reprocess, kind and range,
/// default, owner gate), header first.
pub fn list(stage: Option<&str>, reprocess: Option<&str>) -> String {
    let mut out = String::from("id\tstage\treprocess\tkind\tdefault\towner_gate\n");
    for s in params::registry() {
        if stage.is_some_and(|w| wire(serde_json::to_value(s.stage)) != w)
            || reprocess.is_some_and(|w| wire(serde_json::to_value(s.reprocess)) != w)
        {
            continue;
        }
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            s.id,
            wire(serde_json::to_value(s.stage)),
            wire(serde_json::to_value(s.reprocess)),
            range(&s.kind),
            s.default,
            s.owner_gate.as_deref().unwrap_or("-"),
        ));
    }
    out
}

fn check(path: &Path) -> i32 {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {}: {e}", path.display());
            return 2;
        }
    };
    let body: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {}: not JSON: {e}", path.display());
            return 2;
        }
    };
    let problems = params::validate(DocumentKind::ScoringProfile, &body);
    if problems.is_empty() {
        println!("OK: {}", path.display());
        return 0;
    }
    eprintln!("INVALID: {}", path.display());
    for p in problems {
        eprintln!("  {}", p.message);
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use asd_contract::config::ScoringProfile;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn scratch(name: &str, body: &Value) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("asd-cli-params-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, body.to_string()).unwrap();
        path
    }

    #[test]
    fn list_filters_by_stage_and_reprocess() {
        let all = list(None, None);
        assert_eq!(all.lines().count(), params::registry().len() + 1);
        let frames = list(None, Some("frames"));
        assert!(frames.contains("metrics.ivtVelocityUnitsPerSec\tfixation\tframes"));
        assert!(!frames.contains("flag.greenFloor"));
        let flag = list(Some("flag"), None);
        assert_eq!(flag.lines().count(), 5, "{flag}");
        assert!(all.contains("metrics.ivtUnit\tfixation\tframes\tenum norm|deg\t\"norm\"\tH-G2"));
        assert_eq!(run(&s(&["list", "--reprocess", "frames"])), 0);
        assert_eq!(run(&s(&["list", "--stage", "nope"])), 2);
        assert_eq!(run(&s(&["list", "--stage"])), 2);
    }

    #[test]
    fn check_exits_0_valid_1_invalid_2_unreadable() {
        let good = serde_json::to_value(ScoringProfile::default()).unwrap();
        let ok = scratch("good.json", &good);
        assert_eq!(run(&s(&["check", ok.to_str().unwrap()])), 0);

        let mut bad = good.clone();
        bad["metrics"]["ivtVelocityUnitsPerSec"] = Value::from(60.0);
        let bad_path = scratch("bad.json", &bad);
        assert_eq!(run(&s(&["check", bad_path.to_str().unwrap()])), 1);
        assert_eq!(
            params::validate_messages(DocumentKind::ScoringProfile, &bad),
            ["metrics.ivtVelocityUnitsPerSec must be in [0.5, 50]"]
        );

        assert_eq!(run(&s(&["check", "/nonexistent/profile.json"])), 2);
        assert_eq!(run(&s(&[])), 2);
        assert_eq!(run(&s(&["check"])), 2);
    }
}
