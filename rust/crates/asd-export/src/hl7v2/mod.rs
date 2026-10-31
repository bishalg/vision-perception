//! owner: WP-I.4
//!
//! HL7 v2.5.1 ORU^R01 encoder (HL7-1..7), hand-written: [`escape`] (delimiter escaping,
//! trap 10) and [`segments`] (MSH, PID, PV1, ORC, OBR, OBX, NTE). Identified mode only;
//! refuses a record without an MRN. Stub until WP-I.4.

pub mod escape;
pub mod segments;
