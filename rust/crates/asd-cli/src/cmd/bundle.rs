//! owner: WP-R.3
//!
//! `asd-cli bundle keygen|compose|sign|verify` (CON-5, CON-6): the developer and CI side of
//! config bundles. Signing and verification are `asd_contract::signing`, the same code the
//! backend and the apps use.

use std::collections::BTreeMap;
use std::path::Path;

use asd_contract::config::{ConfigBundle, ScoringProfile};
use asd_contract::signing;
use serde_json::Value;

const USAGE: &str = "\
usage:
  asd-cli bundle keygen [--out FILE]
      New Ed25519 signing key as PKCS#8 PEM (FILE, or stdout). Prints the public key
      (base64url) and the default key id.
  asd-cli bundle compose --catalog FILE --baselines FILE --version V --signed-at RFC3339
                         [--profile FILE] [--flag NAME=true|false]... [--out FILE|-]
      Unsigned bundle; the profile defaults to the built-in ScoringProfile.
  asd-cli bundle sign --in FILE --key FILE [--key-id ID] [--out FILE|-]
      Sets key_id (default: derived from the key) and signature.
  asd-cli bundle verify --in FILE --public-key B64";

/// Runs `bundle <sub> ...`; `args` are the arguments after `bundle`.
pub fn run(args: &[String]) -> i32 {
    let Some(sub) = args.first() else {
        return usage_error("missing bundle subcommand");
    };
    if sub == "-h" || sub == "--help" {
        println!("{USAGE}");
        return 0;
    }
    let flags = match parse_flags(&args[1..]) {
        Ok(f) => f,
        Err(e) => return usage_error(&e),
    };
    let result = match sub.as_str() {
        "keygen" => keygen(&flags),
        "compose" => compose(&flags),
        "sign" => sign(&flags),
        "verify" => verify(&flags),
        other => return usage_error(&format!("unknown bundle subcommand `{other}`")),
    };
    match result {
        Ok(code) => code,
        Err(Failure::Usage(e)) => usage_error(&e),
        Err(Failure::Io(e)) => {
            eprintln!("error: {e}");
            2
        }
    }
}

enum Failure {
    Usage(String),
    Io(String),
}

type Flags = BTreeMap<String, Vec<String>>;

fn usage_error(message: &str) -> i32 {
    eprintln!("error: {message}\n\n{USAGE}");
    2
}

fn parse_flags(args: &[String]) -> Result<Flags, String> {
    let mut flags = Flags::new();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if !flag.starts_with("--") {
            return Err(format!("unexpected argument `{flag}`"));
        }
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        flags.entry(flag.clone()).or_default().push(value.clone());
    }
    Ok(flags)
}

fn one<'a>(flags: &'a Flags, name: &str) -> Result<&'a str, Failure> {
    optional(flags, name)?.ok_or_else(|| Failure::Usage(format!("{name} is required")))
}

fn optional<'a>(flags: &'a Flags, name: &str) -> Result<Option<&'a str>, Failure> {
    match flags.get(name).map(Vec::as_slice) {
        None => Ok(None),
        Some([v]) => Ok(Some(v.as_str())),
        Some(_) => Err(Failure::Usage(format!("{name} given more than once"))),
    }
}

fn read(path: &str) -> Result<String, Failure> {
    std::fs::read_to_string(path).map_err(|e| Failure::Io(format!("{path}: {e}")))
}

/// Reads and decodes a JSON file (a macro so asd-cli needs no direct `serde` dependency).
macro_rules! read_json {
    ($path:expr) => {{
        let path: &str = $path;
        serde_json::from_str(&read(path)?).map_err(|e| Failure::Io(format!("{path}: {e}")))
    }};
}

fn write_out(out: Option<&str>, text: &str) -> Result<(), Failure> {
    match out {
        None | Some("-") => {
            print!("{text}");
            Ok(())
        }
        Some(path) => {
            std::fs::write(path, text).map_err(|e| Failure::Io(format!("{path}: {e}")))?;
            eprintln!("wrote {path}");
            Ok(())
        }
    }
}

fn pretty(value: &Value) -> String {
    let mut s = serde_json::to_string_pretty(value).expect("JSON value");
    s.push('\n');
    s
}

fn keygen(flags: &Flags) -> Result<i32, Failure> {
    let out = optional(flags, "--out")?;
    if let Some(path) = out {
        if Path::new(path).exists() {
            return Err(Failure::Io(format!(
                "{path} exists; refusing to overwrite a key"
            )));
        }
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| Failure::Io(format!("no OS randomness: {e}")))?;
    let key = signing::signing_key_from_seed(seed);
    let pem = signing::signing_key_to_pkcs8_pem(&key).map_err(|e| Failure::Io(e.to_string()))?;
    let vk = key.verifying_key();
    let info = format!(
        "public_key: {}\nkey_id: {}",
        signing::encode_public_key(&vk),
        signing::key_id(&vk)
    );
    match out {
        Some(path) => {
            write_private(path, &pem)?;
            println!("{info}");
        }
        None => {
            print!("{pem}");
            eprintln!("{info}");
        }
    }
    Ok(0)
}

#[cfg(unix)]
fn write_private(path: &str, text: &str) -> Result<(), Failure> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| Failure::Io(format!("{path}: {e}")))?;
    file.write_all(text.as_bytes())
        .map_err(|e| Failure::Io(format!("{path}: {e}")))
}

#[cfg(not(unix))]
fn write_private(path: &str, text: &str) -> Result<(), Failure> {
    std::fs::write(path, text).map_err(|e| Failure::Io(format!("{path}: {e}")))
}

fn compose(flags: &Flags) -> Result<i32, Failure> {
    let profile = match optional(flags, "--profile")? {
        Some(path) => read_json!(path)?,
        None => ScoringProfile::default(),
    };
    let mut feature_flags = BTreeMap::new();
    for raw in flags.get("--flag").into_iter().flatten() {
        let (name, value) = raw
            .split_once('=')
            .ok_or_else(|| Failure::Usage(format!("--flag `{raw}`: expected NAME=true|false")))?;
        let value: bool = value
            .parse()
            .map_err(|_| Failure::Usage(format!("--flag `{raw}`: expected true or false")))?;
        feature_flags.insert(name.to_string(), value);
    }
    let bundle = ConfigBundle {
        bundle_version: one(flags, "--version")?.to_string(),
        key_id: String::new(),
        catalog: read_json!(one(flags, "--catalog")?)?,
        baselines: read_json!(one(flags, "--baselines")?)?,
        scoring_profile: profile,
        feature_flags,
        signed_at: one(flags, "--signed-at")?.to_string(),
        signature: String::new(),
    };
    let value = serde_json::to_value(&bundle).expect("bundle serialises");
    write_out(optional(flags, "--out")?, &pretty(&value))?;
    Ok(0)
}

fn sign(flags: &Flags) -> Result<i32, Failure> {
    let key = signing::parse_signing_key(&read(one(flags, "--key")?)?)
        .map_err(|e| Failure::Io(format!("--key: {e}")))?;
    let input = one(flags, "--in")?;
    let mut value: Value = read_json!(input)?;
    if !value.is_object() {
        return Err(Failure::Io(format!("{input}: not a JSON object")));
    }
    let key_id = match optional(flags, "--key-id")? {
        Some(id) => id.to_string(),
        None => signing::key_id(&key.verifying_key()),
    };
    value["key_id"] = Value::String(key_id);
    let signed = signing::sign_bundle_json(&value.to_string(), &key)
        .map_err(|e| Failure::Io(format!("{input}: {e}")))?;
    let signed: Value = serde_json::from_str(&signed).expect("signer returns JSON");
    write_out(optional(flags, "--out")?, &pretty(&signed))?;
    Ok(0)
}

fn verify(flags: &Flags) -> Result<i32, Failure> {
    let input = one(flags, "--in")?;
    let text = read(input)?;
    match signing::verify_bundle_b64(&text, one(flags, "--public-key")?) {
        Ok(bundle) => {
            println!(
                "OK: bundle {} signed by {}",
                bundle.bundle_version, bundle.key_id
            );
            Ok(0)
        }
        Err(e) => {
            eprintln!("INVALID: {input}: {e}");
            Ok(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn resources() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources")
    }

    #[test]
    fn keygen_compose_sign_verify_and_tamper() {
        let dir = std::env::temp_dir().join(format!("asd-cli-bundle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = |name: &str| dir.join(name).to_str().unwrap().to_string();

        assert_eq!(run(&s(&["keygen", "--out", &p("key.pem")])), 0);
        assert_eq!(
            run(&s(&["keygen", "--out", &p("key.pem")])),
            2,
            "never overwrites"
        );
        let key =
            signing::parse_signing_key(&std::fs::read_to_string(p("key.pem")).unwrap()).unwrap();
        let public = signing::encode_public_key(&key.verifying_key());

        let catalog = resources().join("stimulus_catalog.json");
        let baselines = resources().join("normative_baselines.json");
        assert_eq!(
            run(&s(&[
                "compose",
                "--catalog",
                catalog.to_str().unwrap(),
                "--baselines",
                baselines.to_str().unwrap(),
                "--version",
                "2026.10.30-1",
                "--signed-at",
                "2026-10-30T12:00:00.000Z",
                "--flag",
                "norm_collection=false",
                "--out",
                &p("unsigned.json"),
            ])),
            0
        );
        assert_eq!(
            run(&s(&[
                "sign",
                "--in",
                &p("unsigned.json"),
                "--key",
                &p("key.pem"),
                "--out",
                &p("signed.json"),
            ])),
            0
        );
        assert_eq!(
            run(&s(&[
                "verify",
                "--in",
                &p("signed.json"),
                "--public-key",
                &public
            ])),
            0
        );
        let signed: Value =
            serde_json::from_str(&std::fs::read_to_string(p("signed.json")).unwrap()).unwrap();
        assert_eq!(
            signed["key_id"],
            Value::String(signing::key_id(&key.verifying_key()))
        );
        asd_contract::schema::validate(&asd_contract::schema::config_bundle_v1(), &signed)
            .expect("signed bundle matches config-bundle-v1.json");

        let mut tampered = signed.clone();
        tampered["scoring_profile"]["flag"]["greenFloor"] = Value::from(0.5);
        std::fs::write(p("tampered.json"), tampered.to_string()).unwrap();
        assert_eq!(
            run(&s(&[
                "verify",
                "--in",
                &p("tampered.json"),
                "--public-key",
                &public
            ])),
            1
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn usage_errors_exit_2() {
        assert_eq!(run(&s(&[])), 2);
        assert_eq!(run(&s(&["frobnicate"])), 2);
        assert_eq!(run(&s(&["sign", "--in"])), 2);
        assert_eq!(run(&s(&["verify", "--in", "x.json"])), 2);
        assert_eq!(run(&s(&["compose", "--flag", "x=maybe"])), 2);
    }
}
