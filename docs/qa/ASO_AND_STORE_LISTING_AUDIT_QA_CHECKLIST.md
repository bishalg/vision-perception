# ASO & Store Listing Audit QA Checklist — Vision Perception

**Target:** `apps/ios_palm_app`, `apps/android_palm_app`, `apps/ios_asd_gaze_app`  
**Reference Specification:** [`docs/architecture/APP_STORE_OPTIMIZATION_ASO_SSOT.md`](../architecture/APP_STORE_OPTIMIZATION_ASO_SSOT.md)  
**Agent Skill Standard:** `.agents/skills/aso-audit`, `.agents/skills/metadata-optimization`

---

## 1. Automated Pre-Submission Validation Script

Run before releasing either app:

```bash
python3 -c '
import os, sys

apps = [
    ("PalmReader", "apps/ios_palm_app/fastlane/metadata/en-US", "apps/android_palm_app/fastlane/metadata/android/en-US"),
    ("EarlyGaze", "apps/ios_asd_gaze_app/fastlane/metadata/en-US", "apps/ios_asd_gaze_app/fastlane/metadata/android/en-US")
]

errors = 0
for name, ios_p, android_p in apps:
    ios_name = open(ios_p + "/name.txt").read().strip()
    ios_sub = open(ios_p + "/subtitle.txt").read().strip()
    ios_kw = open(ios_p + "/keywords.txt").read().strip()
    
    android_title = open(android_p + "/title.txt").read().strip()
    android_short = open(android_p + "/short_description.txt").read().strip()
    android_full = open(android_p + "/full_description.txt").read().strip()
    
    if len(ios_name) > 30:
        print(f"❌ {name} iOS: Name > 30 chars ({len(ios_name)})")
        errors += 1
    if len(ios_sub) > 30:
        print(f"❌ {name} iOS: Subtitle > 30 chars ({len(ios_sub)})")
        errors += 1
    if len(ios_kw) > 100:
        print(f"❌ {name} iOS: Keywords > 100 chars ({len(ios_kw)})")
        errors += 1
    if " " in ios_kw:
        print(f"⚠️ {name} iOS: Keywords contain spaces after commas")
        errors += 1
    if len(android_title) > 30:
        print(f"❌ {name} Android: Title > 30 chars ({len(android_title)})")
        errors += 1
    if len(android_short) > 80:
        print(f"❌ {name} Android: Short description > 80 chars ({len(android_short)})")
        errors += 1
    if len(android_full) > 4000:
        print(f"❌ {name} Android: Full description > 4000 chars ({len(android_full)})")
        errors += 1

if errors == 0:
    print("✅ ALL METADATA LIMITS VALIDATED SUCCESSFULLY!")
    sys.exit(0)
else:
    print(f"❌ FAILED with {errors} errors.")
    sys.exit(1)
'
```

---

## 2. Platform QA Verification Gates

### A. iOS App Store
- [ ] Title strictly ≤ 30 characters.
- [ ] Subtitle strictly ≤ 30 characters without repeating words from Title.
- [ ] Keywords strictly ≤ 100 characters, single words, comma-separated with **no spaces**.
- [ ] Zero unapproved medical claims; mandatory disclaimer present in description.

### B. Google Play Store
- [ ] Title strictly ≤ 30 characters.
- [ ] Short Description strictly ≤ 80 characters with 2–3 high-intent keywords.
- [ ] Full Description strictly ≤ 4,000 characters with 2–3% keyword density.
