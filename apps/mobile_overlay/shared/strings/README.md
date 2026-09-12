# Shared palm strings (single source of truth)

`palm_strings.json` holds every user-facing string for the palm reader apps in
English (`en`), Nepali (`ne`), and Hindi (`hi`):

```json
{
  "result.title": { "en": "Palm Line Overlay", "ne": "...", "hi": "..." }
}
```

Both apps load **this same file** at runtime — there are no per-app string
tables, so a fix or translation here ships on both platforms:

- **iOS** (`apps/ios_palm_app`): bundled as a resource via `project.yml`, read
  by `Sources/Models/LocalizedStrings.swift` (`LocalizedStrings.tr`).
- **Android** (`apps/android_palm_app`): wired as an `assets` source dir in
  `app/build.gradle.kts`, read by `model/LocalizedStrings.kt`
  (`LocalizedStrings.tr`, call `LocalizedStrings.init(context)` once from
  `MainActivity`).

Rules:

- Keys are namespaced: `guidance.*`, `line.*`, `button.*`, `sheet.*`,
  `status.*`, `result.*`, `editor.*`.
- Every key must define all three languages with non-empty values.
- Lookup falls back `requested language → en → key itself`, so a missing
  translation never crashes — it just shows English.
- Run `./scripts/check_shared_strings.py` to validate the schema and to catch
  `tr("...")` keys referenced in code but missing from this file.
