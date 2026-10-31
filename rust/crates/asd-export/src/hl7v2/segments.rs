//! owner: WP-I.4
//!
//! ORU^R01 segments (HL7-2..6): MSH, PID, PV1, ORC, OBR, OBX, NTE, with `TS` values in the
//! org time zone and offset (trap 11). Every data value goes through [`escape`]; the
//! separators in this file are the only unescaped delimiters in a message.
//!
//! The literal HL7 table values written here (`ORU^R01^ORU_R01`, `MR`, `L`, `P`/`F`/`C`,
//! the `NM`/`CWE`/`TX`/`ED` value types, `UCUM`) are protocol syntax fixed by the
//! standard or by HL7-1..6, not terminology an integration can change, so they are not
//! `codes::Code` entries. The one optional external code (the LOINC alternate of `OBR-4`)
//! goes through `Code::require` (XC-7).

use chrono::DateTime;
use chrono_tz::Tz;

use super::escape::{escape, COMPONENT, ENCODING_CHARACTERS, FIELD, SEGMENT_END, SUBCOMPONENT};
use crate::ExportError;

/// A segment under construction: 1-based field positions, unset positions are empty.
pub(crate) struct Segment {
    name: &'static str,
    fields: Vec<String>,
}

impl Segment {
    pub(crate) fn new(name: &'static str) -> Self {
        Self {
            name,
            fields: Vec::new(),
        }
    }

    /// Set field `n` (1-based) to an already-encoded value.
    pub(crate) fn raw(mut self, n: usize, value: impl Into<String>) -> Self {
        if self.fields.len() < n {
            self.fields.resize(n, String::new());
        }
        self.fields[n - 1] = value.into();
        self
    }

    /// Set field `n` to escaped text.
    pub(crate) fn text(self, n: usize, value: &str) -> Self {
        self.raw(n, escape(value))
    }

    /// Set field `n` to components, each escaped, joined with `^`.
    pub(crate) fn components(self, n: usize, parts: &[&str]) -> Self {
        self.raw(n, components(parts))
    }

    /// `NAME|f1|f2...` plus the CR terminator; trailing empty fields are dropped.
    pub(crate) fn finish(mut self) -> String {
        while self.fields.last().is_some_and(String::is_empty) {
            self.fields.pop();
        }
        let mut out = String::from(self.name);
        for f in &self.fields {
            out.push(FIELD);
            out.push_str(f);
        }
        out.push(SEGMENT_END);
        out
    }
}

/// Escaped components joined with `^`; trailing empty components are dropped.
pub(crate) fn components(parts: &[&str]) -> String {
    let mut parts: Vec<String> = parts.iter().map(|p| escape(p)).collect();
    while parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts.join(&COMPONENT.to_string())
}

/// `YYYYMMDDHHMMSS+HHMM` of an RFC 3339 instant in `tz` (HL7-2, trap 11).
pub(crate) fn ts(value: &str, tz: Tz, field: &'static str) -> Result<String, ExportError> {
    let instant = DateTime::parse_from_rfc3339(value)
        .map_err(|_| ExportError::InvalidTimestamp { field })?
        .with_timezone(&tz);
    Ok(instant.format("%Y%m%d%H%M%S%z").to_string())
}

/// Facts of the sending and receiving side (the integration profile).
pub(crate) struct MshFacts<'a> {
    pub sending_app: &'a str,
    pub sending_facility: &'a str,
    pub receiving_app: &'a str,
    pub receiving_facility: &'a str,
    pub timestamp: &'a str,
    pub control_id: &'a str,
    pub test: bool,
}

/// `MSH` (HL7-1, HL7-2). `MSH-1` is the field separator and `MSH-2` the encoding
/// characters, written here as the standard requires (not as escaped fields).
pub(crate) fn msh(f: &MshFacts<'_>) -> String {
    let seg = Segment::new("MSH")
        .raw(2, ENCODING_CHARACTERS)
        .text(3, f.sending_app)
        .text(4, f.sending_facility)
        .text(5, f.receiving_app)
        .text(6, f.receiving_facility)
        .raw(7, f.timestamp)
        .raw(9, components(&["ORU", "R01", "ORU_R01"]))
        .text(10, f.control_id)
        .raw(11, if f.test { "T" } else { "P" })
        .raw(12, "2.5.1")
        .raw(18, "UNICODE UTF-8")
        .finish();
    // The builder wrote an empty MSH-1; the field separator itself is MSH-1, so drop the
    // empty field's separator: `MSH||^~\&|` becomes `MSH|^~\&|`.
    seg.replacen("MSH||", "MSH|", 1)
}

/// `PID` (HL7-3): MRN with its assigning authority; `PID-5` empty or the token.
pub(crate) fn pid(mrn: &str, authority: &str, pid5_token: Option<&str>) -> String {
    let id = format!(
        "{}{c}{c}{c}{}{c}MR",
        escape(mrn),
        escape(authority),
        c = COMPONENT
    );
    let mut seg = Segment::new("PID").raw(1, "1").raw(3, id);
    if let Some(token) = pid5_token {
        seg = seg.text(5, token);
    }
    seg.finish()
}

/// `PV1` (HL7-1): outpatient class only.
pub(crate) fn pv1() -> String {
    Segment::new("PV1").raw(1, "1").raw(2, "O").finish()
}

/// `ORC` for a result: `RE`, filler order number, order status, transaction time.
pub(crate) fn orc(filler: &str, app: &str, completed: bool, transaction_ts: &str) -> String {
    Segment::new("ORC")
        .raw(1, "RE")
        .raw(3, components(&[filler, app]))
        .raw(5, if completed { "CM" } else { "IP" })
        .raw(9, transaction_ts)
        .finish()
}

/// Facts of `OBR` (HL7-4).
pub(crate) struct ObrFacts<'a> {
    pub filler: &'a str,
    pub app: &'a str,
    /// Alternate (LOINC) triplet for `OBR-4`, only when XC-7 allows it.
    pub alternate: Option<(&'a str, &'a str, &'a str)>,
    pub observed_ts: &'a str,
    pub signed_ts: Option<&'a str>,
    pub status: char,
    /// `(user id, display name)` of the first signer.
    pub first_signer: Option<(&'a str, &'a str)>,
}

/// `OBR`: local service code, session start, sign time, result status, first signer.
pub(crate) fn obr(f: &ObrFacts<'_>) -> String {
    let mut service = components(&["ASD-GAZE", "Gaze engagement observation review", "L"]);
    if let Some((code, text, system)) = f.alternate {
        service.push(COMPONENT);
        service.push_str(&components(&[code, text, system]));
    }
    let mut seg = Segment::new("OBR")
        .raw(1, "1")
        .raw(3, components(&[f.filler, f.app]))
        .raw(4, service)
        .raw(7, f.observed_ts);
    if let Some(signed) = f.signed_ts {
        seg = seg.raw(22, signed);
    }
    seg = seg.raw(25, f.status.to_string());
    if let Some((id, name)) = f.first_signer {
        seg = seg.raw(32, format!("{}{SUBCOMPONENT}{}", escape(id), escape(name)));
    }
    seg.finish()
}

/// The code of an `OBX-3`: `code^text^system`.
pub(crate) struct ObxCode<'a> {
    pub code: &'a str,
    pub text: &'a str,
    pub system: &'a str,
}

/// What `OBX-2`/`OBX-5` hold.
pub(crate) enum ObxValue<'a> {
    /// `NM` with units `1^{unit}^UCUM`.
    Numeric { value: f64, unit: &'a str },
    /// `CWE`: `code^text^system`.
    Coded(ObxCode<'a>),
    /// `TX`: one paragraph.
    Text(&'a str),
    /// `ED`: `^AP^PDF^Base64^{data}`; `data` must already be base64.
    Encapsulated { base64: &'a str },
}

/// `OBX` (HL7-5). `OBX-11` is the report status, `OBX-14` the observation time.
pub(crate) fn obx(
    set_id: usize,
    code: &ObxCode<'_>,
    value: &ObxValue<'_>,
    status: char,
    observed_ts: &str,
) -> String {
    let mut seg = Segment::new("OBX")
        .raw(1, set_id.to_string())
        .components(3, &[code.code, code.text, code.system]);
    seg = match value {
        ObxValue::Numeric { value, unit } => seg
            .raw(2, "NM")
            .raw(5, number(*value))
            .raw(6, components(&["1", unit, "UCUM"])),
        ObxValue::Coded(c) => seg
            .raw(2, "CWE")
            .raw(5, components(&[c.code, c.text, c.system])),
        ObxValue::Text(t) => seg.raw(2, "TX").text(5, t),
        ObxValue::Encapsulated { base64 } => {
            // Base64 has no HL7 delimiter in its alphabet; it is not escaped.
            seg.raw(2, "ED").raw(5, format!("^AP^PDF^Base64^{base64}"))
        }
    };
    seg.raw(11, status.to_string())
        .raw(14, observed_ts)
        .finish()
}

/// `NTE` (HL7-6): source `L` (local), one comment line.
pub(crate) fn nte(set_id: usize, comment: &str) -> String {
    Segment::new("NTE")
        .raw(1, set_id.to_string())
        .raw(2, "L")
        .text(3, comment)
        .finish()
}

/// Shortest round-trip decimal, never an exponent. Callers skip non-finite values.
fn number(v: f64) -> String {
    format!("{v}")
}
