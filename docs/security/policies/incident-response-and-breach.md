# Incident response and breach notification policy

> **DRAFT. Requires legal review before use.** WP-E.8, 2026-11-02. Written for a small clinic with no IT staff. It does not claim that the product or the clinic is HIPAA compliant or certified. Items marked [OWNER DECISION: O-x] are recommended defaults awaiting the owner.

Maps to: 164.308(a)(6), 164.400-414 (context for US recipients); Privacy Act: duty to protect and respond. The timelines below are recommended defaults [OWNER DECISION: O-11]; a lawyer must confirm what Nepali law and any US recipient's law require.

## What counts as an incident
A lost or stolen tablet, phone or key card; a password or authenticator shared or stolen; someone reading records without need; a share link or passcode sent to the wrong person; a file e-mailed or posted by mistake; the server stolen, flooded or showing signs of tampering; a failed or unexplained backup of the whole server; an alarm from `status.sh`; a report from a US recipient that a copy was exposed. When unsure, treat it as an incident.

## Step 1: report (anyone, immediately)
Tell the PSO (name and phone in `access-control.md`). Do not delete anything or "fix" it first. Write down what, when, who.

## Step 2: contain (PSO, within hours)
- [ ] Lost or stolen tablet: disable its device in the console; the vault is encrypted and the key stays on the device, but revoke anyway.
- [ ] Suspected account: disable the user; reset their password and two-step sign-in (`access-control.md`).
- [ ] Share link wrongly sent: revoke the link (Console, participant, Share links). Links expire in at most 7 days and passcodes are separate, which limits harm.
- [ ] Server suspected: run `sudo ./status.sh`; take the server off the network if it is attacked; do not wipe it. Keep it for evidence.
- [ ] Keys exposed (key card or `.env`): rotate the data key (`asd-backend admin rotate-kek`, `rust/README.md`) and replace the card; see `contingency-and-backup.md`.

## Step 3: assess (PSO with the owner, within 24 hours)
Use the audit log (Console, Audit; or an `audit_csv` export) and the disclosure record (participant, Disclosures) to learn **whose** data, **what** (identified or de-identified, clinical text or only scores), **who** saw or got it, and **whether it was unreadable** (encrypted devices and backups are the usual reason a loss is low risk). Fill the incident record below.

## Step 4: decide whether it is a breach to notify
Notify if identified information of a child or family was, or may have been, seen or taken by someone without a legitimate need, unless the evidence shows it was unreadable or the risk is negligible. Record the reasoning either way. [Legal review needed.]

## Step 5: notify (recommended defaults [OWNER DECISION: O-11])
| Who | When | How |
| --- | --- | --- |
| Owner | immediately | phone |
| Affected guardians | without unreasonable delay, target within 30 days, sooner when they can protect themselves | by the clinic, in plain language: what happened, what data, what we did, what they can do, a contact |
| Relevant Nepali authority | as soon as practicable, as the law requires (confirm) | written notice |
| US recipient who holds the case | within 72 hours of learning, as set in the agreement | per `templates/baa-dpa-us-recipient.md` |
| US recipient's own duty | HIPAA outer limit is 60 days from discovery for a covered entity; they decide their own notices | they inform us of what they notify |
| Operator or vendor, if involved | the same day | phone and written |

## Step 6: learn
Within 14 days: root cause, fix, owner and date; add or change a risk in `risk-analysis.md`; retrain if needed; sanction under `workforce-training.md` if warranted. Keep the record 6 years.

## Incident record
| Field | Entry |
| --- | --- |
| Number, date and time found | |
| Found by, reported to | |
| What happened | |
| Whose data, how many people, what kinds | |
| Readable by the other party? (encrypted?) | |
| Containment done and when | |
| Breach decision and reasoning | |
| Notices sent (who, when, how) | |
| Fix and owner | |
