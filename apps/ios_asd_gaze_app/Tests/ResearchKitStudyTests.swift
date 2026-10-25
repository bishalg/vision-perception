@testable import AsdGazeCollector
import CareKitStore
import CurvAsdGazeKit
import XCTest

final class ResearchKitStudyTests: XCTestCase {
    private var suiteName: String!
    private var testUserDefaults: UserDefaults!
    private var researchStore: ResearchKitStudyStore!

    override func setUp() {
        super.setUp()
        suiteName = "ResearchKitStudyTests_\(UUID().uuidString)"
        testUserDefaults = UserDefaults(suiteName: suiteName)!
        researchStore = ResearchKitStudyStore(userDefaults: testUserDefaults)
    }

    override func tearDown() {
        testUserDefaults.removePersistentDomain(forName: suiteName)
        testUserDefaults = nil
        researchStore = nil
        super.tearDown()
    }

    // MARK: - Consent Tests

    /// ADR-0002 decision 5: the consent document has no defaults; every study
    /// fact comes from `ResearchStudyConfiguration`.
    func testConsentDocumentComesFromConfiguration() {
        let doc = ORKConsentDocument(configuration: ResearchKitSyncServiceTests.studyConfig)
        XCTAssertEqual(doc.studyIdentifier, "EXAMPLE-STUDY-1")
        XCTAssertEqual(doc.irbProtocolId, "ETHICS-REF-42")
        XCTAssertEqual(doc.institution, "Example University")
        XCTAssertEqual(doc.principalInvestigator, "Example Investigator")
        XCTAssertFalse(doc.sections.isEmpty)
        XCTAssertTrue(doc.sections.contains(where: { $0.type == .cameraPrivacy }))
        XCTAssertTrue(doc.sections.contains(where: { $0.type == .withdrawing }))
    }

    /// Without a configured study nothing study-shaped carries the old fictional ids.
    func testUnconfiguredDefaultsCarryNoFictionalStudy() {
        XCTAssertNil(ResearchStudyConfiguration.current)
        let task = ORKTaskResult(participantId: "STUDY-ASD-NONE")
        XCTAssertEqual(task.studyIdentifier, ResearchStudyConfiguration.unconfiguredStudyIdentifier)
        XCTAssertEqual(task.irbProtocolId, ResearchStudyConfiguration.unconfiguredStudyIdentifier)
        let sig = ORKConsentSignature(guardianFullName: "Nobody", signatureToken: "SIG-NONE")
        XCTAssertNotEqual(sig.consentVersion, "v1.3-IRB2026")
        let package = ResearchKitStudyPackage(
            participant: StudyParticipantMetadata(
                participantId: "STUDY-ASD-NONE", isConsented: false, childAgeMonths: 24, consentVersion: "unconsented"
            )
        )
        XCTAssertEqual(package.studyIdentifier, ResearchStudyConfiguration.unconfiguredStudyIdentifier)
        XCTAssertEqual(package.irbProtocolId, ResearchStudyConfiguration.unconfiguredStudyIdentifier)
    }

    func testTaskResultUsesInjectedConfiguration() {
        let task = ORKTaskResult(participantId: "STUDY-ASD-CFG", configuration: ResearchKitSyncServiceTests.studyConfig)
        XCTAssertEqual(task.studyIdentifier, "EXAMPLE-STUDY-1")
        XCTAssertEqual(task.irbProtocolId, "ETHICS-REF-42")
    }

    /// Source-level guard: the fictional study and the loopback endpoint are deleted,
    /// not hidden (ADR-0002 decision 5, trap 10). Reads the app sources from the
    /// host file system, which the simulator shares.
    func testAppSourcesContainNoFictionalStudyOrLoopbackEndpoint() throws {
        let sources = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Sources", isDirectory: true)
        guard FileManager.default.fileExists(atPath: sources.path) else {
            throw XCTSkip("App sources are not reachable from this test run")
        }
        let enumerator = try XCTUnwrap(FileManager.default.enumerator(at: sources, includingPropertiesForKeys: nil))
        let forbidden = ["IRB-ASD-2026-088", "2026-088", "Elena Vasquez", "CURV-ASD-GAZE-2026",
                         "Center for Pediatric Neurodevelopment", "127.0.0.1", "v1.3-IRB2026"]
        var scanned = 0
        for case let url as URL in enumerator where url.pathExtension == "swift" {
            let text = try String(contentsOf: url, encoding: .utf8)
            scanned += 1
            for term in forbidden {
                XCTAssertFalse(text.contains(term), "\(url.lastPathComponent) still contains \(term)")
            }
        }
        XCTAssertGreaterThan(scanned, 20)
    }

    func testConsentSaveAndRevocationRoundTrip() {
        let sig = ORKConsentSignature(
            guardianFullName: "Jane Smith",
            signatureToken: "SIG-TEST-TOKEN-1234"
        )

        let saveExp = expectation(description: "consent saved")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-ASD-P001", childAgeMonths: 28) { enrollment in
            XCTAssertTrue(enrollment.isConsented)
            XCTAssertEqual(enrollment.participantId, "STUDY-ASD-P001")
            XCTAssertEqual(enrollment.childAgeMonths, 28)
            XCTAssertEqual(enrollment.consentSignature?.guardianFullName, "Jane Smith")
            saveExp.fulfill()
        }
        wait(for: [saveExp], timeout: 5.0)

        // Verify query from store
        let current = researchStore.fetchEnrollment()
        XCTAssertTrue(current.isConsented)

        // Revoke
        let revokeExp = expectation(description: "consent revoked")
        researchStore.revokeConsent {
            let updated = self.researchStore.fetchEnrollment()
            XCTAssertFalse(updated.isConsented)
            XCTAssertNil(updated.consentSignature)
            XCTAssertEqual(updated.participantId, "STUDY-ASD-P001", "participant token preserved")
            revokeExp.fulfill()
        }
        wait(for: [revokeExp], timeout: 5.0)
    }

    // MARK: - M-CHAT-R/F Scoring Tests

    func testMCHATScoringAllTypicalGivesLowRisk() {
        // All typical answers: failAnswer is false for 1,3,4,6,7,8,9,10,11,13,14,15,16,17,18,19,20
        // and true for 2,5,12.
        var answers: [Int: Bool] = [:]
        for q in MCHATSurveyEngine.questions {
            answers[q.id] = !q.failAnswer // All pass
        }

        let result = MCHATSurveyEngine.evaluate(answers: answers)
        XCTAssertEqual(result.totalFailed, 0)
        XCTAssertEqual(result.riskLevel, .low)
        XCTAssertTrue(result.failedQuestionIds.isEmpty)
    }

    func testMCHATScoringMediumRisk() {
        var answers: [Int: Bool] = [:]
        for q in MCHATSurveyEngine.questions {
            answers[q.id] = !q.failAnswer
        }
        // Fail 4 questions: 1, 7, 10, 14
        answers[1] = false // fail
        answers[7] = false // fail
        answers[10] = false // fail
        answers[14] = false // fail

        let result = MCHATSurveyEngine.evaluate(answers: answers)
        XCTAssertEqual(result.totalFailed, 4)
        XCTAssertEqual(result.riskLevel, .medium)
        XCTAssertEqual(result.failedQuestionIds.sorted(), [1, 7, 10, 14])
    }

    func testMCHATScoringHighRisk() {
        var answers: [Int: Bool] = [:]
        for q in MCHATSurveyEngine.questions {
            answers[q.id] = !q.failAnswer
        }
        // Fail 9 questions
        for id in 1...9 {
            answers[id] = MCHATSurveyEngine.questions.first(where: { $0.id == id })!.failAnswer
        }

        let result = MCHATSurveyEngine.evaluate(answers: answers)
        XCTAssertEqual(result.totalFailed, 9)
        XCTAssertEqual(result.riskLevel, .high)
    }

    func testRecordSurveyResultPersistsInStore() {
        let sig = ORKConsentSignature(guardianFullName: "Test Guardian", signatureToken: "SIG-001")
        researchStore.saveConsent(signature: sig)

        var answers: [Int: Bool] = [:]
        for q in MCHATSurveyEngine.questions {
            answers[q.id] = !q.failAnswer
        }

        let exp = expectation(description: "record survey")
        researchStore.recordSurveyResult(answers: answers) { result in
            XCTAssertEqual(result.totalFailed, 0)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)

        let completed = researchStore.fetchCompletedSurveys()
        XCTAssertEqual(completed.count, 1)
        XCTAssertEqual(completed.first?.riskLevel, .low)
    }

    // MARK: - M-CHAT-R/F Instrument Verification (data validity)

    /// The official M-CHAT-R/F has exactly 20 items numbered 1–20.
    func testMCHATHasTwentyUniqueQuestions() {
        let questions = MCHATSurveyEngine.questions
        XCTAssertEqual(questions.count, 20)
        XCTAssertEqual(Set(questions.map(\.id)), Set(1...20))
    }

    /// Official polarity: every item fails on "No" except 2, 5, 12 (fail on "Yes").
    func testMCHATFailPolarityMatchesOfficialInstrument() {
        let reversed = Set(MCHATSurveyEngine.questions.filter(\.failAnswer).map(\.id))
        XCTAssertEqual(reversed, [2, 5, 12])
    }

    private func mchatAnswers(failing ids: Set<Int>) -> [Int: Bool] {
        var answers: [Int: Bool] = [:]
        for q in MCHATSurveyEngine.questions {
            answers[q.id] = ids.contains(q.id) ? q.failAnswer : !q.failAnswer
        }
        return answers
    }

    /// Official M-CHAT-R/F bands: 0–2 low, 3–7 medium, 8–20 high.
    func testMCHATScoringBandsBorderlineCases() {
        XCTAssertEqual(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [])).riskLevel, .low)
        XCTAssertEqual(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [1, 2])).riskLevel, .low)
        XCTAssertEqual(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [1, 2, 3])).riskLevel, .medium)
        XCTAssertEqual(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [1, 2, 3, 4, 5, 6, 7])).riskLevel, .medium)
        XCTAssertEqual(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [1, 2, 3, 4, 5, 6, 7, 8])).riskLevel, .high)
        XCTAssertEqual(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: Set(1...20))).riskLevel, .high)
    }

    /// Bands depend only on the failed count — critical-item flags never
    /// change the level (M-CHAT-R/F is total-score based).
    func testMCHATCriticalFlagsDoNotAffectLevel() {
        let criticalFails = Set(MCHATSurveyEngine.questions.filter(\.isCritical).map(\.id))
        let plainFails = Set([3, 4, 6, 8, 11, 12])
        XCTAssertEqual(criticalFails.count, plainFails.count)
        XCTAssertEqual(
            MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: criticalFails)).riskLevel,
            MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: plainFails)).riskLevel
        )
    }

    /// M-CHAT-R/F protocol: only medium scores (3–7) require the Follow-Up interview.
    func testMCHATFollowUpFlagOnlyOnMediumRisk() {
        XCTAssertFalse(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [1, 2])).requiresFollowUpInterview)
        XCTAssertTrue(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [1, 2, 3])).requiresFollowUpInterview)
        XCTAssertTrue(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [1, 2, 3, 4, 5, 6, 7])).requiresFollowUpInterview)
        XCTAssertFalse(MCHATSurveyEngine.evaluate(answers: mchatAnswers(failing: [1, 2, 3, 4, 5, 6, 7, 8])).requiresFollowUpInterview)
    }

    /// The follow-up flag survives the persisted survey round-trip.
    func testRecordSurveyResultPersistsFollowUpFlag() {
        let sig = ORKConsentSignature(guardianFullName: "FollowUp Guardian", signatureToken: "SIG-FU-01")
        researchStore.saveConsent(signature: sig)

        let exp = expectation(description: "record medium survey")
        researchStore.recordSurveyResult(answers: mchatAnswers(failing: [1, 2, 3, 4])) { result in
            XCTAssertEqual(result.riskLevel, .medium)
            XCTAssertTrue(result.requiresFollowUpInterview)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)

        let completed = researchStore.fetchCompletedSurveys()
        XCTAssertEqual(completed.last?.riskLevel, .medium)
        XCTAssertTrue(completed.last?.requiresFollowUpInterview ?? false)
    }

    // MARK: - CLM-4: M-CHAT-R/F display names

    func testMCHATRiskLevelRawValuesAreUnchangedBecauseTheyArePersisted() {
        XCTAssertEqual(MCHATRiskLevel.low.rawValue, "Low Risk")
        XCTAssertEqual(MCHATRiskLevel.medium.rawValue, "Medium Risk")
        XCTAssertEqual(MCHATRiskLevel.high.rawValue, "High Risk")
        let decoded = try? JSONDecoder().decode([MCHATRiskLevel].self, from: Data(#"["Low Risk","Medium Risk","High Risk"]"#.utf8))
        XCTAssertEqual(decoded, [.low, .medium, .high])
    }

    func testMCHATRiskLevelDisplayNameNamesInstrumentAndNextStep() {
        XCTAssertEqual(
            MCHATRiskLevel.medium.displayName,
            "M-CHAT-R/F: medium (follow-up interview needed; discuss with your pediatrician)"
        )
        XCTAssertEqual(
            MCHATRiskLevel.high.displayName,
            "M-CHAT-R/F: high (follow-up needed; discuss with your pediatrician soon)"
        )
        XCTAssertEqual(
            MCHATRiskLevel.low.displayName,
            "M-CHAT-R/F: low (no follow-up needed now; discuss any concerns with your pediatrician)"
        )
        for level in [MCHATRiskLevel.low, .medium, .high] {
            XCTAssertTrue(level.displayName.hasPrefix("M-CHAT-R/F: "))
            XCTAssertTrue(level.displayName.contains("pediatrician"))
            XCTAssertFalse(level.displayName.localizedCaseInsensitiveContains("risk"), "no bare risk label (CLM-4)")
        }
    }

    // MARK: - SIM-5: active step results keep absent metrics absent

    func testActiveStepResultKeepsAbsentMetricsAbsent() throws {
        let result = ORKActiveStepResult(summary: .fixture(eyeDwellRatio: nil, mouthDwellRatio: nil, firstEyeLatencyMs: nil))
        XCTAssertNil(result.eyeDwellRatio)
        XCTAssertNil(result.mouthDwellRatio)
        XCTAssertNil(result.firstEyeLatencyMs)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(result)) as? [String: Any])
        XCTAssertNil(json["firstEyeLatencyMs"])
        XCTAssertNil(json["eyeDwellRatio"])
    }

    @MainActor
    func testSimulatedScreeningNeverReachesResearchStore() {
        let clinicalStore = CareKitClinicalStore(store: OCKStore(name: "TestSimDual-\(UUID().uuidString)", type: .inMemory))
        let suite = "ResearchKitStudyTests-care-\(UUID().uuidString)"
        let careDefaults = UserDefaults(suiteName: suite)!
        defer { careDefaults.removePersistentDomain(forName: suite) }
        let carePlanStore = CarePlanStore(clinicalStore: clinicalStore, researchStore: researchStore, defaults: careDefaults)

        let sig = ORKConsentSignature(guardianFullName: "Test Parent", signatureToken: "SIG-SIM-01")
        let consentExp = expectation(description: "consent set")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-ASD-SIM01") { _ in consentExp.fulfill() }
        wait(for: [consentExp], timeout: 5.0)

        let summary = GazeSessionReportSummary.fixture(sessionId: "sess_sim", captureSource: .simulated)
        carePlanStore.markScreeningCompleted(captureSource: summary.captureSource, socialScore: 0.5, summary: summary)

        let waitExp = expectation(description: "research queue settles")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { waitExp.fulfill() }
        wait(for: [waitExp], timeout: 5.0)
        XCTAssertTrue(researchStore.fetchCompletedTaskResults().isEmpty)
    }

    // MARK: - Active Step & Parallel Dual-Store Writes

    func testActiveStepResultRequiresConsent() {
        let summary = sampleSummary(socialScore: 0.72)
        // Store is not consented yet
        let result = researchStore.recordActiveStepResult(summary: summary)
        XCTAssertNil(result, "Unconsented study recording must return nil")
        XCTAssertTrue(researchStore.fetchCompletedTaskResults().isEmpty)
    }

    @MainActor
    func testParallelCareKitAndResearchKitScreeningWrite() {
        // 1. Setup in-memory CareKit clinical store
        let mockOCKStore = OCKStore(name: "TestCareKitDual", type: .inMemory)
        let clinicalStore = CareKitClinicalStore(store: mockOCKStore)

        // 2. Setup CarePlanStore with injected clinical & research stores
        let carePlanStore = CarePlanStore(clinicalStore: clinicalStore, researchStore: researchStore)

        // 3. Provide research consent
        let sig = ORKConsentSignature(guardianFullName: "Test Parent", signatureToken: "SIG-DUAL-01")
        let consentExp = expectation(description: "consent set")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-ASD-DUAL01") { _ in
            consentExp.fulfill()
        }
        wait(for: [consentExp], timeout: 5.0)

        // 4. Mark screening completed
        let summary = sampleSummary(socialScore: 0.84)
        carePlanStore.markScreeningCompleted(
            captureSource: summary.captureSource,
            socialScore: 0.84,
            validFramePercentage: 96.0,
            summary: summary
        )

        // 5. Verify CareKit track: care task marked completed
        let observationTask = carePlanStore.tasks.first(where: { $0.category == .observation })
        XCTAssertEqual(observationTask?.isCompleted, true)

        // 6. Verify ResearchKit track: study store captures ORKTaskResult with active step
        let waitExp = expectation(description: "wait for async research queue")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            let tasks = self.researchStore.fetchCompletedTaskResults()
            XCTAssertEqual(tasks.count, 1)
            let task = tasks.first!
            XCTAssertEqual(task.participantId, "STUDY-ASD-DUAL01")
            XCTAssertEqual(task.activeStepResult?.socialPreferenceScore, 0.84)
            XCTAssertEqual(task.activeStepResult?.qualityVerdict, "optimal")
            XCTAssertEqual(task.activeStepResult?.telemetrySampleCount, 0)
            XCTAssertTrue(task.stepResults.contains(where: { $0.identifier == "consent_review" }))
            waitExp.fulfill()
        }
        wait(for: [waitExp], timeout: 5.0)
    }

    // MARK: - Study Bundle Compilation & SHA-256 Digest

    func testStudyBundleCompilationContainsIntegrityDigest() {
        let sig = ORKConsentSignature(guardianFullName: "Alice Brown", signatureToken: "SIG-DIGEST-99")
        let consentExp = expectation(description: "consent set")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-ASD-SHA256") { _ in
            consentExp.fulfill()
        }
        wait(for: [consentExp], timeout: 5.0)

        let summary = sampleSummary(socialScore: 0.65)
        let recordExp = expectation(description: "record task")
        researchStore.recordActiveStepResult(summary: summary) { _ in
            recordExp.fulfill()
        }
        wait(for: [recordExp], timeout: 5.0)

        let bundle = self.researchStore.compileStudyBundle()
        XCTAssertEqual(bundle["bundleType"] as? String, "ResearchKitStudyPackage")
        XCTAssertEqual(bundle["studyIdentifier"] as? String, ResearchStudyConfiguration.unconfiguredStudyIdentifier)
        XCTAssertEqual(bundle["irbProtocolId"] as? String, ResearchStudyConfiguration.unconfiguredStudyIdentifier)

        let sha = bundle["payloadSha256"] as? String
        XCTAssertNotNil(sha)
        XCTAssertEqual(sha?.count, 64, "SHA-256 hex string must be 64 characters")

        let json = self.researchStore.exportResearchKitJSON()
        XCTAssertTrue(json.contains("STUDY-ASD-SHA256"))
        XCTAssertTrue(json.contains("ORKActiveStepResult"))
        XCTAssertTrue(json.contains("v1.5"))
    }

    func testStudyBundleCarriesDiaryEmotionAndRecognition() {
        let entry = MoodDiaryEntry(
            emotion: .joy,
            recognition: .respondedToName,
            intensity: 0.7,
            durationMinutes: 8,
            tags: [.therapy],
            notes: "Called their name at the table",
            source: .manual
        )
        let bundle = researchStore.compileStudyBundle(diaryObservations: [DiaryObservationRecord(entry: entry)])
        let rows = bundle["diaryObservations"] as? [[String: Any]]
        XCTAssertEqual(rows?.count, 1)
        XCTAssertEqual(rows?.first?["emotion"] as? String, "joy")
        XCTAssertEqual(rows?.first?["recognition"] as? String, "respondedToName")
        XCTAssertEqual(rows?.first?["notesPresent"] as? Bool, true)
        XCTAssertNil(rows?.first?["notes"])
    }

    // MARK: - Helper

    private func sampleSummary(socialScore: Double) -> GazeSessionReportSummary {
        .fixture(
            sessionId: "sess_unit_test",
            durationSec: 15.0,
            totalFrames: 900,
            validFrames: 850,
            validRatio: 0.94,
            socialPreferenceScore: socialScore,
            socialDwellSec: 10.0,
            geometricDwellSec: 5.0,
            eyeDwellRatio: 0.48,
            mouthDwellRatio: 0.26,
            firstEyeLatencyMs: nil,
            socialSmileIntensityMean: 0.70,
            smileReactivityIndex: 0.78,
            flatAffectScore: 0.15,
            surpriseScoreMean: 0.08,
            qualityVerdict: .optimal
        )
    }
}
