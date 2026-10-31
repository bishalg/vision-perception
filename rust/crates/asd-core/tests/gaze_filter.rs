//! owner: WP-G.3
//!
//! PIPE-3: the 1-Euro filter is a port of `OneEuroFilter.swift` with identical output on that
//! file's test cases. Expected values in `tests/golden/gaze/filter/one_euro_swift_v1.json` were
//! produced by the unchanged Swift source (see the file's `derivation`).

use std::path::PathBuf;

use asd_core::gaze::filter::{OneEuro, OneEuro2D};
use asd_core::types::tracking::FilterParams;
use serde_json::Value;

const EPS: f64 = 1e-12;

fn golden() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/gaze/filter/one_euro_swift_v1.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(doc["schema"], "asd-golden/1");
    assert_eq!(doc["function"], "filter::OneEuro");
    doc
}

fn params(v: &Value) -> FilterParams {
    serde_json::from_value(v.clone()).expect("filter params")
}

fn f(v: &Value) -> f64 {
    v.as_f64().expect("number")
}

#[test]
fn one_euro_matches_swift_on_every_golden_vector() {
    let doc = golden();
    let vectors = doc["vectors"].as_array().unwrap();
    assert!(vectors.len() >= 6);
    for v in vectors {
        let name = v["name"].as_str().unwrap();
        let input = &v["input"];
        let p = params(&input["params"]);
        let expected = v["expected"].as_array().unwrap();
        let mut out = 0;
        match input["dims"].as_u64().unwrap() {
            1 => {
                let mut flt = OneEuro::new(p);
                for step in input["steps"].as_array().unwrap() {
                    if step["op"] == "reset" {
                        flt.reset();
                        continue;
                    }
                    let got = flt.filter(f(&step["value"]), f(&step["t"]));
                    let want = f(&expected[out]);
                    assert!((got - want).abs() <= EPS, "{name}[{out}]: {got} != {want}");
                    out += 1;
                }
            }
            2 => {
                let mut flt = OneEuro2D::new(p);
                for step in input["steps"].as_array().unwrap() {
                    if step["op"] == "reset" {
                        flt.reset();
                        continue;
                    }
                    let pt = [f(&step["point"][0]), f(&step["point"][1])];
                    let got = flt.filter(pt, f(&step["t"]));
                    let want = [f(&expected[out][0]), f(&expected[out][1])];
                    for k in 0..2 {
                        assert!(
                            (got[k] - want[k]).abs() <= EPS,
                            "{name}[{out}][{k}]: {:?} != {:?}",
                            got,
                            want
                        );
                    }
                    out += 1;
                }
            }
            d => panic!("{name}: dims {d}"),
        }
        assert_eq!(
            out,
            expected.len(),
            "{name}: every expected output consumed"
        );
    }
}

/// The behavioural assertions of `OneEuroFilterTests.swift`, on the Rust port.
#[test]
fn swift_test_assertions_hold() {
    // testStationarySignalJitterReduction
    let mut flt = OneEuro::new(FilterParams {
        min_cutoff_hz: 1.0,
        beta: 0.01,
        d_cutoff_hz: 1.0,
    });
    let out: Vec<f64> = (0..60)
        .map(|i| flt.filter(0.5 + 0.03 * (i as f64 * 5.0).sin(), i as f64 / 60.0))
        .collect();
    let max_err = out[30..]
        .iter()
        .map(|v| (v - 0.5).abs())
        .fold(0.0, f64::max);
    assert!(max_err < 0.03 * 0.45, "{max_err}");

    // testFastSaccadeTransitionResponsiveness
    let mut flt = OneEuro::new(FilterParams {
        min_cutoff_hz: 1.0,
        beta: 0.02,
        d_cutoff_hz: 1.0,
    });
    for i in 0..30 {
        flt.filter(0.2, i as f64 / 60.0);
    }
    let mut v = 0.2;
    for i in 30..45 {
        v = flt.filter(0.8, i as f64 / 60.0);
    }
    assert!(v > 0.65, "{v}");

    // testFilterReset and the first-sample pass-through.
    let mut flt = OneEuro::new(FilterParams::default());
    flt.filter(100.0, 1.0);
    flt.reset();
    assert_eq!(flt.filter(0.0, 2.0), 0.0);
    let mut flt2 = OneEuro2D::new(FilterParams::default());
    assert_eq!(flt2.filter([0.35, 0.65], 1.0), [0.35, 0.65]);
}

#[test]
fn default_params_are_the_swift_defaults() {
    let p = FilterParams::default();
    assert_eq!((p.min_cutoff_hz, p.beta, p.d_cutoff_hz), (1.2, 0.015, 1.0));
}
