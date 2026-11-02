# Device and media policy

> **DRAFT. Requires legal review before use.** WP-E.8, 2026-11-02. Written for a small clinic with no IT staff. It does not claim that the product or the clinic is HIPAA compliant or certified. Items marked [OWNER DECISION: O-x] are recommended defaults awaiting the owner.

Maps to: 164.310(a)-(d), 164.312(a)(2)(iv), 164.308(a)(5)(ii)(B).

## Clinic tablets and phones
- [ ] Clinic-owned only (no personal devices for capture). A passcode of at least 6 digits (not 1234), Face ID/Touch ID allowed, auto-lock 2 minutes.
- [ ] Software updates installed within 30 days; only the clinic's app installed beyond the basics. The app stores data in an encrypted vault, excluded from device backups (`threat-model.md` M19).
- [ ] "Find My" on; the PSO knows how to mark a device lost.
- [ ] Kept in a locked cabinet overnight.
- [ ] No screenshots or photos of child data on personal phones.
- [ ] **Lost or stolen:** tell the PSO immediately; disable the device in the console; follow `incident-response-and-breach.md`. Unsent sessions on the device are lost.
- [ ] **Retire or hand over:** make sure everything has uploaded, run "delete all observation data" in the app, remove the device in the console, erase the tablet (Settings, Erase All Content), record it.

## Device register (keep up to date)
| Tablet / serial | Assigned to | Date issued | Date retired and how wiped |
| --- | --- | --- | --- |
| | | | |

## The server
- [ ] A locked room or cabinet; only the owner and server helper have the key. Visitors never alone.
- [ ] Disk encryption on (`install.sh` refuses an unencrypted disk unless `--allow-unencrypted-disk` is used; do not use that flag for patient data).
- [ ] On a surge protector or small UPS; in a dry place.
- [ ] Operating system security updates monthly; after any upgrade run `sudo ./status.sh`.
- [ ] No browsing, e-mail or other software on the server.
- [ ] Only ports 80 and 443 open (Caddy); the database and file store are never exposed.
- [ ] **Disposal:** before selling, repairing or scrapping, take a backup, move the service, then wipe the disk with a full overwrite or destroy it. Because the disk is encrypted, destroying the key is a second layer. Write down date and method.

## Workstations (computers that open the console)
- [ ] Clinic computers or the person's own with a screen lock, updated browser and no extensions you do not know. No public computers (internet cafes).
- [ ] Do not save downloaded exports to the desktop; delete them after use; exports expire in 72 hours by default.
- [ ] Screens not visible to families in the waiting area.

## Removable media and paper
- [ ] USB sticks: use only for the key-card copy or restore tests; label, lock away, wipe when done. Never copy records to a personal stick.
- [ ] The **key card** and BACKUP KEY are the most valuable paper in the clinic: sealed, signed across the seal, two locations.
- [ ] Printed summaries and consent forms are kept in a locked cabinet; shred when the retention period ends (`retention-policy.md`).
- [ ] Off-site backup drives, if used, are encrypted files only and kept by the owner.

## Visitors and repairs
Repair technicians are supervised; they never receive a logged-in device. A device sent for repair is first removed from the console and wiped.
