import Foundation

// MARK: - FHIR namespaces (ADR-0003 decision 7)

/// Every local FHIR URI the app emits derives from `local`. Changing the publisher domain
/// (ADR-0005) is a one-line edit here plus a fixture update (`Tests/Fixtures/expected-bundle.json`).
public enum FHIRNamespaces {
    /// Base for local CodeSystems, NamingSystems and StructureDefinitions.
    public static let local = "http://curv.org/fhir"

    // MARK: CodeSystems

    /// Gaze engagement component codes (FHIR-3); no standard code exists for these phone-app metrics.
    public static let gazeEngagementCodeSystem = "\(local)/CodeSystem/gaze-engagement"
    /// Capture source values (`CaptureSource.rawValue`) carried by `Provenance` (SIM-4).
    public static let captureSourceCodeSystem = "\(local)/CodeSystem/capture-source"
    /// `meta.tag` system for records whose capture source was inferred on decode (SIM-4).
    public static let recordProvenanceCodeSystem = "\(local)/CodeSystem/record-provenance"

    // MARK: NamingSystems (identifier systems)

    public static let participantTokenSystem = "\(local)/NamingSystem/participant-token"
    public static let gazeSessionSystem = "\(local)/NamingSystem/gaze-session"
    public static let sessionVaultSystem = "\(local)/NamingSystem/session-vault"

    // MARK: Extensions

    public static let ageInMonthsExtension = "\(local)/StructureDefinition/age-in-months"
    public static let captureSourceExtension = "\(local)/StructureDefinition/capture-source"
    public static let captureSourceInferredExtension = "\(local)/StructureDefinition/capture-source-inferred"

    // MARK: HL7 terminology

    public static let observationCategorySystem = "http://terminology.hl7.org/CodeSystem/observation-category"
    public static let dataOperationSystem = "http://terminology.hl7.org/CodeSystem/v3-DataOperation"
    public static let provenanceParticipantTypeSystem = "http://terminology.hl7.org/CodeSystem/provenance-participant-type"
    public static let roleCodeSystem = "http://terminology.hl7.org/CodeSystem/v3-RoleCode"

    /// Canonical M-CHAT-R/F questionnaire (FHIR-4).
    public static let mchatQuestionnaireCanonical = "http://loinc.org/q/80628-1"
}
