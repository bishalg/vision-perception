import Foundation

// MARK: - Clinic delivery gate (store-collection-build.md S-17, S-18)

/// Whether sessions may leave this device for the clinic, computed from injected inputs.
///
/// Delivery is authorised only when **all** hold:
/// - the guardian turned on the `clinicDelivery` opt-in (S-12, S-15);
/// - the guardian consent record matches the current consent document (S-10);
/// - outside the store build, the research-study enrolment is consented as well (the existing
///   Debug/DemoRelease behaviour, unchanged). The store build has no screen that sets it, so
///   there the guardian consent is the consent that covers delivery.
///
/// `ResearchKitSyncService` consults it before queueing, flushing or sending (S-17). Turning the
/// opt-in off stops every send; queued items stay on the device and resume when it is turned
/// back on (S-18).
public struct ClinicDeliveryAuthorization: Equatable, Sendable {
    /// Why delivery is refused, first failing condition first.
    public enum Refusal: Equatable, Sendable {
        /// The `clinicDelivery` opt-in is off.
        case optedOut
        /// No current guardian consent, or (outside the store build) no research-study consent.
        case consentInactive
    }

    public let optedIn: Bool
    public let guardianConsentCurrent: Bool
    /// `true` outside the store build: the research-study enrolment must also be consented.
    public let researchConsentRequired: Bool
    public let researchConsented: Bool

    public init(
        optedIn: Bool,
        guardianConsentCurrent: Bool,
        researchConsentRequired: Bool = !StoreBuild.isStoreBuild,
        researchConsented: Bool
    ) {
        self.optedIn = optedIn
        self.guardianConsentCurrent = guardianConsentCurrent
        self.researchConsentRequired = researchConsentRequired
        self.researchConsented = researchConsented
    }

    public var refusal: Refusal? {
        guard optedIn else { return .optedOut }
        guard guardianConsentCurrent else { return .consentInactive }
        if researchConsentRequired && !researchConsented { return .consentInactive }
        return nil
    }

    public var isAuthorized: Bool { refusal == nil }

    /// The neutral reason shown when a manual sync is refused. Never research wording (S-17).
    public var refusalText: String? {
        switch refusal {
        case .optedOut: return Self.deliveryOffText
        case .consentInactive: return Self.consentInactiveText
        case nil: return nil
        }
    }

    public static var deliveryOffText: String { Localizer.tr("clinic_delivery_off") }
    public static var consentInactiveText: String { Localizer.tr("clinic_delivery_consent_inactive") }

    /// The live gate: the shared opt-ins and the shared guardian consent record. The research
    /// consent comes from the caller (the sync service's own study store).
    @MainActor
    public static func live(
        researchConsented: Bool,
        optIns: FeatureOptIns = .shared,
        consent: ConsentRecordStore = .shared,
        storeBuild: Bool = StoreBuild.isStoreBuild
    ) -> ClinicDeliveryAuthorization {
        ClinicDeliveryAuthorization(
            optedIn: optIns.clinicDelivery,
            guardianConsentCurrent: consent.isCurrent,
            researchConsentRequired: !storeBuild,
            researchConsented: researchConsented
        )
    }
}
