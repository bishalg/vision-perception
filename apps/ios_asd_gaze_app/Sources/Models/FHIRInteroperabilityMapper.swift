import CurvAsdGazeKit
import Foundation
import os

// MARK: - FHIR R4 & OMOP CDM Interoperability Mapper
// Reference: DOC-RES-ASD-006
//
// SIM-5: simulated records are refused (no Observation, no OMOP rows).
// PLC-3: a metric that was not measured (`nil`) is omitted; nothing substitutes a default.

public enum FHIRInteroperabilityMapper {
    private static let isoFormatter = ISO8601DateFormatter()
    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "FHIRInteroperabilityMapper")

    /// Local code system tagging records whose capture source was inferred on decode (SIM-4).
    public static let provenanceTagSystem = "http://curv.org/fhir/CodeSystem/record-provenance"

    // MARK: - FHIR R4 Observation Builder

    /// Converts a completed `GazeSessionRecord` into an HL7 FHIR Release 4 `Observation` JSON dictionary.
    /// Returns `nil` for a simulated record (SIM-5).
    public static func makeFHIRObservation(
        from record: GazeSessionRecord,
        participantId: String
    ) -> [String: AnyCodableValue]? {
        guard record.captureSource != .simulated else {
            logger.info("Refusing FHIR export of simulated session \(record.sessionId, privacy: .public)")
            return nil
        }
        let timestamp = isoFormatter.string(from: record.startedAt)

        var components: [[String: AnyCodableValue]] = []

        // Component 1: First Eye Fixation Latency (LOINC 8692-2), only when measured (PLC-3)
        if let firstEyeLatencyMs = record.firstEyeLatencyMs {
            components.append([
                "code": .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string(StandardCodingSystems.loincURI),
                            "code": .string("8692-2"),
                            "display": .string("Reaction time - visual stimulus")
                        ])
                    ]),
                    "text": .string("First Eye Fixation Latency")
                ]),
                "valueQuantity": .dictionary([
                    "value": .double(firstEyeLatencyMs),
                    "unit": .string("ms"),
                    "system": .string(StandardCodingSystems.ucumURI),
                    "code": .string("ms")
                ])
            ])
        }

        // Component 2: Social Preference Score (Continuous index [-1.0, 1.0])
        components.append([
            "code": .dictionary([
                "coding": .array([
                    .dictionary([
                        "system": .string(StandardCodingSystems.loincURI),
                        "code": .string("93044-6"),
                        "display": .string("Gaze tracking assessment panel")
                    ])
                ]),
                "text": .string("Social Visual Preference Score")
            ]),
            "valueQuantity": .dictionary([
                "value": .double(record.socialPreferenceScore),
                "unit": .string("score"),
                "system": .string(StandardCodingSystems.ucumURI),
                "code": .string("1")
            ])
        ])

        // Component 3: Eye Dwell Ratio (Normalized [0.0, 1.0]), only when measured (PLC-3)
        if let eyeDwellRatio = record.eyeDwellRatio {
            components.append([
                "code": .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string(StandardCodingSystems.snomedURI),
                            "code": .string(StandardCodingSystems.SNOMED.abnormalEyeContact.code),
                            "display": .string(StandardCodingSystems.SNOMED.abnormalEyeContact.display)
                        ])
                    ]),
                    "text": .string("Eye Region Dwell Ratio")
                ]),
                "valueQuantity": .dictionary([
                    "value": .double(eyeDwellRatio),
                    "unit": .string("ratio"),
                    "system": .string(StandardCodingSystems.ucumURI),
                    "code": .string("1")
                ])
            ])
        }

        // Component 4: Mouth Dwell Ratio, only when measured (PLC-3)
        if let mouthDwellRatio = record.mouthDwellRatio {
            components.append([
                "code": .dictionary([
                    "text": .string("Mouth Region Dwell Ratio")
                ]),
                "valueQuantity": .dictionary([
                    "value": .double(mouthDwellRatio),
                    "unit": .string("ratio"),
                    "system": .string(StandardCodingSystems.ucumURI),
                    "code": .string("1")
                ])
            ])
        }

        // Component 5: Valid Frame Ratio (Quality Gate Indicator)
        components.append([
            "code": .dictionary([
                "text": .string("Valid Frame Quality Ratio")
            ]),
            "valueQuantity": .dictionary([
                "value": .double(record.validFrameRatio),
                "unit": .string("ratio"),
                "system": .string(StandardCodingSystems.ucumURI),
                "code": .string("1")
            ])
        ])

        // Components 6+: Per-Stimulus Matrix subscores (v1.5 records only; local Curv code system
        // until standardized codes exist for per-stimulus engagement metrics)
        let curvMatrixSystem = "http://curv.org/fhir/CodeSystem/per-stimulus-matrix"
        if let composite = record.perVideoComposite {
            components.append([
                "code": .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string(curvMatrixSystem),
                            "code": .string("composite-engagement"),
                            "display": .string("Per-Stimulus Composite Engagement Score")
                        ])
                    ]),
                    "text": .string("Per-Stimulus Composite Engagement Score")
                ]),
                "valueQuantity": .dictionary([
                    "value": .double(composite),
                    "unit": .string("score"),
                    "system": .string(StandardCodingSystems.ucumURI),
                    "code": .string("1")
                ])
            ])
        }
        for video in record.perVideo ?? [] where video.hasBaseline {
            components.append([
                "code": .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string(curvMatrixSystem),
                            "code": .string("clip-score-\(video.stimulusId)"),
                            "display": .string(video.title)
                        ])
                    ]),
                    "text": .string("Stimulus subscore: \(video.title)")
                ]),
                "valueQuantity": .dictionary([
                    "value": .double(video.subscore),
                    "unit": .string("score"),
                    "system": .string(StandardCodingSystems.ucumURI),
                    "code": .string("1")
                ])
            ])
        }

        var observation: [String: AnyCodableValue] = [
            "resourceType": .string("Observation"),
            "id": .string("obs-asd-gaze-\(record.sessionId)"),
            "status": .string("final"),
            "category": .array([
                .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string("http://terminology.hl7.org/CodeSystem/observation-category"),
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
                "text": .string("Pediatric Oculomotor Social-Visual Attention Screening")
            ]),
            "subject": .dictionary([
                "reference": .string("Patient/\(participantId)"),
                "display": .string("Anonymous Study Participant")
            ]),
            "effectiveDateTime": .string(timestamp),
            "device": .dictionary([
                "display": .string("Apple TrueDepth 3D Optical Sensor (CurvAsdGazeKit)")
            ]),
            "component": .array(components.map { .dictionary($0) })
        ]
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

    // MARK: - FHIR R4 QuestionnaireResponse Builder

    /// Converts an M-CHAT-R/F result and answer set into an HL7 FHIR Release 4 `QuestionnaireResponse`.
    public static func makeFHIRQuestionnaireResponse(
        from result: MCHATScoreResult,
        answers: [Int: Bool] = [:],
        participantId: String
    ) -> [String: AnyCodableValue] {
        let authored = isoFormatter.string(from: result.completionDate)

        var items: [[String: AnyCodableValue]] = []
        for qId in 1...20 {
            let loincCode = StandardCodingSystems.LOINC.loincCode(forMCHATQuestionId: qId) ?? "80628-1-\(qId)"
            let ansBool = answers[qId]
            var itemDict: [String: AnyCodableValue] = [
                "linkId": .string(loincCode),
                "text": .string("M-CHAT-R Question \(qId)")
            ]
            if let ansBool {
                itemDict["answer"] = .array([
                    .dictionary(["valueBoolean": .bool(ansBool)])
                ])
            }
            items.append(itemDict)
        }

        return [
            "resourceType": .string("QuestionnaireResponse"),
            "id": .string("qr-mchat-rf-\(UUID().uuidString.prefix(8))"),
            "status": .string("completed"),
            "questionnaire": .string("\(StandardCodingSystems.loincURI)/80628-1"),
            "subject": .dictionary([
                "reference": .string("Patient/\(participantId)")
            ]),
            "authored": .string(authored),
            "item": .array(items.map { .dictionary($0) })
        ]
    }

    // MARK: - OMOP CDM v5.4 Tabular Mappings

    /// Maps a `GazeSessionRecord` into OMOP CDM `MEASUREMENT` tabular rows.
    /// Empty for a simulated record (SIM-5); rows for unmeasured metrics are omitted (PLC-3).
    public static func makeOMOPMeasurements(
        from record: GazeSessionRecord,
        personId: String
    ) -> [OMOPMeasurementRecord] {
        guard record.captureSource != .simulated else {
            logger.info("Refusing OMOP export of simulated session \(record.sessionId, privacy: .public)")
            return []
        }
        let dateStr = String(isoFormatter.string(from: record.startedAt).prefix(10))
        let datetimeStr = isoFormatter.string(from: record.startedAt)

        var rows = [
            OMOPMeasurementRecord(
                personId: personId,
                measurementConceptId: 40769032, // Eye tracking measurement
                measurementDate: dateStr,
                measurementDatetime: datetimeStr,
                valueAsNumber: record.socialPreferenceScore,
                unitConceptId: 8523, // ratio
                standardName: "Social Visual Preference Score"
            )
        ]
        if let firstEyeLatencyMs = record.firstEyeLatencyMs {
            rows.append(OMOPMeasurementRecord(
                personId: personId,
                measurementConceptId: 37048700, // Visual reaction time
                measurementDate: dateStr,
                measurementDatetime: datetimeStr,
                valueAsNumber: firstEyeLatencyMs,
                unitConceptId: 8555, // millisecond
                rangeLow: 150.0,
                rangeHigh: 450.0,
                standardName: "First Eye Fixation Latency"
            ))
        }
        if let eyeDwellRatio = record.eyeDwellRatio {
            rows.append(OMOPMeasurementRecord(
                personId: personId,
                measurementConceptId: 40769033, // Eye dwell ratio
                measurementDate: dateStr,
                measurementDatetime: datetimeStr,
                valueAsNumber: eyeDwellRatio,
                unitConceptId: 8523,
                rangeLow: 0.35,
                rangeHigh: 0.75,
                standardName: "Eye Region Dwell Ratio"
            ))
        }
        if let mouthDwellRatio = record.mouthDwellRatio {
            rows.append(OMOPMeasurementRecord(
                personId: personId,
                measurementConceptId: 40769034, // Mouth dwell ratio
                measurementDate: dateStr,
                measurementDatetime: datetimeStr,
                valueAsNumber: mouthDwellRatio,
                unitConceptId: 8523,
                standardName: "Mouth Region Dwell Ratio"
            ))
        }
        return rows
    }
}
