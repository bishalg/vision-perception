@testable import AsdGazeCollector
import UserNotifications
import XCTest

/// WP-S.5b: every opt-in gates its feature (store-collection-build.md S-15..S-20).
@MainActor
final class OptInGatingTests: XCTestCase {
    // MARK: Clinic delivery gate (S-17)

    func testClinicDeliveryAuthorizationTruthTable() {
        for optedIn in [false, true] {
            for guardian in [false, true] {
                for required in [false, true] {
                    for research in [false, true] {
                        let gate = ClinicDeliveryAuthorization(
                            optedIn: optedIn,
                            guardianConsentCurrent: guardian,
                            researchConsentRequired: required,
                            researchConsented: research
                        )
                        let expected = optedIn && guardian && (!required || research)
                        XCTAssertEqual(gate.isAuthorized, expected, "\(gate)")
                        if !optedIn {
                            XCTAssertEqual(gate.refusal, .optedOut, "the opt-in is reported first")
                            XCTAssertEqual(gate.refusalText, ClinicDeliveryAuthorization.deliveryOffText)
                        } else if !expected {
                            XCTAssertEqual(gate.refusal, .consentInactive)
                            XCTAssertEqual(gate.refusalText, ClinicDeliveryAuthorization.consentInactiveText)
                        } else {
                            XCTAssertNil(gate.refusal)
                            XCTAssertNil(gate.refusalText)
                        }
                    }
                }
            }
        }
    }

    /// The store build needs no research-study consent: the guardian consent covers delivery.
    func testStoreBuildGateIgnoresTheResearchConsent() {
        let defaults = UserDefaults(suiteName: "OptInGatingTests_\(UUID().uuidString)")!
        let optIns = FeatureOptIns(defaults: defaults)
        let consentURL = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("optin_gate_consent_\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: consentURL) }
        let consent = ConsentRecordStore(fileURL: consentURL)

        func gate(storeBuild: Bool, research: Bool) -> ClinicDeliveryAuthorization {
            .live(researchConsented: research, optIns: optIns, consent: consent, storeBuild: storeBuild)
        }

        XCTAssertEqual(gate(storeBuild: true, research: true).refusal, .optedOut)
        optIns.clinicDelivery = true
        XCTAssertEqual(gate(storeBuild: true, research: true).refusal, .consentInactive, "no guardian consent yet")
        XCTAssertNotNil(consent.agree(guardianName: "A. Parent", relationship: .parent))
        XCTAssertTrue(gate(storeBuild: true, research: false).isAuthorized)
        XCTAssertEqual(gate(storeBuild: false, research: false).refusal, .consentInactive, "Debug/DemoRelease keep research consent")
        XCTAssertTrue(gate(storeBuild: false, research: true).isAuthorized)
    }

    func testRefusalTextsAreNeutral() {
        for text in [ClinicDeliveryAuthorization.deliveryOffText, ClinicDeliveryAuthorization.consentInactiveText,
                     ClinicSyncStatusLine.deliveryOffText(waiting: 2)] {
            XCTAssertFalse(text.isEmpty)
            for word in ["research", "study", "screen", "score", "flag", "risk", "diagnos"] {
                XCTAssertFalse(text.lowercased().contains(word), "\(text) contains \(word)")
            }
        }
        XCTAssertEqual(ClinicDeliveryAuthorization.deliveryOffText, "Clinic delivery is off.")
        XCTAssertEqual(ClinicDeliveryAuthorization.consentInactiveText, "Consent is not active.")
        XCTAssertEqual(
            ClinicSyncStatusLine.deliveryOffText(waiting: 2, lang: .english),
            "Clinic delivery is off. 2 session(s) waiting on this device."
        )
    }

    // MARK: Reminders (S-15)

    func testCareAlertDecisionFollowsTheRemindersOptIn() {
        XCTAssertFalse(CareAlertScheduler.shouldSchedule(optedIn: false, masterEnabled: true))
        XCTAssertFalse(CareAlertScheduler.shouldSchedule(optedIn: true, masterEnabled: false))
        XCTAssertTrue(CareAlertScheduler.shouldSchedule(optedIn: true, masterEnabled: true))

        XCTAssertFalse(CareAlertScheduler.shouldRequestAuthorization(optedIn: false, status: .notDetermined))
        XCTAssertTrue(CareAlertScheduler.shouldRequestAuthorization(optedIn: true, status: .notDetermined))
        XCTAssertFalse(CareAlertScheduler.shouldRequestAuthorization(optedIn: true, status: .denied))
        XCTAssertFalse(CareAlertScheduler.shouldRequestAuthorization(optedIn: true, status: .authorized))
    }

    // MARK: HealthKit mood (S-15)

    func testHealthKitMoodStoreShortCircuitsWhileOptedOut() async {
        let store = HealthKitMoodStore(optIn: { false })
        XCTAssertFalse(store.isOptedIn)
        await store.requestAuthorization()
        XCTAssertEqual(store.authorizationStatusText, Localizer.tr("healthkit_status_optin_off"))
        let saved = await store.save(MoodDiaryEntry(emotion: .joy))
        XCTAssertNil(saved)
        XCTAssertNil(store.lastErrorMessage, "nothing was attempted")
        let fetched = await store.fetchRecentStateOfMind()
        XCTAssertTrue(fetched.isEmpty)
        XCTAssertNil(store.lastErrorMessage, "nothing was attempted")
    }

    // MARK: Dictation (S-15)

    func testDictationRefusesToStartWhileOptedOut() async {
        let controller = ObservationDictationController(optIn: { false })
        do {
            try await controller.start { _ in }
            XCTFail("dictation must not start while its opt-in is off")
        } catch ObservationDictationError.optedOut {
            // expected: nothing is asked of iOS
        } catch {
            XCTFail("unexpected error \(error)")
        }
    }
}
