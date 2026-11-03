//! owner: WP-R.3
//!
//! `asd-cli schema export|check|validate|print` (CON-3, CON-4, GATE-1).
//!
//! The default directory is `docs/spec/schemas/asd/` of the checkout the binary was built
//! from, so `cargo run -p asd-cli -- schema check` works from any working directory.
//!
//! Besides the `asd-contract` documents, `export` and `check` handle
//! `case-package-v1.json`, the case package manifest of `asd-export` (WP-I.12, SHR-2).

use std::path::{Path, PathBuf};

use asd_contract::schema;
use asd_export::case_package;
use serde_json::Value;

const USAGE: &str = "\
usage:
  asd-cli schema export [--out DIR]
  asd-cli schema check [--dir DIR]
  asd-cli schema validate SCHEMA FILE...
  asd-cli schema print session|config-bundle|api|param-registry

DIR defaults to docs/spec/schemas/asd/ of this checkout. `export` and `check` include the
parameter registry (param-registry-v1.json, PRM-4).";

/// `docs/spec/schemas/asd` of the checkout this binary was built from.
pub fn default_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/spec/schemas/asd")
}

/// Runs `schema <sub> ...`; `args` are the arguments after `schema`.
pub fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("export") => match dir_flag(&args[1..], "--out") {
            Ok(dir) => export(&dir),
            Err(e) => usage_error(&e),
        },
        Some("check") => match dir_flag(&args[1..], "--dir") {
            Ok(dir) => check(&dir),
            Err(e) => usage_error(&e),
        },
        Some("validate") if args.len() >= 3 => validate(Path::new(&args[1]), &args[2..]),
        Some("print") if args.len() == 2 => print(&args[1]),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            0
        }
        _ => usage_error("missing or unknown schema subcommand"),
    }
}

fn usage_error(message: &str) -> i32 {
    eprintln!("error: {message}\n\n{USAGE}");
    2
}

fn dir_flag(args: &[String], flag: &str) -> Result<PathBuf, String> {
    match args {
        [] => Ok(default_dir()),
        [f, dir] if f == flag => Ok(PathBuf::from(dir)),
        _ => Err(format!("expected `{flag} DIR` or nothing")),
    }
}

/// `asd-contract` documents plus the case package manifest schema.
fn export_all(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = schema::export_all(dir)?;
    let path = dir.join(case_package::SCHEMA_FILE);
    std::fs::write(&path, schema::render(&case_package::manifest_schema()))?;
    paths.push(path);
    Ok(paths)
}

/// `schema::check` plus the case package manifest schema (compared as JSON values, so
/// formatting never counts as drift).
fn check_all(dir: &Path) -> Result<(), Vec<String>> {
    let mut drift: Vec<String> = match schema::check(dir) {
        Ok(()) => Vec::new(),
        Err(d) => d.iter().map(ToString::to_string).collect(),
    };
    let file = case_package::SCHEMA_FILE;
    match std::fs::read_to_string(dir.join(file)) {
        Err(_) => drift.push(format!("{file}: missing")),
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Err(e) => drift.push(format!("{file}: not JSON ({e})")),
            Ok(v) if v == case_package::manifest_schema() => {}
            Ok(_) => drift.push(format!("{file}: differs from the Rust contract types")),
        },
    }
    if drift.is_empty() {
        Ok(())
    } else {
        Err(drift)
    }
}

fn export(dir: &Path) -> i32 {
    match export_all(dir) {
        Ok(paths) => {
            for p in paths {
                println!("wrote {}", p.display());
            }
            0
        }
        Err(e) => {
            eprintln!("error: cannot write {}: {e}", dir.display());
            2
        }
    }
}

fn check(dir: &Path) -> i32 {
    match check_all(dir) {
        Ok(()) => {
            println!(
                "OK: schemas in {} match the Rust contract types",
                dir.display()
            );
            0
        }
        Err(drift) => {
            for d in &drift {
                eprintln!("DRIFT: {d}");
            }
            eprintln!(
                "run `cargo run --manifest-path rust/Cargo.toml -p asd-cli -- schema export` and commit the result"
            );
            1
        }
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: not JSON: {e}", path.display()))
}

fn validate(schema_path: &Path, files: &[String]) -> i32 {
    let schema_doc = match read_json(schema_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let mut failed = false;
    for file in files {
        let instance = match read_json(Path::new(file)) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        };
        match schema::validate(&schema_doc, &instance) {
            Ok(()) => println!("OK: {file}"),
            Err(errors) => {
                failed = true;
                eprintln!("INVALID: {file}");
                for e in errors {
                    eprintln!("  {e}");
                }
            }
        }
    }
    i32::from(failed)
}

fn print(which: &str) -> i32 {
    let value = match which {
        "session" => schema::session_v1_6(),
        "config-bundle" => schema::config_bundle_v1(),
        "api" => schema::api_v1(),
        "param-registry" => asd_contract::params::registry_json(),
        other => return usage_error(&format!("unknown schema `{other}`")),
    };
    print!("{}", schema::render(&value));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("asd-cli-schema-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json")
    }

    #[test]
    fn export_then_check_passes_and_an_edit_fails() {
        let dir = scratch("drift");
        let out = dir.to_str().unwrap();
        assert_eq!(run(&s(&["export", "--out", out])), 0);
        assert_eq!(run(&s(&["check", "--dir", out])), 0);

        // Acceptance 2: editing one field makes `schema check` fail.
        let path = dir.join(schema::SESSION_FILE);
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        v["properties"]["sessionId"]["type"] = Value::String("integer".into());
        std::fs::write(&path, schema::render(&v)).unwrap();
        assert_eq!(run(&s(&["check", "--dir", out])), 1);

        std::fs::remove_file(dir.join(schema::API_FILE)).unwrap();
        assert_eq!(run(&s(&["check", "--dir", out])), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn case_package_schema_is_exported_and_checked() {
        // WP-I.12: export writes it, check fails on an edit or a missing file.
        let dir = scratch("case-package");
        let out = dir.to_str().unwrap();
        assert_eq!(run(&s(&["export", "--out", out])), 0);
        let path = dir.join(case_package::SCHEMA_FILE);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["$id"], case_package::SCHEMA_ID);
        assert_eq!(run(&s(&["check", "--dir", out])), 0);

        let mut edited = v.clone();
        edited["properties"]["package_id"]["type"] = Value::String("integer".into());
        std::fs::write(&path, schema::render(&edited)).unwrap();
        assert_eq!(run(&s(&["check", "--dir", out])), 1);

        std::fs::remove_file(&path).unwrap();
        assert_eq!(run(&s(&["check", "--dir", out])), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn param_registry_is_exported_and_checked() {
        // PRM-4 (WP-X.0): export writes it, check fails on an edit.
        let dir = scratch("param-registry");
        let out = dir.to_str().unwrap();
        assert_eq!(run(&s(&["export", "--out", out])), 0);
        let path = dir.join(asd_contract::params::PARAM_REGISTRY_FILE);
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["$id"], asd_contract::params::PARAM_REGISTRY_ID);
        assert_eq!(run(&s(&["check", "--dir", out])), 0);
        v["params"][0]["kind"]["max"] = Value::from(2.0);
        std::fs::write(&path, schema::render(&v)).unwrap();
        assert_eq!(run(&s(&["check", "--dir", out])), 1);
        assert_eq!(run(&s(&["print", "param-registry"])), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn committed_schemas_are_current() {
        assert_eq!(run(&s(&["check"])), 0, "run `asd-cli schema export`");
    }

    #[test]
    fn validate_accepts_the_ios_fixture_and_rejects_it_without_session_id() {
        // Acceptance 3 / CON-4.
        let schema_path = default_dir().join(schema::SESSION_FILE);
        let fixture = fixture();
        assert_eq!(
            run(&s(&[
                "validate",
                schema_path.to_str().unwrap(),
                fixture.to_str().unwrap()
            ])),
            0
        );

        let dir = scratch("validate");
        std::fs::create_dir_all(&dir).unwrap();
        let mut v: Value =
            serde_json::from_str(&std::fs::read_to_string(&fixture).unwrap()).unwrap();
        v.as_object_mut().unwrap().remove("sessionId");
        let bad = dir.join("no-session-id.json");
        std::fs::write(&bad, v.to_string()).unwrap();
        assert_eq!(
            run(&s(&[
                "validate",
                schema_path.to_str().unwrap(),
                bad.to_str().unwrap()
            ])),
            1
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn usage_errors_exit_2() {
        assert_eq!(run(&s(&[])), 2);
        assert_eq!(run(&s(&["export", "--bogus", "x"])), 2);
        assert_eq!(run(&s(&["validate", "only-one-arg"])), 2);
        assert_eq!(run(&s(&["print", "nope"])), 2);
        assert_eq!(
            run(&s(&["validate", "/nonexistent/schema.json", "x.json"])),
            2
        );
    }
}
