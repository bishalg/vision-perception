//! owner: WP-R.4
//!
//! `asd-cli synth --seed N --count M --out dir|-` (GATE-2). Replaces `python -m tools.asd_synth`.
//! `--out -` writes one compact JSON document per line to stdout.
//! Requires `asd-core` feature `synth` (set it on the `asd-core` dependency in `Cargo.toml`).
//!
//! Every payload is checked against the CON-2 contract before it is written: it must decode
//! as a typed `SessionIngestRequest` and validate against the exported session-v1.6 schema.
// Dispatch is wired by WP-R.3 (`main.rs`); until then `run` is unreferenced.
#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

use asd_contract::schema::validate_session;
use asd_contract::session_v1_6::SessionIngestRequest;
use asd_core::synth::{generate_session, Profile};
use serde_json::Value;

/// One generated session, checked against the typed contract and the session schema.
fn checked_session(seed: u64, index: u64, profile: Option<Profile>) -> Result<Value, String> {
    let s = generate_session(seed, index, profile);
    serde_json::from_value::<SessionIngestRequest>(s.clone())
        .map_err(|e| format!("session {index}: not a SessionIngestRequest: {e}"))?;
    validate_session(&s).map_err(|e| format!("session {index}: schema: {}", e.join("; ")))?;
    Ok(s)
}

/// Runs the `synth` subcommand; `args` are the arguments after `synth`. Returns the exit code.
pub fn run(args: &[String]) -> i32 {
    let (mut seed, mut count, mut out, mut profile) =
        (0u64, 20u64, None::<String>, None::<Profile>);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let Some(value) = it.next() else {
            eprintln!("error: {flag} needs a value");
            return 2;
        };
        let ok = match flag.as_str() {
            "--seed" => value.parse().map(|v| seed = v).is_ok(),
            "--count" | "-n" => value.parse().map(|v| count = v).is_ok(),
            "--out" => {
                out = Some(value.clone());
                true
            }
            "--profile" => match value.parse() {
                Ok(p) => {
                    profile = Some(p);
                    true
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            },
            _ => {
                eprintln!("error: unexpected argument {flag}");
                return 2;
            }
        };
        if !ok {
            eprintln!("error: invalid value {value:?} for {flag}");
            return 2;
        }
    }
    let Some(out) = out else {
        eprintln!("usage: asd-cli synth --seed N --count M --out DIR|- [--profile P]");
        return 2;
    };

    if out == "-" {
        for i in 0..count {
            match checked_session(seed, i, profile) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    return 1;
                }
            }
        }
        return 0;
    }
    let dir = PathBuf::from(&out);
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("error: cannot create {out}: {e}");
        return 1;
    }
    for i in 0..count {
        let s = match checked_session(seed, i, profile) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {e}");
                return 1;
            }
        };
        let id = s["sessionId"].as_str().expect("sessionId").to_string();
        let text = format!("{s:#}\n");
        if let Err(e) = fs::write(dir.join(format!("{id}.json")), text) {
            eprintln!("error: cannot write {id}.json: {e}");
            return 1;
        }
    }
    eprintln!("Wrote {count} SYNTHETIC sessions to {out}");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Port of `test_cli_writes_valid_files`: count, determinism across two runs.
    #[test]
    fn writes_deterministic_files() {
        let base = std::env::temp_dir().join(format!("asd-cli-synth-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (a, b) = (base.join("a"), base.join("b"));
        for d in [&a, &b] {
            assert_eq!(
                run(&args(&[
                    "--count",
                    "12",
                    "--seed",
                    "9",
                    "--out",
                    d.to_str().unwrap()
                ])),
                0
            );
        }
        let mut names: Vec<_> = fs::read_dir(&a)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names.len(), 12);
        for n in &names {
            let (x, y) = (
                fs::read_to_string(a.join(n)).unwrap(),
                fs::read_to_string(b.join(n)).unwrap(),
            );
            assert_eq!(x, y);
            assert!(x.contains("\"captureSource\": \"synthetic\""));
        }
        let _ = fs::remove_dir_all(&base);
    }

    /// CON-2: every profile decodes as the typed request and passes the full schema validator.
    #[test]
    fn every_profile_satisfies_the_contract() {
        for profile in asd_core::synth::PROFILES {
            for i in 0..10 {
                checked_session(4, i, Some(profile)).unwrap();
            }
        }
        for i in 0..40 {
            checked_session(13, i, None).unwrap();
        }
    }

    #[test]
    fn rejects_bad_args() {
        assert_eq!(run(&args(&["--seed", "x", "--out", "-"])), 2);
        assert_eq!(run(&args(&["--profile", "bogus", "--out", "-"])), 2);
        assert_eq!(run(&args(&["--count", "1"])), 2);
    }
}
