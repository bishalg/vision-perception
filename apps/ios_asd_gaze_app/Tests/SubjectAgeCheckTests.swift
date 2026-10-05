import XCTest
@testable import AsdGazeCollector
import CurvAsdGazeKit

@MainActor
final class SubjectAgeCheckTests: XCTestCase {
    private func signal(
        _ band: FacialAgeBand,
        stable: Bool,
        agreement: Double = 0.9,
        confidence: Double = 0.85,
        samples: Int = 12
    ) -> SubjectAgeSignal {
        SubjectAgeSignal(
            band: band,
            agreement: stable ? agreement : 0.4,
            confidence: confidence,
            sampleCount: stable ? samples : 3,
            isStable: stable
        )
    }

    // MARK: - Cross-Check Verdict

    func testCrossCheckWithoutSignalIsInsufficientData() {
        XCTAssertEqual(
            SubjectAgeCheckController.crossCheckVerdict(estimated: nil, isStable: false, pickedMonths: 28),
            .insufficientData
        )
        XCTAssertEqual(
            SubjectAgeCheckController.crossCheckVerdict(estimated: .toddler, isStable: false, pickedMonths: 28),
            .insufficientData
        )
    }

    func testCrossCheckWithinOneBandIsConsistent() {
        // 24–35 mo pick → toddler band; toddler estimate is a direct match.
        XCTAssertEqual(
            SubjectAgeCheckController.crossCheckVerdict(estimated: .toddler, isStable: true, pickedMonths: 28),
            .consistent
        )
        // 48–60 mo pick → preschool band; a toddler estimate is ONE band off.
        // One band of tolerance absorbs the known overestimation bias on young faces.
        XCTAssertEqual(
            SubjectAgeCheckController.crossCheckVerdict(estimated: .toddler, isStable: true, pickedMonths: 54),
            .consistent
        )
    }

    func testCrossCheckTwoBandsApartTriggersSoftConfirm() {
        // Toddler pick vs a teen/adult face = 3 bands apart.
        XCTAssertEqual(
            SubjectAgeCheckController.crossCheckVerdict(estimated: .teenAdult, isStable: true, pickedMonths: 28),
            .softConfirm(bandsApart: 3)
        )
        // Preschool pick vs teen/adult = 2 bands apart.
        XCTAssertEqual(
            SubjectAgeCheckController.crossCheckVerdict(estimated: .teenAdult, isStable: true, pickedMonths: 54),
            .softConfirm(bandsApart: 2)
        )
    }

    func testCrossCheckChildPickVsToddlerFaceIsSoftConfirm() {
        // A 7–12 yr "child" band pick (not a current bracket, but future-proof)
        // vs a toddler face = 2 bands apart.
        XCTAssertEqual(
            SubjectAgeCheckController.crossCheckVerdict(estimated: .toddler, isStable: true, pickedMonths: 120),
            .softConfirm(bandsApart: 2)
        )
    }

    // MARK: - Adult Holding Hint

    func testAdultHoldingHintRequiresStableTeenAdultSignal() {
        XCTAssertFalse(SubjectAgeCheckController.shouldShowAdultHoldingHint(signal: nil))
        XCTAssertFalse(SubjectAgeCheckController.shouldShowAdultHoldingHint(signal: signal(.teenAdult, stable: false)))
        XCTAssertFalse(SubjectAgeCheckController.shouldShowAdultHoldingHint(signal: signal(.toddler, stable: true)))
        XCTAssertFalse(SubjectAgeCheckController.shouldShowAdultHoldingHint(signal: signal(.preschool, stable: true)))
        XCTAssertTrue(SubjectAgeCheckController.shouldShowAdultHoldingHint(signal: signal(.teenAdult, stable: true)))
    }

    // MARK: - Status Line

    func testStatusLineOnlyForStableSignals() {
        XCTAssertNil(SubjectAgeCheckController.statusLine(for: nil))
        XCTAssertNil(SubjectAgeCheckController.statusLine(for: signal(.toddler, stable: false)))
        let line = SubjectAgeCheckController.statusLine(for: signal(.toddler, stable: true, confidence: 0.86))
        XCTAssertEqual(line, "Toddler (1–3 yrs) • 86% model confidence")
    }

    // MARK: - Controller State

    func testControllerObservesSignalAndVerdictUsesProfileBracket() throws {
        let original = ChildProfileStore.shared.profile.ageBracket
        defer { ChildProfileStore.shared.profile.ageBracket = original }

        ChildProfileStore.shared.profile.ageBracket = .months24_35  // → toddler band

        let controller = SubjectAgeCheckController()
        XCTAssertEqual(controller.currentVerdict, .insufficientData)

        controller.observe(signal(.toddler, stable: true))
        XCTAssertEqual(controller.currentVerdict, .consistent)

        controller.observe(signal(.teenAdult, stable: true))
        XCTAssertEqual(controller.currentVerdict, .softConfirm(bandsApart: 3))

        controller.reset()
        XCTAssertEqual(controller.currentVerdict, .insufficientData)
        XCTAssertNil(controller.lastSignal)
    }

    func testSoftConfirmDismissal() {
        let controller = SubjectAgeCheckController()
        XCTAssertFalse(controller.dismissedSoftConfirm)
        controller.dismissSoftConfirm()
        XCTAssertTrue(controller.dismissedSoftConfirm)
    }

    // MARK: - Age Assurance Gate

    func testAgeAssuranceBypassedInInternalBuilds() {
        // Tests run in the Debug configuration where ASD_APP_STORE_AGE_ASSURANCE
        // is NOT defined — internal builds must bypass the App Store check.
        XCTAssertFalse(AgeAssuranceGate.isAppStoreAgeAssuranceBuild)
    }
}
