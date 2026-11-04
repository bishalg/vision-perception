//! owner: WP-L.0
//!
//! `asd-cli i18n lint` (I18N-8, acceptance 4) through the real binary on the fixture tree
//! `tests/fixtures/i18n/lint` (copied to a scratch directory per test): one Swift and one
//! Kotlin finding, the exemptions, `--catalog`, the baseline (covered -> 0, new literal -> 1,
//! stale entry -> 1) and the `--update-baseline` round trip.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/i18n/lint");

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_tree(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

/// A scratch copy of the fixture tree.
fn tree(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("asd-cli-i18n-lint-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_tree(Path::new(FIXTURES), &dir);
    dir
}

fn lint(dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_asd-cli"))
        .current_dir(dir)
        .args([
            "i18n",
            "lint",
            "--swift",
            "swift",
            "--kotlin",
            "kotlin",
            "--baseline",
            "baseline.txt",
        ])
        .args(extra)
        .output()
        .expect("run asd-cli")
}

fn out(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn entries(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("baseline.txt"))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn findings_exemptions_and_catalog() {
    let dir = tree("findings");
    std::fs::write(dir.join("baseline.txt"), "").unwrap();
    let o = lint(&dir, &["--format", "json"]);
    assert_eq!(o.status.code(), Some(1), "{}", out(&o));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let new: Vec<(String, String, String)> = v["new"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["path"].as_str().unwrap().to_owned(),
                f["rule"].as_str().unwrap().to_owned(),
                f["literal"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    // `home.greeting` is a finding until the String Catalog says it is a localised key.
    assert_eq!(
        new,
        [
            (
                "swift/Views/HomeView.swift".into(),
                "swift-text".into(),
                "Hello".into()
            ),
            (
                "swift/Views/HomeView.swift".into(),
                "swift-text".into(),
                "home.greeting".into()
            ),
            (
                "kotlin/MainActivity.kt".into(),
                "kotlin-text".into(),
                "Start".into()
            ),
        ]
    );
    let o = lint(
        &dir,
        &["--catalog", "Localizable.xcstrings", "--format", "json"],
    );
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["new"].as_array().unwrap().len(), 2, "{}", out(&o));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn baseline_covers_then_new_and_stale_fail() {
    let dir = tree("baseline");
    let catalog = ["--catalog", "Localizable.xcstrings"];

    // No baseline file yet: an I/O error, then --update-baseline writes it.
    assert_eq!(lint(&dir, &catalog).status.code(), Some(2));
    let o = lint(&dir, &[&catalog[..], &["--update-baseline"]].concat());
    assert_eq!(o.status.code(), Some(0), "{}", out(&o));
    assert_eq!(entries(&dir).len(), 2);
    let o = lint(&dir, &catalog);
    assert_eq!(o.status.code(), Some(0), "{}", out(&o));

    // A new literal fails with path:line:col rule "literal".
    let view = dir.join("swift/Views/HomeView.swift");
    let original = std::fs::read_to_string(&view).unwrap();
    let added = original.replace(
        "            Text(\"·\")\n",
        "            Text(\"·\")\n            Text(\"New screen\")\n",
    );
    std::fs::write(&view, &added).unwrap();
    let o = lint(&dir, &catalog);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        out(&o).contains("swift/Views/HomeView.swift:16:18 swift-text \"New screen\""),
        "{}",
        out(&o)
    );
    // ... and passes once the line says why it is fine.
    std::fs::write(
        &view,
        added.replace(
            "Text(\"New screen\")",
            "Text(\"New screen\") // i18n-ok: brand name",
        ),
    )
    .unwrap();
    assert_eq!(lint(&dir, &catalog).status.code(), Some(0));

    // Removing a baselined literal leaves a stale entry; --update-baseline drops it.
    std::fs::write(&view, original.replace("            Text(\"Hello\")\n", "")).unwrap();
    let o = lint(&dir, &catalog);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        out(&o).contains("baseline has 1 stale entry"),
        "{}",
        out(&o)
    );
    assert_eq!(
        lint(&dir, &[&catalog[..], &["--update-baseline"]].concat())
            .status
            .code(),
        Some(0)
    );
    assert_eq!(entries(&dir).len(), 1);
    assert_eq!(lint(&dir, &catalog).status.code(), Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn baseline_lines_have_no_line_numbers() {
    let dir = tree("format");
    assert_eq!(lint(&dir, &["--update-baseline"]).status.code(), Some(0));
    for line in entries(&dir) {
        let parts: Vec<&str> = line.split('\t').collect();
        assert_eq!(parts.len(), 4, "{line}");
        assert_eq!(parts[2].len(), 12, "{line}");
        assert!(parts[3].parse::<usize>().is_ok(), "{line}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exclude_and_usage_errors() {
    let dir = tree("exclude");
    std::fs::write(dir.join("baseline.txt"), "").unwrap();
    let o = lint(&dir, &["--exclude", "swift/**", "--exclude", "kotlin/*.kt"]);
    assert_eq!(o.status.code(), Some(0), "{}", out(&o));
    assert_eq!(lint(&dir, &["--format", "xml"]).status.code(), Some(2));
    std::fs::write(dir.join("baseline.txt"), "only-one-field\n").unwrap();
    assert_eq!(lint(&dir, &[]).status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}
