# Access control policy

> **DRAFT. Requires legal review before use.** WP-E.8, 2026-11-02. Written for a small clinic with no IT staff. It does not claim that the product or the clinic is HIPAA compliant or certified. Items marked [OWNER DECISION: O-x] are recommended defaults awaiting the owner.

Maps to: 164.308(a)(3), (a)(4), (a)(5)(ii)(C)-(D), 164.312(a), (d); Privacy Act: access limited to need.

## Who is responsible
- **Privacy and Security Officer (PSO):** one named person at the clinic. Until another is named the clinic owner is the PSO [OWNER DECISION: O-11]. Write the name and phone here: ____________.
- The PSO keeps a **second owner account** held by a trusted person (see `emergency-access.md`).

## Rules
1. **One person, one login.** No shared accounts. Never share a password or an authenticator.
2. **Give the smallest role that does the job.** Roles: `owner`, `admin` (front desk in v1; creates participants and visits, cannot read clinical text), `clinician` (writes notes and summaries), `researcher`, `reviewer`, `viewer`. The vendor `superadmin` can never read clinical content (`rust/crates/asd-backend/src/auth/rbac.rs`).
3. **Two-step sign-in is required** for `owner`, `admin` and `clinician` (14-day grace from first sight, `asd-contract::auth_policy`) [OWNER DECISION: O-10 to confirm roles]. Use an authenticator app, not SMS.
4. **Passwords:** at least 12 characters, not reused elsewhere, kept in a password manager or locked drawer, never written on the screen or a sticky note.
5. **Sign out when you leave the desk.** The console also signs out after 30 minutes idle and 12 hours total [OWNER DECISION: O-10 second half].
6. **Look only at records you need for your work.** Every read of a child's record is logged with your name (audit log). Curiosity about a neighbour's child is a sanctionable event (`workforce-training.md`).

## Checklist: new person (joiner)
- [ ] PSO confirms the role needed and writes it in the access register (name, role, date, approver).
- [ ] Owner or admin creates the account (Console, Users). Smallest role.
- [ ] Person completes training and signs the confidentiality pledge (`workforce-training.md`) **before** first sign-in.
- [ ] Person sets up the authenticator within 14 days.

## Checklist: role change (mover)
- [ ] PSO approves; change the role; remove access no longer needed the same day.

## Checklist: leaver (same day, ideally before the conversation)
- [ ] Disable the user in the console (an access token stops working at once; refresh tokens are revoked).
- [ ] Revoke any enrol codes they created; remove or re-enrol any tablet they alone used (`device-and-media.md`).
- [ ] Revoke share links they created (Console, participant, Share links).
- [ ] Recover keys, key cards, tablets and any USB media. If they held the key card or backup key, follow `contingency-and-backup.md` "Key card changes".
- [ ] Record the date in the access register.

## Lost phone or authenticator
An `owner` or `admin` resets the person's two-step sign-in from the console (they type their own password and a reason, 10 to 200 characters; audited as `user.totp_reset_by_admin`). An `admin` cannot reset an `owner`. If no owner or admin can sign in, see `emergency-access.md`.

## Reviews
- **Weekly (5 minutes):** Console, Audit: look for sign-in failures you do not recognise, logins at odd hours, a user opening many records.
- **Quarterly:** go through the user list with the PSO; disable anyone who has left or no longer needs access; confirm each role. Note the date.
- **Monthly:** run `sudo ./status.sh` on the server (see `contingency-and-backup.md`).

## Requests from families
A parent may ask to see, correct or delete their child's data. The PSO logs the request, checks who is asking (the guardian recorded on the consent), and the clinician decides on corrections. Deletion: owner or admin files the erasure request in the console; signed clinical records may be held [OWNER DECISION: O-1]. Tell the family what was done and what was kept.

## Sharing a case outside the clinic
Only a clinician or owner creates a share. The child's guardian must have signed the consent in `templates/cross-border-sharing-consent.md` first, and the signed copy is attached to the record. The passcode goes to the recipient by a different channel than the link. A US recipient receives identified data only under a signed agreement (`templates/baa-dpa-us-recipient.md`) [OWNER DECISION: O-15].
