@testable import AsdGazeCollector
import CurvAsdGazeKit
import XCTest

/// FHIR-1..4, FHIR-6, FHIR-7, PLC-3, SIM-5, CNS-7 (ADR-0003).
final class FHIRBundleBuilderTests: XCTestCase {

    // MARK: - Fixtures

    private static let goldenDevice = FHIRDeviceInfo(displayName: "EarlyGaze Observation Diary", version: "1.0.0")

    private static func date(_ iso: String) -> Date {
        ISO8601DateFormatter().date(from: iso)!
    }

    /// The record behind `Tests/Fixtures/expected-bundle.json`. Mouth dwell is absent and first-look
    /// latency is present: neither may appear in the bundle (PLC-3, FHIR-3).
    private static func goldenRecord() -> GazeSessionRecord {
        GazeSessionRecord.fixture(
            id: "REC-GOLDEN-01",
            sessionId: "GOLDEN01-SESSION",
            participantId: "child-GOLDEN01",
            captureSource: .trueDepth,
            startedAt: date("2026-10-01T10:00:00Z"),
            endedAt: date("2026-10-01T10:00:30Z"),
            durationSec: 30,
            socialPreferenceScore: 0.42,
            validFrameRatio: 0.9,
            eyeDwellRatio: 0.55,
            mouthDwellRatio: nil,
            firstEyeLatencyMs: 310,
            qualityVerdict: "optimal",
            socialSmileIntensity: 0.3,
            smileReactivityIndex: 0.25,
            flatAffectScore: 0.2,
            surpriseScoreMean: 0.1,
            perVideoComposite: 0.6,
            perVideo: [
                PerVideoScoreReport(
                    stimulusId: "clip-a", title: "Bubbles", category: "social", rows: [],
                    subscore: 0.7, validSampleRatio: 0.9, lowConfidence: false, hasBaseline: true, weight: 1
                ),
                PerVideoScoreReport(
                    stimulusId: "clip-b", title: "Untagged", category: "geometric", rows: [],
                    subscore: 0, validSampleRatio: 0.9, lowConfidence: false, hasBaseline: false, weight: 1
                )
            ]
        )
    }

    private static func goldenMCHAT() -> (MCHATScoreResult, [Int: Bool]) {
        let result = MCHATScoreResult(
            totalFailed: 2,
            riskLevel: .low,
            failedQuestionIds: [2, 5],
            clinicalRecommendation: "unused",
            completionDate: date("2026-10-01T09:00:00Z")
        )
        var answers: [Int: Bool] = [:]
        for q in MCHATSurveyEngine.questions {
            answers[q.id] = [2, 5].contains(q.id) ? q.failAnswer : !q.failAnswer
        }
        return (result, answers)
    }

    private static let goldenSubject = FHIRSubject(participantToken: "child-GOLDEN01", ageMonths: 24)

    private func goldenBundle() -> [String: AnyCodableValue] {
        FHIRBundleBuilder.bundle(
            for: Self.goldenRecord(),
            mchat: Self.goldenMCHAT(),
            extraResources: [],
            subject: Self.goldenSubject,
            device: Self.goldenDevice
        )
    }

    // MARK: - Helpers

    private func json(_ value: [String: AnyCodableValue]) throws -> String {
        String(data: try FHIRBundleBuilder.jsonData(value), encoding: .utf8) ?? ""
    }

    private func entries(_ bundle: [String: AnyCodableValue]) -> [[String: AnyCodableValue]] {
        guard case let .array(items)? = bundle["entry"] else { return [] }
        return items.compactMap { item in
            guard case let .dictionary(entry) = item else { return nil }
            return entry
        }
    }

    private func resources(_ bundle: [String: AnyCodableValue], type: String) -> [[String: AnyCodableValue]] {
        entries(bundle).compactMap { entry in
            guard case let .dictionary(resource)? = entry["resource"],
                  resource["resourceType"] == .string(type) else { return nil }
            return resource
        }
    }

    private func fullUrl(of resource: [String: AnyCodableValue], in bundle: [String: AnyCodableValue]) -> String? {
        for entry in entries(bundle) {
            if case let .dictionary(r)? = entry["resource"], r == resource, case let .string(url)? = entry["fullUrl"] {
                return url
            }
        }
        return nil
    }

    private func reference(_ value: AnyCodableValue?) -> String? {
        guard case let .dictionary(dict)? = value, case let .string(ref)? = dict["reference"] else { return nil }
        return ref
    }

    private func codes(in value: AnyCodableValue?) -> [String] {
        guard case let .dictionary(cc)? = value, case let .array(codings)? = cc["coding"] else { return [] }
        return codings.compactMap { coding in
            guard case let .dictionary(c) = coding, case let .string(code)? = c["code"] else { return nil }
            return code
        }
    }

    private func componentCodes(_ observation: [String: AnyCodableValue]) -> [String] {
        guard case let .array(components)? = observation["component"] else { return [] }
        return components.flatMap { component -> [String] in
            guard case let .dictionary(dict) = component else { return [] }
            return codes(in: dict["code"])
        }
    }

    private func gazeObservations(_ bundle: [String: AnyCodableValue]) -> [[String: AnyCodableValue]] {
        resources(bundle, type: "Observation").filter { codes(in: $0["code"]).contains("93044-6") }
    }

    // MARK: - Golden (FHIR-1, FHIR-3, FHIR-4)

    func testBundleMatchesGoldenFixture() throws {
        let actual = try FHIRBundleBuilder.jsonData(goldenBundle())
        let expected = try Data(contentsOf: try FHIRFixtures.url("expected-bundle.json"))

        let actualObject = try JSONSerialization.jsonObject(with: actual) as? NSDictionary
        let expectedObject = try JSONSerialization.jsonObject(with: expected) as? NSDictionary
        XCTAssertNotNil(expectedObject)
        if actualObject != expectedObject {
            let out = FileManager.default.temporaryDirectory.appendingPathComponent("actual-bundle.json")
            try? actual.write(to: out)
            XCTFail("bundle differs from Tests/Fixtures/expected-bundle.json; actual written to \(out.path)")
        }
    }

    func testBundleIsDeterministic() throws {
        XCTAssertEqual(try json(goldenBundle()), try json(goldenBundle()))
    }

    // MARK: - FHIR-1 shape

    func testBundleIsCollectionWithUrnUuidFullUrls() {
        let bundle = goldenBundle()
        XCTAssertEqual(bundle["resourceType"], .string("Bundle"))
        XCTAssertEqual(bundle["type"], .string("collection"), "ADR-0003 decision 1: collection, not transaction")
        let urls = entries(bundle).compactMap { entry -> String? in
            guard case let .string(url)? = entry["fullUrl"] else { return nil }
            return url
        }
        XCTAssertEqual(urls.count, entries(bundle).count)
        XCTAssertEqual(Set(urls).count, urls.count, "fullUrls are unique")
        for url in urls {
            XCTAssertTrue(url.hasPrefix("urn:uuid:"), url)
            XCTAssertNotNil(UUID(uuidString: String(url.dropFirst("urn:uuid:".count))), url)
        }
    }

    func testBundleContainsPatientRelatedPersonDeviceOnce() {
        let bundle = goldenBundle()
        XCTAssertEqual(resources(bundle, type: "Patient").count, 1)
        XCTAssertEqual(resources(bundle, type: "RelatedPerson").count, 1)
        XCTAssertEqual(resources(bundle, type: "Device").count, 1)
    }

    func testPatientIsPseudonymousWithAgeExtension() throws {
        let patient = try XCTUnwrap(resources(goldenBundle(), type: "Patient").first)
        XCTAssertEqual(patient["id"], .string("child-GOLDEN01"))
        XCTAssertNil(patient["name"], "CNS-7: no name on Patient")
        XCTAssertNil(patient["birthDate"], "FHIR-1: no birth date")
        guard case let .array(extensions)? = patient["extension"], case let .dictionary(ext)? = extensions.first,
              case let .dictionary(quantity)? = ext["valueQuantity"] else {
            return XCTFail("age-in-months extension missing")
        }
        XCTAssertEqual(ext["url"], .string("\(FHIRNamespaces.local)/StructureDefinition/age-in-months"))
        XCTAssertEqual(quantity["value"], .int(24))
        XCTAssertEqual(quantity["code"], .string("mo"))
    }

    func testPatientWithoutAgeHasNoExtension() throws {
        let bundle = FHIRBundleBuilder.bundle(
            for: Self.goldenRecord(), mchat: nil, extraResources: [],
            subject: FHIRSubject(participantToken: "child-GOLDEN01", ageMonths: nil)
        )
        let patient = try XCTUnwrap(resources(bundle, type: "Patient").first)
        XCTAssertNil(patient["extension"])
    }

    func testChildNameNeverAppearsInBundle() throws {
        let childName = "Aarav Sharma"
        let summary = GazeSessionReportSummary.fixture(sessionId: "SESS-NAME-001")
        let token = GazeSessionRecord.participantToken(enrollment: nil, sessionId: summary.sessionId)
        let record = GazeSessionRecord(summary: summary, participantToken: token)
        let bundle = FHIRBundleBuilder.bundle(
            for: record, mchat: Self.goldenMCHAT(), extraResources: [],
            subject: FHIRSubject(record: record, ageMonths: 30)
        )
        let text = try json(bundle)
        XCTAssertFalse(text.contains("Aarav"))
        XCTAssertFalse(text.contains(childName))
        XCTAssertTrue(text.contains("child-SESS-NAM"))
        for observation in resources(bundle, type: "Observation") {
            XCTAssertEqual(reference(observation["subject"]), FHIRBundleBuilder.patientReference(for: FHIRSubject(record: record, ageMonths: 30)))
            XCTAssertTrue(reference(observation["subject"])?.hasPrefix("urn:uuid:") ?? false, "no bare Patient/<id>")
        }
    }

    func testRelatedPersonIsParentWithoutNameByDefault() throws {
        let person = try XCTUnwrap(resources(goldenBundle(), type: "RelatedPerson").first)
        XCTAssertNil(person["name"])
        guard case let .array(relationships)? = person["relationship"] else { return XCTFail("relationship missing") }
        XCTAssertEqual(relationships.flatMap { codes(in: $0) }, ["PRN"])
        XCTAssertEqual(reference(person["patient"]), FHIRBundleBuilder.patientReference(for: Self.goldenSubject))
    }

    func testRelatedPersonCarriesCaregiverNameOnlyWhenOptedIn() throws {
        let subject = FHIRSubject(participantToken: "child-GOLDEN01", ageMonths: 24, caregiverDisplayName: "Sita")
        let bundle = FHIRBundleBuilder.bundle(for: Self.goldenRecord(), mchat: nil, extraResources: [], subject: subject)
        let person = try XCTUnwrap(resources(bundle, type: "RelatedPerson").first)
        guard case let .array(names)? = person["name"], case let .dictionary(name)? = names.first else {
            return XCTFail("opted-in caregiver name missing")
        }
        XCTAssertEqual(name["text"], .string("Sita"))

        let blank = FHIRSubject(participantToken: "child-GOLDEN01", ageMonths: 24, caregiverDisplayName: "   ")
        let blankBundle = FHIRBundleBuilder.bundle(for: Self.goldenRecord(), mchat: nil, extraResources: [], subject: blank)
        XCTAssertNil(resources(blankBundle, type: "RelatedPerson").first?["name"])
    }

    func testDeviceNamesTheAppAndVersion() throws {
        let device = try XCTUnwrap(resources(goldenBundle(), type: "Device").first)
        guard case let .array(names)? = device["deviceName"], case let .dictionary(name)? = names.first,
              case let .array(versions)? = device["version"], case let .dictionary(version)? = versions.first,
              case let .dictionary(type)? = device["type"] else {
            return XCTFail("Device shape")
        }
        XCTAssertEqual(name["name"], .string("EarlyGaze Observation Diary"))
        XCTAssertEqual(version["value"], .string("1.0.0"))
        XCTAssertEqual(type["text"], .string("Mobile gaze engagement observation app"))
    }

    func testCurrentDeviceInfoUsesMarketingVersion() {
        let info = FHIRDeviceInfo.current
        XCTAssertFalse(info.displayName.isEmpty)
        XCTAssertEqual(info.version, Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String)
    }

    // MARK: - Provenance (FHIR-1, SIM-4, acceptance 4)

    func testEveryObservationResourceHasOneProvenanceNamingTheDevice() throws {
        let bundle = goldenBundle()
        let deviceURL = try XCTUnwrap(resources(bundle, type: "Device").first.flatMap { fullUrl(of: $0, in: bundle) })
        let provenances = resources(bundle, type: "Provenance")
        let observationTypes: Set<String> = ["Observation", "QuestionnaireResponse", "MedicationStatement"]
        let targets = entries(bundle).compactMap { entry -> String? in
            guard case let .dictionary(r)? = entry["resource"], case let .string(type)? = r["resourceType"],
                  observationTypes.contains(type), case let .string(url)? = entry["fullUrl"] else { return nil }
            return url
        }
        XCTAssertEqual(targets.count, 3, "gaze Observation, QuestionnaireResponse, M-CHAT score Observation")
        for target in targets {
            let matching = provenances.filter { provenance in
                guard case let .array(t)? = provenance["target"] else { return false }
                return t.contains { reference($0) == target }
            }
            XCTAssertEqual(matching.count, 1, "exactly one Provenance for \(target)")
            let provenance = try XCTUnwrap(matching.first)
            XCTAssertEqual(codes(in: provenance["activity"]), ["CREATE"])
            guard case let .dictionary(activity)? = provenance["activity"], case let .array(codings)? = activity["coding"],
                  case let .dictionary(coding)? = codings.first else { return XCTFail("activity") }
            XCTAssertEqual(coding["system"], .string("http://terminology.hl7.org/CodeSystem/v3-DataOperation"))
            guard case let .array(agents)? = provenance["agent"], case let .dictionary(agent)? = agents.first else {
                return XCTFail("agent")
            }
            XCTAssertEqual(reference(agent["who"]), deviceURL)
            XCTAssertNotNil(provenance["recorded"])
        }
    }

    func testGazeProvenanceRecordsTrueDepthCaptureSourceAndVaultFile() throws {
        let bundle = goldenBundle()
        let observation = try XCTUnwrap(gazeObservations(bundle).first)
        let obsURL = try XCTUnwrap(fullUrl(of: observation, in: bundle))
        let provenance = try XCTUnwrap(resources(bundle, type: "Provenance").first { p in
            guard case let .array(t)? = p["target"] else { return false }
            return t.contains { reference($0) == obsURL }
        })
        XCTAssertEqual(provenance["recorded"], .string("2026-10-01T10:00:30Z"))
        guard case let .array(extensions)? = provenance["extension"], case let .dictionary(ext)? = extensions.first,
              case let .dictionary(coding)? = ext["valueCoding"] else {
            return XCTFail("capture-source extension missing")
        }
        XCTAssertEqual(ext["url"], .string(FHIRNamespaces.captureSourceExtension))
        XCTAssertEqual(coding["code"], .string("truedepth"))
        guard case let .array(entities)? = provenance["entity"], case let .dictionary(entity)? = entities.first,
              case let .dictionary(what)? = entity["what"], case let .dictionary(identifier)? = what["identifier"] else {
            return XCTFail("vault entity missing")
        }
        XCTAssertEqual(entity["role"], .string("source"))
        XCTAssertEqual(identifier["value"], .string("session_GOLDEN01-SESSION"))
    }

    // MARK: - FHIR-2 status and refusal (SIM-5)

    func testObservationStatusFollowsCaptureSourceAndQuality() {
        func status(_ verdict: String, _ source: CaptureSource = .trueDepth) -> String? {
            FHIRBundleBuilder.observationStatus(for: GazeSessionRecord.fixture(captureSource: source, qualityVerdict: verdict))
        }
        XCTAssertEqual(status("optimal"), "final")
        XCTAssertEqual(status("acceptable"), "final")
        XCTAssertEqual(status("degraded"), "preliminary")
        XCTAssertNil(status("rejected"))
        XCTAssertNil(status("not-a-verdict"))
        XCTAssertNil(status("optimal", .simulated))
    }

    func testDegradedRecordExportsPreliminaryObservation() throws {
        let bundle = FHIRBundleBuilder.bundle(
            for: GazeSessionRecord.fixture(qualityVerdict: "degraded"), mchat: nil, extraResources: [],
            subject: FHIRSubject(participantToken: "child-TEST0000", ageMonths: nil)
        )
        XCTAssertEqual(try XCTUnwrap(gazeObservations(bundle).first)["status"], .string("preliminary"))
    }

    func testSimulatedRecordIsRefused() throws {
        let record = GazeSessionRecord.fixture(sessionId: "SIMSESSION", captureSource: .simulated, eyeDwellRatio: 0.5)
        XCTAssertNotNil(FHIRBundleBuilder.exportRefusalReason(for: record))
        let bundle = FHIRBundleBuilder.bundle(
            for: record, mchat: nil, extraResources: [],
            subject: FHIRSubject(record: record, ageMonths: nil)
        )
        XCTAssertTrue(gazeObservations(bundle).isEmpty, "SIM-5: no Observation for simulated input")
        XCTAssertTrue(resources(bundle, type: "Provenance").isEmpty)
        let text = try json(bundle)
        XCTAssertFalse(text.contains("SIMSESSION"))
        XCTAssertFalse(text.contains("\"simulated\""))
    }

    func testRejectedRecordIsRefused() {
        let record = GazeSessionRecord.fixture(qualityVerdict: "rejected")
        XCTAssertNotNil(FHIRBundleBuilder.exportRefusalReason(for: record))
        let bundle = FHIRBundleBuilder.bundle(
            for: record, mchat: nil, extraResources: [],
            subject: FHIRSubject(record: record, ageMonths: nil)
        )
        XCTAssertTrue(gazeObservations(bundle).isEmpty)
    }

    func testLiveRecordHasNoRefusalReason() {
        XCTAssertNil(FHIRBundleBuilder.exportRefusalReason(for: GazeSessionRecord.fixture()))
        XCTAssertNil(FHIRBundleBuilder.exportRefusalReason(for: GazeSessionRecord.fixture(qualityVerdict: "degraded")))
    }

    func testLegacyProvenanceRecordIsTagged() throws {
        let bundle = FHIRBundleBuilder.bundle(
            for: GazeSessionRecord.fixture(legacyProvenance: true), mchat: nil, extraResources: [],
            subject: FHIRSubject(participantToken: "child-TEST0000", ageMonths: nil)
        )
        let observation = try XCTUnwrap(gazeObservations(bundle).first)
        guard case let .dictionary(meta)? = observation["meta"], case let .array(tags)? = meta["tag"],
              case let .dictionary(tag)? = tags.first else {
            return XCTFail("legacy tag missing")
        }
        XCTAssertEqual(tag["code"], .string("legacy-provenance"))
        XCTAssertNil(try XCTUnwrap(gazeObservations(goldenBundle()).first)["meta"])
    }

    // MARK: - FHIR-3 coding, PLC-3 absent metrics

    func testComponentsUseLocalGazeEngagementCodeSystem() throws {
        let observation = try XCTUnwrap(gazeObservations(goldenBundle()).first)
        XCTAssertEqual(codes(in: observation["code"]), ["93044-6", "713444003"])
        guard case let .array(components)? = observation["component"] else { return XCTFail("components") }
        for component in components {
            guard case let .dictionary(c) = component, case let .dictionary(code)? = c["code"],
                  case let .array(codings)? = code["coding"] else { return XCTFail("component code") }
            for coding in codings {
                guard case let .dictionary(cd) = coding else { continue }
                XCTAssertEqual(cd["system"], .string(FHIRNamespaces.gazeEngagementCodeSystem))
            }
        }
        XCTAssertEqual(FHIRNamespaces.gazeEngagementCodeSystem, "http://curv.org/fhir/CodeSystem/gaze-engagement")
        XCTAssertEqual(componentCodes(observation), [
            "social-preference-score",
            "eye-region-dwell-ratio",
            "valid-frame-ratio",
            "social-smile-intensity-mean",
            "smile-reactivity-index",
            "flat-affect-score",
            "surprise-score-mean",
            "composite-engagement",
            "clip-score-clip-a"
        ])
    }

    func testAbsentMetricsAreAbsentComponents() throws {
        let record = GazeSessionRecord.fixture(eyeDwellRatio: nil, mouthDwellRatio: nil, firstEyeLatencyMs: nil)
        let bundle = FHIRBundleBuilder.bundle(
            for: record, mchat: nil, extraResources: [],
            subject: FHIRSubject(record: record, ageMonths: nil)
        )
        let observation = try XCTUnwrap(gazeObservations(bundle).first)
        let codes = componentCodes(observation)
        XCTAssertFalse(codes.contains("eye-region-dwell-ratio"))
        XCTAssertFalse(codes.contains("mouth-region-dwell-ratio"))
        XCTAssertFalse(codes.contains("composite-engagement"))
        XCTAssertFalse(try json(bundle).contains("dataAbsentReason"))
    }

    func testNoSNOMEDFindingOrLatencyCodeInBundle() throws {
        let text = try json(goldenBundle())
        XCTAssertFalse(text.contains("247540003"), "FHIR-3: SNOMED finding is never a measurement code")
        XCTAssertFalse(text.contains("8692-2"), "PLC-1: no first-look latency component")
        XCTAssertFalse(text.contains("referenceRange"))
    }

    // MARK: - FHIR-4 M-CHAT-R/F

    func testQuestionnaireResponseUsesCanonicalAndLOINCLinkIds() throws {
        let bundle = goldenBundle()
        let response = try XCTUnwrap(resources(bundle, type: "QuestionnaireResponse").first)
        XCTAssertEqual(response["questionnaire"], .string("http://loinc.org/q/80628-1"))
        XCTAssertEqual(response["status"], .string("completed"))
        guard case let .array(items)? = response["item"] else { return XCTFail("items") }
        let linkIds = items.compactMap { item -> String? in
            guard case let .dictionary(i) = item, case let .string(id)? = i["linkId"] else { return nil }
            return id
        }
        XCTAssertEqual(linkIds, (1...20).map { StandardCodingSystems.LOINC.mchatQuestionCodes[$0]! })
        XCTAssertFalse(try json(bundle).contains("80628-1-"), "no invented linkIds")
    }

    func testMCHATScoreObservationCarriesTotalAndCategory() throws {
        let bundle = goldenBundle()
        let score = try XCTUnwrap(resources(bundle, type: "Observation").first { codes(in: $0["code"]) == ["80649-7"] })
        XCTAssertEqual(score["valueInteger"], .int(2))
        XCTAssertEqual(score["status"], .string("final"))
        guard case let .array(components)? = score["component"], case let .dictionary(category)? = components.first,
              case let .dictionary(value)? = category["valueCodeableConcept"] else {
            return XCTFail("risk category component missing")
        }
        XCTAssertEqual(codes(in: category["code"]), ["80650-5"])
        XCTAssertEqual(value["text"], .string(FHIRBundleBuilder.mchatCategoryText(for: .low)))
        let response = try XCTUnwrap(resources(bundle, type: "QuestionnaireResponse").first)
        guard case let .array(derived)? = score["derivedFrom"] else { return XCTFail("derivedFrom") }
        XCTAssertEqual(derived.compactMap { reference($0) }, [fullUrl(of: response, in: bundle)].compactMap { $0 })
    }

    func testNoMCHATMeansNoQuestionnaireResources() {
        let bundle = FHIRBundleBuilder.bundle(
            for: Self.goldenRecord(), mchat: nil, extraResources: [], subject: Self.goldenSubject
        )
        XCTAssertTrue(resources(bundle, type: "QuestionnaireResponse").isEmpty)
        XCTAssertTrue(resources(bundle, type: "Observation").allSatisfy { !codes(in: $0["code"]).contains("80649-7") })
    }

    func testLOINCLookupIsNonOptional() {
        let code: String = StandardCodingSystems.LOINC.loincCode(forMCHATQuestionId: 1)
        XCTAssertEqual(code, "80629-9")
        XCTAssertEqual(StandardCodingSystems.LOINC.loincCode(forMCHATQuestionId: 20), "80648-9")
    }

    func testStoredAnswersAreRecoveredForAResult() {
        let (result, answers) = Self.goldenMCHAT()
        var dict: [String: AnyCodableValue] = ["totalFailed": .int(2)]
        for (id, answer) in answers { dict["q_\(id)"] = .bool(answer) }
        let step = ORKStepResult(
            identifier: "mchat_survey", stepType: "ORKQuestionStep",
            startDate: result.completionDate, endDate: result.completionDate, results: dict
        )
        let task = ORKTaskResult(
            participantId: "P", startDate: result.completionDate, endDate: result.completionDate, stepResults: [step]
        )
        XCTAssertEqual(FHIRBundleBuilder.mchatAnswers(for: result, in: [task]), answers)
        XCTAssertEqual(FHIRBundleBuilder.mchatAnswers(for: result, in: []), [:])
    }

    // MARK: - Extra resources (WP-3.2 hand-off)

    func testExtraResourcesAreWrappedReferencedAndProvenanced() throws {
        let subject = Self.goldenSubject
        let medication: [String: AnyCodableValue] = [
            "resourceType": .string("MedicationStatement"),
            "id": .string("med-1"),
            "status": .string("active"),
            "medicationCodeableConcept": .dictionary(["text": .string("Melatonin")]),
            "subject": .dictionary(["reference": .string(FHIRBundleBuilder.patientReference(for: subject))])
        ]
        let legacyRef: [String: AnyCodableValue] = [
            "resourceType": .string("Observation"),
            "status": .string("final"),
            "code": .dictionary(["text": .string("Diary entry")]),
            "effectiveDateTime": .string("2026-10-01T08:00:00Z"),
            "subject": .dictionary(["reference": .string("Patient/child-GOLDEN01")])
        ]
        let bundle = FHIRBundleBuilder.bundle(
            for: Self.goldenRecord(), mchat: nil, extraResources: [medication, legacyRef], subject: subject
        )
        XCTAssertEqual(resources(bundle, type: "MedicationStatement").count, 1)
        let diary = try XCTUnwrap(resources(bundle, type: "Observation").first { $0["code"] == .dictionary(["text": .string("Diary entry")]) })
        XCTAssertEqual(reference(diary["subject"]), FHIRBundleBuilder.patientReference(for: subject), "Patient/<token> is rewritten")
        XCTAssertNotNil(diary["id"])
        XCTAssertEqual(resources(bundle, type: "Provenance").count, 3, "gaze + medication + diary")
        XCTAssertEqual(FHIRStructuralValidator.validate(bundle), [])
    }

    // MARK: - FHIR-7 structural validation

    func testGoldenBundleValidatesStructurally() {
        XCTAssertEqual(FHIRStructuralValidator.validate(goldenBundle()), [])
    }

    func testValidatorCatchesDanglingReference() {
        var bundle = goldenBundle()
        guard case var .array(items)? = bundle["entry"], case var .dictionary(entry)? = items.last,
              case var .dictionary(resource)? = entry["resource"] else { return XCTFail("shape") }
        resource["subject"] = .dictionary(["reference": .string("urn:uuid:00000000-0000-0000-0000-000000000000")])
        entry["resource"] = .dictionary(resource)
        items[items.count - 1] = .dictionary(entry)
        bundle["entry"] = .array(items)
        XCTAssertFalse(FHIRStructuralValidator.validate(bundle).isEmpty)
    }

    func testValidatorCatchesMissingRequiredElement() {
        var bundle = goldenBundle()
        guard case var .array(items)? = bundle["entry"] else { return XCTFail("shape") }
        for (index, item) in items.enumerated() {
            guard case var .dictionary(entry) = item, case var .dictionary(resource)? = entry["resource"],
                  resource["resourceType"] == .string("Observation") else { continue }
            resource["status"] = nil
            entry["resource"] = .dictionary(resource)
            items[index] = .dictionary(entry)
            break
        }
        bundle["entry"] = .array(items)
        XCTAssertFalse(FHIRStructuralValidator.validate(bundle).isEmpty)
    }

    // MARK: - JSON export

    func testJSONDataIsSortedAndRoundTrips() throws {
        let data = try FHIRBundleBuilder.jsonData(goldenBundle())
        let decoded = try JSONDecoder().decode([String: AnyCodableValue].self, from: data)
        XCTAssertEqual(decoded["resourceType"], .string("Bundle"))
        XCTAssertFalse(String(data: data, encoding: .utf8)!.contains("\\/"), "slashes are not escaped")
    }
}

// MARK: - Fixture loading

enum FHIRFixtures {
    struct Missing: Error { let name: String }

    /// Loads from the test bundle (XcodeGen copies `Tests/Fixtures/*.json` as resources) and
    /// falls back to the source tree, which the simulator can read.
    static func url(_ name: String) throws -> URL {
        let base = (name as NSString).deletingPathExtension
        let ext = (name as NSString).pathExtension
        if let url = Bundle(for: Token.self).url(forResource: base, withExtension: ext) { return url }
        let source = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("Fixtures").appendingPathComponent(name)
        if FileManager.default.fileExists(atPath: source.path) { return source }
        throw Missing(name: name)
    }

    private final class Token {}
}

// MARK: - FHIR-7 minimal structural validator (test-only, ADR-0003 decision 8)

/// Structural FHIR R4 check without a schema dependency: `Bundle` envelope, `urn:uuid` fullUrls,
/// required elements per resource type, and every `reference` resolving to a `fullUrl` in the bundle.
enum FHIRStructuralValidator {
    static let requiredElements: [String: [String]] = [
        "Patient": ["id"],
        "RelatedPerson": ["patient"],
        "Device": [],
        "Observation": ["status", "code", "subject", "effectiveDateTime"],
        "QuestionnaireResponse": ["status", "questionnaire", "subject", "authored", "item"],
        "Provenance": ["target", "recorded", "agent"],
        "MedicationStatement": ["status", "medicationCodeableConcept", "subject"]
    ]

    static func validate(_ bundle: [String: AnyCodableValue]) -> [String] {
        var problems: [String] = []
        guard bundle["resourceType"] == .string("Bundle") else { return ["not a Bundle"] }
        guard bundle["type"] == .string("collection") else { problems.append("Bundle.type is not collection"); return problems }
        guard case let .array(items)? = bundle["entry"] else { return ["Bundle.entry missing"] }

        var fullUrls = Set<String>()
        var resources: [(String, [String: AnyCodableValue])] = []
        for (index, item) in items.enumerated() {
            guard case let .dictionary(entry) = item else { problems.append("entry[\(index)] not an object"); continue }
            guard case let .string(url)? = entry["fullUrl"], url.hasPrefix("urn:uuid:") else {
                problems.append("entry[\(index)].fullUrl missing or not urn:uuid"); continue
            }
            if !fullUrls.insert(url).inserted { problems.append("duplicate fullUrl \(url)") }
            guard case let .dictionary(resource)? = entry["resource"],
                  case let .string(type)? = resource["resourceType"] else {
                problems.append("entry[\(index)].resource has no resourceType"); continue
            }
            guard let required = requiredElements[type] else {
                problems.append("entry[\(index)] unexpected resourceType \(type)"); continue
            }
            for element in required where resource[element] == nil || resource[element] == .null {
                problems.append("\(type) (\(url)) missing \(element)")
            }
            if type == "Provenance", case let .array(agents)? = resource["agent"] {
                for agent in agents {
                    guard case let .dictionary(a) = agent, a["who"] != nil else {
                        problems.append("Provenance (\(url)) agent without who"); continue
                    }
                }
            }
            resources.append((url, resource))
        }
        for (url, resource) in resources {
            for ref in references(in: .dictionary(resource)) where !fullUrls.contains(ref) {
                problems.append("unresolved reference \(ref) in \(url)")
            }
        }
        return problems
    }

    private static func references(in value: AnyCodableValue) -> [String] {
        switch value {
        case let .dictionary(dict):
            var out: [String] = []
            for (key, child) in dict {
                if key == "reference", case let .string(ref) = child { out.append(ref) } else { out += references(in: child) }
            }
            return out
        case let .array(items):
            return items.flatMap { references(in: $0) }
        default:
            return []
        }
    }
}
