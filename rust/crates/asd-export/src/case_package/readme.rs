//! owner: WP-I.12
//!
//! `README.txt` of a case package (SHR-6): plain UTF-8 text, LF line ends. It says what the
//! package is, the mode, both mandatory sentences (PDF-5), the export id and recipient
//! label, how to verify the files (`sha256sum -c` lines generated from the manifest's
//! `files[]`), that the sending clinic remains the controller, that de-identified data must
//! not be re-identified, and the same in Nepali.

use super::manifest::{AbsentFile, FileEntry};
use super::Case;
use crate::claims;
use crate::watermark::MANDATORY_SENTENCE;

/// First line between which the checksum lines are written.
pub(crate) const SUMS_BEGIN: &str = "----- BEGIN SHA256SUMS -----";
pub(crate) const SUMS_END: &str = "----- END SHA256SUMS -----";

/// Nepali paragraph (SHR-6). Plain text needs no embedded font.
const NEPALI: &str = "\
नेपाली: यो प्याकेजमा एक बालबालिकाको नजर सम्बन्धी अवलोकनका मापनहरू छन्, जुन पठाउने
क्लिनिकले साझा गरेको हो। यो निदान गर्ने उपकरण होइन। पठाउने क्लिनिक नै यस डाटाको
नियन्त्रक रहन्छ। पहिचान हटाइएको डाटाबाट कसैको पहिचान फेरि पत्ता लगाउने प्रयास
नगर्नुहोस्। फाइलहरू जाँच्न माथिका SHA256SUMS लाइनहरू प्रयोग गर्नुहोस्।";

/// The README bytes. `files` are the entries after the README (it cannot hash itself).
pub(crate) fn render(case: &Case<'_>, files: &[FileEntry], absent: &[AbsentFile]) -> Vec<u8> {
    let wm = &case.input.watermark;
    let mut t = String::new();
    let mut line = |s: &str| {
        t.push_str(s);
        t.push('\n');
    };

    line("CASE PACKAGE");
    line("");
    line(&format!(
        "This archive is a case shared by {}: measurements from gaze engagement \
         observations of one child, the clinic's review status and, in identified \
         packages, its notes and attachments.",
        wm.org_name
    ));
    line("");
    line(MANDATORY_SENTENCE);
    line(claims::MISSION);
    line("");
    line(&format!("Export id: {}", wm.export_id));
    line(&format!("Mode: {}", wm.mode.label()));
    line(&format!("Purpose: {}", wm.purpose.label()));
    line(&format!("Recipient: {}", one_line(case.recipient_label())));
    line(&format!("Sessions: {}", case.records.len()));
    line(&format!("Watermark: {}", wm.line()));
    line("");
    line("The sending clinic remains the controller of this data. Use it only for the");
    line("purpose stated above and do not pass it on.");
    if case.deidentified() {
        line("This package is de-identified. Do not attempt to re-identify the child or");
        line("link the pseudonyms (P-..., S-...) to any other data set; they are valid");
        line("inside this package only.");
    } else {
        line("This package is identified: it names the participant and may carry the");
        line("hospital record number, clinical notes and attachments. Keep it confidential.");
    }
    line("");
    line("FILES");
    line("");
    line("manifest.json lists every file with its size and SHA-256. To verify the files,");
    line("extract the archive, save the lines between the markers below as SHA256SUMS in");
    line("the extracted folder and run:  sha256sum -c SHA256SUMS");
    line("");
    line(SUMS_BEGIN);
    for f in files {
        line(&format!("{}  {}", f.sha256, f.path));
    }
    line(SUMS_END);
    for a in absent {
        line(&format!("Not included: {} ({})", a.path, a.reason));
    }
    line("");
    line(NEPALI);
    t.into_bytes()
}

/// Control characters and line breaks become single spaces.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
