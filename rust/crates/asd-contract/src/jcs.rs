//! owner: WP-R.3
//!
//! RFC 8785 JSON Canonicalization Scheme over `serde_json::Value`.
//!
//! Written here instead of using `serde_jcs` 0.1.0, which orders object members by their
//! escaped output bytes rather than by UTF-16 code units (RFC 8785 §3.2.3): it puts `"1"`
//! before `"\r"` and U+1F600 after U+FB33. Number formatting is ECMAScript
//! `Number.prototype.toString` via `ryu-js` (the formatter `serde_jcs` uses too).

use std::cmp::Ordering;

use serde_json::{Map, Number, Value};

/// Canonical bytes of `value`.
pub(crate) fn to_vec(value: &Value) -> Vec<u8> {
    let mut out = String::new();
    write_value(&mut out, value);
    out.into_bytes()
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(out, n),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => write_object(out, map),
    }
}

/// Every JSON number is an IEEE 754 double in JCS (I-JSON); integers beyond 2^53 round.
fn write_number(out: &mut String, n: &Number) {
    let f = n.as_f64().unwrap_or(0.0);
    if f == 0.0 {
        // Covers -0, which ECMAScript prints as "0".
        out.push('0');
    } else {
        out.push_str(ryu_js::Buffer::new().format_finite(f));
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{0C}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn write_object(out: &mut String, map: &Map<String, Value>) {
    let mut entries: Vec<(&String, &Value)> = map.iter().collect();
    entries.sort_by(|(a, _), (b, _)| utf16_cmp(a, b));
    out.push('{');
    for (i, (key, value)) in entries.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_string(out, key);
        out.push(':');
        write_value(out, value);
    }
    out.push('}');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn control_characters_use_short_or_lowercase_unicode_escapes() {
        let v = Value::String("\u{0}\u{1f}\u{7f}\u{2028}\u{8}\u{c}".into());
        assert_eq!(
            String::from_utf8(to_vec(&v)).unwrap(),
            "\"\\u0000\\u001f\u{7f}\u{2028}\\b\\f\""
        );
    }

    #[test]
    fn nested_objects_and_arrays_are_canonical() {
        let v = json!({"b": [3, {"z": true, "a": null}], "a": -0.0, "c": 1.0});
        assert_eq!(
            String::from_utf8(to_vec(&v)).unwrap(),
            r#"{"a":0,"b":[3,{"a":null,"z":true}],"c":1}"#
        );
    }
}
