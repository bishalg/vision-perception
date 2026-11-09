# Console replay and expected-versus-actual views (WP-X.20) and contract follow-up (WP-X.19b)

Status: design 2026-11-09. Implements ADR-0015 V-1, V-2, V-3, V-4, V-6 on top of `asd_core::replay`
(WP-X.19, landed `05ca2df`). The console draws; every number comes from the Rust core through WASM.

## Data flow

| Need | Source | Notes |
| --- | --- | --- |
| Frames (`GazeFrame[]`, camelCase) | `api.getSessionRaw(id)` (`GET /sessions/{id}/raw`, audited `session.raw_read`) | already used by the sandbox; de-identified |
| Catalog clip (`Clip` with `areas`) and baselines | `effectiveConfig` (`GET /config/effective`) | the version the session was scored with when available, else current published |
| Scoring profile | effective config; draft from the workbench | `replayEvents` takes the profile for fixation knobs |
| Child's age in months | session detail (`ageMonths`) | bands need a bracket |
| Clip media | none today (V-8, WP-X.21) | wireframe fallback is the default rendering |
| Core calls | `lib/asd/ops.ts` wrappers over `replayEvents`, `replayRibbon`, `replayTrace`, `expectedBands`, `replayHeatmap`, `heatmapCohort`, `heatmapDifference` | add typed wrappers + zod schemas for the JSON envelopes; run in a Web Worker if a session exceeds 20k frames |

## Screens

### Session detail: Replay tab (`?tab=replay`, `SessionTabs` gains `replay`)

Layout (grid, content column max 1280): left, the **ReplayCanvas**; right, a **ClipList** (clips of the
session in play order, recorded-vs-expected bar per clip, V-2); below both, the **TimelineStrip**.

- `ReplayCanvas` (client component, `<canvas>` with `role="img"` and an `aria-label` that states clip,
  time and current AOI): draws the clip's AOI boxes active at the current time (fill 12 % role colour,
  1 px border, label), the gaze point (6 px) with a 300 ms trail (alpha fades), fixations as circles
  (radius = 4 px + 20 px × duration/2 s, capped), saccade lines, blink and lost segments as grey trail;
  when media exists (X.21) it is the background `<video>` and the canvas overlays it; otherwise a neutral
  frame with the clip title. Controls: play/pause (Space), 0.25x/0.5x/1x/2x, frame step (←/→), scrubber
  bound to `clipMediaTimeSec`; clip switcher. Role colours come from design tokens with a legend that
  always prints the role name (colour never alone).
- `TimelineStrip` (SVG): rows for AOI-hit ribbon (`replayRibbon` segments, role colours, hover shows AOI
  and roles), the existing `LossBar` data, scheduled targets (from the frames' `stimulusId` boundaries and
  the raw log clips when present), and a playhead synced with the canvas. Clicking seeks.
- `ClipList` entries: title, duration, recorded value of the clip's target-AOI dwell, the expected band
  (`expectedBands` for the bracket) as a horizontal band with the recorded dot, the status word from
  scoring (never a colour alone). Selecting a clip loads it in the canvas and the trace.
- `TraceChart` (SVG, under the canvas, per selected clip): `replayTrace` for `aoiDwell{targetAoi}` with
  window 1 s, step 0.25 s; the band shaded from `bands[].low..high` per segment, the mean as a dashed line,
  thresholds as labelled lines (`replay.band.*` label keys resolved through messages); x axis clip
  seconds, y axis 0–1 labelled "share of gaze on {aoi}"; hover shows t, value, n frames; playhead synced.
  Every chart has "Table" toggle and "Download CSV" of the plotted points (ADR-0015 R-15).
- `HeatmapPair`: session `replayHeatmap` (64×36, bandwidth 0.04) over the clip frame or wireframe; cohort
  panel from a run's grids when the session detail has a linked run (else hidden with a sentence); the
  difference panel; n and suppression printed.
- `QualityPanel` (exists) gets a playhead marker and shares the scrubber time via a small store.

### Research workbench: tuning view (V-6)

`PreviewPanel` gains a "Replay" toggle: two `ReplayCanvas` + `TimelineStrip` pairs, published vs draft,
driven by `replayEvents`/`replayRibbon` with the two profiles over the same frames; the parameter form on
the left re-runs them on change (debounced 150 ms). The existing scanpath stays as the default view.

### Where this is not shown

Store-build data never reaches the console; nothing here changes the iOS app. The clinician read-out
(V-7, centile strip) is WP-X.23 and not part of this package.

## Messages (en + ne, ne drafts flagged)

`research.replay.*`: tab, play, pause, speed, step, clip, aoi, roles, trail, fixation, saccade, blink, lost,
recorded, expected, table, downloadCsv, noMedia, noCohort, legend; `replay.band.expected`,
`replay.band.atypicalBelow`, `replay.band.atypicalAbove` (the core's label keys). No English in JSX.

## Mock and tests

- Mock API: `getSessionRaw` returns deterministic synthetic frames per mock session (two clips, 60 Hz,
  a fixation pattern that hits the face AOI 60 % of the time, one blink, one lost gap) so e2e and
  screenshots are stable; mock `effectiveConfig` already returns catalog and baselines.
- Unit: canvas draw model (pure functions that turn events + time into draw ops, tested without a canvas),
  trace CSV serialisation, band label resolution.
- e2e `replay.spec.ts`: open the tab, scrub, see the ribbon segments and the trace band, toggle table,
  download CSV (check header), switch clip; workbench tuning toggle renders two canvases; axe (when
  WP-O.7 lands) has no violations.
- Screenshots: add `clinic/sessions/{id}?tab=replay` to `scripts/site-screenshots.spec.ts` (local only).

## WP-X.19b: contract follow-up (asd-contract, before or with X.20)

- `ScoringProfile.metrics` gains `fixationAlgorithm` (`ivt`|`idt`, default `ivt`), `idtDispersionNorm`
  (0.05), `blinkMinSec` (0.05), `blinkMaxSec` (0.5); registry rows in the Fixation group, Frames stage,
  marked replay-only (`affects: ["replay"]`), owner-gated = false; schema regenerated; then remove the four
  `skip_serializing_if` attributes in `asd-core/src/analysis/profile.rs`.
- i18n catalogue (`domain.*.json`): the three `replay.band.*` keys in en and ne; `asd-cli i18n check` green.
- `asd-wasm` parity test unchanged (fields default).

## File ownership (X.20)

New: `src/components/replay/{ReplayCanvas,TimelineStrip,TraceChart,HeatmapPair,ClipList,drawModel,replayStore}.tsx|ts`,
`src/lib/asd/replay.ts` (wrappers + schemas), `e2e/replay.spec.ts`, `tests/replay-*.test.ts(x)`.
Changed: `SessionTabs.tsx`, `app/[org]/clinic/sessions/[id]/page.tsx`, `components/research/PreviewPanel.tsx`,
`components/QualityPanel.tsx` (playhead), `lib/api/mock.ts` (frames), `messages/research.ts`,
`scripts/site-screenshots.spec.ts` (untracked).

Gates: `npm run lint`, `npx tsc --noEmit`, `npm test`, `npm run build` (CSP check), `npm run e2e`.
