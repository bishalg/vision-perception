# Contingency, backup and restore policy

> **DRAFT. Requires legal review before use.** WP-E.8, 2026-11-02. Written for a small clinic with no IT staff. It does not claim that the product or the clinic is HIPAA compliant or certified. Items marked [OWNER DECISION: O-x] are recommended defaults awaiting the owner.

Maps to: 164.308(a)(7), 164.310(a)(2)(i), (d). Scripts are in `deploy/clinic-server/` (WP-B.18) and the helpers in `rust/README.md` "Restore helpers". Re-confirm the scripts exist and pass `test/compose_smoke.sh` after the working-tree changes are integrated.

## What is protected and how
| Thing | Control |
| --- | --- |
| Database, uploaded files, erasure ledger | `backup.sh` every night 02:30 (`systemd/asd-backup.timer`), encrypted with `age` to the public backup key; local copies **14 daily + 8 weekly** (`BACKUP_KEEP_DAILY`, `BACKUP_KEEP_WEEKLY`) [OWNER DECISION: O-12] |
| Off-site copy | `BACKUP_REMOTE` (rclone) with only encrypted files, in a region the owner chooses [OWNER DECISION: O-12, O-7]. Recommended default: one off-site copy, in a country with an adequate legal regime, never with a provider the owner has not signed terms with |
| Keys | The **key card** (printed once by `install.sh`): DATA KEY (`ASD_DATA_KEK`) and BACKUP KEY. Not in the backup on purpose |
| Proof it works | `restore-drill.sh`, first Sunday monthly 03:30 (`systemd/asd-drill.timer`), writes `/var/lib/asd/drills/<date>.PASS` or `.FAIL`; `status.sh` warns when the last backup is over a day old or the last drill is failed or over 45 days old |
| Erased data | The erasure ledger travels in each backup; `restore.sh` replays it (`asd-backend admin requeue-erasure`). Erased data may remain in older backups until they rotate out (up to about 8 weeks plus the off-site retention); this must be explained to families [OWNER DECISION: O-12] |

## Duties (checklists)
**Weekly (owner or PSO, 2 minutes)**
- [ ] `sudo ./status.sh` shows no WARN. A WARN means call the person who set the server up the same day.
**Monthly**
- [ ] The newest `.PASS` file in `/var/lib/asd/drills/` is from this month. If it is `.FAIL`, find out why (see `status.sh` output) and fix before the next week.
- [ ] Keep each PASS/FAIL file (6 years, `workforce-training.md` retention rule applies).
**Every quarter**
- [ ] Check the key card is in its sealed place and a second copy is in a different place (a bank box or trusted person). The card is the only way to read backups and clinical notes.
- [ ] Check off-site copies exist and the newest is under 2 days old.
**Yearly**
- [ ] Do a real restore onto a spare computer following "Restore on a new server" below; time it. Target: back in service in one working day.

## Key card changes
If the key card is lost, copied by someone else, or a key holder leaves: the owner rotates the data key with `asd-backend admin rotate-kek` (`rust/README.md` "Rotating the data KEK") and creates a new backup key pair; backups made under the old backup key remain readable only with the old key, so keep the old BACKUP KEY until those copies age out. Record the date in the incident log if exposure is suspected.

## Restore: same server (data damaged)
```
sudo ./restore.sh /var/lib/asd/backups/asd-<date>.tar.age
```
Paste the BACKUP KEY, type `RESTORE`. Everything after the backup time is lost: re-enter from paper and let the tablets re-upload (they keep unsent sessions). Check the printed counts against the backup's.

## Restore on a new server (server lost)
- [ ] Install Ubuntu or Debian with disk encryption, Docker, and this folder.
- [ ] `sudo ./install.sh` with the **same web address**.
- [ ] Put the DATA KEY from the key card into `ASD_DATA_KEK` in `.env`.
- [ ] Copy the newest backup (local or off-site) to the new server; `sudo ./restore.sh <file>`.
- [ ] Wait for the worker to finish the erasure replay, then `sudo ./status.sh`.
- [ ] Sign in, spot-check three children's records; tell staff what period is missing.

## Emergency mode (server down, clinic still open)
Tablets keep capturing offline and upload later. Use paper forms for clinical notes and consent; enter them when the server is back. Do not use personal e-mail or chat apps to move records. Priority order for recovery: (1) console sign-in and records, (2) tablet uploads, (3) exports and share links, (4) reports.

## Disaster contacts
Server helper: ____________ Owner: ____________ Hosting or ISP: ____________ Cloud provider (if any): ____________
