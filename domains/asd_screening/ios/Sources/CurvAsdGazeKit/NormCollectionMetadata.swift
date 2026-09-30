import Foundation

/// De-identified metadata attached to vault lines and research exports for sessions that
/// opted into normative reference collection. Plain data — the consent gate lives app-side
/// (`NormCollectionGate`); the vault and export serializers only need the payload shape.
public struct NormCollectionMetadata: Codable, Sendable, Equatable {
    public let consentVersion: String
    public let studyIdentifier: String
    public let participantId: String
    public let childAgeMonths: Int
    public let deviceModel: String
    public let isSimulator: Bool
    public let collectedAtUtc: String

    public init(
        consentVersion: String,
        studyIdentifier: String,
        participantId: String,
        childAgeMonths: Int,
        deviceModel: String,
        isSimulator: Bool,
        collectedAtUtc: String
    ) {
        self.consentVersion = consentVersion
        self.studyIdentifier = studyIdentifier
        self.participantId = participantId
        self.childAgeMonths = childAgeMonths
        self.deviceModel = deviceModel
        self.isSimulator = isSimulator
        self.collectedAtUtc = collectedAtUtc
    }

    public func toDictionary() -> [String: Any] {
        [
            "consentVersion": consentVersion,
            "studyIdentifier": studyIdentifier,
            "participantId": participantId,
            "childAgeMonths": childAgeMonths,
            "deviceModel": deviceModel,
            "isSimulator": isSimulator,
            "collectedAtUtc": collectedAtUtc
        ]
    }
}
