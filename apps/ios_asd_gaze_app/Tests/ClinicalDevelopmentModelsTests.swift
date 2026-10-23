import CareKitStore
import XCTest
@testable import AsdGazeCollector

final class ClinicalDevelopmentModelsTests: XCTestCase {
    func testCatalogCoversSixAgesAndFourDomains() {
        let all = ClinicalMilestoneCatalog.all(completedAges: [])
        XCTAssertEqual(all.count, CDCSurveillanceAge.allCases.count * DevelopmentalDomain.allCases.count)
        XCTAssertEqual(Set(all.map(\.ageMonths)), Set(CDCSurveillanceAge.allCases.map(\.rawValue)))
        XCTAssertEqual(Set(all.map(\.domain)), Set(DevelopmentalDomain.allCases))
        XCTAssertTrue(all.allSatisfy { !$0.items.isEmpty })
        XCTAssertTrue(all.allSatisfy { !$0.items[0].whatToLookFor.isEmpty })
    }

    func testScreeningRiskScoreFollowUpOnlyMedium() {
        let low = ScreeningRiskScore(from: MCHATSurveyEngine.evaluate(answers: [:]))
        XCTAssertEqual(low.riskLevel, .low)
        XCTAssertFalse(low.requiresFollowUpInterview)

        var mediumAnswers: [Int: Bool] = [:]
        for q in MCHATSurveyEngine.questions { mediumAnswers[q.id] = !q.failAnswer }
        for id in [1, 2, 3] {
            mediumAnswers[id] = MCHATSurveyEngine.questions.first { $0.id == id }!.failAnswer
        }
        let medium = ScreeningRiskScore(from: MCHATSurveyEngine.evaluate(answers: mediumAnswers))
        XCTAssertEqual(medium.riskLevel, .medium)
        XCTAssertTrue(medium.requiresFollowUpInterview)
        XCTAssertTrue(medium.clinicalRecommendation.contains("Follow-Up"))
    }

    func testGazeTelemetrySessionMapsValidTimeAndSocialRatio() {
        let record = GazeSessionRecord.fixture(
            sessionId: "s1",
            participantId: "p",
            startedAt: Date().addingTimeInterval(-30),
            endedAt: Date(),
            durationSec: 30,
            socialPreferenceScore: 0.2,
            validFrameRatio: 0.9,
            eyeDwellRatio: 0.4,
            mouthDwellRatio: 0.2,
            firstEyeLatencyMs: 400,
            qualityVerdict: "optimal"
        )
        let session = GazeTelemetrySession(from: record)
        XCTAssertEqual(session.totalValidTimeSec, 27, accuracy: 0.01)
        XCTAssertEqual(session.socialTargetRatio, 0.6, accuracy: 0.01)
        XCTAssertEqual(session.stimulusTag, .faceToFace)
        XCTAssertGreaterThan(session.trackingConfidenceIndex, 0.8)
    }

    @MainActor
    func testMilestoneToggleUpdatesDomainGauge() {
        let store = CarePlanStore(clinicalStore: CareKitClinicalStore(
            store: OCKStore(name: "TestMilestones-\(UUID().uuidString)", type: .inMemory)
        ))
        store.milestones = ClinicalMilestoneCatalog.all(completedAges: [])
        guard let first = store.milestones.first, let item = first.items.first else {
            return XCTFail("catalog empty")
        }
        let before = store.domainCompletion(first.domain)
        store.toggleMilestoneItem(milestoneId: first.id, itemId: item.id)
        XCTAssertGreaterThan(store.domainCompletion(first.domain), before)
        XCTAssertTrue(store.milestones.first?.items.first?.isCompleted == true)
    }
}
