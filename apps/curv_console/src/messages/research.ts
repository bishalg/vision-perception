/**
 * User-visible strings of the researcher toolkit (ADR-0012; RUI-*), English and Nepali (ADR-0013 principle 2). Both
 * locales have the same shape (`ResearchMessages`); a `ne` text marked `// ne: draft` is a machine draft until WP-L.3
 * reviews it. WP-X.5 / WP-X.7 add the page strings here.
 *
 * Parameter labels and help (PRM-5) are not here: they live once in the Rust domain catalogue (keys
 * `param.<id>.label|help`) and are read with `paramText(locale, spec)` from `@/lib/research/paramText`.
 */
import type { ExperimentStatus, PARAM_UNITS, REPROCESS_LEVELS, RUN_KINDS, RunStatus } from "@/lib/api/research";

type ParamUnit = (typeof PARAM_UNITS)[number];

export interface ResearchMessages {
  nav: string;
  title: string;
  /** PRV-6: on every research page and export README; the English sentence is always present. */
  disclaimer: string;
  notAvailable: string;
  /** A suppressed cell (PRV-1): never shown as a number. */
  suppressedCell: (k: number) => string;
  runStatus: Record<RunStatus, string>;
  reprocess: Record<(typeof REPROCESS_LEVELS)[number], string>;
  level: { auto: string; score: string; frames: string };
  ownerGated: string;
  /** Group headings of the registry form, keyed by the first path segment (a root-level knob is its own group). */
  groups: Record<string, string>;
  units: Record<ParamUnit, string>;
  form: {
    published: string;
    publishedValue: (v: string) => string;
    defaultValue: (v: string) => string;
    reset: string;
    range: (min: string, max: string) => string;
    outside: (range: string) => string;
    slider: (label: string) => string;
    diffCount: (n: number) => string;
    captureOnlyPreview: string;
    on: string;
    off: string;
    pointCount: (n: number) => string;
    x: string;
    y: string;
    addPoint: string;
    removePoint: string;
    bandId: string;
    minMonths: string;
    maxMonths: string;
    addBand: string;
    removeBand: string;
    pointsOf: (label: string, n: number) => string;
  };
  experiments: {
    title: string;
    intro: string;
    denied: string;
    newTitle: string;
    newIntro: string;
    newLink: string;
    name: string;
    description: string;
    startFrom: string;
    startPublished: string;
    create: string;
    creating: string;
    empty: string;
    cols: { name: string; status: string; base: string; updated: string; lastRun: string };
    status: Record<ExperimentStatus, string>;
    readOnly: string;
    recentRuns: string;
    cohorts: string;
    noRuns: string;
    noCohorts: string;
    allExperiments: string;
    nameRequired: string;
  };
  workbench: {
    title: string;
    back: string;
    nameLabel: string;
    descriptionLabel: string;
    save: string;
    saving: string;
    saved: string;
    problems: (n: number) => string;
    serverRejected: string;
    checking: string;
    valid: string;
    invalid: (n: number) => string;
    coreUnavailable: string;
    runOnCohort: string;
    promote: string;
    promoted: (version: string) => string;
    readOnly: string;
    base: (v: string) => string;
    profileHash: string;
    promoteVersion: string;
    promoteNotes: string;
    promoteSubmit: string;
    promoteCancel: string;
    promoteNoRun: string;
  };
  preview: {
    title: string;
    intro: string;
    pickLegend: string;
    selectedCount: (n: number, max: number) => string;
    maxReached: (max: number) => string;
    noSessions: string;
    noBaselines: string;
    age: (id: string) => string;
    defaultAge: string;
    ageHint: string;
    framesBadge: string;
    scoreBadge: string;
    level: string;
    published: string;
    draft: string;
    composite: string;
    flag: string;
    flagLabel: Record<"green" | "yellow" | "red" | "unreliable", string>;
    clips: string;
    clip: string;
    subscore: string;
    lowConfidence: string;
    paramNotApplied: (id: string) => string;
    framesUnavailable: string;
    coreUnavailable: string;
    computing: string;
    failed: (m: string) => string;
    loadFailed: string;
    scanpath: string;
    scanpathEmpty: string;
    scanpathLabel: string;
    trackOf: string;
    sessionCard: (id: string) => string;
    unchanged: string;
    changed: string;
    corePanic: string;
  };
  sweep: {
    title: string;
    intro: string;
    knob: string;
    from: string;
    to: string;
    step: string;
    run: string;
    running: string;
    tooMany: (max: number) => string;
    session: string;
    curve: (label: string) => string;
    publishedMarker: string;
    noKnobs: string;
    noSession: string;
    cols: { value: string; composite: string; flag: string };
    invalid: string;
  };
  run: {
    title: string;
    intro: string;
    cohort: string;
    cohortPick: string;
    level: string;
    seed: string;
    seedHint: string;
    preview: string;
    previewing: string;
    submit: string;
    submitting: string;
    cancel: string;
    previewCounts: (n: string, frames: number, score: number) => string;
    bandRow: (band: string, n: string) => string;
    excluded: (noConsent: number, withdrawn: number, unscorable: number, noAge: number, quality: number) => string;
    budget: (sessions: string, cells: number) => string;
    noCohorts: string;
    saveFirst: string;
    denied: string;
  };
  cohorts: {
    title: string;
    intro: string;
    denied: string;
    newTitle: string;
    newIntro: string;
    newLink: string;
    readOnly: string;
    empty: string;
    allCohorts: string;
    cols: { name: string; definition: string; created: string; status: string };
    active: string;
    archived: string;
    name: string;
    nameRequired: string;
    ageBands: string;
    ageBandsHint: string;
    from: string;
    to: string;
    qualityVerdicts: string;
    trackingVerdicts: string;
    minValidRatio: string;
    requireFrames: string;
    includeSynthetic: string;
    preview: string;
    previewing: string;
    previewTitle: string;
    save: string;
    saving: string;
    definition: string;
    any: string;
    definitionHash: string;
    archive: string;
    archiving: string;
    archivedNote: string;
    summary: {
      ageBands: (v: string) => string;
      range: (from: string, to: string) => string;
      quality: (v: string) => string;
      tracking: (v: string) => string;
      minValid: (v: string) => string;
      framesOnly: string;
      synthetic: string;
      all: string;
    };
  };
  runs: {
    title: string;
    intro: string;
    empty: string;
    allRuns: string;
    filterStatus: string;
    allStatuses: string;
    apply: string;
    cols: { status: string; kind: string; experiment: string; cohort: string; sessions: string; created: string };
    kind: Record<(typeof RUN_KINDS)[number], string>;
    unknownExperiment: string;
    unknownCohort: string;
    heading: (id: string) => string;
    progress: (done: number, total: number, phase: string) => string;
    updating: string;
    pollStopped: string;
    cancel: string;
    cancelling: string;
    cancelled: string;
    rerun: string;
    rerunning: string;
    exportResults: string;
    failure: { title: string; budget_exceeded: string; "result-too-large": string; fallback: string; code: (c: string) => string };
    provenance: {
      title: string;
      intro: string;
      copy: string;
      copied: string;
      copyField: (label: string) => string;
      notRecorded: string;
      fields: Record<string, string>;
    };
    results: {
      title: string;
      sessions: string;
      byLevel: string;
      excluded: string;
      paramsNotApplied: string;
      suppressedCells: string;
      synthetic: string;
      drift: (added: number, removed: number) => string;
      published: string;
      draft: string;
      overall: string;
      band: (b: string) => string;
      composite: string;
      metric: (id: string) => string;
      flagCounts: string;
      flagTransitions: string;
      qualityTransitions: string;
      deltaTitle: string;
      noSummary: string;
    };
    sessionsLink: string;
    sessionsTitle: string;
    backToRun: string;
    sessionCols: { session: string; level: string; band: string; published: string; draft: string; delta: string; warnings: string };
    noSessions: string;
    next: string;
    first: string;
  };
  charts: {
    histogram: (label: string) => string;
    baseline: (low: string, high: string) => string;
    noBaseline: string;
    bin: (from: string, to: string, count: number) => string;
    matrixFrom: string;
    matrixTo: string;
    noTransition: string;
    deltaBars: (label: string) => string;
    interval: string;
    noData: string;
  };
  /** WP-X.9: the sweep and age-profile sections of a run page (SWP-3, AGE-1). */
  analysis: {
    sweepTitle: string;
    sweepIntro: (cells: number) => string;
    axis: (index: number, label: string, id: string) => string;
    publishedValue: (values: string) => string;
    publishedMarker: string;
    publishedOutline: string;
    offGrid: (values: string) => string;
    compositeMean: string;
    greenShare: string;
    metricMean: (id: string) => string;
    heatmap: (quantity: string, x: string, y: string) => string;
    heatLegend: (low: string, high: string) => string;
    heatSuppressed: (k: number) => string;
    cellTitle: (values: string, n: string, value: string) => string;
    tableTitle: string;
    showTable: string;
    cols: {
      values: string;
      n: string;
      green: string;
      yellow: string;
      red: string;
      unreliable: string;
      compositeMean: string;
      compositeMedian: string;
      changed: string;
    };
    publishedRow: string;
    ageTitle: string;
    ageIntro: string;
    baselines: (published: string, draft: string) => string;
    threshold: (t: number) => string;
    bandTitle: (band: string, from: number, to: number) => string;
    ageCaption: (band: string) => string;
    ageCols: {
      expectation: string;
      n: string;
      median: string;
      range: string;
      publishedShares: string;
      draftShares: string;
      publishedPercentile: string;
      draftPercentile: string;
    };
    expectation: (stimulus: string, segment: string, metric: string) => string;
    shares: (onTarget: string, monitor: string, atypical: string) => string;
    range: (low: string, high: string, expected: string) => string;
    noRange: string;
    noExpectations: string;
    unreadable: string;
  };
  /** Server-action failures, by problem slug; `fallback` otherwise. */
  errors: Record<string, string>;
  /** WP-X.9b: the kind picker and the sweep axis editor of the run dialog (RUI-5, SWP-1). */
  runSetup: {
    kind: string;
    kinds: Record<(typeof RUN_KINDS)[number], string>;
    ageNote: string;
    axes: string;
    axis: (n: number) => string;
    knob: string;
    from: string;
    to: string;
    step: string;
    addAxis: string;
    removeAxis: string;
    framesOnly: string;
    noKnobs: string;
    published: (value: string) => string;
    cells: (n: number, max: number) => string;
    cellsUnknown: string;
    metrics: string;
    metricsHint: string;
    problems: {
      param: string;
      level: string;
      range: string;
      step: string;
      bounds: (min: number, max: number) => string;
      whole: string;
      fine: (step: number) => string;
      tooMany: (steps: number, max: number) => string;
      axes: string;
      sameKnob: string;
      cells: (n: number, max: number) => string;
    };
  };
}

const en: ResearchMessages = {
  nav: "Research",
  title: "Research",
  disclaimer: "This is not a diagnostic tool.",
  notAvailable: "This part of the research area is not available yet.",
  suppressedCell: (k) => `n < ${k}`,
  runStatus: {
    queued: "Queued",
    running: "Running",
    succeeded: "Finished",
    failed: "Failed",
    cancelled: "Cancelled",
  },
  reprocess: {
    score: "score",
    metrics: "metrics",
    frames: "frames",
    capture_only: "capture only",
  },
  level: { auto: "Automatic", score: "Score level", frames: "Frame level" },
  ownerGated: "Fenced for clinical use; analysis only",
  groups: {
    flag: "Analysis labels",
    lowConfidenceValidRatio: "Low-confidence gate",
    ageBands: "Age bands",
    qualityGate: "Quality gate",
    trackingQuality: "Tracking quality",
    metrics: "Gaze metrics",
    aoi: "Areas of interest",
    norms: "Norms",
    review: "Review",
    display: "Result display",
    sessionQuality: "Session quality",
  },
  units: {
    none: "",
    ratio: "ratio",
    percent: "%",
    seconds: "s",
    millis: "ms",
    cm: "cm",
    mm: "mm",
    deg: "°",
    deg_per_sec: "°/s",
    rad: "rad",
    hz: "Hz",
    norm_units: "screen units",
    norm_units_per_sec: "screen units/s",
    months: "months",
    count: "",
  },
  form: {
    published: "Published",
    publishedValue: (v) => `Published: ${v}`,
    defaultValue: (v) => `Default: ${v}`,
    reset: "Reset to default",
    range: (min, max) => `${min} to ${max}`,
    outside: (range) => `Outside the allowed range (${range}).`,
    slider: (label) => `Slider for ${label}`,
    diffCount: (n) => (n === 1 ? "1 parameter differs from the published profile" : `${n} parameters differ from the published profile`),
    captureOnlyPreview: "Applies at capture only; not changed in the preview",
    on: "On",
    off: "Off",
    pointCount: (n) => `${n} point${n === 1 ? "" : "s"}`,
    x: "x",
    y: "y",
    addPoint: "Add point",
    removePoint: "Remove point",
    bandId: "Band id",
    minMonths: "From (months)",
    maxMonths: "To (months)",
    addBand: "Add band",
    removeBand: "Remove band",
    pointsOf: (label, n) => `${label}: point ${n}`,
  },
  experiments: {
    title: "Experiments",
    intro: "An experiment is a draft scoring profile you tune against recorded sessions. Nothing here changes the published profile or any stored session.",
    denied: "Your role cannot view research experiments.",
    newTitle: "New experiment",
    newIntro: "Start from the published scoring profile or from an existing experiment.",
    newLink: "New experiment",
    name: "Name",
    description: "Description",
    startFrom: "Start from",
    startPublished: "Published scoring profile",
    create: "Create experiment",
    creating: "Creating…",
    empty: "No experiments yet.",
    cols: { name: "Name", status: "Status", base: "Base profile", updated: "Updated", lastRun: "Last run" },
    status: { draft: "Draft", archived: "Archived", promoted: "Promoted" },
    readOnly: "Viewing only: creating and editing experiments is limited to owners and researchers.",
    recentRuns: "Recent runs",
    cohorts: "Cohorts",
    noRuns: "No runs yet.",
    noCohorts: "No cohorts yet.",
    allExperiments: "All experiments",
    nameRequired: "A name is required.",
  },
  workbench: {
    title: "Workbench",
    back: "Back to experiments",
    nameLabel: "Name",
    descriptionLabel: "Description",
    save: "Save experiment",
    saving: "Saving…",
    saved: "Saved.",
    problems: (n) => `Fix ${n} problem${n === 1 ? "" : "s"} before saving.`,
    serverRejected: "The server rejected the experiment:",
    checking: "Checking with the Rust core…",
    valid: "The Rust core accepts this profile.",
    invalid: (n) => `The Rust core found ${n} problem${n === 1 ? "" : "s"}:`,
    coreUnavailable: "The Rust core could not be loaded, so values are not checked here. The server checks them on save.",
    runOnCohort: "Run on cohort",
    promote: "Promote to draft",
    promoted: (version) => `Promoted as scoring profile draft ${version}.`,
    readOnly: "Viewing only: editing experiments is limited to owners and researchers.",
    base: (v) => `Based on profile ${v}`,
    profileHash: "Profile hash",
    promoteVersion: "New scoring profile version",
    promoteNotes: "Notes",
    promoteSubmit: "Promote",
    promoteCancel: "Cancel",
    promoteNoRun: "Run the experiment on a cohort before promoting it.",
  },
  preview: {
    title: "Live preview",
    intro: "Re-scores the selected sessions in your browser with the published profile and with this unsaved draft. Nothing is written.",
    pickLegend: "Sessions to preview",
    selectedCount: (n, max) => `${n} of ${max} selected`,
    maxReached: (max) => `At most ${max} sessions at a time.`,
    noSessions: "No sessions to preview.",
    noBaselines: "No published normative baselines, so nothing can be re-scored.",
    age: (id) => `Age in months for session ${id}`,
    defaultAge: "Default age (months)",
    ageHint: "Sessions do not carry the child's age; set it per session where you know it.",
    framesBadge: "frames",
    scoreBadge: "score only",
    level: "Level",
    published: "Published",
    draft: "Draft",
    composite: "Composite",
    flag: "Analysis label",
    flagLabel: { green: "within expected range", yellow: "near range edge", red: "outside expected range", unreliable: "unreliable" },
    clips: "Per-clip subscores",
    clip: "Clip",
    subscore: "Subscore",
    lowConfidence: "low confidence",
    paramNotApplied: (id) => `param_not_applied: ${id}`,
    framesUnavailable: "Frame-level re-processing needs a newer Rust core; showing the score-level result.",
    coreUnavailable: "The Rust core could not be loaded.",
    computing: "Computing…",
    failed: (m) => `Could not re-score: ${m}`,
    loadFailed: "Could not load the stored session records.",
    scanpath: "Scanpath",
    scanpathEmpty: "No trace yet for this session.",
    scanpathLabel: "Gaze trace with fixations, published and draft overlaid",
    trackOf: "Published trace",
    sessionCard: (id) => `Session ${id}`,
    unchanged: "unchanged",
    changed: "changed",
    corePanic: "The Rust core stopped unexpectedly; it is reloaded for the next change.",
  },
  sweep: {
    title: "Single-session sweep",
    intro: "Re-scores one session at each step of one numeric parameter. Runs in your browser; no job is created.",
    knob: "Parameter",
    from: "From",
    to: "To",
    step: "Step",
    run: "Run sweep",
    running: "Sweeping…",
    tooMany: (max) => `At most ${max} steps.`,
    session: "Session",
    curve: (label) => `Composite score against ${label}`,
    publishedMarker: "Published value",
    noKnobs: "No numeric parameter can be swept at score level.",
    noSession: "Select a session in the preview first.",
    cols: { value: "Value", composite: "Composite", flag: "Analysis label" },
    invalid: "Enter a range inside the parameter's limits.",
  },
  run: {
    title: "Run on cohort",
    intro: "Evaluates this experiment against every session of a saved cohort. The run reads stored data and writes nothing to the sessions.",
    cohort: "Cohort",
    cohortPick: "Choose a cohort",
    level: "Level",
    seed: "Seed (optional)",
    seedHint: "Whole number; the same seed gives the same confidence intervals.",
    preview: "Preview counts",
    previewing: "Counting…",
    submit: "Start run",
    submitting: "Starting…",
    cancel: "Cancel",
    previewCounts: (n, frames, score) => `${n} sessions (${frames} with frames, ${score} score only)`,
    bandRow: (band, n) => `${band}: ${n}`,
    excluded: (noConsent, withdrawn, unscorable, noAge, quality) =>
      `Excluded: ${noConsent} without research consent, ${withdrawn} withdrawn, ${unscorable} unscorable, ${noAge} without age, ${quality} by quality`,
    budget: (sessions, cells) => `${sessions} sessions × ${cells} cell${cells === 1 ? "" : "s"}`,
    noCohorts: "There is no saved cohort yet.",
    saveFirst: "Save the experiment first; unsaved changes are not part of the run.",
    denied: "Your role cannot start analysis runs.",
  },
  cohorts: {
    title: "Cohorts",
    intro: "A cohort is a saved selection of consented sessions. Counts below the small-cell threshold are hidden.",
    denied: "Your role cannot view cohorts.",
    newTitle: "New cohort",
    newIntro: "Choose the sessions to include. Preview the counts before saving.",
    newLink: "New cohort",
    readOnly: "Your role can view cohorts but not create them.",
    empty: "There is no saved cohort yet.",
    allCohorts: "All cohorts",
    cols: { name: "Name", definition: "Selection", created: "Created", status: "Status" },
    active: "Active",
    archived: "Archived",
    name: "Name",
    nameRequired: "Enter a name.",
    ageBands: "Age bands",
    ageBandsHint: "Band ids separated by commas, for example 24-35, 36-47. Empty means every band.",
    from: "From",
    to: "To",
    qualityVerdicts: "Capture quality",
    trackingVerdicts: "Tracking verdict",
    minValidRatio: "Minimum valid-frame ratio (0 to 1)",
    requireFrames: "Only sessions with a frame log",
    includeSynthetic: "Include synthetic sessions",
    preview: "Preview counts",
    previewing: "Counting…",
    previewTitle: "Preview",
    save: "Save cohort",
    saving: "Saving…",
    definition: "Selection",
    any: "any",
    definitionHash: "Definition hash",
    archive: "Archive cohort",
    archiving: "Archiving…",
    archivedNote: "This cohort is archived.",
    summary: {
      ageBands: (v) => `Age bands: ${v}`,
      range: (from, to) => `Dates: ${from} to ${to}`,
      quality: (v) => `Capture quality: ${v}`,
      tracking: (v) => `Tracking: ${v}`,
      minValid: (v) => `Valid frames at least ${v}`,
      framesOnly: "With frame log only",
      synthetic: "Includes synthetic sessions",
      all: "All consented sessions",
    },
  },
  runs: {
    title: "Analysis runs",
    intro: "Each run evaluates an experiment against a cohort. Results are research output, not clinical findings.",
    empty: "There is no analysis run yet.",
    allRuns: "All runs",
    filterStatus: "Status",
    allStatuses: "All",
    apply: "Apply filter",
    cols: { status: "Status", kind: "Kind", experiment: "Experiment", cohort: "Cohort", sessions: "Sessions", created: "Created" },
    kind: { cohort_rescore: "Cohort re-score", param_sweep: "Parameter sweep", age_profile: "Age profile" },
    unknownExperiment: "(unknown experiment)",
    unknownCohort: "(unknown cohort)",
    heading: (id) => `Run ${id.slice(0, 8)}`,
    progress: (done, total, phase) => `${done} of ${total} sessions (${phase})`,
    updating: "Updating the status…",
    pollStopped: "The status is no longer updating. Reload the page to check again.",
    cancel: "Cancel run",
    cancelling: "Cancelling…",
    cancelled: "This run was cancelled.",
    rerun: "Run again",
    rerunning: "Starting…",
    exportResults: "Export results…",
    failure: {
      title: "The run did not finish.",
      budget_exceeded: "The run was over the compute budget. Choose a smaller cohort, a narrower sweep or the score level.",
      "result-too-large": "The result was too large to store. Choose a smaller cohort or fewer metrics.",
      fallback: "The run stopped before it produced a result.",
      code: (c) => `Reason code: ${c}`,
    },
    provenance: {
      title: "Provenance",
      intro: "Everything needed to reproduce this run. Each value can be copied.",
      copy: "Copy",
      copied: "Copied",
      copyField: (label) => `Copy ${label}`,
      notRecorded: "not recorded",
      fields: {
        definition_hash: "Definition hash",
        cohort_hash: "Cohort hash",
        membership_hash: "Membership hash",
        profile_hash: "Profile hash",
        published_profile_version: "Published profile version",
        baselines_version: "Baselines version",
        catalog_version: "Catalog version",
        bundle_version: "Bundle version",
        core_version: "Core version",
        contract_version: "Contract version",
        pipeline_version: "Pipeline version",
        registry_version: "Registry version",
        seed: "Seed",
        level_requested: "Level requested",
        level_resolved: "Level resolved",
      },
    },
    results: {
      title: "Results",
      sessions: "Sessions",
      byLevel: "By level",
      excluded: "Excluded sessions",
      paramsNotApplied: "Parameters not applied to some sessions",
      suppressedCells: "Hidden cells",
      synthetic: "This run includes synthetic sessions.",
      drift: (added, removed) => `Membership changed since the first run: ${added} added, ${removed} removed.`,
      published: "Published",
      draft: "Draft",
      overall: "Overall",
      band: (b) => `Age band ${b}`,
      composite: "Composite",
      metric: (id) => `Metric ${id}`,
      flagCounts: "Analysis label counts",
      flagTransitions: "Analysis label changes (published to draft)",
      qualityTransitions: "Quality status changes (published to draft)",
      deltaTitle: "Change in the composite, draft minus published",
      noSummary: "No results yet.",
    },
    sessionsLink: "Per-session results",
    sessionsTitle: "Per-session results",
    backToRun: "Back to the run",
    sessionCols: { session: "Session", level: "Level", band: "Age band", published: "Published", draft: "Draft", delta: "Change", warnings: "Notes" },
    noSessions: "There are no per-session rows for this run.",
    next: "Next",
    first: "First",
  },
  charts: {
    histogram: (label) => `Distribution of ${label}`,
    baseline: (low, high) => `Shaded: published range ${low} to ${high}`,
    noBaseline: "No published range for this age band.",
    bin: (from, to, count) => `${from} to ${to}: ${count}`,
    matrixFrom: "From",
    matrixTo: "To",
    noTransition: "-",
    deltaBars: (label) => `Change in ${label} by age band`,
    interval: "95% interval of the mean",
    noData: "No data.",
  },
  analysis: {
    sweepTitle: "Parameter sweep",
    sweepIntro: (cells) =>
      `${cells} cells. Each cell re-scores the cohort with the experiment profile at these values; the published profile is the reference.`,
    axis: (index, label, id) => `Axis ${index}: ${label} (${id})`,
    publishedValue: (values) => `Published value: ${values}`,
    publishedMarker: "Published value",
    publishedOutline: "Outlined: the published value.",
    offGrid: (values) => `The published value ${values} is outside the grid: it is listed in the table, not drawn in the grid.`,
    compositeMean: "Mean composite",
    greenShare: "Share labelled green",
    metricMean: (id) => `Mean ${id}`,
    heatmap: (quantity, x, y) => `${quantity} by ${x} and ${y}`,
    heatLegend: (low, high) => `Lighter: ${low}. Darker: ${high}.`,
    heatSuppressed: (k) => `Grey: n < ${k} (hidden).`,
    cellTitle: (values, n, value) => `${values}: n ${n}, ${value}`,
    tableTitle: "Sweep cells",
    showTable: "Show every cell as a table",
    cols: {
      values: "Values",
      n: "Sessions",
      green: "Green",
      yellow: "Yellow",
      red: "Red",
      unreliable: "Unreliable",
      compositeMean: "Composite mean",
      compositeMedian: "Composite median",
      changed: "Label changed vs published",
    },
    publishedRow: "published",
    ageTitle: "Age profile",
    ageIntro:
      "Per age band and expected response: how the cohort's values fall against the baseline range, under the published baselines and under the experiment's baselines.",
    baselines: (published, draft) => `Published baselines ${published}; experiment baselines ${draft}.`,
    threshold: (t) => `Cells with fewer than ${t} sessions are hidden (the larger of the small-cell threshold and the profile's minimum n).`,
    bandTitle: (band, from, to) => `Age band ${band} (${from}-${to} months)`,
    ageCaption: (band) => `Expected responses, age band ${band}`,
    ageCols: {
      expectation: "Expected response",
      n: "Sessions",
      median: "Median [IQR]",
      range: "Baseline range",
      publishedShares: "Published baselines",
      draftShares: "Experiment baselines",
      publishedPercentile: "Expected value percentile (published)",
      draftPercentile: "Expected value percentile (experiment)",
    },
    expectation: (stimulus, segment, metric) => `${stimulus} / ${segment} / ${metric}`,
    shares: (onTarget, monitor, atypical) => `${onTarget} on target, ${monitor} monitor, ${atypical} atypical`,
    range: (low, high, expected) => `${low} to ${high} (expected ${expected})`,
    noRange: "Differs within the band",
    noExpectations: "No scored expected responses in this band.",
    unreadable: "The age profile of this run could not be read.",
  },
  errors: {
    "experiment-name-exists": "An experiment with this name already exists.",
    "stale-version": "The experiment was changed elsewhere. Reload the page and apply your change again.",
    "experiment-no-run": "Run the experiment on a cohort before promoting it.",
    "experiment-promoted": "This experiment was already promoted.",
    "config-unanalysed": "This scoring profile has no finished analysis run over it. Run it on a cohort as an experiment, promote that experiment and publish the promoted draft.",
    "config-invalid": "The profile has invalid values.",
    "validation-failed": "Some values are invalid.",
    "analysis-quota-exceeded": "Too many analysis runs right now. Try again later.",
    "sweep-too-large": "The sweep is over budget.",
    "cohort-too-large": "The cohort is over budget.",
    "job-finished": "The run already finished.",
    "analysis-level-unavailable": "This level is not available for the cohort.",
    conflict: "A cohort with this name already exists.",
    "not-implemented": "This part is not available on the server yet.",
    forbidden: "Your role cannot do this.",
    "not-found": "Not found.",
    fallback: "Something went wrong. Try again.",
  },
  runSetup: {
    kind: "Analysis",
    kinds: { cohort_rescore: "Cohort re-score (published vs draft)", param_sweep: "Parameter sweep", age_profile: "Age profile" },
    ageNote: "Compares this experiment's baselines with the published ones for each age band.",
    axes: "Sweep axes",
    axis: (n) => `Axis ${n}`,
    knob: "Parameter",
    from: "From",
    to: "To",
    step: "Step",
    addAxis: "Add a second axis",
    removeAxis: "Remove axis",
    framesOnly: "Parameters of the metrics and frames stages can be chosen only at level “Frames”.",
    noKnobs: "No parameter can be swept at this level.",
    published: (v) => `The published value ${v} is always included.`,
    cells: (n, max) => `${n} cells (at most ${max})`,
    cellsUnknown: "The cell count shows once the axes are valid.",
    metrics: "Metrics to summarise per cell",
    metricsHint: "Optional; with none chosen only the composite and the flag rates are summarised.",
    problems: {
      param: "This parameter cannot be swept.",
      level: "This parameter needs level “Frames”.",
      range: "From must not exceed To; both must be numbers.",
      step: "Step must be a number above zero.",
      bounds: (min, max) => `Values must lie between ${min} and ${max}.`,
      whole: "For a whole-number parameter, From, To and Step must be whole numbers.",
      fine: (step) => `Step must be at least ${step}.`,
      tooMany: (steps, max) => `${steps} steps; at most ${max} per axis.`,
      axes: "One or two axes are needed.",
      sameKnob: "Each axis must sweep a different parameter.",
      cells: (n, max) => `${n} cells; at most ${max}.`,
    },
  },
};

const ne: ResearchMessages = {
  nav: "अनुसन्धान", // ne: draft
  title: "अनुसन्धान", // ne: draft
  // ne: draft (the English sentence stays, PRV-6)
  disclaimer: "यो निदान गर्ने उपकरण होइन। This is not a diagnostic tool.",
  notAvailable: "अनुसन्धानको यो भाग अझै उपलब्ध छैन।", // ne: draft
  suppressedCell: (k) => `n < ${k}`,
  runStatus: {
    queued: "पालोमा", // ne: draft
    running: "चलिरहेको", // ne: draft
    succeeded: "सम्पन्न", // ne: draft
    failed: "असफल", // ne: draft
    cancelled: "रद्द गरिएको", // ne: draft
  },
  reprocess: {
    score: "स्कोर", // ne: draft
    metrics: "मेट्रिक्स", // ne: draft
    frames: "फ्रेम", // ne: draft
    capture_only: "क्याप्चर मात्र", // ne: draft
  },
  level: { auto: "स्वचालित", score: "स्कोर तह", frames: "फ्रेम तह" }, // ne: draft
  ownerGated: "क्लिनिकल प्रयोगका लागि बन्द; विश्लेषणमा मात्र", // ne: draft
  // ne: draft (everything below: WP-X.5 writes the drafts, WP-L.3 reviews)
  groups: {
    flag: "विश्लेषण लेबल",
    lowConfidenceValidRatio: "कम-विश्वास गेट",
    ageBands: "उमेर समूह",
    qualityGate: "गुणस्तर गेट",
    trackingQuality: "ट्र्याकिङ गुणस्तर",
    metrics: "नजर मेट्रिक्स",
    aoi: "रुचि क्षेत्र",
    norms: "मानक",
    review: "समीक्षा",
    display: "नतिजा प्रदर्शन",
    sessionQuality: "सत्र गुणस्तर",
  },
  units: {
    none: "",
    ratio: "अनुपात",
    percent: "%",
    seconds: "से.",
    millis: "मि.से.",
    cm: "से.मि.",
    mm: "मि.मि.",
    deg: "°",
    deg_per_sec: "°/से.",
    rad: "रेडियन",
    hz: "हर्ज",
    norm_units: "स्क्रिन एकाइ",
    norm_units_per_sec: "स्क्रिन एकाइ/से.",
    months: "महिना",
    count: "",
  },
  form: {
    published: "प्रकाशित",
    publishedValue: (v) => `प्रकाशित: ${v}`,
    defaultValue: (v) => `पूर्वनिर्धारित: ${v}`,
    reset: "पूर्वनिर्धारितमा फर्काउनुहोस्",
    range: (min, max) => `${min} देखि ${max}`,
    outside: (range) => `अनुमत दायराबाहिर (${range})।`,
    slider: (label) => `${label} को स्लाइडर`,
    diffCount: (n) => `प्रकाशित प्रोफाइलभन्दा फरक प्यारामिटर: ${n}`,
    captureOnlyPreview: "क्याप्चरका बेला मात्र लागू हुन्छ; पूर्वावलोकनमा परिवर्तन हुँदैन",
    on: "सक्रिय",
    off: "निष्क्रिय",
    pointCount: (n) => `${n} बिन्दु`,
    x: "x",
    y: "y",
    addPoint: "बिन्दु थप्नुहोस्",
    removePoint: "बिन्दु हटाउनुहोस्",
    bandId: "समूह आईडी",
    minMonths: "देखि (महिना)",
    maxMonths: "सम्म (महिना)",
    addBand: "समूह थप्नुहोस्",
    removeBand: "समूह हटाउनुहोस्",
    pointsOf: (label, n) => `${label}: बिन्दु ${n}`,
  },
  experiments: {
    title: "प्रयोगहरू",
    intro: "प्रयोग भनेको रेकर्ड गरिएका सत्रमा परीक्षण गरिने स्कोरिङ प्रोफाइलको मस्यौदा हो। यहाँको कुराले प्रकाशित प्रोफाइल वा कुनै सत्र परिवर्तन गर्दैन।",
    denied: "तपाईंको भूमिकाले अनुसन्धान प्रयोगहरू हेर्न मिल्दैन।",
    newTitle: "नयाँ प्रयोग",
    newIntro: "प्रकाशित स्कोरिङ प्रोफाइल वा अर्को प्रयोगबाट सुरु गर्नुहोस्।",
    newLink: "नयाँ प्रयोग",
    name: "नाम",
    description: "विवरण",
    startFrom: "यहाँबाट सुरु गर्नुहोस्",
    startPublished: "प्रकाशित स्कोरिङ प्रोफाइल",
    create: "प्रयोग बनाउनुहोस्",
    creating: "बनाउँदै…",
    empty: "अहिलेसम्म कुनै प्रयोग छैन।",
    cols: { name: "नाम", status: "स्थिति", base: "आधार प्रोफाइल", updated: "अद्यावधिक", lastRun: "पछिल्लो रन" },
    status: { draft: "मस्यौदा", archived: "अभिलेखित", promoted: "प्रवर्द्धित" },
    readOnly: "हेर्न मात्र: प्रयोग बनाउने र सम्पादन गर्ने अधिकार मालिक र अनुसन्धानकर्ताको मात्र हो।",
    recentRuns: "हालका रनहरू",
    cohorts: "कोहोर्टहरू",
    noRuns: "अहिलेसम्म कुनै रन छैन।",
    noCohorts: "अहिलेसम्म कुनै कोहोर्ट छैन।",
    allExperiments: "सबै प्रयोगहरू",
    nameRequired: "नाम आवश्यक छ।",
  },
  workbench: {
    title: "कार्यक्षेत्र",
    back: "प्रयोगहरूमा फर्कनुहोस्",
    nameLabel: "नाम",
    descriptionLabel: "विवरण",
    save: "प्रयोग सुरक्षित गर्नुहोस्",
    saving: "सुरक्षित गर्दै…",
    saved: "सुरक्षित गरियो।",
    problems: (n) => `सुरक्षित गर्नुअघि ${n} समस्या सच्याउनुहोस्।`,
    serverRejected: "सर्भरले प्रयोग अस्वीकार गर्‍यो:",
    checking: "Rust कोरले जाँच गर्दै…",
    valid: "Rust कोरले यो प्रोफाइल स्वीकार गर्छ।",
    invalid: (n) => `Rust कोरले ${n} समस्या फेला पार्‍यो:`,
    coreUnavailable: "Rust कोर लोड हुन सकेन, त्यसैले मान यहाँ जाँचिएको छैन। सुरक्षित गर्दा सर्भरले जाँच्छ।",
    runOnCohort: "कोहोर्टमा चलाउनुहोस्",
    promote: "मस्यौदामा प्रवर्द्धन गर्नुहोस्",
    promoted: (version) => `स्कोरिङ प्रोफाइल मस्यौदा ${version} का रूपमा प्रवर्द्धन गरियो।`,
    readOnly: "हेर्न मात्र: प्रयोग सम्पादन गर्ने अधिकार मालिक र अनुसन्धानकर्ताको मात्र हो।",
    base: (v) => `प्रोफाइल ${v} मा आधारित`,
    profileHash: "प्रोफाइल ह्यास",
    promoteVersion: "नयाँ स्कोरिङ प्रोफाइल संस्करण",
    promoteNotes: "टिप्पणी",
    promoteSubmit: "प्रवर्द्धन गर्नुहोस्",
    promoteCancel: "रद्द गर्नुहोस्",
    promoteNoRun: "प्रवर्द्धन गर्नुअघि प्रयोगलाई कोहोर्टमा चलाउनुहोस्।",
  },
  preview: {
    title: "प्रत्यक्ष पूर्वावलोकन",
    intro: "चयन गरिएका सत्रलाई तपाईंको ब्राउजरमा प्रकाशित प्रोफाइल र यो असुरक्षित मस्यौदाले पुनः स्कोर गर्छ। केही पनि लेखिँदैन।",
    pickLegend: "पूर्वावलोकन गर्ने सत्र",
    selectedCount: (n, max) => `${max} मध्ये ${n} चयन`,
    maxReached: (max) => `एकै पटक बढीमा ${max} सत्र।`,
    noSessions: "पूर्वावलोकन गर्ने सत्र छैन।",
    noBaselines: "प्रकाशित मानक बेसलाइन छैन, त्यसैले पुनः स्कोर गर्न सकिँदैन।",
    age: (id) => `सत्र ${id} को उमेर (महिना)`,
    defaultAge: "पूर्वनिर्धारित उमेर (महिना)",
    ageHint: "सत्रमा बच्चाको उमेर हुँदैन; थाहा भएमा प्रत्येक सत्रमा राख्नुहोस्।",
    framesBadge: "फ्रेम",
    scoreBadge: "स्कोर मात्र",
    level: "तह",
    published: "प्रकाशित",
    draft: "मस्यौदा",
    composite: "कम्पोजिट",
    flag: "विश्लेषण लेबल",
    flagLabel: { green: "अपेक्षित दायराभित्र", yellow: "दायराको किनारानजिक", red: "अपेक्षित दायराबाहिर", unreliable: "अविश्वसनीय" },
    clips: "क्लिप अनुसार उप-स्कोर",
    clip: "क्लिप",
    subscore: "उप-स्कोर",
    lowConfidence: "कम विश्वास",
    paramNotApplied: (id) => `param_not_applied: ${id}`,
    framesUnavailable: "फ्रेम-तह पुनः प्रशोधनलाई नयाँ Rust कोर चाहिन्छ; स्कोर-तहको नतिजा देखाइएको छ।",
    coreUnavailable: "Rust कोर लोड हुन सकेन।",
    computing: "गणना गर्दै…",
    failed: (m) => `पुनः स्कोर गर्न सकिएन: ${m}`,
    loadFailed: "सुरक्षित सत्र रेकर्ड लोड गर्न सकिएन।",
    scanpath: "नजर मार्ग",
    scanpathEmpty: "यो सत्रको ट्रेस अझै छैन।",
    scanpathLabel: "फिक्सेसनसहितको नजर ट्रेस, प्रकाशित र मस्यौदा एकमाथि अर्को",
    trackOf: "प्रकाशित ट्रेस",
    sessionCard: (id) => `सत्र ${id}`,
    unchanged: "अपरिवर्तित",
    changed: "परिवर्तित",
    corePanic: "Rust कोर अप्रत्याशित रूपमा रोकियो; अर्को परिवर्तनमा पुनः लोड हुन्छ।",
  },
  sweep: {
    title: "एकल-सत्र स्विप",
    intro: "एउटा संख्यात्मक प्यारामिटरको प्रत्येक चरणमा एउटा सत्रलाई पुनः स्कोर गर्छ। तपाईंको ब्राउजरमा चल्छ; कुनै जब बन्दैन।",
    knob: "प्यारामिटर",
    from: "देखि",
    to: "सम्म",
    step: "चरण",
    run: "स्विप चलाउनुहोस्",
    running: "स्विप गर्दै…",
    tooMany: (max) => `बढीमा ${max} चरण।`,
    session: "सत्र",
    curve: (label) => `${label} विरुद्ध कम्पोजिट स्कोर`,
    publishedMarker: "प्रकाशित मान",
    noKnobs: "स्कोर-तहमा स्विप गर्न मिल्ने संख्यात्मक प्यारामिटर छैन।",
    noSession: "पहिले पूर्वावलोकनमा सत्र चयन गर्नुहोस्।",
    cols: { value: "मान", composite: "कम्पोजिट", flag: "विश्लेषण लेबल" },
    invalid: "प्यारामिटरको सीमाभित्रको दायरा राख्नुहोस्।",
  },
  run: {
    title: "कोहोर्टमा चलाउनुहोस्",
    intro: "सुरक्षित कोहोर्टका सबै सत्रमा यो प्रयोगको मूल्यांकन गर्छ। रनले सुरक्षित डाटा पढ्छ र सत्रमा केही लेख्दैन।",
    cohort: "कोहोर्ट",
    cohortPick: "कोहोर्ट छान्नुहोस्",
    level: "तह",
    seed: "सिड (ऐच्छिक)",
    seedHint: "पूर्णांक; उही सिडले उही विश्वास अन्तराल दिन्छ।",
    preview: "गणना पूर्वावलोकन",
    previewing: "गणना गर्दै…",
    submit: "रन सुरु गर्नुहोस्",
    submitting: "सुरु गर्दै…",
    cancel: "रद्द गर्नुहोस्",
    previewCounts: (n, frames, score) => `${n} सत्र (${frames} फ्रेमसहित, ${score} स्कोर मात्र)`,
    bandRow: (band, n) => `${band}: ${n}`,
    excluded: (noConsent, withdrawn, unscorable, noAge, quality) =>
      `बहिष्कृत: अनुसन्धान सहमति नभएका ${noConsent}, फिर्ता गरिएका ${withdrawn}, स्कोर गर्न नमिल्ने ${unscorable}, उमेर नभएका ${noAge}, गुणस्तरका कारण ${quality}`,
    budget: (sessions, cells) => `${sessions} सत्र × ${cells} कक्ष`,
    noCohorts: "अहिलेसम्म कुनै सुरक्षित कोहोर्ट छैन।",
    saveFirst: "पहिले प्रयोग सुरक्षित गर्नुहोस्; असुरक्षित परिवर्तन रनमा पर्दैन।",
    denied: "तपाईंको भूमिकाले विश्लेषण रन सुरु गर्न मिल्दैन।",
  },
  cohorts: {
    title: "कोहोर्ट",
    intro: "कोहोर्ट भनेको सहमति भएका सत्रको सुरक्षित चयन हो। सानो-कक्ष सीमाभन्दा कम गणना लुकाइन्छ।",
    denied: "तपाईंको भूमिकाले कोहोर्ट हेर्न मिल्दैन।",
    newTitle: "नयाँ कोहोर्ट",
    newIntro: "समावेश गर्ने सत्र छान्नुहोस्। सुरक्षित गर्नुअघि गणना हेर्नुहोस्।",
    newLink: "नयाँ कोहोर्ट",
    readOnly: "तपाईंको भूमिकाले कोहोर्ट हेर्न मिल्छ, बनाउन मिल्दैन।",
    empty: "अहिलेसम्म कुनै सुरक्षित कोहोर्ट छैन।",
    allCohorts: "सबै कोहोर्ट",
    cols: { name: "नाम", definition: "चयन", created: "बनाइएको", status: "स्थिति" },
    active: "सक्रिय",
    archived: "अभिलेखित",
    name: "नाम",
    nameRequired: "नाम लेख्नुहोस्।",
    ageBands: "उमेर समूह",
    ageBandsHint: "अल्पविरामले छुट्याइएका समूह आईडी, जस्तै 24-35, 36-47। खाली भए सबै समूह।",
    from: "देखि",
    to: "सम्म",
    qualityVerdicts: "क्याप्चर गुणस्तर",
    trackingVerdicts: "ट्र्याकिङ नतिजा",
    minValidRatio: "न्यूनतम वैध फ्रेम अनुपात (० देखि १)",
    requireFrames: "फ्रेम लग भएका सत्र मात्र",
    includeSynthetic: "कृत्रिम सत्र समावेश गर्नुहोस्",
    preview: "गणना पूर्वावलोकन",
    previewing: "गणना गर्दै…",
    previewTitle: "पूर्वावलोकन",
    save: "कोहोर्ट सुरक्षित गर्नुहोस्",
    saving: "सुरक्षित गर्दै…",
    definition: "चयन",
    any: "कुनै पनि",
    definitionHash: "परिभाषा ह्यास",
    archive: "कोहोर्ट अभिलेखित गर्नुहोस्",
    archiving: "अभिलेखित गर्दै…",
    archivedNote: "यो कोहोर्ट अभिलेखित छ।",
    summary: {
      ageBands: (v) => `उमेर समूह: ${v}`,
      range: (from, to) => `मिति: ${from} देखि ${to}`,
      quality: (v) => `क्याप्चर गुणस्तर: ${v}`,
      tracking: (v) => `ट्र्याकिङ: ${v}`,
      minValid: (v) => `वैध फ्रेम कम्तीमा ${v}`,
      framesOnly: "फ्रेम लग भएका मात्र",
      synthetic: "कृत्रिम सत्र समावेश",
      all: "सहमति भएका सबै सत्र",
    },
  },
  runs: {
    title: "विश्लेषण रन",
    intro: "प्रत्येक रनले कोहोर्टमा एउटा प्रयोगको मूल्यांकन गर्छ। नतिजा अनुसन्धान सामग्री हो, क्लिनिकल निष्कर्ष होइन।",
    empty: "अहिलेसम्म कुनै विश्लेषण रन छैन।",
    allRuns: "सबै रन",
    filterStatus: "स्थिति",
    allStatuses: "सबै",
    apply: "फिल्टर लागू गर्नुहोस्",
    cols: { status: "स्थिति", kind: "प्रकार", experiment: "प्रयोग", cohort: "कोहोर्ट", sessions: "सत्र", created: "बनाइएको" },
    kind: { cohort_rescore: "कोहोर्ट पुनः-स्कोर", param_sweep: "प्यारामिटर स्वीप", age_profile: "उमेर प्रोफाइल" },
    unknownExperiment: "(अज्ञात प्रयोग)",
    unknownCohort: "(अज्ञात कोहोर्ट)",
    heading: (id) => `रन ${id.slice(0, 8)}`,
    progress: (done, total, phase) => `${total} मध्ये ${done} सत्र (${phase})`,
    updating: "स्थिति अद्यावधिक गर्दै…",
    pollStopped: "स्थिति अब अद्यावधिक भइरहेको छैन। फेरि हेर्न पृष्ठ पुनः लोड गर्नुहोस्।",
    cancel: "रन रद्द गर्नुहोस्",
    cancelling: "रद्द गर्दै…",
    cancelled: "यो रन रद्द गरियो।",
    rerun: "फेरि चलाउनुहोस्",
    rerunning: "सुरु गर्दै…",
    exportResults: "नतिजा निर्यात गर्नुहोस्…",
    failure: {
      title: "रन सकिएन।",
      budget_exceeded: "रन गणना बजेटभन्दा बढी भयो। सानो कोहोर्ट, साँघुरो स्वीप वा स्कोर तह छान्नुहोस्।",
      "result-too-large": "नतिजा भण्डारण गर्न धेरै ठूलो भयो। सानो कोहोर्ट वा कम मेट्रिक छान्नुहोस्।",
      fallback: "रनले नतिजा दिनुअघि नै रोकियो।",
      code: (c) => `कारण कोड: ${c}`,
    },
    provenance: {
      title: "उत्पत्ति विवरण",
      intro: "यो रन दोहोर्याउन चाहिने सबै कुरा। प्रत्येक मान प्रतिलिपि गर्न सकिन्छ।",
      copy: "प्रतिलिपि",
      copied: "प्रतिलिपि भयो",
      copyField: (label) => `${label} प्रतिलिपि गर्नुहोस्`,
      notRecorded: "अभिलेख छैन",
      fields: {
        definition_hash: "परिभाषा ह्यास",
        cohort_hash: "कोहोर्ट ह्यास",
        membership_hash: "सदस्यता ह्यास",
        profile_hash: "प्रोफाइल ह्यास",
        published_profile_version: "प्रकाशित प्रोफाइल संस्करण",
        baselines_version: "आधाररेखा संस्करण",
        catalog_version: "क्याटलग संस्करण",
        bundle_version: "बन्डल संस्करण",
        core_version: "कोर संस्करण",
        contract_version: "सम्झौता संस्करण",
        pipeline_version: "पाइपलाइन संस्करण",
        registry_version: "रजिस्ट्री संस्करण",
        seed: "सिड",
        level_requested: "अनुरोध गरिएको तह",
        level_resolved: "लागू भएको तह",
      },
    },
    results: {
      title: "नतिजा",
      sessions: "सत्र",
      byLevel: "तह अनुसार",
      excluded: "बहिष्कृत सत्र",
      paramsNotApplied: "केही सत्रमा लागू नभएका प्यारामिटर",
      suppressedCells: "लुकाइएका कक्ष",
      synthetic: "यो रनमा कृत्रिम सत्र समावेश छन्।",
      drift: (added, removed) => `पहिलो रनपछि सदस्यता फेरियो: ${added} थपिए, ${removed} हटाइए।`,
      published: "प्रकाशित",
      draft: "मस्यौदा",
      overall: "समग्र",
      band: (b) => `उमेर समूह ${b}`,
      composite: "संयुक्त",
      metric: (id) => `मेट्रिक ${id}`,
      flagCounts: "विश्लेषण लेबल गणना",
      flagTransitions: "विश्लेषण लेबल परिवर्तन (प्रकाशितबाट मस्यौदा)",
      qualityTransitions: "गुणस्तर स्थिति परिवर्तन (प्रकाशितबाट मस्यौदा)",
      deltaTitle: "संयुक्त मानमा परिवर्तन, मस्यौदा घटाएर प्रकाशित",
      noSummary: "अहिलेसम्म नतिजा छैन।",
    },
    sessionsLink: "सत्र अनुसार नतिजा",
    sessionsTitle: "सत्र अनुसार नतिजा",
    backToRun: "रनमा फर्कनुहोस्",
    sessionCols: { session: "सत्र", level: "तह", band: "उमेर समूह", published: "प्रकाशित", draft: "मस्यौदा", delta: "परिवर्तन", warnings: "टिप्पणी" },
    noSessions: "यो रनका लागि सत्र अनुसारका पङ्क्ति छैनन्।",
    next: "अर्को",
    first: "पहिलो",
  },
  charts: {
    histogram: (label) => `${label} को वितरण`,
    baseline: (low, high) => `छायाँ: प्रकाशित दायरा ${low} देखि ${high}`,
    noBaseline: "यो उमेर समूहका लागि प्रकाशित दायरा छैन।",
    bin: (from, to, count) => `${from} देखि ${to}: ${count}`,
    matrixFrom: "बाट",
    matrixTo: "मा",
    noTransition: "-",
    deltaBars: (label) => `उमेर समूह अनुसार ${label} मा परिवर्तन`,
    interval: "औसतको ९५% अन्तराल",
    noData: "डाटा छैन।",
  },
  // ne: draft (WP-X.9; WP-L.3 reviews)
  analysis: {
    sweepTitle: "प्यारामिटर स्वीप",
    sweepIntro: (cells) =>
      `${cells} कक्ष। प्रत्येक कक्षले यी मानहरूमा प्रयोग प्रोफाइलसहित कोहोर्टलाई पुनः स्कोर गर्छ; प्रकाशित प्रोफाइल सन्दर्भ हो।`,
    axis: (index, label, id) => `अक्ष ${index}: ${label} (${id})`,
    publishedValue: (values) => `प्रकाशित मान: ${values}`,
    publishedMarker: "प्रकाशित मान",
    publishedOutline: "रेखाले घेरिएको: प्रकाशित मान।",
    offGrid: (values) => `प्रकाशित मान ${values} ग्रिडबाहिर छ: यो तालिकामा छ, ग्रिडमा कोरिएको छैन।`,
    compositeMean: "कम्पोजिटको औसत",
    greenShare: "हरियो लेबलको अंश",
    metricMean: (id) => `${id} को औसत`,
    heatmap: (quantity, x, y) => `${x} र ${y} अनुसार ${quantity}`,
    heatLegend: (low, high) => `हल्का: ${low}। गाढा: ${high}।`,
    heatSuppressed: (k) => `खरानी: n < ${k} (लुकाइएको)।`,
    cellTitle: (values, n, value) => `${values}: n ${n}, ${value}`,
    tableTitle: "स्वीप कक्षहरू",
    showTable: "सबै कक्ष तालिकामा देखाउनुहोस्",
    cols: {
      values: "मान",
      n: "सत्र",
      green: "हरियो",
      yellow: "पहेँलो",
      red: "रातो",
      unreliable: "अविश्वसनीय",
      compositeMean: "कम्पोजिट औसत",
      compositeMedian: "कम्पोजिट मध्यिका",
      changed: "प्रकाशितको तुलनामा लेबल फेरिएको",
    },
    publishedRow: "प्रकाशित",
    ageTitle: "उमेर प्रोफाइल",
    ageIntro:
      "प्रत्येक उमेर समूह र अपेक्षित प्रतिक्रियाका लागि: कोहोर्टका मान आधार दायराको तुलनामा कहाँ पर्छन्, प्रकाशित आधाररेखा र प्रयोगको आधाररेखा अन्तर्गत।",
    baselines: (published, draft) => `प्रकाशित आधाररेखा ${published}; प्रयोगको आधाररेखा ${draft}।`,
    threshold: (t) => `${t} भन्दा कम सत्र भएका कक्ष लुकाइन्छन् (सानो-कक्ष सीमा र प्रोफाइलको न्यूनतम n मध्ये ठूलो)।`,
    bandTitle: (band, from, to) => `उमेर समूह ${band} (${from}-${to} महिना)`,
    ageCaption: (band) => `अपेक्षित प्रतिक्रियाहरू, उमेर समूह ${band}`,
    ageCols: {
      expectation: "अपेक्षित प्रतिक्रिया",
      n: "सत्र",
      median: "मध्यिका [IQR]",
      range: "आधार दायरा",
      publishedShares: "प्रकाशित आधाररेखा",
      draftShares: "प्रयोगको आधाररेखा",
      publishedPercentile: "अपेक्षित मानको प्रतिशतक (प्रकाशित)",
      draftPercentile: "अपेक्षित मानको प्रतिशतक (प्रयोग)",
    },
    expectation: (stimulus, segment, metric) => `${stimulus} / ${segment} / ${metric}`,
    shares: (onTarget, monitor, atypical) => `${onTarget} लक्ष्यमा, ${monitor} निगरानी, ${atypical} असामान्य`,
    range: (low, high, expected) => `${low} देखि ${high} (अपेक्षित ${expected})`,
    noRange: "समूहभित्र फरक छ",
    noExpectations: "यो समूहमा स्कोर गरिएका अपेक्षित प्रतिक्रिया छैनन्।",
    unreadable: "यो रनको उमेर प्रोफाइल पढ्न सकिएन।",
  },
  errors: {
    "experiment-name-exists": "यो नामको प्रयोग पहिले नै छ।",
    "stale-version": "प्रयोग अन्यत्र परिवर्तन भयो। पृष्ठ पुनः लोड गरेर आफ्नो परिवर्तन फेरि लागू गर्नुहोस्।",
    "experiment-no-run": "प्रवर्द्धन गर्नुअघि प्रयोगलाई कोहोर्टमा चलाउनुहोस्।",
    "experiment-promoted": "यो प्रयोग पहिले नै प्रवर्द्धन गरिसकिएको छ।",
    "config-unanalysed": "यो स्कोरिङ प्रोफाइलमाथि कुनै पूरा भएको विश्लेषण रन छैन। यसलाई प्रयोगका रूपमा कोहोर्टमा चलाउनुहोस्, त्यो प्रयोग प्रवर्द्धन गर्नुहोस् र प्रवर्द्धित मस्यौदा प्रकाशित गर्नुहोस्।", // ne: draft
    "config-invalid": "प्रोफाइलमा अमान्य मान छन्।",
    "validation-failed": "केही मान अमान्य छन्।",
    "analysis-quota-exceeded": "अहिले धेरै विश्लेषण रन छन्। पछि प्रयास गर्नुहोस्।",
    "sweep-too-large": "स्विप बजेटभन्दा बढी छ।",
    "cohort-too-large": "कोहोर्ट बजेटभन्दा ठूलो छ।",
    "job-finished": "रन पहिले नै सकिइसकेको छ।",
    "analysis-level-unavailable": "यो कोहोर्टका लागि यो तह उपलब्ध छैन।",
    conflict: "यो नामको कोहोर्ट पहिले नै छ।",
    "not-implemented": "सर्भरमा यो भाग अझै उपलब्ध छैन।",
    forbidden: "तपाईंको भूमिकाले यो गर्न मिल्दैन।",
    "not-found": "फेला परेन।",
    fallback: "केही गडबड भयो। फेरि प्रयास गर्नुहोस्।",
  },
  runSetup: {
    kind: "विश्लेषणको प्रकार", // ne: draft
    kinds: { cohort_rescore: "कोहोर्ट पुनः-स्कोर (प्रकाशित बनाम मस्यौदा)", param_sweep: "प्यारामिटर स्वीप", age_profile: "उमेर प्रोफाइल" }, // ne: draft
    ageNote: "यसले प्रयोगका मानकलाई प्रत्येक उमेर समूहमा प्रकाशित मानकसँग तुलना गर्छ।", // ne: draft
    axes: "स्वीप अक्ष", // ne: draft
    axis: (n) => `अक्ष ${n}`, // ne: draft
    knob: "प्यारामिटर", // ne: draft
    from: "देखि", // ne: draft
    to: "सम्म", // ne: draft
    step: "चरण", // ne: draft
    addAxis: "दोस्रो अक्ष थप्नुहोस्", // ne: draft
    removeAxis: "अक्ष हटाउनुहोस्", // ne: draft
    framesOnly: "मेट्रिक्स र फ्रेम चरणका प्यारामिटर स्तर “फ्रेम” हुँदा मात्र चयन गर्न मिल्छ।", // ne: draft
    noKnobs: "यो स्तरमा स्वीप गर्न मिल्ने प्यारामिटर छैन।", // ne: draft
    published: (v) => `प्रकाशित मान ${v} सधैं समावेश हुन्छ।`, // ne: draft
    cells: (n, max) => `${n} कक्ष (बढीमा ${max})`, // ne: draft
    cellsUnknown: "अक्षहरू मिलेपछि कक्ष संख्या देखिन्छ।", // ne: draft
    metrics: "प्रति कक्ष सारांशका मेट्रिक्स", // ne: draft
    metricsHint: "वैकल्पिक; कुनै नचुनेमा कम्पोजिट र फ्लागको दर मात्र देखिन्छ।", // ne: draft
    problems: {
      param: "यो प्यारामिटर स्वीप गर्न मिल्दैन।", // ne: draft
      level: "यो प्यारामिटरलाई स्तर “फ्रेम” चाहिन्छ।", // ne: draft
      range: "“देखि” “सम्म” भन्दा बढी हुनु हुँदैन; दुवै संख्या हुनुपर्छ।", // ne: draft
      step: "चरण शून्यभन्दा ठुलो संख्या हुनुपर्छ।", // ne: draft
      bounds: (min, max) => `मान ${min} र ${max} बीच हुनुपर्छ।`, // ne: draft
      whole: "पूर्णाङ्क प्यारामिटरमा देखि, सम्म र चरण पूर्ण संख्या हुनुपर्छ।", // ne: draft
      fine: (step) => `चरण कम्तीमा ${step} हुनुपर्छ।`, // ne: draft
      tooMany: (steps, max) => `${steps} चरण छन्; प्रति अक्ष बढीमा ${max}।`, // ne: draft
      axes: "एक वा दुई अक्ष चाहिन्छ।", // ne: draft
      sameKnob: "प्रत्येक अक्षले फरक प्यारामिटर स्वीप गर्नुपर्छ।", // ne: draft
      cells: (n, max) => `${n} कक्ष छन्; बढीमा ${max}।`, // ne: draft
    },
  },
};

export const researchMessages = { en, ne } as const;
export type ResearchLocale = keyof typeof researchMessages;

/** The console's current catalogue (English until the locale switch of WP-L.1). */
export const researchText = en;
