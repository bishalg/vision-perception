//! owner: WP-B.1
//!
//! OBS-1: `tracing` output as JSON lines with redaction built into the formatter, so a
//! sensitive value is never serialised in the first place:
//!
//! - fields whose name is sensitive ([`is_sensitive_field`]: `participant_token`,
//!   `email`, `notes`, `rawPayload`, passwords, tokens, secrets, and the clinical fields
//!   `impression`, `recommendations`, `body`, `recipient`, `mrn` of KEY-6) are dropped, on
//!   events and on spans;
//! - in every remaining string value (and the message) e-mail addresses become
//!   `[redacted-email]` and app participant tokens (`child-...`) become
//!   `[redacted-token]` ([`redact_str`]);
//! - HTTP request spans record the matched route template, never the raw path or query
//!   (`app::router`), because paths such as `/v1/participants/{token}/summary` carry tokens.
//!
//! Line shape: `{"timestamp","level","target","message","fields":{..},"spans":[{"name",..}]}`.

use std::borrow::Cow;
use std::fmt;
use std::io::Write;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(?:\.[A-Za-z0-9\-]+)*\.[A-Za-z]{2,}")
        .expect("valid regex")
});
/// `GazeSessionRecord.participantToken` fallback form: `child-<session prefix>`.
static CHILD_TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"child-[A-Za-z0-9_\-]+").expect("valid regex"));

/// True for field names whose values must never be logged. Case, `_`, `-` and `.` are
/// ignored, so `participant_token`, `participantToken` and `participant.token` match.
pub fn is_sensitive_field(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| !matches!(c, '_' | '-' | '.'))
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        n.as_str(),
        "notes" | "note" | "rawpayload" | "authorization" | "cookie" | "setcookie" | "enrolcode"
    ) || is_clinical_field(&n)
        || n.contains("email")
        || n.contains("password")
        || n.contains("secret")
        || n.contains("pwhash")
        || n.ends_with("token")
}

/// KEY-6: clinical free text and identifiers (`n` is already lower-cased without `_-.`).
/// `mrn` matches anywhere (`mrn`, `patient_mrn`, `mrn_value`); the others exactly or as a
/// suffix (`letter_body`, `review_impression`), so `body_len` or `recipient_count` stay.
fn is_clinical_field(n: &str) -> bool {
    n.contains("mrn")
        || [
            "impression",
            "recommendations",
            "body",
            "recipient",
            "amendreason",
        ]
        .iter()
        .any(|f| n.ends_with(f))
}

/// Masks e-mail addresses and child participant tokens inside free text.
pub fn redact_str(s: &str) -> Cow<'_, str> {
    let step = EMAIL_RE.replace_all(s, "[redacted-email]");
    let owned = match CHILD_TOKEN_RE.replace_all(&step, "[redacted-token]") {
        Cow::Owned(o) => Some(o),
        Cow::Borrowed(_) => None,
    };
    match owned {
        Some(o) => Cow::Owned(o),
        None => step,
    }
}

#[derive(Default)]
struct JsonFields(Map<String, Value>);

impl JsonFields {
    fn put(&mut self, field: &Field, value: Value) {
        if !is_sensitive_field(field.name()) {
            self.0.insert(field.name().to_owned(), value);
        }
    }
}

impl Visit for JsonFields {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.put(field, Value::from(value));
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, Value::from(value));
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, Value::from(value));
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, Value::from(value));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, Value::from(redact_str(value).into_owned()));
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if is_sensitive_field(field.name()) {
            return; // do not even format it
        }
        let text = format!("{value:?}");
        self.put(field, Value::from(redact_str(&text).into_owned()));
    }
}

/// Redacted fields stored on each span.
struct SpanFields(Map<String, Value>);

/// A [`Layer`] writing one redacted JSON object per event to `W`.
pub struct RedactingJsonLayer<W> {
    make_writer: W,
}

impl<W> RedactingJsonLayer<W>
where
    W: for<'a> MakeWriter<'a> + 'static,
{
    pub fn new(make_writer: W) -> Self {
        Self { make_writer }
    }
}

impl<S, W> Layer<S> for RedactingJsonLayer<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = JsonFields::default();
        attrs.record(&mut fields);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanFields(fields.0));
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let mut fields = JsonFields::default();
        values.record(&mut fields);
        if let Some(span) = ctx.span(id) {
            let mut ext = span.extensions_mut();
            match ext.get_mut::<SpanFields>() {
                Some(existing) => existing.0.extend(fields.0),
                None => ext.insert(SpanFields(fields.0)),
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = JsonFields::default();
        event.record(&mut fields);
        let mut fields = fields.0;
        let message = fields.remove("message").unwrap_or(Value::from(""));

        let meta = event.metadata();
        let mut line = Map::new();
        line.insert(
            "timestamp".into(),
            chrono::Utc::now()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                .into(),
        );
        line.insert("level".into(), meta.level().as_str().into());
        line.insert("target".into(), meta.target().into());
        line.insert("message".into(), message);
        if !fields.is_empty() {
            line.insert("fields".into(), Value::Object(fields));
        }
        if let Some(scope) = ctx.event_scope(event) {
            let spans: Vec<Value> = scope
                .from_root()
                .map(|span| {
                    let mut obj = Map::new();
                    obj.insert("name".into(), span.name().into());
                    if let Some(f) = span.extensions().get::<SpanFields>() {
                        obj.extend(f.0.clone());
                    }
                    Value::Object(obj)
                })
                .collect();
            if !spans.is_empty() {
                line.insert("spans".into(), Value::Array(spans));
            }
        }

        let Ok(mut bytes) = serde_json::to_vec(&Value::Object(line)) else {
            return;
        };
        bytes.push(b'\n');
        // A failed log write must never fail the request.
        let _ = self.make_writer.make_writer().write_all(&bytes);
    }
}

/// Installs the global subscriber: `filter` (RUST_LOG syntax) + redacting JSON to stdout.
pub fn init(filter: &str) -> Result<(), String> {
    let filter = EnvFilter::try_new(filter).map_err(|e| format!("invalid RUST_LOG: {e}"))?;
    tracing_subscriber::registry()
        .with(filter)
        .with(RedactingJsonLayer::new(std::io::stdout))
        .try_init()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_names() {
        for name in [
            "participant_token",
            "participantToken",
            "participant.token",
            "email",
            "user_email",
            "notes",
            "rawPayload",
            "raw_payload",
            "password",
            "pw_hash",
            "authorization",
            "device_token",
            "refresh_token",
            "access_token",
            "token",
            "totp_secret",
            "enrol_code",
            // KEY-6 (WP-C.9).
            "impression",
            "recommendations",
            "body",
            "note_body",
            "recipient",
            "mrn",
            "patient_mrn",
            "mrn_value",
            "amend_reason",
            "password",
        ] {
            assert!(is_sensitive_field(name), "{name}");
        }
        for name in [
            "message",
            "session_id",
            "org_id",
            "route",
            "status",
            "token_count",
            "latency_ms",
            "body_len",
            "recipient_count",
            "key_id",
            "kek_id",
        ] {
            assert!(!is_sensitive_field(name), "{name}");
        }
    }

    #[test]
    fn free_text_masking() {
        assert_eq!(
            redact_str("mail a.b+c@clinic.example.co.uk now"),
            "mail [redacted-email] now"
        );
        assert_eq!(
            redact_str("for child-SESS-GOL ok"),
            "for [redacted-token] ok"
        );
        assert!(matches!(redact_str("nothing here"), Cow::Borrowed(_)));
        assert_eq!(
            redact_str("postgres://asd:***@db:5432/asd"),
            "postgres://asd:***@db:5432/asd"
        );
    }
}
