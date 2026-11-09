@testable import AsdGazeCollector
import XCTest

/// WP-S.5 (store-collection-build.md S-10 .. S-13): consent document, signed record,
/// opt-ins and permission copy. The real claims gate is
/// `python3 -I scripts/check_asd_claims.py --swift --prose`; the denylist check here is a
/// local mirror so a copy edit fails fast in Xcode too.
@MainActor
final class ConsentFlowTests: XCTestCase {
    private var tempURLs: [URL] = []
    private var suiteName: String!
    private var defaults: UserDefaults!

    override func setUp() async throws {
        try await super.setUp()
        suiteName = "ConsentFlowTests_\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)!
    }

    override func tearDown() async throws {
        for url in tempURLs { try? FileManager.default.removeItem(at: url) }
        defaults.removePersistentDomain(forName: suiteName)
        try await super.tearDown()
    }

    private func tempFile() -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("ConsentFlowTests_\(UUID().uuidString)", isDirectory: true)
            .appendingPathComponent("consent_record.json")
        tempURLs.append(url.deletingLastPathComponent())
        return url
    }

    // MARK: ConsentDocument

    func testDocumentHasSectionsInOrder() {
        XCTAssertEqual(ConsentDocument.current.version, "2026-11-08.2")
        XCTAssertEqual(ConsentDocument.current.sections.map(\.id), [
            "overview", "recorded", "neverRecorded", "dataLocation", "access",
            "choices", "withdrawing", "notDiagnostic", "time",
        ])
        XCTAssertEqual(Set(ConsentDocument.current.sections.map(\.id)).count, ConsentDocument.current.sections.count)
    }

    func testShaIsStableAndHex() {
        let a = ConsentDocument.current.sha256Hex
        let b = ConsentDocument(version: ConsentDocument.current.version, sections: ConsentDocument.current.sections).sha256Hex
        XCTAssertEqual(a, b)
        XCTAssertEqual(a.count, 64)
        XCTAssertTrue(a.allSatisfy { "0123456789abcdef".contains($0) })
    }

    func testShaChangesWhenTextOrVersionChanges() {
        let doc = ConsentDocument.current
        var sections = doc.sections
        let first = sections[0]
        sections[0] = ConsentSection(id: first.id, systemImage: first.systemImage, title: first.title,
                                     summary: first.summary + " Changed.", learnMore: first.learnMore)
        XCTAssertNotEqual(ConsentDocument(version: doc.version, sections: sections).sha256Hex, doc.sha256Hex)
        XCTAssertNotEqual(ConsentDocument(version: "2099-01-01.1", sections: doc.sections).sha256Hex, doc.sha256Hex)
    }

    func testNotDiagnosticSectionCarriesMandatorySentences() {
        let section = ConsentDocument.current.sections.first { $0.id == "notDiagnostic" }!
        XCTAssertTrue(section.learnMore.contains(ClaimsCopy.notDiagnostic))
        XCTAssertTrue(section.learnMore.contains(ClaimsCopy.clinicReviews))
        XCTAssertTrue(section.learnMore.contains(ConsentDocument.interpretationSentence))
        XCTAssertTrue(ConsentDocument.current.fullText.contains(ClaimsCopy.notDiagnostic))
    }

    // MARK: ConsentRecordStore

    func testRecordRoundTripsAndIsCurrent() throws {
        let url = tempFile()
        let agreedAt = Date(timeIntervalSince1970: 1_793_000_000)
        let store = ConsentRecordStore(fileURL: url, now: { agreedAt })
        XCTAssertNil(store.record)
        XCTAssertFalse(store.isCurrent)

        XCTAssertNil(store.agree(guardianName: "   ", relationship: .parent), "empty name is refused")
        XCTAssertNil(store.record)

        let record = try XCTUnwrap(store.agree(guardianName: "  Sam Guardian ", relationship: .legalGuardian))
        XCTAssertEqual(record.guardianName, "Sam Guardian")
        XCTAssertEqual(record.documentVersion, ConsentDocument.current.version)
        XCTAssertEqual(record.documentSha256, ConsentDocument.current.sha256Hex)
        XCTAssertTrue(store.isCurrent)
        XCTAssertFalse(store.needsReconsent)

        let reloaded = ConsentRecordStore(fileURL: url)
        XCTAssertEqual(reloaded.record, record)
        XCTAssertEqual(reloaded.record?.agreedAt, agreedAt)
        XCTAssertEqual(reloaded.record?.relationship, .legalGuardian)
        XCTAssertTrue(reloaded.isCurrent)
    }

    func testRecordIsExcludedFromBackup() throws {
        let url = tempFile()
        ConsentRecordStore(fileURL: url).agree(guardianName: "Sam", relationship: .parent)
        let values = try url.resourceValues(forKeys: [.isExcludedFromBackupKey])
        XCTAssertEqual(values.isExcludedFromBackup, true)
    }

    func testStaleVersionIsNotCurrent() {
        let url = tempFile()
        let old = ConsentDocument(version: "2026-01-01.1", sections: ConsentDocument.current.sections)
        ConsentRecordStore(fileURL: url, document: old).agree(guardianName: "Sam", relationship: .parent)

        let store = ConsentRecordStore(fileURL: url)
        XCTAssertNotNil(store.record)
        XCTAssertFalse(store.isCurrent)
        XCTAssertTrue(store.needsReconsent)
    }

    func testChangedTextWithSameVersionIsNotCurrent() {
        let url = tempFile()
        var sections = ConsentDocument.current.sections
        let last = sections.removeLast()
        sections.append(ConsentSection(id: last.id, systemImage: last.systemImage, title: last.title,
                                       summary: "Older wording.", learnMore: last.learnMore))
        let edited = ConsentDocument(version: ConsentDocument.current.version, sections: sections)
        ConsentRecordStore(fileURL: url, document: edited).agree(guardianName: "Sam", relationship: .parent)
        XCTAssertFalse(ConsentRecordStore(fileURL: url).isCurrent)
    }

    func testRevokeClearsRecordAndFile() {
        let url = tempFile()
        let store = ConsentRecordStore(fileURL: url)
        store.agree(guardianName: "Sam", relationship: .clinicStaff)
        XCTAssertTrue(FileManager.default.fileExists(atPath: url.path))
        store.revoke()
        XCTAssertNil(store.record)
        XCTAssertFalse(store.isCurrent)
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        XCTAssertNil(ConsentRecordStore(fileURL: url).record)
    }

    func testRelationshipsHaveDisplayNames() {
        XCTAssertEqual(GuardianRelationship.allCases.count, 3)
        for r in GuardianRelationship.allCases { XCTAssertFalse(r.displayName.isEmpty) }
    }

    // MARK: FeatureOptIns

    func testFacialExpressionIsNotAnOptInAndIsDisclosed() {
        XCTAssertFalse(FeatureOptIn.allCases.map(\.rawValue).contains("facialExpressionMetrics"))
        let recorded = ConsentDocument.current.sections.first { $0.id == "recorded" }!
        XCTAssertTrue(recorded.learnMore.contains("Facial-expression measurements, for example how much your child smiles."))
        XCTAssertFalse(recorded.learnMore.contains("if you turn them on"))
    }

    func testOptInsDefaultAllOff() {
        let optIns = FeatureOptIns(defaults: defaults)
        for feature in FeatureOptIn.allCases {
            XCTAssertFalse(optIns.isOn(feature), "\(feature) must default to off (S-12)")
        }
    }

    func testOptInsPersistAndReset() {
        let optIns = FeatureOptIns(defaults: defaults)
        optIns.set(.reminders, true)
        optIns.clinicDelivery = true
        let reloaded = FeatureOptIns(defaults: defaults)
        XCTAssertTrue(reloaded.reminders)
        XCTAssertTrue(reloaded.clinicDelivery)
        XCTAssertFalse(reloaded.dictation)
        reloaded.resetAll()
        XCTAssertFalse(FeatureOptIns(defaults: defaults).reminders)
    }

    func testOptInCopyIsCompleteAndHonest() {
        for feature in FeatureOptIn.allCases {
            XCTAssertFalse(feature.title.isEmpty)
            XCTAssertFalse(feature.systemImage.isEmpty)
            XCTAssertFalse(feature.oneLineWhatItDoes.isEmpty)
            XCTAssertFalse(feature.whatIsStored.isEmpty)
            XCTAssertFalse(feature.whatIsNeverStored.isEmpty)
            // S-15: every opt-in gates its feature, so no row hedges about it any more.
            XCTAssertTrue(feature.isGated, "\(feature)")
            XCTAssertFalse(feature.rowDescription.contains("Controls whether"), "\(feature)")
            XCTAssertEqual(feature.rowDescription, feature.oneLineWhatItDoes)
        }
        XCTAssertNotNil(FeatureOptIn.clinicDelivery.whenTurnedOff, "S-18: the detail page says what off does")
    }

    /// S-16: the store build shows only the opt-ins whose feature it has.
    func testStoreBuildOffersOnlyAgeRangeAndClinicDelivery() {
        XCTAssertEqual(FeatureOptIn.available(storeBuild: true), [.ageRangeCheck, .clinicDelivery])
        XCTAssertEqual(FeatureOptIn.available(storeBuild: false), FeatureOptIn.allCases)
        #if ASD_STORE_BUILD
        XCTAssertEqual(FeatureOptIn.available(), [.ageRangeCheck, .clinicDelivery])
        #else
        XCTAssertEqual(FeatureOptIn.available(), FeatureOptIn.allCases)
        #endif
    }

    // MARK: PermissionKind

    func testEveryPermissionHasCopy() {
        for kind in PermissionKind.allCases {
            XCTAssertFalse(kind.title.isEmpty, "\(kind)")
            XCTAssertFalse(kind.systemImage.isEmpty, "\(kind)")
            XCTAssertFalse(kind.whatWeUse.isEmpty, "\(kind)")
            XCTAssertFalse(kind.why.isEmpty, "\(kind)")
            XCTAssertFalse(kind.neverStored.isEmpty, "\(kind)")
            XCTAssertEqual(kind.howToChangeLater, "You can change this any time in iOS Settings > EarlyGaze.")
        }
        XCTAssertNotNil(PermissionKind.motion.statusNote, "motion has no runtime prompt; the primer says so")
    }

    func testAgeRangeStatusIsOnRequestAndMotionNeedsNoPrompt() async {
        let age = await PermissionKind.ageRange.currentStatus()
        XCTAssertEqual(age, .onRequest)
        let motion = await PermissionKind.motion.currentStatus()
        XCTAssertTrue([PermissionStatus.noPromptNeeded, .unavailable].contains(motion))
    }

    // MARK: Claims (local mirror of scripts/check_asd_claims.py)

    private static let deny: [String] = [
        #"\bFDA\b"#, #"\bHIPAA\b"#, #"\bIRB\b"#, #"clinically\s+validated"#, #"\bclinical(ly)?\b"#,
        #"bio-?markers?"#, #"\bADHD\b"#, #"autism\s+(screening|screener|test)"#, #"\bscreen(ing|er)s?\b"#,
        #"risk\s+(score|percentage|stratification)"#, #"\bbayesian\b"#, #"\bprobabilit(y|ies)\b"#,
        #"\b(research\s+study|principal\s+investigator)\b"#, #"\bresearch\b"#, #"\bstudy\b"#, #"\btests?\b"#,
    ]
    private static let negation = #"\b(not|no|never|cannot|without|nor)\b|n't"#

    private func violations(_ text: String) -> [String] {
        var out: [String] = []
        for pattern in Self.deny where text.range(of: pattern, options: [.regularExpression, .caseInsensitive]) != nil {
            out.append(pattern)
        }
        let sentences = text.components(separatedBy: CharacterSet(charactersIn: ".!?\n"))
        for s in sentences where s.range(of: "diagnos", options: .caseInsensitive) != nil
            && s.range(of: Self.negation, options: [.regularExpression, .caseInsensitive]) == nil {
            out.append("diagnos outside a negating sentence: \(s)")
        }
        return out
    }

    func testConsentCopyHasNoDeniedTerms() {
        var texts: [String] = []
        for section in ConsentDocument.current.sections {
            texts += [section.title, section.summary, section.learnMore]
        }
        for feature in FeatureOptIn.allCases {
            texts += [feature.title, feature.rowDescription, feature.whatIsStored, feature.whatIsNeverStored,
                      feature.whenTurnedOff ?? ""]
        }
        for kind in PermissionKind.allCases {
            texts += [kind.title, kind.whatWeUse, kind.why, kind.neverStored, kind.statusNote ?? ""]
        }
        texts += BeforeYouBeginView.checklist.map(\.text)
        let optIns = FeatureOptIns(defaults: defaults)
        texts += BeforeYouBeginView.recordedItems(optIns: optIns, deliveryConfigured: true, isEnrolled: true)
        optIns.clinicDelivery = true
        for enrolled in [false, true] {
            texts += BeforeYouBeginView.recordedItems(optIns: optIns, deliveryConfigured: true, isEnrolled: enrolled)
        }
        // WP-S.5b copy outside the consent files (Localizer keys).
        texts += [ClinicDeliveryAuthorization.deliveryOffText, ClinicDeliveryAuthorization.consentInactiveText,
                  ClinicSyncStatusLine.deliveryOffText(waiting: 1), SessionStorageState.deliveryOff.text]
        texts += ["enrolment_title", "enrolment_code_placeholder", "enrolment_submit", "enrolment_status_unavailable",
                  "enrolment_status_unregistered", "enrolment_status_registered", "enrolment_status_expired",
                  "enrolment_code_rejected", "enrolment_needs_optin", "settings_alerts_optin_off",
                  "healthkit_optin_off", "healthkit_status_optin_off", "quicklog_optins_off",
                  "quicklog_dictation_off"].map { Localizer.tr($0, lang: .english) }
        texts.append(ConsentReviewView.confirmationSentence)
        // Submission hardening: delete-all copy (S-24), plain Settings rows (S-23), sync status (S-26).
        texts += [DeleteAllCopy.title, DeleteAllCopy.button, DeleteAllCopy.confirmTitle, DeleteAllCopy.confirm,
                  DeleteAllCopy.message, DeleteAllCopy.detail, DeleteAllCopy.done, DeleteAllCopy.failed]
        texts += ["settings_sensors_section", "settings_truedepth_title", "settings_truedepth_available",
                  "settings_truedepth_unavailable", "settings_status_active", "settings_status_unavailable",
                  "settings_coremotion", "settings_coremotion_sub", "settings_calibrated_sub",
                  "settings_zero_video_title", "settings_zero_video_desc", "settings_encrypted_vault_title",
                  "settings_encrypted_vault_desc", "sync_state_ready", "sync_state_sending", "sync_state_sent",
                  "sync_state_failed"].map { Localizer.tr($0, lang: .english) }
        texts += [ResearchSyncState.idle.description, ResearchSyncState.syncing.description,
                  ResearchSyncState.failed(reason: "Offline. Data safely queued.").description]
        texts += [UnsupportedDeviceContent.make(for: .consentOutdated).title,
                  UnsupportedDeviceContent.make(for: .consentOutdated).message]
        for text in texts {
            XCTAssertEqual(violations(text), [], "denied wording in: \(text)")
        }
    }

    func testDenylistMirrorCatchesSeededViolations() {
        XCTAssertFalse(violations("A screening tool.").isEmpty)
        XCTAssertFalse(violations("It diagnoses early.").isEmpty)
        XCTAssertTrue(violations("This is not a diagnostic tool.").isEmpty)
    }

    func testBeforeYouBeginIsAdditiveToTheCaptureOrder() {
        XCTAssertFalse(ScreeningWizardStep.captureOrder.contains(.beforeYouBegin))
        XCTAssertNil(ScreeningWizardStep.beforeYouBegin.nextCaptureStep)
        XCTAssertNotEqual(ScreeningWizardStep.beforeYouBegin, .positioning)
    }

    func testBeforeYouBeginReflectsOptIns() {
        let optIns = FeatureOptIns(defaults: defaults)
        func items(configured: Bool, enrolled: Bool) -> [String] {
            BeforeYouBeginView.recordedItems(optIns: optIns, deliveryConfigured: configured, isEnrolled: enrolled)
        }
        // S-12 (amended): facial-expression measurements are always recorded, so always listed.
        XCTAssertTrue(items(configured: true, enrolled: true).contains { $0.contains("Facial") })

        // S-20: opt-in off -> kept on this device only, whatever the enrolment.
        XCTAssertEqual(items(configured: true, enrolled: true).last, "Kept on this device only")
        optIns.clinicDelivery = true
        // On, but no upload configured in this build.
        XCTAssertEqual(items(configured: false, enrolled: false).last, "Kept on this device only")
        // On and configured, not enrolled yet.
        XCTAssertEqual(items(configured: true, enrolled: false).last, "Kept on this device until your clinic enrols it")
        // On, configured and enrolled.
        XCTAssertEqual(items(configured: true, enrolled: true).last, "Delivered to your clinic")
        XCTAssertEqual(BeforeYouBeginView.checklist.count, 4)
    }
}
