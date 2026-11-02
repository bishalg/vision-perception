# Consent to share a child's information with a doctor or researcher abroad

> **DRAFT TEMPLATE. Requires legal review (Nepali counsel at minimum) before use, including review of consent rules for minors and of cross-border transfer rules.** WP-E.8, 2026-11-02. This form does not claim any legal certification of the clinic, the product or the recipient. [OWNER DECISION: O-6, O-2] Recommended defaults are shown in square brackets and in the design notes.

Clinic: [name] Privacy contact: [name, phone] [OWNER DECISION: O-11]
Version: v0.1-draft Date of version: 2026-11-02 (the form's version number is written into the record so we know which words the family saw)

## What this is, in plain words
Our clinic keeps a record of your child's visit and observations. A doctor or researcher in another country (for example the United States) may help us with a second opinion. To do that we would send them a copy of your child's information. **This is your choice. Saying no does not change your child's care here.**

## What would be sent
[ ] Everything in the record, with your child's name and details (identified)
[ ] Observations and results only, without name or contact details (de-identified)
Items: [ ] observation results [ ] clinician's notes [ ] attached photos or papers [ ] printed summary

## Who would receive it
Name and organisation: ____________ Country: ____________ Reason: [ ] second opinion for my child's care [ ] my own request [ ] research (de-identified only)
They have signed an agreement with us (`baa-dpa-us-recipient.md`): [ ] yes, ref ________ [ ] not required (de-identified)

## What you should know
- The copy is sent through a private link that expires in [3] days and has a separate password given to the recipient by us. We keep a record of when it was opened.
- Laws in the other country may protect your child's information differently from Nepal. We ask the recipient to protect it, to use it only for the reason above, and to return or destroy it when finished.
- We cannot take back a copy once the recipient has it, but you may tell us at any time that you no longer agree, and we will stop sharing and ask them to delete it. Erasure of the record held by this clinic follows our retention rules; a signed clinical note may have to be kept [OWNER DECISION: O-1].
- You may ask what we shared, with whom and when.
- Sharing is a different decision from the consent to collect observations.

## Consent
I am the child's parent or legal guardian. I understand the above and agree to this disclosure. (Tick) [ ] I agree to the items and recipient named above [ ] I do not agree.
Child (name or record number): ____________ Guardian name: ____________ Relationship: ____________
ID seen by staff (type only, number not copied): [ ] Citizenship [ ] Passport [ ] Other ______
Signature or thumbprint: ____________ Date: ________
Witnessed by staff member: ____________ Signature: ________ Date: ________
If the guardian cannot read: the form was read aloud in [language] by ____________ and the guardian agreed.

**Nepali version: [NEPALI TRANSLATION PLACEHOLDER: नेपाली अनुवाद, to be provided by a qualified translator and checked by Nepali counsel. The Nepali text, not only the English, must be what the guardian signs.]**

## Design notes for the clinic and engineering (not part of the form)
1. **Who consents** [OWNER DECISION: O-6, recommended default]: the parent or legal guardian, in person with staff witnessing. The code does not verify identity or relationship (dpia-lite section 5.3), so the staff check is the control.
2. **Evidence:** scan or photograph the signed form and attach it to the child's record (attachments, REC-6/7). The case-share request carries a **consent reference** (which consent, when, by whom); without it the system refuses to build an identified package (`case-consent-missing`, ADR-0011 D5).
3. **Versioning:** write the form's version (above) on the record; a new wording gets a new version; keep old versions for 6 years.
4. **Separate** from the collection consent in the app; one form per recipient and purpose; expires when the purpose ends or after [12] months, whichever is first.
5. **Withdrawal:** the clinic revokes the link, notifies the recipient, files erasure if requested, and writes the date on the record.
6. **Lawful basis** under Nepal's Privacy Act and any transfer conditions: confirm with counsel [OWNER DECISION: O-2]. Until then this consent plus the recipient agreement is the safe default.
7. **Languages:** the signed language must be one the guardian understands; keep both versions.

### Staff checklist before pressing "Share"
- [ ] Guardian signed this form; scan attached to the record; version noted.
- [ ] Recipient agreement signed (identified data) [OWNER DECISION: O-15].
- [ ] Minimum data selected; de-identified if the purpose allows.
- [ ] Link and passcode will go by **different channels**.
- [ ] Expiry and download limit are the defaults or less.
- [ ] Reminder set to confirm deletion by the recipient.
