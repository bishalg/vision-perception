import CryptoKit
import CurvAsdGazeKit
import Foundation
import os

// MARK: - FHIR R4 Bundle export (FHIR-1..4, ADR-0003)

/// Who the bundle is about. The child is identified only by a pseudonymous token (CNS-7):
/// `Patient` carries no name and no birth date.
public struct FHIRSubject: Sendable, Equatable {
    /// `Patient.id`: `ResearchStudyEnrollment.participantId` when enrolled, else `child-<session prefix>`.
    public let participantToken: String
    /// Age in months, exported as an extension instead of a birth date.
    public let ageMonths: Int?
    /// Caregiver name for `RelatedPerson.name`, only when the caregiver typed one and chose to include it.
    public let caregiverDisplayName: String?

    public init(participantToken: String, ageMonths: Int?, caregiverDisplayName: String? = nil) {
        self.participantToken = participantToken
        self.ageMonths = ageMonths
        let trimmed = caregiverDisplayName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        self.caregiverDisplayName = trimmed.isEmpty ? nil : trimmed
    }

    /// The subject of a stored record: its participant token, which is never a name (CNS-7).
    public init(record: GazeSessionRecord, ageMonths: Int?) {
        self.init(participantToken: record.participantId, ageMonths: ageMonths)
    }
}

/// The recording app, exported as the `Device` every `Provenance` names.
public struct FHIRDeviceInfo: Sendable, Equatable {
    public static let defaultDisplayName = "EarlyGaze Observation Diary"

    public let displayName: String
    /// `MARKETING_VERSION` (`CFBundleShortVersionString`).
    public let version: String

    public init(displayName: String, version: String) {
        self.displayName = displayName
        self.version = version
    }

    /// The running app's display name and marketing version.
    public static var current: FHIRDeviceInfo {
        let info = Bundle.main.infoDictionary ?? [:]
        let name = (info["CFBundleDisplayName"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? defaultDisplayName
        let version = info["CFBundleShortVersionString"] as? String ?? "0"
        return FHIRDeviceInfo(displayName: name, version: version)
    }
}

/// Builds the export unit: a FHIR R4 `Bundle` of `type: collection` (ADR-0003 decision 1).
///
/// Entries, in order: `Patient`, `RelatedPerson`, `Device`, the gaze `Observation` (omitted when
/// the record is refused: simulated or rejected, SIM-5/FHIR-2) and its `Provenance`, the M-CHAT-R/F
/// `QuestionnaireResponse` and score `Observation` (each with a `Provenance`), then `extraResources`
/// (each Observation-like one with a `Provenance`). Every `fullUrl` is a name-based (v5)
/// `urn:uuid`, so the same input always yields the same bundle; all references are resolved
/// inside the bundle.
public enum FHIRBundleBuilder {
    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "FHIRBundleBuilder")

    /// Resource types that get a `Provenance` (FHIR-1: one per observation resource).
    static let provenancedTypes: Set<String> = ["Observation", "QuestionnaireResponse", "MedicationStatement"]

    // MARK: Public API (WP-3.1 contract)

    public static func bundle(
        for record: GazeSessionRecord,
        mchat: (MCHATScoreResult, [Int: Bool])? = nil,
        extraResources: [[String: AnyCodableValue]] = [],
        subject: FHIRSubject
    ) -> [String: AnyCodableValue] {
        bundle(for: record, mchat: mchat, extraResources: extraResources, subject: subject, device: .current)
    }

    /// `urn:uuid` reference to the bundle's `Patient`, for building `extraResources` (WP-3.2).
    public static func patientReference(for subject: FHIRSubject) -> String {
        urn(uuid("Patient/\(subject.participantToken)"))
    }

    /// Why the record's gaze Observation cannot be exported (shown next to a disabled export
    /// button, acceptance 2), or `nil` when it can.
    public static func exportRefusalReason(for record: GazeSessionRecord) -> String? {
        FHIRInteroperabilityMapper.refusalReason(for: record)
    }

    /// `Observation.status` for the record (FHIR-2); `nil` when refused.
    public static func observationStatus(for record: GazeSessionRecord) -> String? {
        FHIRInteroperabilityMapper.observationStatus(for: record)
    }

    /// Official M-CHAT-R/F category text used for LOINC `80650-5`.
    public static func mchatCategoryText(for level: MCHATRiskLevel) -> String {
        FHIRInteroperabilityMapper.mchatCategoryText(for: level)
    }

    /// Pretty-printed, key-sorted JSON without escaped slashes.
    public static func jsonData(_ resource: [String: AnyCodableValue]) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(resource)
    }

    /// The stored answers of an M-CHAT-R/F result: the `mchat_survey` step recorded by
    /// `ResearchKitStudyStore.recordSurveyResult` at the result's completion date. Empty when none.
    public static func mchatAnswers(for result: MCHATScoreResult, in taskResults: [ORKTaskResult]) -> [Int: Bool] {
        for task in taskResults.reversed() {
            for step in task.stepResults where step.identifier == "mchat_survey"
                && abs(step.startDate.timeIntervalSince(result.completionDate)) < 1 {
                var answers: [Int: Bool] = [:]
                for qId in 1...20 {
                    if case let .bool(answer)? = step.results["q_\(qId)"] { answers[qId] = answer }
                }
                return answers
            }
        }
        return [:]
    }

    // MARK: Builder

    static func bundle(
        for record: GazeSessionRecord,
        mchat: (MCHATScoreResult, [Int: Bool])?,
        extraResources: [[String: AnyCodableValue]],
        subject: FHIRSubject,
        device: FHIRDeviceInfo
    ) -> [String: AnyCodableValue] {
        let token = subject.participantToken
        let patientURL = patientReference(for: subject)
        let personId = uuid("RelatedPerson/\(token)")
        let deviceId = uuid("Device/\(device.displayName)/\(device.version)")
        let personURL = urn(personId)
        let deviceURL = urn(deviceId)

        var entries: [AnyCodableValue] = []
        func append(_ url: String, _ resource: [String: AnyCodableValue]) {
            entries.append(.dictionary(["fullUrl": .string(url), "resource": .dictionary(resource)]))
        }
        func appendProvenance(target: String, recorded: Date, gazeRecord: GazeSessionRecord? = nil) {
            let id = uuid("Provenance/\(target.dropFirst("urn:uuid:".count))")
            append(urn(id), provenance(
                id: id, target: target, recorded: recorded,
                deviceURL: deviceURL, personURL: personURL, gazeRecord: gazeRecord
            ))
        }

        append(patientURL, patient(subject))
        append(personURL, relatedPerson(id: personId, subject: subject, patientURL: patientURL))
        append(deviceURL, deviceResource(id: deviceId, device))

        let gazeId = uuid("Observation/gaze/\(record.sessionId)")
        if let observation = FHIRInteroperabilityMapper.makeGazeObservation(
            from: record, id: gazeId, subjectReference: patientURL, deviceReference: deviceURL
        ) {
            append(urn(gazeId), observation)
            appendProvenance(target: urn(gazeId), recorded: record.endedAt, gazeRecord: record)
        } else {
            logger.info("Bundle for \(token, privacy: .private) has no gaze Observation (refused)")
        }

        if let mchat {
            let (result, answers) = mchat
            let authored = FHIRInteroperabilityMapper.timestamp(result.completionDate)
            let responseId = uuid("QuestionnaireResponse/mchat/\(token)/\(authored)")
            let scoreId = uuid("Observation/mchat-score/\(token)/\(authored)")
            append(urn(responseId), FHIRInteroperabilityMapper.makeQuestionnaireResponse(
                from: result, answers: answers, id: responseId,
                subjectReference: patientURL, authorReference: personURL
            ))
            appendProvenance(target: urn(responseId), recorded: result.completionDate)
            append(urn(scoreId), FHIRInteroperabilityMapper.makeMCHATScoreObservation(
                from: result, id: scoreId, subjectReference: patientURL,
                questionnaireResponseReference: urn(responseId)
            ))
            appendProvenance(target: urn(scoreId), recorded: result.completionDate)
        }

        for (index, original) in extraResources.enumerated() {
            let type: String
            if case let .string(t)? = original["resourceType"] { type = t } else { type = "Resource" }
            let key: String
            if case let .string(id)? = original["id"] { key = "\(type)/extra/\(id)" } else {
                key = "\(type)/extra/\(record.sessionId)/\(index)"
            }
            let id = uuid(key)
            var resource = rewritingPatientReferences(in: original, token: token, patientURL: patientURL)
            if resource["id"] == nil { resource["id"] = .string(id) }
            append(urn(id), resource)
            if provenancedTypes.contains(type) {
                appendProvenance(target: urn(id), recorded: recordedDate(of: resource) ?? record.endedAt)
            }
        }

        return [
            "resourceType": .string("Bundle"),
            "id": .string(uuid("Bundle/\(token)/\(record.sessionId)")),
            "type": .string("collection"),
            "entry": .array(entries)
        ]
    }

    // MARK: Resources

    private static func patient(_ subject: FHIRSubject) -> [String: AnyCodableValue] {
        var patient: [String: AnyCodableValue] = [
            "resourceType": .string("Patient"),
            "id": .string(subject.participantToken),
            "identifier": .array([
                .dictionary([
                    "system": .string(FHIRNamespaces.participantTokenSystem),
                    "value": .string(subject.participantToken)
                ])
            ])
        ]
        if let months = subject.ageMonths {
            patient["extension"] = .array([
                .dictionary([
                    "url": .string(FHIRNamespaces.ageInMonthsExtension),
                    "valueQuantity": .dictionary([
                        "value": .int(months),
                        "unit": .string("mo"),
                        "system": .string(StandardCodingSystems.ucumURI),
                        "code": .string("mo")
                    ])
                ])
            ])
        }
        return patient
    }

    private static func relatedPerson(id: String, subject: FHIRSubject, patientURL: String) -> [String: AnyCodableValue] {
        var person: [String: AnyCodableValue] = [
            "resourceType": .string("RelatedPerson"),
            "id": .string(id),
            "patient": .dictionary(["reference": .string(patientURL)]),
            "relationship": .array([
                .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string(FHIRNamespaces.roleCodeSystem),
                            "code": .string("PRN"),
                            "display": .string("parent")
                        ])
                    ])
                ])
            ])
        ]
        if let name = subject.caregiverDisplayName {
            person["name"] = .array([.dictionary(["text": .string(name)])])
        }
        return person
    }

    private static func deviceResource(id: String, _ device: FHIRDeviceInfo) -> [String: AnyCodableValue] {
        [
            "resourceType": .string("Device"),
            "id": .string(id),
            "deviceName": .array([
                .dictionary(["name": .string(device.displayName), "type": .string("user-friendly-name")])
            ]),
            "version": .array([.dictionary(["value": .string(device.version)])]),
            "type": .dictionary(["text": .string("Mobile gaze engagement observation app")])
        ]
    }

    /// Agent = the app (`Device`) acting for the caregiver; activity CREATE. For a gaze session it
    /// also names the capture source (SIM-4) and the on-device vault file it came from.
    private static func provenance(
        id: String,
        target: String,
        recorded: Date,
        deviceURL: String,
        personURL: String,
        gazeRecord: GazeSessionRecord?
    ) -> [String: AnyCodableValue] {
        var provenance: [String: AnyCodableValue] = [
            "resourceType": .string("Provenance"),
            "id": .string(id),
            "target": .array([.dictionary(["reference": .string(target)])]),
            "recorded": .string(FHIRInteroperabilityMapper.timestamp(recorded)),
            "activity": .dictionary([
                "coding": .array([
                    .dictionary([
                        "system": .string(FHIRNamespaces.dataOperationSystem),
                        "code": .string("CREATE"),
                        "display": .string("create")
                    ])
                ])
            ]),
            "agent": .array([
                .dictionary([
                    "type": .dictionary([
                        "coding": .array([
                            .dictionary([
                                "system": .string(FHIRNamespaces.provenanceParticipantTypeSystem),
                                "code": .string("device"),
                                "display": .string("Device")
                            ])
                        ])
                    ]),
                    "who": .dictionary(["reference": .string(deviceURL)]),
                    "onBehalfOf": .dictionary(["reference": .string(personURL)])
                ])
            ])
        ]
        guard let record = gazeRecord else { return provenance }

        var extensions: [AnyCodableValue] = [
            .dictionary([
                "url": .string(FHIRNamespaces.captureSourceExtension),
                "valueCoding": .dictionary([
                    "system": .string(FHIRNamespaces.captureSourceCodeSystem),
                    "code": .string(record.captureSource.rawValue),
                    "display": .string(captureSourceDisplay(record.captureSource))
                ])
            ])
        ]
        if record.legacyProvenance == true {
            extensions.append(.dictionary([
                "url": .string(FHIRNamespaces.captureSourceInferredExtension),
                "valueBoolean": .bool(true)
            ]))
        }
        provenance["extension"] = .array(extensions)
        // The vault file is `session_<sessionId>_<start date><suffix>` (SessionVaultManager); the
        // session-id prefix identifies it without depending on the device clock at write time.
        provenance["entity"] = .array([
            .dictionary([
                "role": .string("source"),
                "what": .dictionary([
                    "identifier": .dictionary([
                        "system": .string(FHIRNamespaces.sessionVaultSystem),
                        "value": .string("session_\(record.sessionId)")
                    ]),
                    "display": .string("On-device encrypted session vault file")
                ])
            ])
        ])
        return provenance
    }

    private static func captureSourceDisplay(_ source: CaptureSource) -> String {
        switch source {
        case .trueDepth: return "Apple TrueDepth front camera (live capture)"
        case .simulated: return "Simulated data"
        }
    }

    // MARK: Helpers

    /// Rewrites `Patient/<token>` references (as emitted by standalone mappers) to the bundle's
    /// `urn:uuid` so they resolve inside the bundle.
    private static func rewritingPatientReferences(
        in resource: [String: AnyCodableValue],
        token: String,
        patientURL: String
    ) -> [String: AnyCodableValue] {
        func rewrite(_ value: AnyCodableValue, key: String?) -> AnyCodableValue {
            switch value {
            case let .string(ref) where key == "reference" && ref == "Patient/\(token)":
                return .string(patientURL)
            case let .dictionary(dict):
                return .dictionary(Dictionary(uniqueKeysWithValues: dict.map { ($0.key, rewrite($0.value, key: $0.key)) }))
            case let .array(items):
                return .array(items.map { rewrite($0, key: nil) })
            default:
                return value
            }
        }
        guard case let .dictionary(out) = rewrite(.dictionary(resource), key: nil) else { return resource }
        return out
    }

    private static func recordedDate(of resource: [String: AnyCodableValue]) -> Date? {
        let formatter = ISO8601DateFormatter()
        for key in ["effectiveDateTime", "authored", "dateAsserted"] {
            if case let .string(value)? = resource[key], let date = formatter.date(from: value) { return date }
        }
        if case let .dictionary(period)? = resource["effectivePeriod"], case let .string(start)? = period["start"] {
            return formatter.date(from: start)
        }
        return nil
    }

    static func urn(_ id: String) -> String { "urn:uuid:\(id)" }

    /// RFC 4122 version 5 (SHA-1, name-based) UUID in the URL namespace for
    /// `<FHIRNamespaces.local>/<name>`, lower-cased.
    static func uuid(_ name: String) -> String {
        // 6ba7b811-9dad-11d1-80b4-00c04fd430c8 (RFC 4122 NameSpace_URL)
        let namespace: [UInt8] = [0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1,
                                  0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8]
        var data = Data(namespace)
        data.append(Data("\(FHIRNamespaces.local)/\(name)".utf8))
        var bytes = Array(Insecure.SHA1.hash(data: data).prefix(16))
        bytes[6] = (bytes[6] & 0x0F) | 0x50
        bytes[8] = (bytes[8] & 0x3F) | 0x80
        let value = UUID(uuid: (bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]))
        return value.uuidString.lowercased()
    }
}
