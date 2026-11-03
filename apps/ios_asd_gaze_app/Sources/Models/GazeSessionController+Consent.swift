import CurvAsdGazeKit
import Foundation

// MARK: - Consent-gated capture (CNS-1, ADR-0002 decision 1)

public extension GazeSessionController {
    /// App-layer entry point for live capture. Requires a `ConsentToken`, which
    /// only `ChildProfileStore` can mint after the onboarding consent step, and
    /// re-checks the store so a token minted before consent was withdrawn
    /// (profile reset) cannot start a session.
    ///
    /// Views MUST call this instead of the kit's consent-agnostic `startSession`.
    ///
    /// - Returns: `true` when the kit's `startSession` was invoked. The kit may
    ///   still decline to record (for example `.unavailable` on unsupported
    ///   hardware); observe `isRecording` / `trackingStage` for that.
    @MainActor
    @discardableResult
    func startConsentedSession(
        token: ConsentToken,
        battery: StimulusAOITracker? = nil,
        stimulusId: String? = nil,
        beginsClip: Bool = true,
        store: ChildProfileStore = .shared
    ) -> Bool {
        guard ConsentToken.mint(from: store) != nil else { return false }
        armResearchLog(researchLogConfiguration(participantToken: store.participantToken()))
        startSession(battery: battery, stimulusId: stimulusId, beginsClip: beginsClip)
        return true
    }

    /// RFL-2: the research frame log is armed only through `NormCollectionGate.isActive`
    /// (research consent, opt-in, live TrueDepth; the kit also refuses a simulated session).
    /// The log sits next to the sealed vault file, so deleting the vault removes it.
    @MainActor
    private func researchLogConfiguration(participantToken profileToken: String) -> ResearchLogConfiguration? {
        let enrollment = ResearchKitStudyStore.shared.fetchEnrollment()
        guard NormCollectionGate.isActive(enrollment: enrollment, captureSource: .trueDepth) else { return nil }
        return ResearchLogConfiguration(
            directory: SessionVaultManager().vaultDirectory,
            participantToken: GazeSessionRecord.participantToken(enrollment: enrollment, profileToken: profileToken),
            bundleVersion: scoringConfiguration.bundleVersion,
            appVersion: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String
        )
    }
}
