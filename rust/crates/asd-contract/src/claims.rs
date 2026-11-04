//! owner: WP-L.0
//!
//! ADR-0004 claims terms in Rust (I18N-3 rule 7). [`DENY_TERMS`] is a verbatim copy of
//! `GLOBAL_DENY` in `scripts/check_asd_claims.py` (id, Python regex, message); [`NEGATION`]
//! and [`DIAGNOS`] copy its "diagnos* only in a negating sentence" rule. The Python file stays
//! the scanner of store metadata and Swift; `asd-cli`'s test `i18n_claims` parses it as text
//! and fails when the two lists differ, so this copy cannot drift. It is the start of the
//! WP-D.8 port.
//!
//! The patterns use a small regex subset (literals, `\b`, `\s`, escaped punctuation, groups
//! with `|`, and `?`, `*`, `+`), matched case-insensitively by [`find`]. No regex crate: the
//! module is linked into the mobile and wasm builds.

/// `(id, pattern, message)`, in the Python order.
pub const DENY_TERMS: &[(&str, &str, &str)] = &[
    (
        "test",
        r"\btests?\b",
        "\"test\" as a noun describing the app",
    ),
    ("fda", r"\bFDA\b", "FDA claim"),
    ("hipaa", r"\bHIPAA\b", "HIPAA claim"),
    (
        "irb",
        r"\bIRB\b",
        "IRB claim (no research study at launch, H6)",
    ),
    ("protocol", r"\bprotocol\s*#", "study protocol number"),
    (
        "clin-valid",
        r"clinically\s+validated",
        "clinical validation claim",
    ),
    (
        "clinical",
        r"\bclinical(ly)?\b",
        "clinical wording (ADR-0004 decision 7)",
    ),
    ("biomarker", r"bio-?markers?", "biomarker wording"),
    ("adhd", r"\bADHD\b", "condition name ADHD"),
    (
        "autism-screen",
        r"autism\s+(screening|screener|test)",
        "autism screening/test claim",
    ),
    ("screening", r"\bscreen(ing|er)s?\b", "screening wording"),
    (
        "risk-score",
        r"risk\s+(score|percentage|stratification)",
        "risk score claim",
    ),
    ("bayesian", r"\bbayesian\b", "Bayesian claim"),
    ("probability", r"\bprobabilit(y|ies)\b", "probability claim"),
    ("smart-fhir", r"SMART\s+on\s+FHIR", "SMART on FHIR claim"),
    (
        "ane-facs",
        r"\b(ANE|FACS)\b",
        "technical jargon (ADR-0004 decision 7)",
    ),
    (
        "study",
        r"\b(research\s+study|principal\s+investigator)\b",
        "research study claim (H6)",
    ),
    (
        "astro-fusion",
        r"astro-fusion\.com",
        "URL not controlled by the publisher (CI-5)",
    ),
    (
        "hi-biomarker",
        r"बायोमार्कर|बायोमार्कर्स",
        "biomarker wording (hi/ne)",
    ),
    (
        "hi-screening",
        r"स्क्रीनिंग|स्क्रिनिङ|स्क्रीनर",
        "screening wording (hi/ne)",
    ),
    ("hi-adhd", r"एडीएचडी", "condition name ADHD (hi/ne)"),
    ("hi-test", r"परीक्षण", "test wording (hi/ne)"),
    (
        "hi-clinical",
        r"नैदानिक|क्लिनिकल|क्लिनिकली",
        "clinical wording (hi/ne)",
    ),
];

/// A sentence with "diagnos*" passes only when it also matches this (Python `NEGATION`).
pub const NEGATION: &str = r"\b(not|no|never|cannot|without|nor)\b|n't|नहीं|होइन|गर्दैन|नहीं करता";

/// "diagnos*" wording (Python `DIAGNOS`).
pub const DIAGNOS: &str = r"diagnos|निदान";

/// Rule id of a non-negated "diagnos*" sentence (the Python finding id).
pub const DIAGNOS_RULE: &str = "diagnos";

/// One claims finding: the rule id and the matched text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimHit {
    pub rule: &'static str,
    pub matched: String,
}

/// Every claims finding in `text`: the first match of each [`DENY_TERMS`] pattern, plus one
/// [`DIAGNOS_RULE`] finding per sentence that says "diagnos*" without a negation.
pub fn check(text: &str) -> Vec<ClaimHit> {
    let mut out = Vec::new();
    for (id, pattern, _) in DENY_TERMS {
        if let Some(m) = find(pattern, text) {
            out.push(ClaimHit {
                rule: id,
                matched: m,
            });
        }
    }
    if find(DIAGNOS, text).is_some() {
        for sentence in text.split(['.', '!', '?', '।', '\n', '•']) {
            if find(DIAGNOS, sentence).is_some() && find(NEGATION, sentence).is_none() {
                out.push(ClaimHit {
                    rule: DIAGNOS_RULE,
                    matched: sentence.trim().to_owned(),
                });
            }
        }
    }
    out
}

/// The first case-insensitive match of `pattern` in `text`, or `None`. A pattern outside the
/// supported subset never matches (the unit tests compile every [`DENY_TERMS`] pattern).
pub fn find(pattern: &str, text: &str) -> Option<String> {
    let alts = parse(pattern).ok()?;
    let chars: Vec<char> = text.chars().collect();
    for start in 0..=chars.len() {
        let mut end = None;
        let hit = alts.iter().any(|seq| {
            match_seq(seq, &chars, start, &mut |e| {
                end = Some(e);
                true
            })
        });
        if hit {
            return Some(chars[start..end.unwrap_or(start)].iter().collect());
        }
    }
    None
}

// ---- the regex subset ---------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Space,
    Boundary,
    /// Alternatives, each a sequence.
    Group(Vec<Vec<Node>>),
    Repeat(Box<Node>, usize, usize),
}

/// Parses a pattern into its top-level alternatives.
fn parse(pattern: &str) -> Result<Vec<Vec<Node>>, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    let alts = parse_alts(&chars, &mut i)?;
    if i != chars.len() {
        return Err(format!("unbalanced `)` at {i}"));
    }
    Ok(alts)
}

fn parse_alts(c: &[char], i: &mut usize) -> Result<Vec<Vec<Node>>, String> {
    let mut alts = vec![Vec::new()];
    while *i < c.len() {
        let ch = c[*i];
        let node = match ch {
            ')' => break,
            '|' => {
                *i += 1;
                alts.push(Vec::new());
                continue;
            }
            '(' => {
                *i += 1;
                let inner = parse_alts(c, i)?;
                if c.get(*i) != Some(&')') {
                    return Err("missing `)`".into());
                }
                *i += 1;
                Node::Group(inner)
            }
            '\\' => {
                let next = *c.get(*i + 1).ok_or("trailing `\\`")?;
                *i += 2;
                match next {
                    'b' => Node::Boundary,
                    's' => Node::Space,
                    c if c.is_ascii_alphanumeric() => {
                        return Err(format!("unsupported escape `\\{c}`"))
                    }
                    c => Node::Char(c),
                }
            }
            '*' | '+' | '?' => return Err(format!("quantifier `{ch}` without an atom")),
            '.' | '[' | ']' | '{' | '}' | '^' | '$' => {
                return Err(format!("unsupported metacharacter `{ch}`"))
            }
            c => {
                *i += 1;
                Node::Char(c)
            }
        };
        let node = match c.get(*i) {
            Some('?') => {
                *i += 1;
                Node::Repeat(Box::new(node), 0, 1)
            }
            Some('*') => {
                *i += 1;
                Node::Repeat(Box::new(node), 0, usize::MAX)
            }
            Some('+') => {
                *i += 1;
                Node::Repeat(Box::new(node), 1, usize::MAX)
            }
            _ => node,
        };
        alts.last_mut().expect("never empty").push(node);
    }
    Ok(alts)
}

fn is_word(c: Option<&char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || *c == '_')
}

fn same(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

fn match_seq(seq: &[Node], t: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match seq.split_first() {
        None => k(i),
        Some((node, rest)) => match_node(node, t, i, &mut |j| match_seq(rest, t, j, k)),
    }
}

fn match_node(node: &Node, t: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match node {
        Node::Char(c) => t.get(i).is_some_and(|x| same(*x, *c)) && k(i + 1),
        Node::Space => t.get(i).is_some_and(|x| x.is_whitespace()) && k(i + 1),
        Node::Boundary => {
            let before = if i == 0 { None } else { t.get(i - 1) };
            is_word(before) != is_word(t.get(i)) && k(i)
        }
        Node::Group(alts) => alts.iter().any(|seq| match_seq(seq, t, i, k)),
        Node::Repeat(inner, min, max) => match_repeat(inner, *min, *max, 0, t, i, k),
    }
}

/// Greedy repetition with backtracking; an iteration must consume input.
fn match_repeat(
    inner: &Node,
    min: usize,
    max: usize,
    count: usize,
    t: &[char],
    i: usize,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    if count < max
        && match_node(inner, t, i, &mut |j| {
            j != i && match_repeat(inner, min, max, count + 1, t, j, k)
        })
    {
        return true;
    }
    count >= min && k(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_parses() {
        for (id, pattern, _) in DENY_TERMS {
            assert!(parse(pattern).is_ok(), "{id}: {pattern}");
        }
        assert!(parse(NEGATION).is_ok());
        assert!(parse(DIAGNOS).is_ok());
    }

    #[test]
    fn matches_like_python_re_ignorecase() {
        assert_eq!(
            find(r"\btests?\b", "Run the Tests now").as_deref(),
            Some("Tests")
        );
        assert_eq!(find(r"\btests?\b", "attestation"), None);
        assert_eq!(
            find(r"\bprotocol\s*#", "Protocol  #12").as_deref(),
            Some("Protocol  #")
        );
        assert_eq!(
            find(r"bio-?markers?", "16 bio-markers").as_deref(),
            Some("bio-markers")
        );
        assert_eq!(
            find(r"\bclinical(ly)?\b", "clinically"),
            Some("clinically".into())
        );
        assert_eq!(find(r"\bscreen(ing|er)s?\b", "Screen time"), None);
        assert_eq!(
            find(r"\bscreen(ing|er)s?\b", "screeners").as_deref(),
            Some("screeners")
        );
        assert_eq!(find(r"astro-fusion\.com", "astro-fusionXcom"), None);
        assert_eq!(
            find(r"स्क्रीनिंग|स्क्रिनिङ", "यो स्क्रिनिङ हो").as_deref(),
            Some("स्क्रिनिङ")
        );
    }

    #[test]
    fn diagnos_needs_a_negation() {
        let hits = check("This is a diagnosis.");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rule, DIAGNOS_RULE);
        assert!(check("This is not a diagnosis.").is_empty());
        assert!(check("Looking time. It does not diagnose.").is_empty());
        let rules: Vec<_> = check("A clinical screening test")
            .iter()
            .map(|h| h.rule)
            .collect();
        assert_eq!(rules, ["test", "clinical", "screening"]);
    }
}
