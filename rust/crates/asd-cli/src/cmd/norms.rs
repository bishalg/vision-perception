//! owner: WP-R.4
//!
//! `asd-cli norms aggregate --input dir --output file [--min-n 12]` (GATE-3). Replaces
//! `tools/asd_norm_aggregator.py`. File walking lives here; the maths is `asd_core::norms`.
// Dispatch is wired by WP-R.3 (`main.rs`); until then `run` is unreferenced.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use asd_core::norms::{
    aggregate, default_age_brackets, extract_observations, load_session_records, qualifying_cells,
    AggregateParams, Buckets, DEFAULT_MIN_N,
};

/// Recursively lists files whose name contains `.json` (JSON or JSONL), sorted.
fn session_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            session_files(&path, out)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(".json"))
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}

/// Runs the `norms` subcommand; `args` are the arguments after `norms`. Returns the exit code.
pub fn run(args: &[String]) -> i32 {
    let mut it = args.iter();
    if it.next().map(String::as_str) != Some("aggregate") {
        eprintln!("usage: asd-cli norms aggregate --input DIR [--output FILE] [--min-n N]");
        return 2;
    }
    let (mut input, mut output, mut min_n) = (
        None,
        "candidate_normative_baselines.json".to_string(),
        DEFAULT_MIN_N,
    );
    while let Some(flag) = it.next() {
        let value = it.next();
        match (flag.as_str(), value) {
            ("--input", Some(v)) => input = Some(PathBuf::from(v)),
            ("--output", Some(v)) => output = v.clone(),
            ("--min-n", Some(v)) => match v.parse() {
                Ok(n) => min_n = n,
                Err(_) => {
                    eprintln!("error: --min-n expects a non-negative integer");
                    return 2;
                }
            },
            _ => {
                eprintln!("error: unexpected argument {flag}");
                return 2;
            }
        }
    }
    let Some(input) = input.filter(|p| p.is_dir()) else {
        eprintln!("error: input directory not found (--input DIR)");
        return 2;
    };

    let params = AggregateParams {
        min_n,
        brackets: default_age_brackets(),
        ..Default::default()
    };
    let mut files = Vec::new();
    if let Err(e) = session_files(&input, &mut files) {
        eprintln!("error: cannot read {}: {e}", input.display());
        return 1;
    }
    let mut records = Vec::new();
    for f in &files {
        // Unreadable files are skipped, as the Python tool did.
        if let Ok(text) = fs::read_to_string(f) {
            records.extend(load_session_records(&text));
        }
    }
    let mut buckets = Buckets::new();
    extract_observations(&records, &params, &mut buckets);
    let observations: usize = buckets.values().map(Vec::len).sum();
    let candidate = aggregate(&records, &params);

    if let Err(e) = fs::write(&output, candidate.to_json_pretty()) {
        eprintln!("error: cannot write {output}: {e}");
        return 1;
    }
    println!(
        "aggregated {observations} observations into {} qualifying cells (min n={min_n}) -> {output}",
        qualifying_cells(&candidate)
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("asd-cli-norms-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn record(i: usize) -> String {
        format!(
            r#"{{"type":"per_video_score","score":{{"ageMonths":24,"videos":[{{"stimulusId":"stim_05_geopref_split_10s","rows":[{{"biomarker":"social_dwell_ratio","observed":{},"imputed":false}}]}}]}}}}"#,
            0.60 + (i % 5) as f64 * 0.02
        )
    }

    /// Port of `test_cli_end_to_end`.
    #[test]
    fn cli_end_to_end() {
        let dir = scratch("e2e");
        let lines: Vec<String> = (0..12).map(record).collect();
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("nested/seed.jsonl"), lines.join("\n") + "\n").unwrap();
        let out = dir.join("candidate.json");
        let args: Vec<String> = [
            "aggregate",
            "--input",
            dir.to_str().unwrap(),
            "--output",
            out.to_str().unwrap(),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(run(&args), 0);
        let c = fs::read_to_string(&out).unwrap();
        assert!(c.contains("\"schemaVersion\": \"1.0.0-candidate\""));
        assert!(c.contains("\"status\": \"candidate-collected\""));
        assert!(
            c.contains("\"expected\": 0.63"),
            "12 rows in one cell must qualify"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_input_dir_exits_2() {
        let args: Vec<String> = ["aggregate", "--input", "/nonexistent/asd"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(run(&args), 2);
    }
}
