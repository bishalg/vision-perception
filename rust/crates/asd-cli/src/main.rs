//! owner: WP-R.3
//!
//! `asd-cli` entry point. Each subcommand module exposes `run(args) -> i32` taking the
//! arguments after its name. Exit codes: 0 success, 1 check or validation failure,
//! 2 usage or I/O error.

mod cmd;

use std::process::ExitCode;

const USAGE: &str = "\
usage: asd-cli <command> [args]

commands:
  schema export [--out DIR]           write session-v1.6.json, config-bundle-v1.json, api-v1.json
  schema check [--dir DIR]            fail when the committed schemas differ from the Rust types
  schema validate SCHEMA FILE...      validate JSON documents against a schema file
  schema print session|config-bundle|api
  bundle keygen [--out FILE]          new Ed25519 bundle signing key (PKCS#8 PEM)
  bundle compose ...                  unsigned bundle from catalog, baselines and profile
  bundle sign --in FILE --key FILE    sign a bundle (CON-6)
  bundle verify --in FILE --public-key B64
  synth --seed N --count M --out DIR|-
  norms aggregate --input DIR --output FILE [--min-n 12]
  gaze synth --scenario NAME|all --seed N --out FILE|DIR|-
  gaze replay FILE [--check] [--model KIND]
  gaze bench-report --logs DIR [--models all|KIND,...]

`asd-cli <command> --help` shows the options of one command.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let code = match args.first().map(String::as_str) {
        Some("schema") => cmd::schema::run(rest),
        Some("bundle") => cmd::bundle::run(rest),
        Some("synth") => cmd::synth::run(rest),
        Some("norms") => cmd::norms::run(rest),
        Some("gaze") => cmd::gaze::run(rest),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            0
        }
        Some(other) => {
            eprintln!("error: unknown command `{other}`\n\n{USAGE}");
            2
        }
        None => {
            eprintln!("{USAGE}");
            2
        }
    };
    ExitCode::from(u8::try_from(code).unwrap_or(2))
}
