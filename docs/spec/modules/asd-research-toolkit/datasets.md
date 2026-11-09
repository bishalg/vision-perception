# Research datasets (WP-X.11)

Decision record: `docs/spec/adr/ADR-0015-research-workbench-v2.md` R-7, R-8, R-9 (and V-5 for the rule
kinds). Builds on ADR-0012 / `technical.md` here: COH-1..3 (cohort membership), RUN-1..9 (runs and their
reproducibility record), PRV-1/PRV-2 (suppression, per-session rows), GOV-1 (RBAC). Written 2026-11-09.

A **dataset** is a frozen member list of sessions taken from a cohort at one point in time, with the
exclusion rules applied, optional clinician labels, an optional seeded split stratified by age band, a
lineage record and a data dictionary. It is immutable: a change is a new dataset that names the old one as
`lineage.parent_dataset_id`. Its identity is a content hash computed in Rust.

## Contract (`asd_contract::datasets`, `api-v1.json`)

| Type | Shape |
| --- | --- |
| `Dataset` | `id, org_id, name, description?, status (frozen, archived), cohort_id?, frozen_at, content_hash, member_count, erased_count, exclusion_rules: [ExclusionRule], exclusions: [Exclusion], label_source?: LabelSource, split?: Split, split_counts: [SplitStratumCount], lineage: Lineage, data_dictionary: [Column], created_by?, created_at, archived_at?` |
| `DatasetMember` | `session_id, participant_pseudonym?, age_months?, age_band?, split?: train/validation/holdout, label?: Label, excluded?` (`"erased"` after an erasure, R-9) |
| `ExclusionRule` | tagged by `kind`: `quality_gates {id, min_valid_ratio?, exclude_quality_verdicts[], exclude_tracking_verdicts[], min_clips_completed?}`, `tukey {id, metric, k = 1.5}`, `mad_z {id, metric, z = 3.5}`, `explicit_list {id, sessions: [{session_id, reason}]}` |
| `Exclusion` | `session_id, rule_id?, reason` (`rule_id` null for the system exclusion `no_age`) |
| `LabelSource` | `kind (review_outcomes, import), taken_at, count` |
| `Label` | `value, source, taken_at, reviewer_pseudonym?` |
| `Split` | `seed (i64), fractions {train, validation, holdout}, strata: age_band` |
| `SplitStratumCount` | `stratum, train, validation, holdout` (stratum `unbanded` for a member without a band) |
| `Lineage` | `cohort_id?, cohort_hash, parent_dataset_id?, run_id?, profile_hash, profile_version, catalog_version, baselines_version, code_version` |
| `Column` | `name, type (string, integer, number, datetime), unit?, description_key` (`dataset_column.<name>`, domain catalogue) |
| `CreateDatasetRequest` | `name, description?, cohort_id, parent_dataset_id?, run_id?, exclusion_rules[], labels?, split?: {seed?, fractions (default 0.7/0.15/0.15), strata}` |
| `DatasetPage`, `DatasetMemberPage` | `{items, next_cursor}` |
| `DatasetRef` | `{id, content_hash}`: `AnalysisDefinition.dataset` of a run started from a dataset |

Run DTOs: `AnalysisRunRequest.dataset_id` (exactly one of `cohort_id`, `cohort`, `dataset_id`),
`AnalysisDefinition.dataset` (omitted for cohort runs, so their `definition_hash` is unchanged),
`AnalysisRun.dataset_id`, `AnalysisRun.dataset_hash`. Limits: 10 000 sessions per snapshot, 20 rules,
1 000 sessions per explicit list, rule ids 1..64 characters, reasons 1..200.

Domain catalogue groups added: `dataset_status`, `dataset_split`, `label_source_kind`,
`exclusion_rule_kind`, `dataset_column` (en + ne machine draft).

## Endpoints

| Route | Claim | Notes |
| --- | --- | --- |
| `POST /v1/research/datasets` | `ExperimentWrite` | freezes from a cohort of the caller's org; `201 Dataset`; `400 validation-failed` (shape, unknown/archived cohort, unknown explicit session, rule kinds not applied yet, `labels: import`, bad fractions), `400 cohort-empty`, `400 cohort-too-large`, `409 conflict` (name). Audit `dataset.create` (counts per rule, hash, seed, no values). |
| `GET /v1/research/datasets?status&cursor&limit` | `AnalysisRead` | newest first; a superadmin passes `?org_id=` |
| `GET /v1/research/datasets/{id}` | `AnalysisRead` | another org's dataset is a 404 |
| `GET /v1/research/datasets/{id}/members?split&cursor&limit` | `AnalysisRead` + `SessionRead` | by session id; audit `dataset.members_read` once per page (PRV-2, like run rows) |
| `POST /v1/research/datasets/{id}/archive` | `ExperimentWrite` | `409 invalid-transition` the second time; audit `dataset.archive`; an archived dataset starts no new run |
| `POST /v1/orgs/{id}/analysis-runs` with `dataset_id` | `AnalysisRun` | membership from the snapshot; stores `dataset_id`, `dataset_hash` |

## Freeze algorithm

One `REPEATABLE READ` transaction:

1. The cohort (org, not archived) and the published scoring profile, baselines and catalog (the bundled
   default when nothing is published). The profile's `ageBands` place each session's age.
2. Membership through the analysis membership query (`jobs::analysis::membership`, level `score`): research
   consent, not withdrawn, real capture, the cohort's filters. Every cohort session is either a member or
   an exclusion.
3. Exclusions in this order; the first that matches names the exclusion:
   `no_age` (system, `rule_id` null), then the request's rules in order. `quality_gates`: valid-frame
   ratio below `min_valid_ratio` (`valid_ratio_below`), capture quality in the list (`quality_verdict`),
   tracking verdict in the list (`tracking_verdict`), fewer `session_clip_scores` rows than
   `min_clips_completed` (`clips_below`). `explicit_list`: the named session, with the researcher's reason.
4. Split (when asked): [`assign_split`] below.
5. Labels (when `labels: review_outcomes`): each member's signed clinical review version's disposition
   (`clinical_review_versions.status = signed`), `taken_at` = the `sign` signature's time, reviewer = `R-`
   pseudonym of the signer.
6. Pseudonyms: `P-` + 12 hex of sha256(salt, "P", participant id), `R-` + 8 hex of sha256(salt, "R", user
   id), with a 32-byte salt drawn for this freeze and never stored: stable within the dataset, not linkable
   across datasets, not recomputable.
7. Lineage, data dictionary (`datasets::data_dictionary()`), content hash; insert `datasets` and every
   member row (`UNNEST`) in the same transaction.

## Content hash

`content_hash = hex(sha256(JCS(doc)))` with RFC 8785 canonicalisation (`asd_contract::jcs`) and

```
doc = {
  "v": "dataset-hash/1",
  "members": [ {session_id, age_months, age_band, split, label: {value, source} | null} ... ]   // sorted by session_id
  "exclusions": [ {session_id, rule_id, reason} ... ]                                          // sorted
  "exclusion_rules": [ ...as requested... ],
  "split": {seed, fractions, strata} | null,
  "labels": "review_outcomes" | null,
  "lineage": {cohort_id, cohort_hash, parent_dataset_id, run_id, profile_hash, profile_version,
              catalog_version, baselines_version, code_version}
}
```

Not hashed: pseudonyms (random per freeze), label times and reviewer pseudonyms, name, description, freeze
time. So the same snapshot frozen again with the same rules, seed, label source and lineage has the same
hash (tested), and any change to a member, its split or label, an exclusion, a rule, the seed or a version
changes it. A client holding the dataset and its members recomputes it with
`asd_contract::datasets::content_hash` (the backend test does exactly that from the API responses).

## Split algorithm

`assign_split(members, seed, fractions)`:

1. Group the included members by stratum: the age band, or `unbanded`.
2. Within a stratum, order the members by `sha256(seed as 8 bytes little endian || session_id)`, ties by id.
3. Part sizes by the largest-remainder method: `floor(f * n)` per part, the remaining members one at a time
   to the parts with the largest fractional rest (ties: train, validation, holdout; a part with fraction 0
   gets none).
4. The first `train` members of the order are train, the next `validation` are validation, the rest holdout.

The result depends only on the member set, the seed and the fractions (not on input order or on a random
number generator version). Fractions: each in `[0, 1]`, `train > 0`, sum 1 (±1e-9). Without a seed the
server draws one and stores it.

## Immutability and erasure (R-9)

Migration **0029** (`0029_datasets.sql`): `datasets`, `dataset_members` (primary key `(dataset_id,
session_id)`, no foreign key to `sessions`), triggers `datasets_frozen` (only `status`/`archived_at` and a
growing `erased_count` may change) and `dataset_members_frozen` (no delete; the only update is the erasure
marking, whose `CHECK` clears every data column). `analysis_runs.dataset_id` (same-org foreign key) and
`dataset_hash`.

The erasure job (`jobs::erasure`), in the transaction that deletes the participant's sessions, marks every
member row of an erased session (`erased_at`, pseudonym, age, band, split and label cleared) and
recomputes `erased_count` of the datasets touched. The members response lists the session as
`{session_id, excluded: "erased"}`; the content hash stays the freeze-time identity. A held session (signed
clinical record) is not erased and stays a member.

## Runs from a dataset

`POST /analysis-runs` with `dataset_id`: the dataset must be the org's and `frozen`. The definition stores
the dataset's cohort definition (a record) and `dataset: {id, content_hash}`; enqueue and the job both read
the member ids from `dataset_members` (every frozen member) and resolve them with the same membership query
restricted to those ids (`membership(..., only)`): consent, withdrawal and real capture still apply, the
cohort's other filters do not; a member erased or withdrawn since the freeze is excluded `session_gone`.
Age bands come from the experiment profile as for cohort runs. A re-run of a dataset run reads the same
snapshot.

## Console (`apps/curv_console`)

`research/datasets` (list), `research/datasets/new` (cohort, name, description, quality gates, named
sessions with a reason, labels on/off, split with seed and shares), `research/datasets/[id]` (facts with
the copyable hash, lineage, split table, labels, paged members, rules and exclusions with reasons, data
dictionary, "Start run from this dataset", export link, archive). "Start run from this dataset" opens the
chosen experiment's workbench with `?dataset=<id>`: the run dialog opens with the dataset selected and its
split seed as the run seed. Runs show `dataset_id` and `dataset_hash` in the provenance block. "Datasets"
sits in the Research sidebar (`navTree`). Strings in `messages/research.ts` (`datasets`, `run.source*`;
ne drafts marked), enum and dictionary texts from `messages/domain.ts`.

## Deviations and follow-ups

- **Split and hash live in `asd-contract`**, not `asd-core` (another package owned `asd-core` at the
  time). They are pure functions with no I/O; moving them into `asd-core::datasets` (and a WASM export so
  the console can verify a hash) is a follow-up.
- **Labels** are the signed clinical review disposition (the clinician's outcome), not the review-queue
  item's free `decision` JSON, which has no fixed vocabulary.
- **`tukey`, `mad_z`** are in the schema and refused with `400` until WP-X.22 (they need per-session metric
  values); **`labels: import`** until WP-X.16.
- **Export pre-fill**: the page links to `data/exports?format=dataset&dataset=<id>`; the exports page does
  not read these parameters yet (the dataset package format is WP-X.15).
- Lineage records the published profile used to place ages, not an experiment profile; `run_id` is
  lineage only (the dataset is not derived from the run's rows).
- Routes are `/v1/research/datasets` (org from the token), not `/v1/orgs/{id}/...` like cohorts; the
  vendor names the org with `?org_id=` on the list.
