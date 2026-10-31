//! owner: WP-I.4
//!
//! HL7 v2 escaping (HL7-1, trap 10) for the default encoding characters `|^~\&`.
//!
//! The backslash is the escape character, so it is escaped in the same left-to-right pass
//! as the other delimiters (a `replace` chain that handled it after `|` would re-escape the
//! backslashes it had just introduced). Line breaks become `\.br\` (a CR LF pair is one
//! break). Every other control character (NUL, VT and FS included, which frame MLLP)
//! becomes a space, so no data can end a segment or a message. Non-ASCII text passes
//! through as UTF-8 (`MSH-18 = UNICODE UTF-8`).

/// Field separator.
pub const FIELD: char = '|';
/// Component separator.
pub const COMPONENT: char = '^';
/// Subcomponent separator.
pub const SUBCOMPONENT: char = '&';
/// Segment terminator (carriage return).
pub const SEGMENT_END: char = '\r';
/// `MSH-2`: component, repetition, escape and subcomponent characters.
pub const ENCODING_CHARACTERS: &str = "^~\\&";

/// Escape `text` for use inside a field, component or subcomponent.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => out.push_str("\\E\\"),
            '|' => out.push_str("\\F\\"),
            '^' => out.push_str("\\S\\"),
            '&' => out.push_str("\\T\\"),
            '~' => out.push_str("\\R\\"),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str("\\.br\\");
            }
            '\n' => out.push_str("\\.br\\"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Reverse of [`escape`] for the escapes it writes (tests and round-trip checks).
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('\\') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        let Some(end) = after.find('\\') else {
            out.push_str(&rest[pos..]);
            return out;
        };
        match &after[..end] {
            "E" => out.push('\\'),
            "F" => out.push('|'),
            "S" => out.push('^'),
            "T" => out.push('&'),
            "R" => out.push('~'),
            ".br" => out.push('\n'),
            other => {
                out.push('\\');
                out.push_str(other);
                out.push('\\');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_delimiter_has_its_escape() {
        for (raw, esc) in [
            ("|", "\\F\\"),
            ("^", "\\S\\"),
            ("&", "\\T\\"),
            ("~", "\\R\\"),
            ("\\", "\\E\\"),
        ] {
            assert_eq!(escape(raw), esc);
        }
    }

    #[test]
    fn backslash_is_not_double_escaped() {
        assert_eq!(escape("a\\|b"), "a\\E\\\\F\\b");
    }

    #[test]
    fn line_breaks_become_br_and_crlf_is_one_break() {
        assert_eq!(escape("a\r\nb\nc\rd"), "a\\.br\\b\\.br\\c\\.br\\d");
    }

    #[test]
    fn other_control_characters_become_spaces() {
        assert_eq!(escape("a\u{b}b\u{1c}c\0d\te"), "a b c d e");
    }

    #[test]
    fn devanagari_passes_through() {
        assert_eq!(escape("नेपाल परीक्षण"), "नेपाल परीक्षण");
    }

    #[test]
    fn unescape_inverts_escape() {
        let raw = "x|y^z&w~v\\u\nend";
        assert_eq!(unescape(&escape(raw)), raw);
    }
}
