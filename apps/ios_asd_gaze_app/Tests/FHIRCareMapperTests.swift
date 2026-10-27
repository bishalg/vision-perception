@testable import AsdGazeCollector
import XCTest

/// FHIR-5 (ADR-0003 decision 6): mood diary entries, developmental milestones and
/// caregiver-logged medications map to FHIR R4 `Observation` / `MedicationStatement`
/// resources with real codings (HL7 terminology plus local CodeSystems), a
/// pseudonymous subject and stable ids.
final class FHIRCareMapperTests: XCTestCase {

    private let subject = "Patient/child-1a2b3c4d"
    private let observationCategorySystem = "http://terminology.hl7.org/CodeSystem/observation-category"

    // MARK: - Fixtures

    private func diaryEntry(
        id: String = "6F9619FF-8B86-D011-B42D-00C04FC964FF",
        loggedAt: Date = Date(timeIntervalSince1970: 1_790_000_000),
        emotion: PediatricEmotion = .joy,
        recognition: SocialRecognition? = .respondedToName,
        intensity: Double = 0.7,
        duration: Double = 12,
        tags: [MoodContextTag] = [.postSchool, .loudEnvironment],
        notes: String = "Laughed during the bubble game.",
        source: MoodDiarySource = .manual
    ) -> MoodDiaryEntry {
        MoodDiaryEntry(
            id: UUID(uuidString: id)!,
            loggedAt: loggedAt,
            emotion: emotion,
            recognition: recognition,
            intensity: intensity,
            durationMinutes: duration,
            tags: tags,
            notes: notes,
            source: source
        )
    }

    private func milestone(completed: Bool) -> Milestone {
        Milestone(
            id: "ms_12_social",
            ageMonths: 12,
            title: "12-month Social",
            dueDateString: "CDC / AAP 12-month well-child window",
            domain: .socialEmotional,
            items: [
                MilestoneChecklistItem(
                    id: "item-wave",
                    title: "Waves bye-bye",
                    whatToLookFor: "look",
                    howToPrompt: "prompt",
                    whenToConsult: "consult text must not be exported",
                    isCompleted: completed
                )
            ],
            isCompleted: completed
        )
    }

    private func medication() -> MedicationItem {
        MedicationItem(
            id: "med_vitamin_d",
            name: "Vitamin D3 Drops",
            dose: "400 IU",
            instructions: "Give with morning meal.",
            schedule: ["08:00", "20:00"],
            lastTakenAt: Date(timeIntervalSince1970: 1_790_000_000),
            notes: "Started after the paediatrician visit."
        )
    }

    // MARK: - Helpers

    private func plain(_ resource: [String: AnyCodableValue]) -> [String: Any] {
        resource.mapValues { $0.anyValue as Any }
    }

    private func dict(_ value: Any?) -> [String: Any] {
        value as? [String: Any] ?? [:]
    }

    private func list(_ value: Any?) -> [[String: Any]] {
        value as? [[String: Any]] ?? []
    }

    private func codings(_ concept: Any?) -> [[String: Any]] {
        list(dict(concept)["coding"])
    }

    private func json(_ resources: [[String: AnyCodableValue]]) throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(resources)
        return String(decoding: data, as: UTF8.self)
    }

    private func component(_ observation: [String: Any], code: String) -> [String: Any]? {
        list(observation["component"]).first { comp in
            codings(comp["code"]).contains { ($0["code"] as? String) == code }
        }
    }

    private func components(_ observation: [String: Any], code: String) -> [[String: Any]] {
        list(observation["component"]).filter { comp in
            codings(comp["code"]).contains { ($0["code"] as? String) == code }
        }
    }

    private func assertFHIRId(_ id: Any?, file: StaticString = #filePath, line: UInt = #line) {
        guard let id = id as? String else {
            return XCTFail("missing id", file: file, line: line)
        }
        XCTAssertNotNil(
            id.range(of: "^[A-Za-z0-9\\-.]{1,64}$", options: .regularExpression),
            "id \(id) is not a valid FHIR id",
            file: file,
            line: line
        )
    }

    // MARK: - Empty inputs

    func testEmptyInputsGiveNoResources() {
        let resources = FHIRCareMapper.resources(diary: [], milestones: [], medications: [], subjectRef: subject)
        XCTAssertTrue(resources.isEmpty)
    }

    // MARK: - Subject

    func testSubjectIsThePseudonymousReferenceOnly() {
        let resources = FHIRCareMapper.resources(
            diary: [diaryEntry()],
            milestones: [milestone(completed: true)],
            medications: [medication()],
            subjectRef: subject
        )
        XCTAssertEqual(resources.count, 3)
        for resource in resources.map(plain) {
            let subjectValue = dict(resource["subject"])
            XCTAssertEqual(subjectValue["reference"] as? String, subject)
            XCTAssertEqual(subjectValue.count, 1, "subject must carry no display/name, only the token reference")
        }
    }

    func testUrnUuidSubjectReferenceIsAccepted() {
        let urn = "urn:uuid:0b1c2d3e-4f50-4a6b-8c7d-9e0f1a2b3c4d"
        let resources = FHIRCareMapper.resources(diary: [diaryEntry()], milestones: [], medications: [], subjectRef: urn)
        XCTAssertEqual(dict(plain(resources[0])["subject"])["reference"] as? String, urn)
    }

    /// A subject that is not a `Patient/<token>` or `urn:uuid:` reference (for example a
    /// child's name typed into the participant field) fails closed: nothing is exported.
    func testNonTokenSubjectReferenceFailsClosed() {
        for bad in ["", "Aarav Sharma", "Patient/Aarav Sharma", "Patient/", "Aarav"] {
            let resources = FHIRCareMapper.resources(
                diary: [diaryEntry()],
                milestones: [milestone(completed: true)],
                medications: [medication()],
                subjectRef: bad
            )
            XCTAssertTrue(resources.isEmpty, "subjectRef \(bad.debugDescription) must be rejected")
        }
    }

    // MARK: - Mood diary

    func testMoodDiaryEntryMapsToSurveyObservation() {
        let entry = diaryEntry()
        let resources = FHIRCareMapper.resources(diary: [entry], milestones: [], medications: [], subjectRef: subject)
        XCTAssertEqual(resources.count, 1)
        let obs = plain(resources[0])

        XCTAssertEqual(obs["resourceType"] as? String, "Observation")
        XCTAssertEqual(obs["status"] as? String, "final")
        assertFHIRId(obs["id"])

        let category = list(obs["category"]).flatMap { codings($0) }
        XCTAssertTrue(category.contains {
            ($0["system"] as? String) == observationCategorySystem && ($0["code"] as? String) == "survey"
        })

        let code = codings(obs["code"])
        XCTAssertEqual(code.first?["system"] as? String, FHIRCareCodings.careObservationSystem)
        XCTAssertEqual(code.first?["code"] as? String, "mood-diary-entry")

        let mood = codings(obs["valueCodeableConcept"])
        XCTAssertEqual(mood.first?["system"] as? String, FHIRCareCodings.moodSystem)
        XCTAssertEqual(mood.first?["code"] as? String, "joy")
        XCTAssertEqual(mood.first?["display"] as? String, "Joy")

        XCTAssertEqual(obs["effectiveDateTime"] as? String, "2026-09-21T14:13:20Z")

        let notes = list(obs["note"])
        XCTAssertEqual(notes.first?["text"] as? String, "Laughed during the bubble game.")

        let intensity = component(obs, code: "mood-intensity")
        XCTAssertNotNil(intensity)
        let quantity = dict(intensity?["valueQuantity"])
        XCTAssertEqual(quantity["value"] as? Double ?? -1, 0.7, accuracy: 1e-9)
        XCTAssertEqual(quantity["system"] as? String, "http://unitsofmeasure.org")
        XCTAssertEqual(quantity["code"] as? String, "1")

        let duration = dict(component(obs, code: "mood-duration")?["valueQuantity"])
        XCTAssertEqual(duration["value"] as? Double ?? -1, 12, accuracy: 1e-9)
        XCTAssertEqual(duration["code"] as? String, "min")

        let recognition = codings(component(obs, code: "social-recognition")?["valueCodeableConcept"])
        XCTAssertEqual(recognition.first?["system"] as? String, FHIRCareCodings.socialRecognitionSystem)
        XCTAssertEqual(recognition.first?["code"] as? String, "responded-to-name")

        let contexts = components(obs, code: "mood-context").flatMap { codings($0["valueCodeableConcept"]) }
        XCTAssertEqual(contexts.compactMap { $0["code"] as? String }, ["post-school", "loud-environment"])
        XCTAssertTrue(contexts.allSatisfy { ($0["system"] as? String) == FHIRCareCodings.moodContextSystem })
    }

    func testEmptyNotesAndMissingRecognitionAreAbsentNotPlaceholders() {
        let entry = diaryEntry(recognition: nil, tags: [], notes: "   ")
        let obs = plain(FHIRCareMapper.resources(diary: [entry], milestones: [], medications: [], subjectRef: subject)[0])
        XCTAssertNil(obs["note"])
        XCTAssertNil(component(obs, code: "social-recognition"))
        XCTAssertNil(component(obs, code: "mood-context"))
        XCTAssertNil(obs["dataAbsentReason"])
    }

    func testDeviceSuggestedEntryIsMarkedByMethod() {
        let suggested = diaryEntry(source: .arkitSuggested)
        let obs = plain(FHIRCareMapper.resources(diary: [suggested], milestones: [], medications: [], subjectRef: subject)[0])
        let method = codings(obs["method"])
        XCTAssertEqual(method.first?["system"] as? String, FHIRCareCodings.diaryMethodSystem)
        XCTAssertEqual(method.first?["code"] as? String, "device-suggested")
    }

    func testEveryEmotionHasADistinctCode() {
        let codes = Set(PediatricEmotion.allCases.map { FHIRCareCodings.moodCoding($0).code })
        XCTAssertEqual(codes.count, PediatricEmotion.allCases.count)
        let contexts = Set(MoodContextTag.allCases.map { FHIRCareCodings.moodContextCoding($0).code })
        XCTAssertEqual(contexts.count, MoodContextTag.allCases.count)
        for code in contexts {
            XCTAssertNotNil(code.range(of: "^[a-z0-9-]+$", options: .regularExpression), "code \(code) not kebab-case")
        }
    }

    // MARK: - Milestones

    func testAchievedMilestoneMapsToSurveyObservation() {
        let obs = plain(FHIRCareMapper.resources(
            diary: [], milestones: [milestone(completed: true)], medications: [], subjectRef: subject
        )[0])

        XCTAssertEqual(obs["resourceType"] as? String, "Observation")
        XCTAssertEqual(obs["status"] as? String, "final")
        assertFHIRId(obs["id"])
        let category = list(obs["category"]).flatMap { codings($0) }
        XCTAssertTrue(category.contains { ($0["code"] as? String) == "survey" })

        let code = codings(obs["code"])
        XCTAssertEqual(code.first?["system"] as? String, FHIRCareCodings.careObservationSystem)
        XCTAssertEqual(code.first?["code"] as? String, "developmental-milestone")
        XCTAssertEqual(dict(obs["code"])["text"] as? String, "12-month Social")

        let value = codings(obs["valueCodeableConcept"])
        XCTAssertEqual(value.first?["system"] as? String, FHIRCareCodings.milestoneStatusSystem)
        XCTAssertEqual(value.first?["code"] as? String, "achieved")

        let domain = codings(component(obs, code: "developmental-domain")?["valueCodeableConcept"])
        XCTAssertEqual(domain.first?["system"] as? String, FHIRCareCodings.developmentalDomainSystem)
        XCTAssertEqual(domain.first?["code"] as? String, "social-emotional")

        let age = dict(component(obs, code: "milestone-age")?["valueQuantity"])
        XCTAssertEqual(age["value"] as? Int, 12)
        XCTAssertEqual(age["code"] as? String, "mo")

        let item = component(obs, code: "milestone-checklist-item")
        XCTAssertEqual(dict(item?["code"])["text"] as? String, "Waves bye-bye")
        XCTAssertEqual(codings(item?["valueCodeableConcept"]).first?["code"] as? String, "achieved")
    }

    func testNotYetAchievedMilestone() throws {
        let resources = FHIRCareMapper.resources(
            diary: [], milestones: [milestone(completed: false)], medications: [], subjectRef: subject
        )
        let obs = plain(resources[0])
        XCTAssertEqual(codings(obs["valueCodeableConcept"]).first?["code"] as? String, "not-yet")
        XCTAssertFalse(try json(resources).contains("consult text must not be exported"))
    }

    // MARK: - Medications

    func testMedicationMapsToMedicationStatement() {
        let statement = plain(FHIRCareMapper.resources(
            diary: [], milestones: [], medications: [medication()], subjectRef: subject
        )[0])

        XCTAssertEqual(statement["resourceType"] as? String, "MedicationStatement")
        XCTAssertEqual(statement["status"] as? String, "active")
        assertFHIRId(statement["id"])
        XCTAssertEqual(dict(statement["medicationCodeableConcept"])["text"] as? String, "Vitamin D3 Drops")

        let category = codings(statement["category"])
        XCTAssertEqual(
            category.first?["system"] as? String,
            "http://terminology.hl7.org/CodeSystem/medication-statement-category"
        )
        XCTAssertEqual(category.first?["code"] as? String, "patientspecified")

        let dosage = list(statement["dosage"]).first ?? [:]
        let dosageText = dosage["text"] as? String ?? ""
        XCTAssertTrue(dosageText.contains("400 IU"))
        XCTAssertTrue(dosageText.contains("Give with morning meal."))
        let timeOfDay = dict(dict(dosage["timing"])["repeat"])["timeOfDay"] as? [String]
        XCTAssertEqual(timeOfDay, ["08:00:00", "20:00:00"])

        let period = dict(statement["effectivePeriod"])
        XCTAssertEqual(period["start"] as? String, "2026-09-21T14:13:20Z")

        XCTAssertEqual(list(statement["note"]).first?["text"] as? String, "Started after the paediatrician visit.")
    }

    func testMedicationWithoutDatesOmitsEffectivePeriodAndInvalidTimes() {
        let med = MedicationItem(id: "med x", name: "Omega-3", dose: "500 mg", instructions: "", schedule: ["8am"])
        let statement = plain(FHIRCareMapper.resources(diary: [], milestones: [], medications: [med], subjectRef: subject)[0])
        XCTAssertNil(statement["effectivePeriod"])
        XCTAssertNil(statement["note"])
        assertFHIRId(statement["id"])
        let dosage = list(statement["dosage"]).first ?? [:]
        XCTAssertEqual(dosage["text"] as? String, "500 mg")
        XCTAssertNil(dosage["timing"], "an unparseable dose time must not become a FHIR time")
    }

    // MARK: - Stable ids

    func testIdsAreStableAcrossCallsAndUniquePerResource() throws {
        let diary = [
            diaryEntry(),
            diaryEntry(id: "11111111-2222-3333-4444-555555555555", emotion: .meltdown)
        ]
        let milestones = ClinicalMilestoneCatalog.all(completedAges: [9, 12])
        let meds = [medication(), MedicationItem(id: "med_omega_3", name: "Omega-3", dose: "500 mg", instructions: "", schedule: [])]

        let first = FHIRCareMapper.resources(diary: diary, milestones: milestones, medications: meds, subjectRef: subject)
        let second = FHIRCareMapper.resources(diary: diary, milestones: milestones, medications: meds, subjectRef: subject)

        XCTAssertEqual(first.count, diary.count + milestones.count + meds.count)
        XCTAssertEqual(try json(first), try json(second), "the same input must give byte-identical output")

        let ids = first.map { plain($0)["id"] as? String ?? "" }
        XCTAssertEqual(Set(ids).count, ids.count, "ids must be unique")
        ids.forEach { assertFHIRId($0) }

        XCTAssertTrue(ids.contains("mood-6f9619ff-8b86-d011-b42d-00c04fc964ff"))
        XCTAssertTrue(ids.contains("milestone-ms-12-social"))
        XCTAssertTrue(ids.contains("medication-med-vitamin-d"))
    }

    func testOverlongModelIdIsHashedToAValidStableFHIRId() {
        let longId = String(repeating: "a", count: 100)
        let med = MedicationItem(id: longId, name: "Item", dose: "1", instructions: "", schedule: [])
        let a = plain(FHIRCareMapper.resources(diary: [], milestones: [], medications: [med], subjectRef: subject)[0])["id"]
        let b = plain(FHIRCareMapper.resources(diary: [], milestones: [], medications: [med], subjectRef: subject)[0])["id"]
        assertFHIRId(a)
        XCTAssertEqual(a as? String, b as? String)
    }

    // MARK: - Wording and coding hygiene

    /// No diagnostic or risk wording, and no SNOMED finding codes (ADR-0003 decision 3,
    /// ADR-0004): care records are caregiver observations, not findings.
    func testNoDiagnosticWordingOrFindingCodes() throws {
        let diary = PediatricEmotion.allCases.enumerated().map { index, emotion in
            diaryEntry(
                id: String(format: "00000000-0000-0000-0000-%012d", index),
                emotion: emotion,
                recognition: SocialRecognition.allCases[index % SocialRecognition.allCases.count],
                tags: MoodContextTag.allCases,
                notes: ""
            )
        }
        let resources = FHIRCareMapper.resources(
            diary: diary,
            milestones: ClinicalMilestoneCatalog.all(),
            medications: [medication()],
            subjectRef: subject
        )
        let text = try json(resources).lowercased()
        let denied = [
            "autis", "asd", "diagnos", "disorder", "risk", "impair", "delay", "abnormal",
            "symptom", "patholog", "screening", "clinical", "finding", "snomed.info", "biomarker"
        ]
        for word in denied {
            XCTAssertFalse(text.contains(word), "FHIR care export contains forbidden wording \"\(word)\"")
        }
    }

    func testLocalCodeSystemsLiveUnderTheNamespaceBase() {
        let systems = [
            FHIRCareCodings.careObservationSystem,
            FHIRCareCodings.moodSystem,
            FHIRCareCodings.moodContextSystem,
            FHIRCareCodings.socialRecognitionSystem,
            FHIRCareCodings.diaryMethodSystem,
            FHIRCareCodings.milestoneStatusSystem,
            FHIRCareCodings.developmentalDomainSystem
        ]
        for system in systems {
            XCTAssertTrue(system.hasPrefix(FHIRCareCodings.namespaceBase + "/CodeSystem/"), system)
        }
        XCTAssertEqual(Set(systems).count, systems.count)
    }
}
