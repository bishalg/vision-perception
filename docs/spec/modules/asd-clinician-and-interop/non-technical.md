# ASD Clinician Workspace and Interoperability Bridge - Non-Technical Summary

## What changes

Doctors and therapists who already use the clinic console get their own working area in
it. They see a list of new recordings that still need a clinician's look. They open one,
see the measurements and how they compare with the reference values, write what they think
and what should happen next, and sign it with their password. A clinic can require a
second clinician to countersign. They can keep private notes about a child over time,
write a referral or summary letter, and print a one-page clinical summary.

The system also learns to hand results to the hospital's own software. A Nepali or Indian
hospital usually takes HL7 files or spreadsheets. A hospital on Epic or Cerner takes FHIR.
Results can be downloaded as CSV, Excel or a PDF. They can also be sent automatically
after sign-off, to a folder on the hospital's server or as a notification the hospital's
system collects from.

## What it is not

It does not replace the hospital record and does not make diagnoses. The clinician's
judgement is recorded as theirs. Every printed or exported document still says "This is
not a diagnostic tool."

## Privacy choices

- Children's names are never stored. A clinic can choose to store the hospital record
  number (MRN) so results can be matched in the hospital system. It is encrypted, and
  every look at it is logged.
- Clinical notes and the clinician's written opinion are encrypted. The company running
  the service (and the clinic's IT admin) cannot read them.
- Research exports remove record numbers, pseudonymous ids, exact dates and all free text.
  They include only sessions whose family agreed to research use.
- Every file that leaves the system carries an export number and is logged: who took it,
  when, why, and about which children.

## Decisions the owner still needs to make

Whether any customer needs names stored (proposed: no). How long signed clinical records
must be kept when a family asks for deletion. Approval of the open font licence for PDFs.
Who may countersign. The first hospital partner's technical details. Verification of one
LOINC code.
