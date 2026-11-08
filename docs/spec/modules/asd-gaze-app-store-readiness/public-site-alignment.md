# Public site alignment for the EarlyGaze App Store build (2026-11-08)

Status: draft copy, not published. Binding inputs: `store-collection-build.md` (S-1..S-13), `claims-denylist.md`,
`apps/ios_asd_gaze_app/fastlane/metadata/en-US/description.txt`, `Sources/Models/ConsentDocument.swift` (version `2026-11-08.1`).
Site: https://open-spectrum-tracking.vercel.app/ (source `github.com/bishalg/open-spectrum-tracking`, `apps/web/landing`, private).
All replacement copy below follows the denylist: no research-study framing, no screening, no HIPAA, no "clinical" wording, no detection,
no biomarker, and "diagnos*" only inside a negating sentence. Quoted current phrases (the "Current" column) are the only places those words appear.

## 1. Why it matters

1. App Review follows the "Learn more" link in the listing and searches the brand; the first thing they read is this site, and today it says "research tool", "detection", "HIPAA- and GDPR-aligned" and "diagnostic assessment".
2. Guideline 1.4.1 (health claims) is judged on the app and on pages it links to; a site that implies detection or diagnosis contradicts the listing.
3. Guideline 5.1.1(i) needs a live, complete privacy policy for the app; the current `/privacy` is marked "incomplete or under review", names no controller and covers only the static site.
4. Guideline 5.1.3 treats "research tool" language as human-subject research (ethics approval, consent evidence); the listing deliberately avoids it, so the site must too.
5. The listing says "Part of Open Spectrum Tracking", so a reviewer will open this site to confirm that statement; it must describe the same observation-capture product.

## 2. Landing copy changes

Apply in the landing page component. Where the exact current sentence is not known, the "Current" column holds the phrase to find; replace the whole sentence or paragraph around it.

| # | Current phrase | Replacement |
| --- | --- | --- |
| L1 | "Bridging the gap between a parent's daily observation and a clinician's diagnostic assessment." (tagline) | "Bridging the gap between a family's daily observations and the clinic that reviews them." |
| L2 | "Passive detection. Zero intrusion." (headline) | "Quiet observation. Nothing to wear." Supporting line: "Capture happens on a phone or tablet the family already has. Nothing is interpreted on the device." |
| L3 | "This project is a research tool. It does not diagnose." (disclaimer, also footer if repeated) | "This is not a diagnostic tool. It does not measure, screen for or diagnose any condition; your clinic reviews each session." |
| L4 | "HIPAA- and GDPR-aligned handling" with "No audit or certification has been completed" | "Careful handling of children's data: sessions stay on the device unless a clinic enrols it, camera images are never saved, and there is no advertising or tracking. No external audit or certification has been completed." (Delete the regime names; the DPIA has not assessed any regime yet.) |
| L5 | "diagnostic assessment" (anywhere outside L1) | "the clinic's own review". If the sentence needs a verb: "clinicians review observations on their own console." |
| L6 | The paragraph that mentions "clinical sessions", ADOS-2 and the "diagnostic team" | "Clinics can enrol a device with a clinic code. Sessions are then delivered to the clinic's console, where clinicians review them with their own tools and decide how to use them. Open Spectrum Tracking does not replace any assessment a clinic already performs, and it does not interpret an observation for the family." Remove the ADOS-2 name and "diagnostic team". |
| L7 | Companion-app status line: "companion iOS app ... in a separate, proprietary codebase" | "EarlyGaze is the iOS observation-capture companion. It is in App Store review preparation and is not yet available. It lives in a separate codebase, shows no result on the device, and sends nothing unless a clinic enrols the device. This is not a diagnostic tool." with a link to `/earlygaze`. |
| L8 | Project description in the hero or "Architecture" intro | "Open Spectrum Tracking is an open-source observation platform: capture on the device, review by clinicians on their own console." |
| L9 | Status module "ICD-11 mapping" | "Terminology mapping (planned)". Keep the other modules (face mapping, eye tracking, sleep and activity synthetic, Wi-Fi sensing, gait) but add "in development" to each and do not describe any module as detecting or measuring a condition. |
| L10 | Nav: Architecture, Whitepaper, Roadmap, Status, GitHub | Add "EarlyGaze" (to `/earlygaze`). Footer: add "EarlyGaze Privacy" (`/privacy/earlygaze`) and "EarlyGaze Support" (`/support/earlygaze`) beside Docs and Privacy. |
| L11 | `/privacy` opening "Written but incomplete or under review; content may change." | Keep the page for the website only, but replace the opener with "This notice covers this website only. For the EarlyGaze iOS app, see EarlyGaze Privacy." and add the same controller and contact lines as section 4.1 (placeholder). |

Also scan the Whitepaper and Roadmap pages for "detect", "screen", "clinical", "biomarker", "HIPAA", "research" and apply the same replacements (open item for WP-W.1).

## 3. New page: /earlygaze

App Store "Learn more" and marketing URL. Copy below is final-draft English; keep it identical in meaning to `description.txt`.

---

**EarlyGaze Observation Capture**
*Part of Open Spectrum Tracking.*

EarlyGaze records a short gaze observation session for your clinic. The child watches short, child-friendly clips while the front TrueDepth camera notes where they look and how their face moves. Everything is processed on the device, and no photos or video are saved. Nothing is interpreted on the device, and the app shows no result.

**This is not a diagnostic tool.**
It does not measure, screen for or diagnose any condition; your clinic reviews each session. EarlyGaze does not diagnose any condition and is not a medical device. Only a qualified professional, using the clinic's analysis tools, can interpret an observation.

**How a session works**
1. Check the camera and calibrate with the operator.
2. The child watches short clips. A session takes about 2 to 5 minutes.
3. The app notes where the child looks and how their face moves.
4. The session is stored on the device, or delivered to your clinic once the device is enrolled.

**Privacy in short**
- Camera frames are processed in memory and are never saved or sent.
- Sessions are encrypted on the device and excluded from backups.
- No advertising, no analytics, no tracking, and your data is never sold.
- Every optional feature is off until you turn it on, and you can delete everything from Settings.
- Read the full [EarlyGaze privacy policy](/privacy/earlygaze).

**Device requirements**
Requires iOS 18 or later and an iPhone or iPad with Face ID (TrueDepth camera). On other devices the app can be used for settings only.

**Who it is for**
EarlyGaze is used by a parent or guardian, or by a clinic operator, to record an observation of a child. The app is not directed at children: it is operated by an adult, with the guardian's consent recorded in the app before the first session. There is no account.

**For clinics**
A clinic enrols a device with a clinic code. Until a device is enrolled, sessions stay on it. Once enrolled, sessions are delivered over an encrypted connection to the clinic's console, where clinicians review them on their own platform and keep or delete them under the clinic's own rules. The clinic decides what a session is used for. To set up a clinic, write to support@curv.engineering.

**Support**
[EarlyGaze support](/support/earlygaze), or support@curv.engineering.

Open Spectrum Tracking is an open-source observation platform. [Learn about the project](/).

---

## 4. New page: /privacy/earlygaze

> Not legal advice: owner/legal review required before publishing. Every bracketed item is a placeholder. The text follows consent document `2026-11-08.1`; if the consent text changes, this page changes in the same release.

---

# EarlyGaze privacy policy

Effective date: [TODO: date of publication]. Version 1.

## 4.1 Who we are

EarlyGaze is published by [Publisher legal name], [address], contact support@curv.engineering. (TODO H-6: replace with the organisation Apple account legal name and a postal address.)
For sessions delivered to a clinic, the clinic decides why and how the data is used (the data controller) and we operate the console on the clinic's behalf (the processor). For everything that stays on your device, no one but you has the data.

## 4.2 Scope

This policy covers the EarlyGaze iOS app only. The website at open-spectrum-tracking.vercel.app has its own, separate privacy notice.

## 4.3 What the app records

During an observation session the app saves:
- Gaze points: the spots on the screen the child looks at, many times each second.
- Head position and distance from the screen.
- Facial-expression measurements, for example how much the child smiles.
- The time the session started and how long it lasted.
- The age bracket you enter for the child.

The app also keeps a record of your consent: the consent document version, a fingerprint of its text, the time, and the name and relationship you typed. On your device this is stored with the sessions.
If you turn on optional features, the app uses the data those features need (see 4.8).
The app shows no score, result or report. Each session is saved as measurements only.

## 4.4 What the app never records

- Photos, video or audio of the session. Camera frames are processed in memory and thrown away right after.
- The child's name beyond the device: it never leaves the device.
- Advertising identifiers, location, contacts.

## 4.5 Where the data is stored

Sessions are stored on your device, encrypted, with the key kept in the iOS Keychain on that device only. They are excluded from iCloud and computer backups. If you lose the device or delete the app, the sessions are gone.

## 4.6 When data leaves the device

Nothing leaves the device unless your clinic enrols it with a clinic code. After enrolment, sessions are delivered over an encrypted connection to your clinic's console. We operate that console for the clinic. Hosting region: [TODO O-7: region and provider]. We never see camera images, because the app never saves or sends them.
Delivered sessions identify the child only by a random code, not by name. The clinic knows which family the code belongs to.
Without enrolment, nothing is sent to us or anyone else.

## 4.7 Children

EarlyGaze records observations of a child under the consent of a parent or guardian, recorded in the app before the first session. The app is operated by an adult and is not directed at children. There is no account, no advertising and no tracking. The age bracket you enter is the only age information kept.

## 4.8 Permissions the app asks for, and why

The app explains each request on its own screen before iOS asks. "Not now" is always available, and the app keeps working with that feature off. You can change any choice in iOS Settings.

| Permission | Why | Optional? |
| --- | --- | --- |
| Camera | Follows where the child looks during a session. Frames are never saved. | No. Without it the app cannot record a session. |
| Motion | Notes how the device moves during a session. | Yes |
| Notifications | Reminders you ask for. | Yes |
| Speech and microphone | Dictating a note. No audio is saved. | Yes |
| Photos | Attaching a photo you choose to a session. | Yes |
| Health | Only if you turn it on; the app asks you which items. | Yes |
| Age range | Asks Apple for the age range of the adult using the app. It is not about the child. | Yes |

## 4.9 Analytics and tracking

None. The app contains no advertising and no third-party code that collects data. It does not track you across other apps or websites. We never sell data. The App Store privacy label matches this policy.

## 4.10 Retention and deletion

- On the device: until you delete it. Go to Settings > Privacy & Your Data > Delete all to remove every session and record from the device.
- Sessions already delivered to a clinic: ask the clinic to delete them. We delete them on the clinic's instruction, except where the clinic must keep a record by law.
- Our own retention periods for the console: [TODO O-3: periods; see `docs/security/retention-policy.md`].

## 4.11 Security

Sessions are encrypted on the device; the key never leaves it. Complete file protection is on, so data is locked when the device is locked. Backups exclude the data. Delivery uses an encrypted connection. Camera frames are never stored. Access to the console is limited to the clinic's staff, and we cannot read a clinic's review content.

## 4.12 Your rights

You can ask to see the data held about your child, ask for it to be deleted, withdraw your consent, and complain to the data protection authority where you live. On the device you can delete everything and withdraw consent yourself (Settings > Privacy & Your Data); the app asks again before the next session. For delivered sessions, ask your clinic, or write to support@curv.engineering and we will pass your request to the clinic.

## 4.13 Changes to this policy

If the policy changes, we update this page and its effective date. If the change affects what the app records, the app shows a new consent and asks again before the next session.

---

### Conflicts with `dpia-lite.md` and `retention-policy.md` (owner and legal, not for publication)

The policy above follows the consent document. Differences found:

| # | Source | Says | Consent document says | Resolution used |
| --- | --- | --- | --- | --- |
| C1 | DPIA section 1 and 2 | Output is a "screening aid"; purposes P4/P5 are research datasets, norm baselines, quality improvement | No research, no screening; sessions are for the clinic to review | Policy lists no research or quality-improvement use. DPIA needs updating, or the consent needs a new version before such use |
| C2 | DPIA P7, care diary and reminders | Care diary is a purpose | Store build hides care alerts and diary (S-6) | Not in the policy |
| C3 | DPIA 4, 5.3 and `retention-policy.md` O-13 | Frame-level data (`frames.jsonl.enc`) is uploaded after enrolment, sealed with a device key | "We never see camera images"; recorded list names only measurements | The sentence stays true (frames are measurements, not images, and the server cannot read them) but the policy says "sessions" only. Decide O-13 |
| C4 | DPIA 1 | Backend stores a pseudonymous summary; optional hospital MRN per org | Child's name never leaves the device | Consistent for the app; the MRN is a clinic-side addition and is not mentioned |
| C5 | DPIA 5.3 finding 2, `retention-policy.md` 1.2 | App cannot trigger server-side erasure; erasure by org admin only; signed review or note puts the request on hold; summaries kept forever (G1) | "To delete sessions already sent to your clinic, ask your clinic"; "You can delete everything at any time" | Policy says "ask the clinic" and adds "except where the clinic must keep a record by law". Retention placeholder left open (O-1, O-3) |
| C6 | DPIA 1 | Sync summary carries the on-device record including an attention flag (S-5) | The recorded list does not name the flag | Policy lists measurements only; TODO owner: add the flag to the consent list and bump the version, or drop it from the payload |
| C7 | DPIA 6 and `retention-policy.md` G3 | Audit log keeps staff IPs with no end date | Not applicable to the guardian | Not in the policy |
| C8 | DPIA 3, O-2b | Controller and processor roles are an open owner decision | n/a | The brief fixes clinic as controller and publisher as processor; confirm before publishing |
| C9 | Consent record | Guardian name and relationship are in the consent record | Not stated whether they are uploaded | Policy says they stay with the sessions on the device; TODO verify sync payload |

## 5. New page: /support/earlygaze

---

# EarlyGaze support

Write to **support@curv.engineering**. We reply within [TODO: response time].

**What to include in a report**
- The iPhone or iPad model and the iOS version.
- The app version (Settings > About).
- What you were doing and what you expected to happen.
- A screenshot, if it shows no child or personal details.
- Do not send session data, the child's name or photos of the child by email.

**Questions and answers**

**Does the app tell me anything about my child?**
No. The app shows no result, score or report. After a session it shows only the recording quality (how many usable samples it got) and where the session is saved. This is not a diagnostic tool. It does not measure, screen for or diagnose any condition; your clinic reviews each session.

**Where are my sessions?**
On your device, encrypted and left out of backups. If your clinic has enrolled the device, sessions are also delivered to the clinic's console. The Sessions tab shows where each one is.

**How do I delete everything?**
Open Settings > Privacy & Your Data > Delete all. This removes every session and record from the device. Sessions already delivered to a clinic are deleted by asking the clinic.

**Why does it need the camera?**
The front camera follows where the child looks while they watch the clips. Frames are processed in memory and are never saved or sent.

**Which devices work?**
iOS 18 or later on an iPhone or iPad with Face ID (TrueDepth camera). On other devices the app can be used for settings only.

**How does my clinic get the sessions?**
The clinic gives you a clinic code. Enter it in the app's Privacy & Your Data settings to enrol the device. Until then, nothing leaves the device. [TODO: confirm the in-app label for enrolment.]

**How do I withdraw consent?**
In the app, go to Settings > Privacy & Your Data and withdraw your consent. The app asks again before the next session. Delete all removes the data on the device. For sessions already at a clinic, ask the clinic, or write to us and we will pass the request on.

---

## 6. App Review notes (App Store Connect "Notes", about 2,900 characters)

```
EarlyGaze is an observation-capture app for use with a clinic. It records a short gaze observation session of a child (where on the screen the child looks, head position, facial-expression measurements) while the child watches short clips, and stores it on the device. It shows no result, score, flag or report on the device. After a session it shows only recording quality (usable samples) and where the session is saved. It is not a diagnostic tool and is not a medical device. It does not measure, screen for or diagnose any condition. Analysis happens only on the clinic's own console, outside this app.

This is not research. There is no study, no investigator and no ethics approval involved. The consent screens are guardian consent for observation capture.

Device requirement: a TrueDepth camera (iPhone or iPad with Face ID) and iOS 18 or later. Capture needs that camera. On other devices only Settings can be used.

How to try it on a TrueDepth device:
1. Launch the app and read the first-launch notice. Go through the consent screens and type any guardian name and a relationship.
2. Tap Capture, then follow "Before you begin". Allow the camera when the app explains the request. Run the camera check and calibration, then watch the short clips (2 to 5 minutes).
3. The post-session screen shows a replay of where the child looked, the recording quality, where the session is saved, and the not-diagnostic notice.
4. Open the Sessions tab to see the saved session. Open Settings > Privacy & Your Data to see every toggle, permission status, the signed consent record, and Delete all.

Clinic enrolment: delivery to a clinic needs a clinic code issued by the clinic. Without a code, which is the default and the state in which you will review the app, sessions stay on the device and nothing is uploaded. No account or sign-in exists. [TODO: decide whether a reviewer clinic code is offered; do not put credentials in the repo, enter them only in App Store Connect.]

Permissions, each shown first on an in-app screen that explains the use, with "Not now" always available:
- Camera: when the first session starts. Required for capture. Frames are processed in memory and never saved.
- Motion: during a session, optional.
- Notifications: only if the user turns on reminders.
- Speech and microphone: only if the user turns on dictation for notes. No audio is saved.
- Photos: only if the user attaches a photo.
- Health: only if the user turns it on.
- Age range: the app asks for the age range of the adult user. It is optional, and the app works if it is declined.

Privacy: no analytics, no advertising, no tracking, no third-party SDK that collects data. Sessions are encrypted on the device and excluded from backups. The privacy policy and support pages are linked in the app (Settings > About) and in the listing.

Contact: support@curv.engineering
```

## 7. URL plan

Interim host `https://open-spectrum-tracking.vercel.app`. Later swap to a custom domain once H-6 lands; change only the host, keep the paths (set up a redirect from the old host).

| Where | File or key | Current value | New value |
| --- | --- | --- | --- |
| fastlane (en-US, hi, ne-NP) | `privacy_url.txt` | `https://curv.engineering/privacy/asd-gaze` | `https://open-spectrum-tracking.vercel.app/privacy/earlygaze` |
| fastlane | `privacy_policy_url.txt` | `https://curv.engineering/privacy/asd-gaze` | `https://open-spectrum-tracking.vercel.app/privacy/earlygaze` |
| fastlane | `support_url.txt` | `https://curv.engineering/support` | `https://open-spectrum-tracking.vercel.app/support/earlygaze` |
| fastlane | `marketing_url.txt` | `https://curv.engineering/asd-gaze` | `https://open-spectrum-tracking.vercel.app/earlygaze` |
| fastlane | `age_suitability_url.txt` | `https://curv.engineering/asd-gaze` | `https://open-spectrum-tracking.vercel.app/earlygaze#who-it-is-for` |
| `Config/Info-Additions.plist` | `ASDPrivacyPolicyURL` | `https://curv.engineering/privacy/asd-gaze` | `https://open-spectrum-tracking.vercel.app/privacy/earlygaze` |
| `Config/Info-Additions.plist` | `ASDSupportURL` | `https://curv.engineering/support` | `https://open-spectrum-tracking.vercel.app/support/earlygaze` |

Also change in the same commit: the line "Learn more: https://curv.engineering/asd-gaze" at the end of every `description.txt` (en-US, hi, ne-NP, android full description) and any test that pins the old URLs (`BuildConfigurationTests`). Per S-9, the listing and the in-app links must carry identical URLs. The `#who-it-is-for` anchor needs a matching heading id on the page. Add the new URLs to the claims script URL check if it pins a host.

## 8. WP-W.1 (public site alignment)

| Field | Value |
| --- | --- |
| WP | WP-W.1 |
| Tier | impl (Sonnet): mechanical, fully specified copy |
| Repo | `github.com/bishalg/open-spectrum-tracking`, `apps/web/landing` (private; needs `gh` access, which the session does not have until granted) |
| Files | The landing page component (copy rows L1-L11); new routes `earlygaze`, `privacy/earlygaze`, `support/earlygaze` (App Router `page.tsx` each, plain text pages, no client JS); header nav and footer links; sitemap entries (`sitemap.ts` or `sitemap.xml`) and `robots` unchanged; `/privacy` opener (L11) |
| Copy source | Sections 2 to 5 of this file; the placeholders in 4.1, 4.6, 4.10, 4.13 and 5 stay visible as `[TODO ...]` until the owner fills them. Do not publish `/privacy/earlygaze` with a placeholder still in it; the route can ship behind `noindex` until then |
| Gates | (1) Claims: export the rendered text of all pages and run the denylist regexes of `scripts/check_asd_claims.py` against it (rules `test`, `fda`, `hipaa`, `irb`, `clinical`, `biomarker`, `screening`, `study`, plus "diagnos*" only in negating sentences, and the mandatory sentences present on `/earlygaze` and `/support/earlygaze`). (2) Lighthouse on the four pages: accessibility and best practices at least 95. (3) Link check: every internal link and the seven URLs of section 7 return 200. (4) Text of `/earlygaze` agrees with `description.txt` (device requirements, both mandatory sentences, "Part of Open Spectrum Tracking") |
| Depends on | `gh` access to the private repo; H-6 (publisher legal name, address); O-7 hosting region; O-3 retention; owner/legal review of section 4 |
| Then | Main session updates the fastlane URL files and `Info-Additions.plist` (section 7), re-runs `python3 scripts/check_asd_claims.py --swift --prose --secrets`, `fastlane validate_metadata` and `scripts/ios_test.sh`; update the status table in `docs/roadmap_2026q4/04_asd_phased_work_packages.md` |
