@testable import AsdGazeCollector
import XCTest

final class ClinicalInteroperabilityTests: XCTestCase {

    // MARK: - LOINC Semantic Standards Tests

    func testMCHATQuestionLOINCCodes() {
        XCTAssertEqual(StandardCodingSystems.LOINC.mchatRPanel.code, "80628-1")
        XCTAssertEqual(StandardCodingSystems.LOINC.mchatRTotalScore.code, "80649-7")
        XCTAssertEqual(StandardCodingSystems.LOINC.mchatRRiskLevel.code, "80650-5")

        // Verify all 20 questions have non-nil, distinct LOINC codes from 80629-9 to 80648-9
        var seenCodes = Set<String>()
        for q in MCHATSurveyEngine.questions {
            let code = q.loincCode
            XCTAssertFalse(code.isEmpty, "Question \(q.id) must have a valid LOINC code")
            XCTAssertTrue(code.hasPrefix("806"), "LOINC code must follow M-CHAT-R range")
            XCTAssertFalse(seenCodes.contains(code), "Duplicate LOINC code found: \(code)")
            seenCodes.insert(code)
        }
        XCTAssertEqual(seenCodes.count, 20, "All 20 questions must have unique LOINC codes")
    }

    func testMCHATScoreResultCodingProperties() {
        let result = MCHATScoreResult(
            totalFailed: 4,
            riskLevel: .medium,
            failedQuestionIds: [1, 7, 9, 14],
            clinicalRecommendation: "Follow-up interview recommended."
        )

        XCTAssertEqual(result.loincScoreCoding.code, "80649-7")
        XCTAssertEqual(result.loincRiskCoding.code, "80650-5")
        XCTAssertEqual(result.loincScoreCoding.system, "http://loinc.org")
    }

    // MARK: - SNOMED CT Semantic Standards Tests

    func testSNOMEDCodes() {
        XCTAssertEqual(StandardCodingSystems.SNOMED.autismSpectrumDisorder.code, "35919005")
        XCTAssertEqual(StandardCodingSystems.SNOMED.adhd.code, "406506008")
        XCTAssertEqual(StandardCodingSystems.SNOMED.abnormalEyeContact.code, "247540003")
        XCTAssertEqual(StandardCodingSystems.SNOMED.stereotypicMovement.code, "247547000")
        XCTAssertEqual(StandardCodingSystems.SNOMED.flatAffect.code, "247528003")
        XCTAssertEqual(StandardCodingSystems.SNOMED.eyeTrackingProcedure.code, "713444003")
    }

    // MARK: - FHIR R4 Observation Generation Tests

    func testFHIRObservationGeneration() {
        let gazeRecord = GazeSessionRecord.fixture(
            sessionId: "SESS-FHIR-001",
            participantId: "STUDY-ASD-TEST1",
            startedAt: Date(),
            durationSec: 45.0,
            socialPreferenceScore: 0.68,
            validFrameRatio: 0.96,
            eyeDwellRatio: 0.52,
            mouthDwellRatio: 0.16,
            firstEyeLatencyMs: 290.0,
            calibratedDistanceCm: 48.0,
            qualityVerdict: "optimal"
        )

        guard let fhirObs = FHIRInteroperabilityMapper.makeFHIRObservation(
            from: gazeRecord,
            participantId: "STUDY-ASD-TEST1"
        ) else {
            return XCTFail("live TrueDepth record must export")
        }

        XCTAssertEqual(fhirObs["resourceType"], .string("Observation"))
        XCTAssertEqual(fhirObs["status"], .string("final"))
        XCTAssertEqual(fhirObs["id"], .string("obs-asd-gaze-SESS-FHIR-001"))

        if case let .dictionary(subjectDict)? = fhirObs["subject"] {
            XCTAssertEqual(subjectDict["reference"], .string("Patient/STUDY-ASD-TEST1"))
        } else {
            XCTFail("Missing or invalid subject dictionary")
        }

        // Validate component digital biomarkers
        if case let .array(components)? = fhirObs["component"] {
            XCTAssertGreaterThanOrEqual(components.count, 5, "Must include at least 5 standard biomarker components")
        } else {
            XCTFail("Missing components array in FHIR Observation")
        }
    }

    // MARK: - FHIR R4 QuestionnaireResponse Generation Tests

    func testFHIRQuestionnaireResponseGeneration() {
        let result = MCHATScoreResult(
            totalFailed: 1,
            riskLevel: .low,
            failedQuestionIds: [2],
            clinicalRecommendation: "Low risk profile."
        )

        let answers: [Int: Bool] = [
            1: true,
            2: true, // Failed
            3: true,
            7: true
        ]

        let fhirQR = FHIRInteroperabilityMapper.makeFHIRQuestionnaireResponse(
            from: result,
            answers: answers,
            participantId: "STUDY-ASD-TEST2"
        )

        XCTAssertEqual(fhirQR["resourceType"], .string("QuestionnaireResponse"))
        XCTAssertEqual(fhirQR["status"], .string("completed"))
        XCTAssertEqual(fhirQR["questionnaire"], .string("http://loinc.org/80628-1"))

        if case let .array(items)? = fhirQR["item"] {
            XCTAssertEqual(items.count, 20, "QuestionnaireResponse must contain 20 item nodes")
            // Check first item has LOINC linkId 80629-9
            if case let .dictionary(item1)? = items.first {
                XCTAssertEqual(item1["linkId"], .string("80629-9"))
            }
        } else {
            XCTFail("Missing item array in FHIR QuestionnaireResponse")
        }
    }

    // MARK: - OMOP CDM v5.4 Measurement Generation Tests

    func testOMOPMeasurementsGeneration() {
        let gazeRecord = GazeSessionRecord.fixture(
            sessionId: "SESS-OMOP-001",
            participantId: "P001",
            startedAt: Date(),
            durationSec: 60.0,
            socialPreferenceScore: 0.75,
            validFrameRatio: 0.94,
            eyeDwellRatio: 0.48,
            mouthDwellRatio: 0.18,
            firstEyeLatencyMs: 310.0,
            calibratedDistanceCm: 50.0,
            qualityVerdict: "optimal"
        )

        let measurements = FHIRInteroperabilityMapper.makeOMOPMeasurements(
            from: gazeRecord,
            personId: "10001"
        )

        XCTAssertEqual(measurements.count, 4)
        let latencyRow = measurements.first(where: { $0.standardName == "First Eye Fixation Latency" })
        XCTAssertNotNil(latencyRow)
        XCTAssertEqual(latencyRow?.valueAsNumber, 310.0)
        XCTAssertEqual(latencyRow?.unitConceptId, 8555) // ms
        XCTAssertEqual(latencyRow?.rangeLow, 150.0)
        XCTAssertEqual(latencyRow?.rangeHigh, 450.0)

        let socialRow = measurements.first(where: { $0.standardName == "Social Visual Preference Score" })
        XCTAssertNotNil(socialRow)
        XCTAssertEqual(socialRow?.valueAsNumber, 0.75)
    }

    // MARK: - SIM-5: simulated input is refused

    func testFHIRObservationRefusesSimulatedRecord() {
        let simulated = GazeSessionRecord.fixture(
            sessionId: "SESS-SIM-001",
            captureSource: .simulated,
            eyeDwellRatio: 0.5,
            mouthDwellRatio: 0.2
        )
        XCTAssertNil(FHIRInteroperabilityMapper.makeFHIRObservation(from: simulated, participantId: "child-SIM"))
        XCTAssertNil(simulated.toFHIRObservation())
        XCTAssertNil(simulated.toFHIRObservation(basedOn: "SR-1"))
    }

    func testOMOPMeasurementsRefuseSimulatedRecord() {
        let simulated = GazeSessionRecord.fixture(captureSource: .simulated, firstEyeLatencyMs: 300)
        XCTAssertTrue(FHIRInteroperabilityMapper.makeOMOPMeasurements(from: simulated, personId: "1").isEmpty)
        XCTAssertTrue(simulated.toOMOPMeasurements(personId: "1").isEmpty)
    }

    // MARK: - PLC-3: absent metrics are absent components / rows

    private func componentTexts(_ obs: [String: AnyCodableValue]) -> [String] {
        guard case let .array(components)? = obs["component"] else { return [] }
        return components.compactMap { component -> String? in
            guard case let .dictionary(dict) = component,
                  case let .dictionary(code)? = dict["code"],
                  case let .string(text)? = code["text"] else { return nil }
            return text
        }
    }

    private func componentCodes(_ obs: [String: AnyCodableValue]) -> [String] {
        guard case let .array(components)? = obs["component"] else { return [] }
        return components.flatMap { component -> [String] in
            guard case let .dictionary(dict) = component,
                  case let .dictionary(code)? = dict["code"],
                  case let .array(codings)? = code["coding"] else { return [] }
            return codings.compactMap { coding in
                guard case let .dictionary(c) = coding, case let .string(value)? = c["code"] else { return nil }
                return value
            }
        }
    }

    func testFHIRObservationOmitsAbsentMetricComponents() throws {
        let record = GazeSessionRecord.fixture(eyeDwellRatio: nil, mouthDwellRatio: nil, firstEyeLatencyMs: nil)
        let obs = try XCTUnwrap(FHIRInteroperabilityMapper.makeFHIRObservation(from: record, participantId: "child-1"))
        let texts = componentTexts(obs)
        XCTAssertFalse(componentCodes(obs).contains("8692-2"), "no first-look latency component without a measurement")
        XCTAssertFalse(texts.contains("First Eye Fixation Latency"))
        XCTAssertFalse(texts.contains("Eye Region Dwell Ratio"))
        XCTAssertFalse(texts.contains("Mouth Region Dwell Ratio"))
        XCTAssertTrue(texts.contains("Social Visual Preference Score"))
        XCTAssertTrue(texts.contains("Valid Frame Quality Ratio"))
    }

    func testFHIRObservationIncludesMeasuredDwellComponents() throws {
        let record = GazeSessionRecord.fixture(eyeDwellRatio: 0.6, mouthDwellRatio: 0.4, firstEyeLatencyMs: nil)
        let obs = try XCTUnwrap(FHIRInteroperabilityMapper.makeFHIRObservation(from: record, participantId: "child-1"))
        let texts = componentTexts(obs)
        XCTAssertTrue(texts.contains("Eye Region Dwell Ratio"))
        XCTAssertTrue(texts.contains("Mouth Region Dwell Ratio"))
        XCTAssertFalse(componentCodes(obs).contains("8692-2"))
    }

    func testOMOPMeasurementsOmitAbsentRows() {
        let record = GazeSessionRecord.fixture(eyeDwellRatio: nil, mouthDwellRatio: nil, firstEyeLatencyMs: nil)
        let rows = FHIRInteroperabilityMapper.makeOMOPMeasurements(from: record, personId: "10001")
        XCTAssertEqual(rows.map(\.standardName), ["Social Visual Preference Score"])
        XCTAssertFalse(rows.contains { $0.measurementConceptId == 37048700 })
    }

    func testLegacyProvenanceRecordIsLabelledInFHIR() throws {
        let legacy = GazeSessionRecord.fixture(legacyProvenance: true)
        let obs = try XCTUnwrap(FHIRInteroperabilityMapper.makeFHIRObservation(from: legacy, participantId: "child-1"))
        guard case let .dictionary(meta)? = obs["meta"], case let .array(tags)? = meta["tag"],
              case let .dictionary(tag)? = tags.first else {
            return XCTFail("legacy record must carry a meta.tag")
        }
        XCTAssertEqual(tag["code"], .string("legacy-provenance"))

        let live = GazeSessionRecord.fixture()
        let liveObs = try XCTUnwrap(FHIRInteroperabilityMapper.makeFHIRObservation(from: live, participantId: "child-1"))
        XCTAssertNil(liveObs["meta"], "records with recorded provenance carry no legacy tag")
    }

    // MARK: - Outcome Assessment Taxonomy Tests

    func testClinicalOutcomeAssessmentTaxonomy() {
        XCTAssertEqual(ClinicalOutcomeAssessmentType.allCases.count, 5)
        XCTAssertEqual(ClinicalOutcomeAssessmentType.ePRO.rawValue, "ePRO")
        XCTAssertEqual(ClinicalOutcomeAssessmentType.obsRO.rawValue, "ObsRO")
        XCTAssertEqual(ClinicalOutcomeAssessmentType.clinRO.rawValue, "ClinRO")
        XCTAssertEqual(ClinicalOutcomeAssessmentType.perfO.rawValue, "PerfO")
        XCTAssertEqual(ClinicalOutcomeAssessmentType.digitalBiomarker.rawValue, "DigitalBiomarker")
    }
}
