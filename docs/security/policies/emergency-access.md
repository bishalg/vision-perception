# Emergency access procedure

> **DRAFT. Requires legal review before use.** WP-E.8, 2026-11-02. Written for a small clinic with no IT staff. It does not claim that the product or the clinic is HIPAA compliant or certified. Items marked [OWNER DECISION: O-x] are recommended defaults awaiting the owner.

Maps to: 164.312(a)(2)(ii). This describes how the clinic reaches records when the usual sign-in fails, without a hidden back door.

## Rule: no vendor break-glass in v1
The vendor cannot open clinical content, even in an emergency (ADR-0008 D2; `superadmin` is denied clinical content). [OWNER DECISION: O-14] Recommended default: keep it that way. Any future break-glass for the vendor needs a separate, written, audited design and a signed customer instruction first.

## Prepare in advance (owner, once)
- [ ] At least **two** active `owner` accounts held by two different trusted people.
- [ ] Two-step sign-in enrolled for both; authenticator backups stored safely.
- [ ] Key card in a sealed envelope in a locked place; second copy elsewhere; the two keyholders are named here: ____________ / ____________.
- [ ] This page printed and kept with the key card.

## Situations and what to do
| Situation | Do |
| --- | --- |
| One person lost their phone or authenticator | Another `owner` or `admin` resets it in the console (reason required, audited), `access-control.md` |
| The only owner lost the phone | Server helper runs `asd-backend admin reset-totp --user <id>` on the server; this is audited as an operator action. Record who ran it and why |
| A clinician is away and a child needs urgent care | The treating clinician with an account looks it up. If none is available the owner may view under their own login. Do not share anyone's login. Write the reason in the clinic's own emergency log |
| Console down but records are needed | Restore per `contingency-and-backup.md`; meanwhile the child's family or the last paper note |
| Password forgotten by the only owner | An `owner` password reset is a server task by the server helper using the documented admin command; the owner proves identity to the PSO and the event is logged. [Confirm the exact command with engineering before relying on this] |
| Lost KEK/key card and no copy | Clinical text and TOTP seeds cannot be recovered; scores and records without encrypted fields remain. This is why two copies exist |

## After any emergency use
- [ ] Write one entry: who, when, why, what was opened, who approved.
- [ ] The PSO reviews the audit log for that window within 3 working days.
- [ ] If a rule was bent, treat it as an incident (`incident-response-and-breach.md`) and fix the cause.
