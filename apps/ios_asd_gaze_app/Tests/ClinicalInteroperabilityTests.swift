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
        let gazeRecord = GazeSessionRecord(
            sessionId: "SESS-FHIR-001",
            participantId: "STUDY-ASD-TEST1",
            startedAt: Date(),
            endedAt: Date().addingTimeInterval(45),
            durationSec: 45.0,
            socialPreferenceScore: 0.68,
            validFrameRatio: 0.96,
            eyeDwellRatio: 0.52,
            mouthDwellRatio: 0.16,
            firstEyeLatencyMs: 290.0,
            calibratedDistanceCm: 48.0,
            qualityVerdict: "optimal"
        )

        let fhirObs = FHIRInteroperabilityMapper.makeFHIRObservation(
            from: gazeRecord,
            participantId: "STUDY-ASD-TEST1"
        )

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
        let gazeRecord = GazeSessionRecord(
            sessionId: "SESS-OMOP-001",
            participantId: "P001",
            startedAt: Date(),
            endedAt: Date().addingTimeInterval(60),
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
