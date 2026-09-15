import Foundation

// MARK: - Clinical Interoperability Standards & Semantic Vocabularies
// Reference: DOC-RES-ASD-006 (Clinical Interoperability, SMART on FHIR & OMOP CDM)

/// Represents a standardized clinical concept tuple adhering to HL7 FHIR and USCDI standards.
public struct ClinicalCoding: Sendable, Codable, Equatable {
    public let system: String
    public let code: String
    public let display: String
    public let version: String?

    public init(system: String, code: String, display: String, version: String? = nil) {
        self.system = system
        self.code = code
        self.display = display
        self.version = version
    }

    public var fhirDictionary: [String: AnyCodableValue] {
        var dict: [String: AnyCodableValue] = [
            "system": .string(system),
            "code": .string(code),
            "display": .string(display)
        ]
        if let version {
            dict["version"] = .string(version)
        }
        return dict
    }
}

// MARK: - Clinical Coding Systems

public enum StandardCodingSystems {
    public static let loincURI = "http://loinc.org"
    public static let snomedURI = "http://snomed.info/sct"
    public static let icd10URI = "http://hl7.org/fhir/sid/icd-10-cm"
    public static let icd11URI = "http://id.who.int/icd/release/11/mms"
    public static let omopURI = "https://www.ohdsi.org/web/wiki/doku.php?id=documentation:cdm"
    public static let ucumURI = "http://unitsofmeasure.org"

    // MARK: - LOINC Standards (Assessments, Screeners & Sensory Panels)
    public enum LOINC {
        /// Modified Checklist for Autism in Toddlers, Revised (M-CHAT-R) panel
        public static let mchatRPanel = ClinicalCoding(
            system: loincURI,
            code: "80628-1",
            display: "Modified Checklist for Autism in Toddlers, Revised [M-CHAT-R]"
        )

        /// M-CHAT-R total failed items score
        public static let mchatRTotalScore = ClinicalCoding(
            system: loincURI,
            code: "80649-7",
            display: "M-CHAT-R total score [Score]"
        )

        /// M-CHAT-R risk level interpretation
        public static let mchatRRiskLevel = ClinicalCoding(
            system: loincURI,
            code: "80650-5",
            display: "M-CHAT-R risk level [Interpretation]"
        )

        /// Visual gaze tracking assessment panel
        public static let gazeTrackingPanel = ClinicalCoding(
            system: loincURI,
            code: "93044-6",
            display: "Gaze tracking assessment panel"
        )

        /// Visual reaction time (saccade latency)
        public static let visualReactionTime = ClinicalCoding(
            system: loincURI,
            code: "8692-2",
            display: "Reaction time - visual stimulus"
        )

        /// Behavioral affect observation score
        public static let behavioralAffectAssessment = ClinicalCoding(
            system: loincURI,
            code: "72514-3",
            display: "Pain and behavioral observation score"
        )

        /// Item-level LOINC codes for M-CHAT-R questions 1 through 20
        public static let mchatQuestionCodes: [Int: String] = [
            1: "80629-9",
            2: "80630-7",
            3: "80631-5",
            4: "80632-3",
            5: "80633-1",
            6: "80634-9",
            7: "80635-6",
            8: "80636-4",
            9: "80637-2",
            10: "80638-0",
            11: "80639-8",
            12: "80640-6",
            13: "80641-4",
            14: "80642-2",
            15: "80643-0",
            16: "80644-8",
            17: "80645-5",
            18: "80646-3",
            19: "80647-1",
            20: "80648-9"
        ]

        public static func loincCode(forMCHATQuestionId id: Int) -> String? {
            mchatQuestionCodes[id]
        }
    }

    // MARK: - SNOMED CT Standards (Clinical Findings, Behaviors & Procedures)
    public enum SNOMED {
        public static let autismSpectrumDisorder = ClinicalCoding(
            system: snomedURI,
            code: "35919005",
            display: "Autism spectrum disorder (disorder)"
        )

        public static let adhd = ClinicalCoding(
            system: snomedURI,
            code: "406506008",
            display: "Attention deficit hyperactivity disorder (disorder)"
        )

        public static let abnormalEyeContact = ClinicalCoding(
            system: snomedURI,
            code: "247540003",
            display: "Abnormal eye contact (finding)"
        )

        public static let poorEyeContact = ClinicalCoding(
            system: snomedURI,
            code: "247541004",
            display: "Poor eye contact (finding)"
        )

        public static let impairedJointAttention = ClinicalCoding(
            system: snomedURI,
            code: "708151006",
            display: "Impaired joint attention (finding)"
        )

        public static let stereotypicMovement = ClinicalCoding(
            system: snomedURI,
            code: "247547000",
            display: "Motor stereotypy (finding)"
        )

        public static let repetitiveBehavior = ClinicalCoding(
            system: snomedURI,
            code: "247546009",
            display: "Repetitive behavior (finding)"
        )

        public static let flatAffect = ClinicalCoding(
            system: snomedURI,
            code: "247528003",
            display: "Flat affect (finding)"
        )

        public static let lackOfSocialSmile = ClinicalCoding(
            system: snomedURI,
            code: "247535008",
            display: "Lack of social smile (finding)"
        )

        public static let delayedLanguageMilestone = ClinicalCoding(
            system: snomedURI,
            code: "229721007",
            display: "Language development delayed (finding)"
        )

        public static let eyeTrackingProcedure = ClinicalCoding(
            system: snomedURI,
            code: "713444003",
            display: "Eye tracking examination (procedure)"
        )
    }

    // MARK: - ICD-10-CM Standards (Diagnostic Classification)
    public enum ICD10 {
        public static let autisticDisorder = ClinicalCoding(
            system: icd10URI,
            code: "F84.0",
            display: "Autistic disorder"
        )

        public static let pervasiveDevelopmentalDisorderUnspecified = ClinicalCoding(
            system: icd10URI,
            code: "F84.9",
            display: "Pervasive developmental disorder, unspecified"
        )

        public static let adhdPredominantlyInattentive = ClinicalCoding(
            system: icd10URI,
            code: "F90.0",
            display: "Attention-deficit hyperactivity disorder, predominantly inattentive type"
        )

        public static let adhdHyperactiveImpulsive = ClinicalCoding(
            system: icd10URI,
            code: "F90.1",
            display: "Attention-deficit hyperactivity disorder, predominantly hyperactive-impulsive type"
        )

        public static let adhdCombined = ClinicalCoding(
            system: icd10URI,
            code: "F90.2",
            display: "Attention-deficit hyperactivity disorder, combined type"
        )

        public static let expressiveLanguageDisorder = ClinicalCoding(
            system: icd10URI,
            code: "F80.1",
            display: "Expressive language disorder"
        )
    }

    // MARK: - ICD-11 Standards (WHO Classification)
    public enum ICD11 {
        public static let autismSpectrumDisorder = ClinicalCoding(
            system: icd11URI,
            code: "6A02",
            display: "Autism spectrum disorder"
        )

        public static let adhd = ClinicalCoding(
            system: icd11URI,
            code: "6A05",
            display: "Attention deficit hyperactivity disorder"
        )
    }
}

// MARK: - Clinical Outcome Assessment (eCOA) Taxonomy

/// FDA CDRH/CDER Clinical Outcome Assessment (eCOA) classification.
public enum ClinicalOutcomeAssessmentType: String, Sendable, Codable, CaseIterable {
    /// Electronic Patient-Reported Outcome (direct child/patient self-report).
    case ePRO = "ePRO"
    /// Observer-Reported Outcome (caregiver diary, parent-reported observations).
    case obsRO = "ObsRO"
    /// Clinician-Reported Outcome (physician judgment, ADOS-2 evaluation).
    case clinRO = "ClinRO"
    /// Performance Outcome (bubble-pop minigame motor performance).
    case perfO = "PerfO"
    /// Objective continuous digital biomarker (TrueDepth gaze & affect telemetry).
    case digitalBiomarker = "DigitalBiomarker"
}

// MARK: - OMOP CDM v5.4 Tabular Record Schemas

/// Represents a single observation/biomarker row in the OMOP Common Data Model `MEASUREMENT` table.
public struct OMOPMeasurementRecord: Sendable, Codable, Equatable {
    public let personId: String
    public let measurementConceptId: Int
    public let measurementDate: String
    public let measurementDatetime: String
    public let measurementTypeConceptId: Int
    public let valueAsNumber: Double
    public let unitConceptId: Int
    public let rangeLow: Double?
    public let rangeHigh: Double?
    public let standardName: String

    public init(
        personId: String,
        measurementConceptId: Int,
        measurementDate: String,
        measurementDatetime: String,
        measurementTypeConceptId: Int = 32817, // EHR / Digital Health Technology assessment
        valueAsNumber: Double,
        unitConceptId: Int,
        rangeLow: Double? = nil,
        rangeHigh: Double? = nil,
        standardName: String
    ) {
        self.personId = personId
        self.measurementConceptId = measurementConceptId
        self.measurementDate = measurementDate
        self.measurementDatetime = measurementDatetime
        self.measurementTypeConceptId = measurementTypeConceptId
        self.valueAsNumber = valueAsNumber
        self.unitConceptId = unitConceptId
        self.rangeLow = rangeLow
        self.rangeHigh = rangeHigh
        self.standardName = standardName
    }
}

/// Represents a single survey or symptom observation in the OMOP Common Data Model `OBSERVATION` table.
public struct OMOPObservationRecord: Sendable, Codable, Equatable {
    public let personId: String
    public let observationConceptId: Int
    public let observationDate: String
    public let observationTypeConceptId: Int
    public let valueAsString: String?
    public let valueAsConceptId: Int?
    public let qualifierConceptId: Int?
    public let observationSourceValue: String

    public init(
        personId: String,
        observationConceptId: Int,
        observationDate: String,
        observationTypeConceptId: Int = 32817,
        valueAsString: String? = nil,
        valueAsConceptId: Int? = nil,
        qualifierConceptId: Int? = nil,
        observationSourceValue: String
    ) {
        self.personId = personId
        self.observationConceptId = observationConceptId
        self.observationDate = observationDate
        self.observationTypeConceptId = observationTypeConceptId
        self.valueAsString = valueAsString
        self.valueAsConceptId = valueAsConceptId
        self.qualifierConceptId = qualifierConceptId
        self.observationSourceValue = observationSourceValue
    }
}
