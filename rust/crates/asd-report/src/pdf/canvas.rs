//! A one-page drawing surface over `pdf-writer` with UIKit-style coordinates (origin top
//! left, y down), so the layout code reads like the Swift it ports.
//!
//! Text uses the PDF base-14 fonts Helvetica and Helvetica-Bold with `WinAnsiEncoding`:
//! nothing is embedded, so the output is small and byte-for-byte reproducible. Characters
//! outside WinAnsi are written as `?`.

use pdf_writer::{Content, Name, Pdf, Rect, Ref, Str, TextStr};

pub(crate) const PAGE_WIDTH: f32 = 612.0;
pub(crate) const PAGE_HEIGHT: f32 = 792.0;

/// Baseline offset below the top of a text line, as a fraction of the font size (UIKit
/// draws text with the line's top at the given point).
const ASCENT: f32 = 0.95;
/// Line height for wrapped text, as a fraction of the font size.
const LEADING: f32 = 1.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Font {
    /// UIKit regular and medium weights.
    Regular,
    /// UIKit semibold and bold weights.
    Bold,
}

impl Font {
    fn resource(self) -> Name<'static> {
        match self {
            Self::Regular => Name(b"F1"),
            Self::Bold => Name(b"F2"),
        }
    }

    /// Advance width of a WinAnsi byte in 1/1000 em (Adobe Helvetica AFM metrics).
    fn width(self, byte: u8) -> u16 {
        let table = match self {
            Self::Regular => &HELVETICA,
            Self::Bold => &HELVETICA_BOLD,
        };
        match byte {
            32..=126 => table[usize::from(byte - 32)],
            0x95 => 350,        // bullet
            0x96 => 556,        // en dash
            0x97 => 1000,       // em dash
            0x85 => 1000,       // ellipsis
            0x91 | 0x92 => 222, // single quotes
            0x93 | 0x94 => 333, // double quotes
            _ => 556,
        }
    }

    pub(crate) fn text_width(self, text: &str, size: f32) -> f32 {
        let units: u32 = encode(text).iter().map(|&b| u32::from(self.width(b))).sum();
        units as f32 * size / 1000.0
    }
}

/// An sRGB colour, components in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rgb(pub f32, pub f32, pub f32);

/// UIKit light-mode system colours, pre-blended on white (no transparency groups).
pub(crate) mod colors {
    use super::Rgb;
    pub const LABEL: Rgb = Rgb(0.0, 0.0, 0.0);
    /// `secondaryLabel` (60,60,67 @ 60%).
    pub const SECONDARY_LABEL: Rgb = Rgb(0.541, 0.541, 0.557);
    /// `separator` (60,60,67 @ 29%).
    pub const SEPARATOR: Rgb = Rgb(0.778, 0.778, 0.786);
    /// `secondarySystemBackground` (242,242,247).
    pub const SECONDARY_BACKGROUND: Rgb = Rgb(0.949, 0.949, 0.969);
    /// `systemGreen` (52,199,89).
    pub const GREEN: Rgb = Rgb(0.204, 0.780, 0.349);
    /// `systemGreen` @ 8%.
    pub const GREEN_TINT: Rgb = Rgb(0.936, 0.982, 0.948);
    /// `systemBlue` @ 6%.
    pub const BLUE_TINT: Rgb = Rgb(0.940, 0.969, 1.0);
}

/// Unicode to WinAnsi (cp1252); anything else becomes `?`.
pub(crate) fn encode(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| match c {
            ' '..='~' => c as u8,
            '\u{A0}'..='\u{FF}' => u8::try_from(u32::from(c)).unwrap_or(b'?'),
            '€' => 0x80,
            '…' => 0x85,
            '\u{2018}' => 0x91,
            '\u{2019}' => 0x92,
            '\u{201C}' => 0x93,
            '\u{201D}' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            _ => b'?',
        })
        .collect()
}

pub(crate) struct Canvas {
    content: Content,
}

impl Canvas {
    pub(crate) fn new() -> Self {
        Self {
            content: Content::new(),
        }
    }

    fn fill(&mut self, c: Rgb) {
        self.content.set_fill_rgb(c.0, c.1, c.2);
    }

    /// Single-line text with its top-left corner at `(x, y)`.
    pub(crate) fn text(&mut self, text: &str, x: f32, y: f32, font: Font, size: f32, color: Rgb) {
        self.fill(color);
        let bytes = encode(text);
        self.content
            .begin_text()
            .set_font(font.resource(), size)
            .next_line(x, PAGE_HEIGHT - y - size * ASCENT)
            .show(Str(&bytes))
            .end_text();
    }

    /// Word-wrapped text inside a box; lines that do not fit the box height are dropped
    /// (UIKit `draw(in:)` clips the same way).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn text_in_rect(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        font: Font,
        size: f32,
        color: Rgb,
    ) {
        let line_height = size * LEADING;
        let mut top = y;
        for line in wrap(text, width, font, size) {
            if top + line_height > y + height + 0.01 {
                break;
            }
            self.text(&line, x, top, font, size, color);
            top += line_height;
        }
    }

    /// Filled rounded rectangle, top-left at `(x, y)`.
    pub(crate) fn rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Rgb) {
        // Cubic Bezier approximation of a quarter circle.
        const K: f32 = 0.552_284_8;
        let (left, right) = (x, x + w);
        let (top, bottom) = (PAGE_HEIGHT - y, PAGE_HEIGHT - y - h);
        let c = r * K;
        self.fill(color);
        self.content
            .move_to(left + r, top)
            .line_to(right - r, top)
            .cubic_to(right - r + c, top, right, top - r + c, right, top - r)
            .line_to(right, bottom + r)
            .cubic_to(
                right,
                bottom + r - c,
                right - r + c,
                bottom,
                right - r,
                bottom,
            )
            .line_to(left + r, bottom)
            .cubic_to(left + r - c, bottom, left, bottom + r - c, left, bottom + r)
            .line_to(left, top - r)
            .cubic_to(left, top - r + c, left + r - c, top, left + r, top)
            .close_path()
            .fill_nonzero();
    }

    /// Horizontal rule at `y` from `x1` to `x2`.
    pub(crate) fn hline(&mut self, x1: f32, x2: f32, y: f32, width: f32, color: Rgb) {
        self.content
            .set_stroke_rgb(color.0, color.1, color.2)
            .set_line_width(width)
            .move_to(x1, PAGE_HEIGHT - y)
            .line_to(x2, PAGE_HEIGHT - y)
            .stroke();
    }

    /// Wraps the drawing into a complete one-page US Letter PDF.
    pub(crate) fn finish(self, title: &str) -> Vec<u8> {
        let catalog = Ref::new(1);
        let pages = Ref::new(2);
        let page = Ref::new(3);
        let contents = Ref::new(4);
        let regular = Ref::new(5);
        let bold = Ref::new(6);
        let info = Ref::new(7);

        let mut pdf = Pdf::new();
        pdf.catalog(catalog).pages(pages);
        pdf.pages(pages).kids([page]).count(1);
        {
            let mut p = pdf.page(page);
            p.parent(pages)
                .media_box(Rect::new(0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT))
                .contents(contents);
            p.resources()
                .fonts()
                .pair(Font::Regular.resource(), regular)
                .pair(Font::Bold.resource(), bold);
        }
        pdf.type1_font(regular)
            .base_font(Name(b"Helvetica"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(bold)
            .base_font(Name(b"Helvetica-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.stream(contents, &self.content.finish());
        pdf.document_info(info).title(TextStr(title));
        pdf.finish()
    }
}

/// Greedy word wrap; a single word wider than the line is kept whole on its own line.
fn wrap(text: &str, width: f32, font: Font, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if current.is_empty() || font.text_width(&candidate, size) <= width {
            current = candidate;
        } else {
            lines.push(std::mem::replace(&mut current, word.to_owned()));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

// Adobe Core14 AFM advance widths for codes 32..=126.
#[rustfmt::skip]
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // ' '../
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0..?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // @..O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // P.._
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // `..o
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,      // p..~
];

#[rustfmt::skip]
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611,
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556,
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611,
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winansi_maps_latin1_and_typographic_marks() {
        assert_eq!(encode("Zoë • – x"), b"Zo\xeb \x95 \x96 x");
        assert_eq!(encode("\u{0928}"), b"?");
    }

    #[test]
    fn helvetica_widths_follow_the_afm() {
        assert_eq!(Font::Regular.width(b'W'), 944);
        assert_eq!(Font::Bold.width(b'i'), 278);
        assert!((Font::Regular.text_width("ab", 10.0) - 11.12).abs() < 1e-4);
    }

    #[test]
    fn wrap_breaks_at_word_boundaries_within_width() {
        let lines = wrap("one two three four five", 40.0, Font::Regular, 10.0);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(Font::Regular.text_width(l, 10.0) <= 40.0 || !l.contains(' '));
        }
        assert_eq!(lines.join(" "), "one two three four five");
    }
}
