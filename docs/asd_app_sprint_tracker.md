# 🏃 ASD Gaze App — Sprint Task Tracker
## Month Sprint: CareKit + ResearchKit + FHIR MVP

> **Reference**: Full hourly breakdown → `monthly_dev_roadmap.md` (artifact)
> **Branch convention**: `agent-a/week-N`, `agent-b/week-N`, `agent-c/week-N`
> **Integration gate**: EOD Friday each week — PR merge to `dev`

---

## WEEK 1 — Foundation & Data Collection Frontend

### Agent-A: iOS/Swift
- [x] L-A: Fix cumulative medication occurrence (not per-session)
- [x] L-B: Drop `hasSeeded` gate — use idempotent upsert
- [x] L-C: Inject `CareKitClinicalStore` via constructor (not singleton)
- [x] L-D: Normalize `doses` — no duplicates on merge
- [x] L-E: Fix `pendingDoseCount` off-by-one
- [x] L-F: Make `mergeSeeding` cumulative
- [x] L-G: Bump `exportVersion` to `v1.3`, add v1.2 migration
- [x] L-H: Structured error capture in `CareKitClinicalStore`
- [x] Medication partial-dose toggle preserves `lastTakenAt` across day boundaries
- [x] `parseClockTime` whitespace and newline resilience
- [x] `ResearchKitStudyStore` thread safety (synchronous queue dispatch with direct accessors)
- [x] Add offline queue to `ResearchKitSyncService` (atomic filesystem queue + 4xx quarantine)
- [x] Add `NetworkMonitor` — trigger `flushQueue()` on reconnect
- [x] Display pending sync badge on `ResearchStudyDashboardCard` (verified: `pendingQueueCount` badge in sync status row)

### Agent-B: Backend/Python
- [ ] Add structured logging (replace `print()`)
- [x] Add `POST /api/v1/carekit/sync` with event, medication & diary models
- [x] Add SQLite persistence layer (`carekit.db`)
- [x] Add `GET /api/v1/carekit/summary/<child_id>`
- [x] Add idempotency key support (`X-Idempotency-Key` header)
- [x] Add `POST /api/v1/batch/sync` for mixed event batches

### Agent-C: QA/CI
- [ ] Capture baseline test coverage report
- [ ] Add CI coverage gate (>=75% iOS, >=80% Python)
- [x] testCumulativeOccurrence
- [x] testDosesNormalizationNoDuplicates
- [x] testPendingCounterAccuracy
- [x] testExportVersionMigrationV12ToV13
- [x] testMedicationPartialTogglePreservesLastTakenAt
- [x] testEnqueuePayloadIncrementsPendingCount
- [x] testGenerateDoctorShareFileCreatesValidJson
- [x] testSyncNowRequiresParentalConsent
- [x] testSyncNowWithSuccessfulTransmissionDrainsQueue
- [x] testSyncNowQuarantines400BadRequest
- [x] testSyncNowNetworkFailureLeavesQueueIntact
- [x] testNetworkReconnectTriggersAutoSyncWhenPendingItemsExist
- [x] testGenerateDoctorReportPDFCreatesValidPdf
- [x] testGazeSessionRecordRoundtripSerialization
- [x] testGazeSessionRecordInitFromSummary
- [x] testDoctorReportPDFGenerationWithFullData
- [x] testDoctorReportPDFGenerationWithMinimalData
- [x] testDoctorReportPDFGenerationWithHighRiskIndicators
- [x] test_carekit_sync_endpoint
- [x] test_carekit_idempotency_key_deduplication
- [x] test_carekit_summary_endpoint
- [x] test_batch_sync_endpoint

### Progress: Phase 2 Delivery Complete | iOS 65 tests green | Python 11 tests green | CTest 41 tests green | Parity & Theme Gates Passed

---

## WEEK 2 — Data Collection UI Polish & Survey Integrity

### Agent-A: iOS/Swift
- [x] Add GazeSessionRecord model (FHIR-ready fields)
- [x] Wire MainGazeScannerView -> GazeSessionRecord on session end (`completeSession` builds + persists via `GazeSessionHistoryStore`; passes full summary + measured validity to `markScreeningCompleted` so the ResearchKit track fires)
- [x] Verify all 20 M-CHAT-R/F questions + scoring algorithm (tests: 20 unique ids 1–20, reversed polarity exactly {2,5,12}, bands 0–2/3–7/8–20, critical flags level-neutral)
- [x] Add follow-up flag for Medium risk (M-CHAT-F protocol: `requiresFollowUpInterview` on medium only, banner in result card, persisted round-trip)
- [x] Add MCHATResult model + MCHATHistoryView (persisted `MCHATScoreResult` history + newest-first history view with risk/follow-up badges, opened from the research card)
- [x] Replace all static data in CaregiverDashboardView with live bindings (trailing-7-day range + weekly mean replace hardcoded "7–13 Sep"/"68%"; bars already live)
- [x] Add weekly progress summary card + M-CHAT badge + gaze count badge (live trend average header + M-CHAT risk badge with follow-up tag + screenings-this-week badge on the screening card)
- [x] Add pull-to-refresh -> CarePlanStore.sync() (dashboard `.refreshable` flushes the research sync queue, then re-reads stores)
- [x] Implement DoctorReport + PDF generation (PDFKit)
- [x] Add share sheet for PDF export

### Agent-B: Backend/Python
- [ ] Add POST /api/v1/gaze/session with S3 upload for raw telemetry
- [ ] Add GET /api/v1/gaze/sessions/<child_id>
- [ ] Add M-CHAT scoring validation + POST /api/v1/mchat/result
- [ ] Add SNS alert for high M-CHAT risk score
- [ ] Add GET /api/v1/dashboard/<child_id> aggregated summary
- [ ] Add GET /api/v1/report/<child_id> + PDF generation (reportlab)

### Agent-C: QA/CI
- [x] testGazeSessionRecordEncoding (round-trip serialization)
- [x] testGazeSessionPersistence (history store persist/reopen/dedupe/week-filter)
- [x] testMCHATScoringAlgorithm_borderlineCases (bands 2/3 and 7/8 boundaries)
- [x] testMCHATFollowUpFlagOnMediumRisk (flag + persisted round-trip)
- [x] Dashboard live-data feeds (`testSessionsThisWeekFiltersOldRecords`, `testSessionHistoryDedupesBySessionId`, `testRecordSurveyResultPersistsFollowUpFlag`) (trend average/range derived from stores; badges read history)
- [x] testDoctorReportContainsAllSections
- [x] testPDFGenerationNotEmpty
- [ ] Theme gate: scan Views/*.swift for hardcoded hex colors
- [ ] Parity gate: iOS GazePoint <-> Python GazePoint field match

### W2 Gate: M-CHAT complete ✅ | Dashboard live ✅ | PDF export ✅ | iOS 65/50 ✅ | Python 11/30 ⏳ (backend endpoints out of scope for on-device pass)

---

## WEEK 3 — Sync Infrastructure & Data Integrity

### Agent-A: iOS/Swift
- [ ] Define Core Data schema (4 entities)
- [ ] Implement CoreDataStack with NSPersistentCloudKitContainer
- [ ] Migrate ResearchKitStudyStore to Core Data backend
- [ ] Add @MainActor isolation + actor SyncQueue
- [ ] Implement exponential backoff (1s->2s->4s->8s->60s max)
- [ ] Add dead letter queue + UI for stuck items
- [ ] Add SyncStatusView component
- [ ] Add AuditEvent model + AuditLogView
- [ ] Enable iCloud sync + graceful degradation when signed out
- [ ] Add CloudSyncStatusBadge to dashboard

### Agent-B: Backend/Python
- [ ] Migrate SQLite -> PostgreSQL (asyncpg + SQLAlchemy 2.0)
- [ ] Add alembic migrations
- [ ] Add AES-256 encryption at rest for PHI fields
- [ ] Add rate limiting with 429 + Retry-After
- [ ] Add dead letter queue table + admin endpoint
- [ ] Add HIPAA audit trail table + GET /api/v1/audit/<child_id>
- [ ] Add GET /api/v1/health detailed status

### Agent-C: QA/CI
- [ ] testConcurrentWritesDoNotCorruptStore
- [ ] testPartialSeedIdempotentResume
- [ ] testExponentialBackoffTiming
- [ ] testDeadLetterAfterMaxRetries
- [ ] testManualRetryFromDeadLetter
- [ ] testICloudSyncDegradesToLocalWhenSignedOut
- [ ] testAuditEventCreatedOnSave
- [ ] testEncryptedFieldsNotReadableInDB
- [ ] Full regression W1+W2+W3

### W3 Gate: Core Data+iCloud | Backoff+dead letter | PHI encrypted | iOS>=65 | Python>=45

---

## WEEK 4 — FHIR Bridge, Backend MVP & Final Gate

### Agent-A: iOS/Swift
- [ ] FHIRMapper: GazeSessionRecord -> FHIR Observation
- [ ] FHIRMapper: MCHATResult -> FHIR QuestionnaireResponse
- [ ] FHIRMapper: CareTask -> FHIR CarePlan
- [ ] FHIRMapper: ConsentRecord -> FHIR Consent
- [ ] SpeziHealthKit integration scaffold
- [ ] SpeziAccount scaffold
- [ ] ResearchAdminView + consent management list
- [ ] StudyEnrollmentView + withdrawal flow
- [ ] PrivacyInfo.xcprivacy + required reason APIs
- [ ] Crash reporting + analytics scaffold
- [ ] TestFlight archive build (v1.0.0-beta)

### Agent-B: Backend/Python
- [ ] GET /api/v1/fhir/Patient/<child_id>
- [ ] GET /api/v1/fhir/Observation/<child_id>
- [ ] GET /api/v1/fhir/QuestionnaireResponse/<child_id>
- [ ] POST /api/v1/fhir/Bundle (Transaction Bundle from Spezi)
- [ ] SMART on FHIR token validation middleware
- [ ] /.well-known/smart-configuration endpoint
- [ ] Role-based access control (researcher, clinician, admin)
- [ ] DELETE /api/v1/admin/child/<child_id> (right to erasure)
- [ ] Dockerfile + docker-compose.yml
- [ ] backend_ci.yml GitHub Actions workflow

### Agent-C: QA/CI
- [ ] testFHIRPatientResourceValid (schema validation)
- [ ] testFHIRObservationHasRequiredFields
- [ ] testFHIRQuestionnaireResponseScoreEncoded
- [ ] testSMARTTokenValidation_validJWT
- [ ] testSMARTTokenValidation_expiredJWT
- [ ] testDataDeletionRemovesAllPHI
- [ ] testRoleBasedAccessControl_researcherCannotDelete
- [ ] Final coverage gate: >=90% iOS + Python
- [ ] xcodebuild analyze — zero warnings
- [ ] swiftlint — zero errors
- [ ] Privacy audit: no PHI in logs/analytics
- [ ] docs/final_confidence_report.md

### W4 Gate (MVP DONE): FHIR R4 endpoints | Spezi scaffold | TestFlight uploaded | iOS>=80 | Python>=60 | coverage>=90%

---

*Updated by agents after each task. See monthly_dev_roadmap.md for full hourly breakdown.*
