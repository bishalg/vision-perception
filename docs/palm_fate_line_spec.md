# Fate Line Detection Spec — from palmistry text to CV code

**Source:** https://www.palmistindian.com/fate-line-money-line-in-palmistry-destiny-luck/ by Manish Paliwal (32 types, Oct 2020) — `fate line palm reading` header image `triangle-on-fate-line-702x526.jpg`.
**Purpose:** Turn descriptive palmistry cases into concrete detection primitives for `domains/palm` so we can tick them off phase-by-phase. Fate is **optional** — if absent/faint we discard (per your `~/Desktop/canonical.png` note; major focus is heart/head/life, fate later).

**Core shared:** `core/src/ridge/StegerRidgeExtractor.cpp:1` candidates → `domains/palm/src/CreaseChainLinker.cpp:520` chaining → `domains/palm/src/PalmZones.cpp:5` Saturn zones. Cases below add zone-intersection / branching / shape detectors on top.

---

## Detection primitives mapping

| # | Article case (TOC) | Image ref (article) | CV primitive needed | Use / interpretation trigger | Priority | Code TODO |
|---|---|---|---|---|---:|---|
| 1 | Beggars & Billionaire do have fate line — everyone has one | `fate-line-present-or-abscent-1024x606.jpg` | `present = chain length>40px && conf>0.55` else `false` | Gate: if `!present` → still report `status: no_fate` (billionaire may have none) | P0 | `PalmDomainPack.cpp:89` emit `fate_present` bool |
| 2 | Too much importance — fate = 15% | `fate-line-too-much-of-importance-1024x606.jpg` | Weight `fate` 0.15 in `palm_analysis.overall` | Down-weight fate in VLM prompt `VlmPromptFormatter.hpp:28` systemInstruction | P1 | `PalmDomainPack` `overall_score` |
| 3 | Role in money — need other mounts | `fate-and-other-line-importance-1024x606.jpg` | Require `fate && mercury/saturn/sun mount` fusion | If fate good but mounts grilled → downgrade `PalmZones` check | P1 | `PalmZones` grill detector |
| 4 | Fate + Mount Saturn good/bad | `fate-line-good-but-bad-saturn-mount-1-1024x606.jpg` | Detect `grill/cross/star` on `saturn_zone` (Hough cross, star peak) | If grill on Saturn & fate present → `hardship` flag | P2 | `domains/palm/src/SaturnMountAnalyzer.cpp` (new) |
| 5 | Fate + Mount Mercury | `fate-line-good-but-bad-mercury-1024x606.jpg` | Same as 4 for `mercury_zone` | Bad Mercury → `poor decision` flag even if fate good | P2 | same |
| 6 | Starts from Mount Moon | `fate-line-starts-from-mount-of-moon-1-1024x606.jpg` | `start_point` in `moon zone` (hypothenar ulnar `x<0.35, y>0.65` in `512`) | `from_moon=true` → `foreign/women help` tag | P0 | `CreaseChainLinker` `origin_zone` |
| 7 | Y-shaped Fate (on Saturn) | `fate-line-Y-shaped-1024x606.jpg` | Fork at top: one segment splits into 2 with `angle 25-60°` near `y<0.25` (Saturn) | `Y_on_saturn=true` → `accumulation` good | P1 | `CreaseChainLinker` `fork_at_y` |
| 8 | Fish/Island sign on Fate | `fate-line-fish-sign-1024x606.jpg` | Closed loop + 2 tails (`island` topology: `gapCount>0 && fork>0 && enclosed`) | Currently **negated** by author as bad, not lucky — flag `island=true` → `setback` | P2 | `CreaseChainLinker` `island` (loop detection) |
| 9 | Short fate line | `short-fate-line-1024x606.jpg` | `arc_length < 0.4*PalmHeight` but `tort<1.2` & `mercury` good | `short=true` still `high-pay possible` if Mercury prominent | P0 | length gate `>40` already allows |
| 10 | Stops at Heart (age 53) | `fate-line-ends-on-heart-line-1024x606.jpg` | Intersection `fate` with `heart_line` polyline (`ZoneIntersector`) | `stops_at_heart=true` → negatable by strong Venus/Mercury/Sun | P0 | `ZoneIntersector` |
| 11 | Broken Fate | `fate-line-broken-1024x606.jpg` | `gapCount>=1` or `chain discontinuity >35px` (`CreaseChainLinker.cpp:570` `max_gap 45`) | `broken=true` → temporary instability, age from `y` | P0 | already `gapCount` |
| 12 | Branches toward fingers (good) | `fate-line-branches-towards-fingers-1024x606.jpg` | `end fork` pointing `+y` (toward `saturn_zone`) with `angle<30°` | `branches_up=true` → good hard work | P1 | `end_branch_dir` |
| 13 | Branches toward Mars-Negative (bad) | `fate-line-branches-towards-mars-negative-1024x606.jpg` | Branch vector to `mars_negative` zone (`x<0.25 y 0.40-0.55`) | `toward_mars_negative=true` → loss flag | P1 | zone vector |
| 14 | Double / Parallel Fate | `double-fate-line-1024x606.jpg` | `2` parallel chains `dist 8-25px` overlap `y>0.5` length | `double=true` → dual income, extra pillar | P1 | `parallel_chain` detector |
| 15 | Age on Fate (35/53) | `age-on-fate-line-1024x606.jpg` | Map `y` param to age: `heart 53, head 35, 33, 28` via linear `y` → `age` | `ageAt(y)` for report | P0 | `PalmDomainPack` `age_on_fate()` |
| 16 | No Fate line | `no-fate-line-1024x606.jpg` | `fate_present==false` && mounts good → `unlimited possibilities` | Positive if Mercury/Mars/Venus good | P0 | as #1 |
| 17 | Island on Fate (loss) | `isalnd-on-fate-line-1024x606.jpg` | Loop `enclosed area 4-15px` | `island=true` → elongated financial distress | P2 | loop |
| 18 | Triangle on Fate (property) | `triangle-on-fate-line-1024x606.jpg` | `3` segments forming closed triangle `area>30` | `triangle=true` → property purchase luck | P2 | triangle |
| 19 | Cross on Fate (loss) | `cross-on-fate-line-1024x606.jpg` | `X` intersection `2` segments `cross angle 70-110°` | `cross=true` → job loss; strength ∝ `tort*len` | P2 | cross |
| 20 | Square on Fate (preservation) | `square-on-fate-line-1-1024x606.jpg` | `4` segments rectangle `area>40` enclosing gap | `square=true` → preservation from loss | P2 | square |
| 21 | Stops at Head (age 35) | `fate-line-ends-on-head-line-1024x606.jpg` | Intersection `fate` with `head_line` | `stops_at_head=true` → trouble 35 | P0 | as #10 |
| 22 | Joined with Life (self-made) | `fate-and-life-line-joint-at-start-1024x606.jpg` | `start` within `10px` of `life_line` lower `y>0.75` | `joined_life_start=true` → self-effort | P1 | proximity |
| 23 | Fate & Sun cross | `fate-and-sun-line-cross-each-other-1024x606.jpg` | Intersection `fate` with `sun` (apollo vertical) | `cross_sun=true` → neutral if both firm | P1 | as #10 |
| 24 | Fate & Life cross (Mars negative) | `fate-line-crosses-the-life-line-1024x606.jpg` + `fate-line-moves-towards-mars-negative-1024x606.jpg` | `fate` crosses `life` from radial side toward `mars_negative` | `cross_life_to_mars=true` → fire loss | P1 | as #13 |
| 25 | Merged with Head | `fate-line-and-headline-merged-at-start-1024x606.jpg` + `fate-line-originating-form-headline-1-1024x606.jpg` | Overlap `>30px` collinear `col>0.85` with `head` | `merged_head=true` → mental idea changes life | P1 | `colinear_overlap` |
| 26 | Parallel with Life (till 32) | `fate-line-and-life-line-run-parallel-1-1024x606.jpg` | Parallel `dist 5-15px` overlapping `y 0.75-0.45` | `parallel_life=true` → own effort till 32 | P1 | as #14 |
| 27 | Long Fate | `long-fate-line-1024x606.jpg` | `length >0.70*palmHeight` `deep/clear` (`conf>0.75`) | `long=true` ≠ more money, just stability | P0 | length tier |
| 28 | Forked toward fingers (good) | `forked-fate-line-1024x606.jpg` | Trident `3` tips toward `saturn` `y<0.25` | `forked_up=true` → excellent | P1 | fork count 2 |
| 29 | Luck/Career line alias | `fate-line-luck-line-1024x606.jpg` + `career-line-1024x606.jpg` | Alias handling `luck==fate` | `career_stable` if long | P1 | alias |
| 30 | Government job ( इंडिया ) | `fate-line-and-government-job-1024x606.jpg` | Not fate alone — needs `sun+jupiter+mercury` triple | Future `GovernmentJobPack` | P3 | `domains/government` |

---

## How to use

* **Routing in code:** `domains/palm/src/FateLineAnalyzer.cpp` (new) will expose `struct FateReport { bool present; bool broken; bool isY; bool hasIsland; ... }` called from `PalmDomainPack.cpp:89` after `CreaseChainLinker`. Start with P0 (present/stops/broken/long/age/no_fate) then P1 branches, P2 shapes.
* **Images:** Download article images via `wget https://www.palmistindian.com/wp-content/uploads/2020/10/<name>.jpg` into `data/fixtures/palm_fate/` for `hand_inspector_cli --image <fixture> --out-full-overlay` regression.
* **Broken vs smooth:** Your `~/Desktop/canonical.png` broken fate (2 purple strokes) is canonical for `gapCount` test — keep as `data/samples/mobile_captures/01_left_USER_CORRECTED.png` with `user_polylines.json` as weak label for retune.

**Code array (single source of truth):** `domains/palm/include/curv/domain/FateLineCases.hpp` — `AllFateCases()` segregates every figure above into 7 groups (`Presence / Origin / Course / Termination / Marks / Age / Context`) with `id / blog_anchor / image_url / image_file / primitive / use / priority / implemented`. Flip `implemented` per case as detectors land. P0 first (present/stops/broken/age), then P1 branches, then P2 shapes.

**Prediction note:** every case feeds the reading report, and the human can
override any of them in the app editor — overrides become labels per
[`product_vision.md`](product_vision.md).

**Figure manifest:** `data/fixtures/palm_fate/manifest.json` (+ `README.md`) groups the same image basenames for visual review without vendoring copyrighted binaries.

Status: P0 + P1 implemented in `domains/palm/src/FateLineAnalyzer.cpp:1` (wired as `fate_analysis` in `domains/palm/src/PalmDomainPack.cpp:1`, 19/30 cases `implemented=true` in header, `tests/unit/test_fate_line_analyzer.cpp:1` 6/6 green, desktop smoke `01_left` → `present/broken/stops/age` live). P2 shapes remain heuristic stubs; Context P3 is future work.
