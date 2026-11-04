//! owner: WP-L.0
//!
//! `asd-cli i18n check` and `fmt` through the real binary (acceptance 2): clean on the
//! committed catalogue and console module; a hand-edited `domain.ts` fails naming the file.

use std::path::PathBuf;
use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_asd-cli"))
        .args(args)
        .output()
        .expect("run asd-cli")
}

fn committed_console() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../apps/curv_console/src/messages/domain.ts"
    ))
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("asd-cli-i18n-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn committed_catalogue_and_console_module_are_clean() {
    let o = cli(&["i18n", "check"]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn a_hand_edited_console_module_fails_naming_the_file() {
    let dir = scratch("check");
    let edited = dir.join("domain.ts");
    let text = std::fs::read_to_string(committed_console()).unwrap();
    let changed = text.replacen("\"Observation visit\"", "\"Screening visit\"", 1);
    assert_ne!(changed, text);
    std::fs::write(&edited, changed).unwrap();
    let o = cli(&["i18n", "check", "--console", edited.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("DRIFT") && err.contains("domain.ts"), "{err}");

    std::fs::write(&edited, text).unwrap();
    let o = cli(&["i18n", "check", "--console", edited.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_exports_are_checked_when_given() {
    let dir = scratch("native");
    let xc = dir.join("Domain.xcstrings");
    let android = dir.join("res");
    for (target, out) in [("xcstrings", &xc), ("android", &android)] {
        let o = cli(&[
            "i18n",
            "export",
            "--target",
            target,
            "--out",
            out.to_str().unwrap(),
        ]);
        assert!(o.status.success());
    }
    let args = [
        "i18n",
        "check",
        "--xcstrings",
        xc.to_str().unwrap(),
        "--android",
        android.to_str().unwrap(),
    ];
    assert_eq!(cli(&args).status.code(), Some(0));
    std::fs::write(
        android.join("values-ne/domain_strings.xml"),
        "<resources/>\n",
    )
    .unwrap();
    assert_eq!(cli(&args).status.code(), Some(1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fmt_sorts_and_formats_a_catalogue_directory() {
    let dir = scratch("fmt");
    std::fs::write(dir.join("domain.en.json"), "{\"b.x\":\"B\",\"a.x\":\"A\"}").unwrap();
    std::fs::write(dir.join("domain.ne.json"), "{\n  \"a.x\": \"क\"\n}\n").unwrap();
    let o = cli(&["i18n", "fmt", "--dir", dir.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        std::fs::read_to_string(dir.join("domain.en.json")).unwrap(),
        "{\n  \"a.x\": \"A\",\n  \"b.x\": \"B\"\n}\n"
    );
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("unchanged") && out.contains("wrote"), "{out}");
    std::fs::write(dir.join("domain.en.json"), "not json").unwrap();
    assert_eq!(
        cli(&["i18n", "fmt", "--dir", dir.to_str().unwrap()])
            .status
            .code(),
        Some(2)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
