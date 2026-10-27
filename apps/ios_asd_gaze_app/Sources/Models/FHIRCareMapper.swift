import CryptoKit
import Foundation

/// Maps caregiver care records to FHIR R4 resources (FHIR-5, ADR-0003 decision 6).
///
/// - `MoodDiaryEntry` -> `Observation` (category `survey`, code `mood-diary-entry`,
///   mood as `valueCodeableConcept`, free text as `note`, intensity, duration,
///   recognition and context as components).
/// - `Milestone` -> `Observation` (category `survey`, code `developmental-milestone`,
///   value achieved / not yet; domain, age window and checklist items as components).
/// - `MedicationItem` -> `MedicationStatement` (`medicationCodeableConcept.text`,
///   `dosage.text`, `effectivePeriod`).
///
/// The subject is only the reference passed in, which must be a pseudonymous
/// `Patient/<token>` or `urn:uuid:` reference (CNS-7, ADR-0002 decision 4); any other
/// value fails closed and nothing is exported. Resource ids derive from model ids, so
/// the same input always produces the same output. Absent values are absent elements,
/// never placeholders (PLC-3).
public enum FHIRCareMapper {

    public static func resources(
        diary: [MoodDiaryEntry],
        milestones: [Milestone],
        medications: [MedicationItem],
        subjectRef: String
    ) -> [[String: AnyCodableValue]] {
        guard isPseudonymousReference(subjectRef) else { return [] }
        let subject: AnyCodableValue = .dictionary(["reference": .string(subjectRef)])

        let sortedDiary = diary.sorted {
            ($0.loggedAt, $0.id.uuidString) < ($1.loggedAt, $1.id.uuidString)
        }
        let sortedMilestones = milestones.sorted { ($0.ageMonths, $0.id) < ($1.ageMonths, $1.id) }
        let sortedMedications = medications.sorted { $0.id < $1.id }

        return sortedDiary.map { observation(for: $0, subject: subject) }
            + sortedMilestones.map { observation(for: $0, subject: subject) }
            + sortedMedications.map { statement(for: $0, subject: subject) }
    }

    // MARK: - Subject

    private static let fhirIdPattern = "[A-Za-z0-9\\-.]{1,64}"

    static func isPseudonymousReference(_ reference: String) -> Bool {
        let patientRef = "^Patient/\(fhirIdPattern)$"
        let urnRef = "^urn:uuid:[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}$"
        return reference.range(of: patientRef, options: .regularExpression) != nil
            || reference.range(of: urnRef, options: .regularExpression) != nil
    }

    // MARK: - Mood diary

    private static func observation(for entry: MoodDiaryEntry, subject: AnyCodableValue) -> [String: AnyCodableValue] {
        var components: [AnyCodableValue] = [
            component(FHIRCareCodings.moodIntensity, valueQuantity: quantity(.double(entry.intensity), unit: "1")),
            component(FHIRCareCodings.moodDuration, valueQuantity: quantity(.double(entry.durationMinutes), unit: "min"))
        ]
        if let recognition = entry.recognition {
            components.append(component(
                FHIRCareCodings.socialRecognition,
                valueConcept: concept(FHIRCareCodings.socialRecognitionCoding(recognition))
            ))
        }
        for tag in entry.tags {
            components.append(component(
                FHIRCareCodings.moodContext,
                valueConcept: concept(FHIRCareCodings.moodContextCoding(tag))
            ))
        }

        var resource: [String: AnyCodableValue] = [
            "resourceType": .string("Observation"),
            "id": .string(fhirId(prefix: "mood", entry.id.uuidString.lowercased())),
            "status": .string("final"),
            "category": .array([concept(FHIRCareCodings.surveyCategory)]),
            "code": concept(FHIRCareCodings.moodDiaryEntry),
            "subject": subject,
            "effectiveDateTime": .string(dateTime(entry.loggedAt)),
            "valueCodeableConcept": concept(FHIRCareCodings.moodCoding(entry.emotion)),
            "method": concept(FHIRCareCodings.diaryMethodCoding(entry.source)),
            "component": .array(components)
        ]
        if let note = note(entry.notes) {
            resource["note"] = note
        }
        return resource
    }

    // MARK: - Milestones

    private static func observation(for milestone: Milestone, subject: AnyCodableValue) -> [String: AnyCodableValue] {
        var components: [AnyCodableValue] = [
            component(
                FHIRCareCodings.developmentalDomain,
                valueConcept: concept(FHIRCareCodings.developmentalDomainCoding(milestone.domain))
            ),
            component(FHIRCareCodings.milestoneAge, valueQuantity: quantity(.int(milestone.ageMonths), unit: "mo"))
        ]
        for item in milestone.items {
            components.append(.dictionary([
                "code": concept(FHIRCareCodings.milestoneChecklistItem, text: item.title),
                "valueCodeableConcept": concept(FHIRCareCodings.milestoneStatusCoding(achieved: item.isCompleted))
            ]))
        }

        return [
            "resourceType": .string("Observation"),
            "id": .string(fhirId(prefix: "milestone", milestone.id)),
            "status": .string("final"),
            "category": .array([concept(FHIRCareCodings.surveyCategory)]),
            "code": concept(FHIRCareCodings.developmentalMilestone, text: milestone.title),
            "subject": subject,
            "valueCodeableConcept": concept(FHIRCareCodings.milestoneStatusCoding(achieved: milestone.isCompleted)),
            "component": .array(components)
        ]
    }

    // MARK: - Medications

    private static func statement(for item: MedicationItem, subject: AnyCodableValue) -> [String: AnyCodableValue] {
        let dosageText = [item.dose, item.instructions]
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: "; ")
        var dosage: [String: AnyCodableValue] = [:]
        if !dosageText.isEmpty {
            dosage["text"] = .string(dosageText)
        }
        let times = item.schedule.compactMap(fhirTime)
        if !times.isEmpty {
            dosage["timing"] = .dictionary([
                "repeat": .dictionary(["timeOfDay": .array(times.map(AnyCodableValue.string))])
            ])
        }

        var resource: [String: AnyCodableValue] = [
            "resourceType": .string("MedicationStatement"),
            "id": .string(fhirId(prefix: "medication", item.id)),
            "status": .string("active"),
            "category": concept(FHIRCareCodings.patientSpecifiedCategory),
            "medicationCodeableConcept": .dictionary(["text": .string(item.name)]),
            "subject": subject
        ]
        if !dosage.isEmpty {
            resource["dosage"] = .array([.dictionary(dosage)])
        }
        if let lastTaken = item.lastTakenAt {
            // The only date the app records is the last logged dose; it is the start of
            // the known period. No end is claimed.
            resource["effectivePeriod"] = .dictionary(["start": .string(dateTime(lastTaken))])
        }
        if let note = note(item.notes) {
            resource["note"] = note
        }
        return resource
    }

    // MARK: - Building blocks

    private static func concept(_ coding: FHIRCareCodings.Coding, text: String? = nil) -> AnyCodableValue {
        var value: [String: AnyCodableValue] = ["coding": .array([.dictionary(coding.fhir)])]
        if let text {
            value["text"] = .string(text)
        }
        return .dictionary(value)
    }

    private static func component(_ code: FHIRCareCodings.Coding, valueQuantity: AnyCodableValue) -> AnyCodableValue {
        .dictionary(["code": concept(code), "valueQuantity": valueQuantity])
    }

    private static func component(_ code: FHIRCareCodings.Coding, valueConcept: AnyCodableValue) -> AnyCodableValue {
        .dictionary(["code": concept(code), "valueCodeableConcept": valueConcept])
    }

    /// UCUM quantity.
    private static func quantity(_ value: AnyCodableValue, unit: String) -> AnyCodableValue {
        .dictionary([
            "value": value,
            "unit": .string(unit),
            "system": .string(FHIRCareCodings.ucumSystem),
            "code": .string(unit)
        ])
    }

    private static func note(_ text: String) -> AnyCodableValue? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        return .array([.dictionary(["text": .string(trimmed)])])
    }

    private static func dateTime(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.timeZone = TimeZone(identifier: "UTC")
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: date)
    }

    /// `"HH:MM"` (24-hour) to FHIR `time` `"HH:MM:00"`; anything else is dropped.
    private static func fhirTime(_ clock: String) -> String? {
        let parts = clock.split(separator: ":", omittingEmptySubsequences: false)
        guard parts.count == 2,
              parts.allSatisfy({ $0.count == 2 && $0.allSatisfy(\.isASCII) }),
              let hour = Int(parts[0]), let minute = Int(parts[1]),
              (0...23).contains(hour), (0...59).contains(minute)
        else { return nil }
        return String(format: "%02d:%02d:00", hour, minute)
    }

    /// Stable FHIR id `<prefix>-<model id>` restricted to `[A-Za-z0-9-.]{1,64}`.
    /// Ids that would be longer are replaced by a SHA-256 prefix of the model id.
    static func fhirId(prefix: String, _ modelId: String) -> String {
        let sanitized = String(modelId.map { character -> Character in
            character.isASCII && (character.isLetter || character.isNumber || character == "-" || character == ".")
                ? character
                : "-"
        })
        let candidate = "\(prefix)-\(sanitized)"
        if candidate.count <= 64 && !sanitized.isEmpty {
            return candidate
        }
        let digest = SHA256.hash(data: Data(modelId.utf8))
        let hex = digest.map { String(format: "%02x", $0) }.joined()
        return "\(prefix)-\(hex.prefix(64 - prefix.count - 1))"
    }
}
