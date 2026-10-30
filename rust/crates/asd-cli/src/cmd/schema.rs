//! owner: WP-R.3
//!
//! `asd-cli schema export|check|validate|print` (CON-3, CON-4, GATE-1).
//!
//! The default directory is `docs/spec/schemas/asd/` of the checkout the binary was built
//! from, so `cargo run -p asd-cli -- schema check` works from any working directory.

use std::path::{Path, PathBuf};

use asd_contract::schema;
use serde_json::Value;

const USAGE: &str = "\
usage:
  asd-cli schema export [--out DIR]
  asd-cli schema check [--dir DIR]
  asd-cli schema validate SCHEMA FILE...
  asd-cli schema print session|config-bundle|api

DIR defaults to docs/spec/schemas/asd/ of this checkout.";

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

fn export(dir: &Path) -> i32 {
    match schema::export_all(dir) {
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
    match schema::check(dir) {
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
