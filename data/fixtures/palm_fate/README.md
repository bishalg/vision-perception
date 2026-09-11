# Palm fate figures — segregated groups (reference only)

Source: https://www.palmistindian.com/fate-line-money-line-in-palmistry-destiny-luck/ (35y experience, logic-based).

Images are **not vendored** (copyright). `manifest.json` segregates every figure
into 7 detection groups matching `domains/palm/include/curv/domain/FateLineCases.hpp`:

- `Presence` (present / no_fate) — gate: absent is valid
- `Origin` (moon / life-joint / headline-origin) — `start_point` zone test
- `Course` (short / long / broken / double / Y / fork / branches / parallel) — chaining + fork detector
- `Termination` (stops heart 53 / stops head 35 / merged / cross sun/life) — `ZoneIntersector`
- `Marks` (island-fish / triangle / cross / square / saturn-mercury grill) — shape detectors, P2
- `Age` (28 / 33 / 35 / 53) — `y -> age` map
- `Context` (15% weight / luck-career alias / govt-job) — report weighting, no geometry

To review a figure: open `manifest.json`, take `base_url_patterns + image file`,
e.g. `.../2020/10/fate-line-broken-1024x606.jpg`. For local regression, download
manually to this folder (gitignored) and run:
`./build/tools/hand_inspector_cli --image data/fixtures/palm_fate/<file> --out-full-overlay /tmp/x.png`.
