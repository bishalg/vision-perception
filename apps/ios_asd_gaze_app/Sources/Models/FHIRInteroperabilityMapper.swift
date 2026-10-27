import CurvAsdGazeKit
import Foundation
import os

// MARK: - FHIR R4 & OMOP CDM Interoperability Mapper
// Reference: DOC-RES-ASD-006, ADR-0003
//
// Resource-level builders. The export unit is the Bundle built by `FHIRBundleBuilder` (FHIR-1);
// the standalone `makeFHIR*` functions remain for callers that need a single resource.
//
// SIM-5: simulated records are refused (no Observation, no OMOP rows).
// FHIR-2: rejected sessions are refused; degraded ones are `preliminary`.
// FHIR-3: components use the local gaze-engagement CodeSystem; SNOMED findings are never codes.
// PLC-3: a metric that was not measured (`nil`) is omitted; nothing substitutes a default.

public enum FHIRInteroperabilityMapper {
    private static let isoFormatter = ISO8601DateFormatter()
    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "FHIRInteroperabilityMapper")

    /// Local code system tagging records whose capture source was inferred on decode (SIM-4).
    public static let provenanceTagSystem = FHIRNamespaces.recordProvenanceCodeSystem

    static func timestamp(_ date: Date) -> String { isoFormatter.string(from: date) }

    // MARK: - Status and refusal (FHIR-2, SIM-5)

    /// `final` for live optimal/acceptable sessions, `preliminary` for degraded ones, `nil`
    /// (no Observation) for rejected, unknown-quality or simulated sessions.
    public static func observationStatus(for record: GazeSessionRecord) -> String? {
        guard record.captureSource == .trueDepth else { return nil }
        switch GazeQualityGate.Verdict(rawValue: record.qualityVerdict) {
        case .optimal?, .acceptable?: return "final"
        case .degraded?: return "preliminary"
        case .rejected?, nil: return nil
        }
    }

    /// Why a record cannot be exported as a gaze Observation, or `nil` when it can.
    public static func refusalReason(for record: GazeSessionRecord) -> String? {
        if record.captureSource == .simulated {
            return "Simulated data cannot be exported as a health record."
        }
        if observationStatus(for: record) == nil {
            return "This observation did not have enough usable samples to export."
        }
        return nil
    }

    // MARK: - FHIR R4 Observation Builder

    /// A standalone HL7 FHIR R4 `Observation` for a session, with a `Patient/<participantId>`
    /// subject. Returns `nil` when the record is refused (SIM-5, FHIR-2).
    public static func makeFHIRObservation(
        from record: GazeSessionRecord,
        participantId: String
    ) -> [String: AnyCodableValue]? {
        makeGazeObservation(
            from: record,
            id: "obs-asd-gaze-\(record.sessionId)",
            subjectReference: "Patient/\(participantId)",
            deviceReference: nil
        )
    }

    /// The gaze Observation with explicit references (used inside the Bundle).
    static func makeGazeObservation(
        from record: GazeSessionRecord,
        id: String,
        subjectReference: String,
        deviceReference: String?
    ) -> [String: AnyCodableValue]? {
        guard let status = observationStatus(for: record) else {
            logger.info("Refusing FHIR export of session \(record.sessionId, privacy: .public): \(refusalReason(for: record) ?? "", privacy: .public)")
            return nil
        }
        var observation: [String: AnyCodableValue] = [
            "resourceType": .string("Observation"),
            "id": .string(id),
            "identifier": .array([
                .dictionary([
                    "system": .string(FHIRNamespaces.gazeSessionSystem),
                    "value": .string(record.sessionId)
                ])
            ]),
            "status": .string(status),
            "category": .array([
                .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string(FHIRNamespaces.observationCategorySystem),
                            "code": .string("exam"),
                            "display": .string("Exam")
                        ])
                    ])
                ])
            ]),
            "code": .dictionary([
                "coding": .array([
                    .dictionary(StandardCodingSystems.LOINC.gazeTrackingPanel.fhirDictionary),
                    .dictionary(StandardCodingSystems.SNOMED.eyeTrackingProcedure.fhirDictionary)
                ]),
                "text": .string("Gaze engagement observation (parent-run phone app)")
            ]),
            "subject": .dictionary(["reference": .string(subjectReference)]),
            "effectiveDateTime": .string(timestamp(record.startedAt)),
            "component": .array(gazeComponents(for: record))
        ]
        if let deviceReference {
            observation["device"] = .dictionary(["reference": .string(deviceReference)])
        } else {
            observation["device"] = .dictionary(["display": .string(FHIRDeviceInfo.current.displayName)])
        }
        // SIM-4: records written before provenance existed say so.
        if record.legacyProvenance == true {
            observation["meta"] = .dictionary([
                "tag": .array([
                    .dictionary([
                        "system": .string(provenanceTagSystem),
                        "code": .string("legacy-provenance"),
                        "display": .string("Capture source inferred from a record written before provenance was recorded")
                    ])
                ])
            ])
        }
        return observation
    }

    /// Components in a fixed order; unmeasured metrics are absent (PLC-3). First-look latency is
    /// never exported: there is no real first-fixation detector yet (PLC-1).
    static func gazeComponents(for record: GazeSessionRecord) -> [AnyCodableValue] {
        typealias Codes = StandardCodingSystems.GazeEngagement
        var components: [AnyCodableValue] = []
        func add(_ coding: ClinicalCoding, text: String, value: Double?, unit: String) {
            guard let value else { return }
            components.append(.dictionary([
                "code": .dictionary([
                    "coding": .array([.dictionary(coding.fhirDictionary)]),
                    "text": .string(text)
                ]),
                "valueQuantity": .dictionary([
                    "value": .double(value),
                    "unit": .string(unit),
                    "system": .string(StandardCodingSystems.ucumURI),
                    "code": .string("1")
                ])
            ]))
        }
        add(Codes.socialPreferenceScore, text: "Social Visual Preference Score", value: record.socialPreferenceScore, unit: "score")
        add(Codes.eyeRegionDwellRatio, text: "Eye Region Dwell Ratio", value: record.eyeDwellRatio, unit: "ratio")
        add(Codes.mouthRegionDwellRatio, text: "Mouth Region Dwell Ratio", value: record.mouthDwellRatio, unit: "ratio")
        add(Codes.validFrameRatio, text: "Valid Frame Quality Ratio", value: record.validFrameRatio, unit: "ratio")
        add(Codes.socialSmileIntensityMean, text: "Social Smile Intensity Mean", value: record.socialSmileIntensity, unit: "score")
        add(Codes.smileReactivityIndex, text: "Smile Reactivity Index", value: record.smileReactivityIndex, unit: "score")
        add(Codes.flatAffectScore, text: "Neutral Expression Score", value: record.flatAffectScore, unit: "score")
        add(Codes.surpriseScoreMean, text: "Surprise Score Mean", value: record.surpriseScoreMean, unit: "score")
        add(Codes.compositeEngagement, text: "Per-Stimulus Composite Engagement Score", value: record.perVideoComposite, unit: "score")
        for video in record.perVideo ?? [] where video.hasBaseline {
            add(
                Codes.clipScore(stimulusId: video.stimulusId, title: video.title),
                text: "Stimulus subscore: \(video.title)",
                value: video.subscore,
                unit: "score"
            )
        }
        return components
    }

    // MARK: - FHIR R4 QuestionnaireResponse Builder (FHIR-4)

    /// A standalone M-CHAT-R/F `QuestionnaireResponse` with a `Patient/<participantId>` subject.
    public static func makeFHIRQuestionnaireResponse(
        from result: MCHATScoreResult,
        answers: [Int: Bool] = [:],
        participantId: String
    ) -> [String: AnyCodableValue] {
        makeQuestionnaireResponse(
            from: result,
            answers: answers,
            id: "qr-mchat-rf-\(UUID().uuidString.prefix(8))",
            subjectReference: "Patient/\(participantId)",
            authorReference: nil
        )
    }

    static func makeQuestionnaireResponse(
        from result: MCHATScoreResult,
        answers: [Int: Bool],
        id: String,
        subjectReference: String,
        authorReference: String?
    ) -> [String: AnyCodableValue] {
        let items: [AnyCodableValue] = (1...20).map { qId in
            var item: [String: AnyCodableValue] = [
                "linkId": .string(StandardCodingSystems.LOINC.loincCode(forMCHATQuestionId: qId)),
                "text": .string("M-CHAT-R/F item \(qId)")
            ]
            if let answer = answers[qId] {
                item["answer"] = .array([.dictionary(["valueBoolean": .bool(answer)])])
            }
            return .dictionary(item)
        }
        var response: [String: AnyCodableValue] = [
            "resourceType": .string("QuestionnaireResponse"),
            "id": .string(id),
            "status": .string("completed"),
            "questionnaire": .string(FHIRNamespaces.mchatQuestionnaireCanonical),
            "subject": .dictionary(["reference": .string(subjectReference)]),
            "authored": .string(timestamp(result.completionDate)),
            "item": .array(items)
        ]
        if let authorReference {
            response["author"] = .dictionary(["reference": .string(authorReference)])
            response["source"] = .dictionary(["reference": .string(authorReference)])
        }
        return response
    }

    /// Official M-CHAT-R/F scoring category for `80650-5` (FHIR-4).
    static func mchatCategoryText(for level: MCHATRiskLevel) -> String {
        switch level {
        case .low: return "Low-Risk (total score 0-2)" // claims-ok: official M-CHAT-R/F scoring category
        case .medium: return "Medium-Risk (total score 3-7)" // claims-ok: official M-CHAT-R/F scoring category
        case .high: return "High-Risk (total score 8-20)" // claims-ok: official M-CHAT-R/F scoring category
        }
    }

    /// Total score (`80649-7`, `valueInteger`) with the category (`80650-5`) as a component,
    /// derived from the QuestionnaireResponse (FHIR-4).
    static func makeMCHATScoreObservation(
        from result: MCHATScoreResult,
        id: String,
        subjectReference: String,
        questionnaireResponseReference: String
    ) -> [String: AnyCodableValue] {
        [
            "resourceType": .string("Observation"),
            "id": .string(id),
            "status": .string("final"),
            "category": .array([
                .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string(FHIRNamespaces.observationCategorySystem),
                            "code": .string("survey"),
                            "display": .string("Survey")
                        ])
                    ])
                ])
            ]),
            "code": .dictionary([
                "coding": .array([.dictionary(StandardCodingSystems.LOINC.mchatRTotalScore.fhirDictionary)]),
                "text": .string("M-CHAT-R/F total score")
            ]),
            "subject": .dictionary(["reference": .string(subjectReference)]),
            "effectiveDateTime": .string(timestamp(result.completionDate)),
            "valueInteger": .int(result.totalFailed),
            "derivedFrom": .array([.dictionary(["reference": .string(questionnaireResponseReference)])]),
            "component": .array([
                .dictionary([
                    "code": .dictionary([
                        "coding": .array([.dictionary(StandardCodingSystems.LOINC.mchatRRiskLevel.fhirDictionary)])
                    ]),
                    "valueCodeableConcept": .dictionary(["text": .string(mchatCategoryText(for: result.riskLevel))])
                ])
            ])
        ]
    }

    // MARK: - OMOP CDM v5.4 Tabular Mappings (FHIR-6)

    /// Maps a `GazeSessionRecord` into OMOP CDM `MEASUREMENT` tabular rows.
    /// Empty for a simulated record (SIM-5); rows for unmeasured metrics are omitted (PLC-3).
    ///
    /// No measurement or unit concept id used by earlier builds (40769032/33/34, 37048700, 8523,
    /// 8555) could be verified against OHDSI Athena, so each row carries concept id `0` ("no matching
    /// concept") with the local metric code in `measurement_source_value` and the unit in
    /// `unit_source_value`. No reference ranges: the app has no validated norms.
    public static func makeOMOPMeasurements(
        from record: GazeSessionRecord,
        personId: String
    ) -> [OMOPMeasurementRecord] {
        guard record.captureSource != .simulated else {
            logger.info("Refusing OMOP export of simulated session \(record.sessionId, privacy: .public)")
            return []
        }
        let datetimeStr = timestamp(record.startedAt)
        let dateStr = String(datetimeStr.prefix(10))

        func row(_ name: String, sourceValue: String, value: Double, unit: String) -> OMOPMeasurementRecord {
            OMOPMeasurementRecord(
                personId: personId,
                measurementConceptId: 0,
                measurementDate: dateStr,
                measurementDatetime: datetimeStr,
                valueAsNumber: value,
                unitConceptId: 0,
                standardName: name,
                measurementSourceValue: sourceValue,
                unitSourceValue: unit
            )
        }

        var rows = [
            row("Social Visual Preference Score", sourceValue: "social-preference-score",
                value: record.socialPreferenceScore, unit: "score")
        ]
        if let firstEyeLatencyMs = record.firstEyeLatencyMs {
            rows.append(row("First Eye Fixation Latency", sourceValue: "first-eye-fixation-latency-ms",
                            value: firstEyeLatencyMs, unit: "ms"))
        }
        if let eyeDwellRatio = record.eyeDwellRatio {
            rows.append(row("Eye Region Dwell Ratio", sourceValue: "eye-region-dwell-ratio",
                            value: eyeDwellRatio, unit: "ratio"))
        }
        if let mouthDwellRatio = record.mouthDwellRatio {
            rows.append(row("Mouth Region Dwell Ratio", sourceValue: "mouth-region-dwell-ratio",
                            value: mouthDwellRatio, unit: "ratio"))
        }
        return rows
    }
}
