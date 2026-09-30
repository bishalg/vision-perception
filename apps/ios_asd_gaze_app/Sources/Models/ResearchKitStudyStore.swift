import CryptoKit
import CurvAsdGazeKit
import Foundation
import os

/// On-device coordinator for the Apple ResearchKit clinical study track.
/// Runs in parallel to `CareKitClinicalStore`:
/// - `CareKitClinicalStore` (CareKit / OCKStore) manages local caregiver care plans & milestones.
/// - `ResearchKitStudyStore` manages IRB e-consent, standardized M-CHAT-R/F surveys,
///   and de-identified `ORKActiveStepResult` / `ORKTaskResult` study records for doctor & research institute analysis.
///
/// Thread safety: All mutations are serialized on a dedicated background `researchQueue`.
/// Public queries return value copies or dispatch to callers on requested queues.
public final class ResearchKitStudyStore: @unchecked Sendable {
    public static let shared = ResearchKitStudyStore()

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "ResearchKitStudyStore")

    private let userDefaults: UserDefaults
    private let researchQueue = DispatchQueue(label: "org.curv.researchkit.store", qos: .utility)
    private let enrollmentKey = "curv_research_study_enrollment_v1"
    private let taskResultsKey = "curv_research_study_task_results_v1"
    private let surveyResultsKey = "curv_research_study_surveys_v1"

    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
    }

    // MARK: - Study Enrollment & Consent Management

    /// Retrieve the current research study enrollment profile.
    public func fetchEnrollment() -> ResearchStudyEnrollment {
        researchQueue.sync { fetchEnrollmentDirect() }
    }

    private func fetchEnrollmentDirect() -> ResearchStudyEnrollment {
        guard let data = userDefaults.data(forKey: enrollmentKey),
              let profile = try? JSONDecoder().decode(ResearchStudyEnrollment.self, from: data) else {
            return ResearchStudyEnrollment()
        }
        return profile
    }

    /// Save parental informed consent and activate study participation.
    public func saveConsent(
        signature: ORKConsentSignature,
        participantId: String? = nil,
        childAgeMonths: Int = 24,
        completion: (@Sendable (ResearchStudyEnrollment) -> Void)? = nil
    ) {
        researchQueue.sync {
            var profile = self.fetchEnrollmentDirect()
            if let participantId, !participantId.isEmpty {
                profile.participantId = participantId
            }
            profile.isConsented = true
            profile.consentSignature = signature
            profile.childAgeMonths = childAgeMonths

            if let data = try? JSONEncoder().encode(profile) {
                self.userDefaults.set(data, forKey: self.enrollmentKey)
                Self.logger.info("ResearchKit e-consent recorded for participant \(profile.participantId, privacy: .public)")
            } else {
                Self.logger.error("Failed to encode ResearchStudyEnrollment")
            }

            Self.dispatch(completion, value: profile)
        }
    }

    /// Revoke parental consent. Disables research synchronization without
    /// deleting local caregiver CareKit care plans.
    public func revokeConsent(completion: (@Sendable () -> Void)? = nil) {
        researchQueue.sync {
            var profile = self.fetchEnrollmentDirect()
            profile.isConsented = false
            profile.consentSignature = nil
            profile.normCollectionOptIn = false

            if let data = try? JSONEncoder().encode(profile) {
                self.userDefaults.set(data, forKey: self.enrollmentKey)
                Self.logger.info("ResearchKit consent revoked for \(profile.participantId, privacy: .public)")
            }
            Self.dispatch(completion)
        }
    }

    /// Toggles contribution of de-identified per-video metrics to normative reference collection.
    /// Only meaningful while research consent is active.
    public func setNormCollectionOptIn(_ enabled: Bool, completion: (@Sendable (ResearchStudyEnrollment) -> Void)? = nil) {
        researchQueue.sync {
            var profile = self.fetchEnrollmentDirect()
            profile.normCollectionOptIn = enabled
            if let data = try? JSONEncoder().encode(profile) {
                self.userDefaults.set(data, forKey: self.enrollmentKey)
                Self.logger.info("Norm collection opt-in \(enabled) for \(profile.participantId, privacy: .public)")
            }
            Self.dispatch(completion, value: profile)
        }
    }

    // MARK: - Active Step & Screening Recording

    /// Package a completed gaze screening session into a ResearchKit `ORKActiveStepResult`
    /// and assemble an `ORKTaskResult` linked to the enrolled participant ID.
    @discardableResult
    public func recordActiveStepResult(
        summary: GazeSessionReportSummary,
        telemetrySampleCount: Int = 0,
        completion: (@Sendable (ORKTaskResult?) -> Void)? = nil
    ) -> ORKTaskResult? {
        researchQueue.sync {
            let profile = fetchEnrollmentDirect()
            guard profile.isConsented else {
                Self.logger.warning("Screening completed but ResearchKit study participation is not consented; skipping study write")
                Self.dispatch(completion, value: nil)
                return nil
            }

            let activeResult = ORKActiveStepResult(summary: summary, telemetrySampleCount: telemetrySampleCount)
            var taskResult = ORKTaskResult(
                participantId: profile.participantId,
                startDate: Date().addingTimeInterval(-summary.durationSec),
                endDate: Date(),
                activeStepResult: activeResult
            )

            // Attach consent reference step result
            if let sig = profile.consentSignature {
                let consentStep = ORKStepResult(
                    identifier: "consent_review",
                    stepType: "ORKConsentReviewStep",
                    startDate: sig.signedDate,
                    endDate: sig.signedDate,
                    results: [
                        "signatureToken": .string(sig.signatureToken),
                        "consentVersion": .string(sig.consentVersion),
                        "participantId": .string(profile.participantId)
                    ]
                )
                taskResult.stepResults.append(consentStep)
            }

            var results = self.fetchStoredTaskResults()
            results.append(taskResult)
            self.persistTaskResults(results)
            Self.logger.info("Persisted ORKTaskResult for session \(summary.sessionId, privacy: .public) with participant \(profile.participantId, privacy: .public)")
            Self.dispatch(completion, value: taskResult)
            return taskResult
        }
    }

    // MARK: - Standardized Survey Recording (M-CHAT-R/F)

    /// Record completed M-CHAT-R/F responses and evaluate clinical score.
    public func recordSurveyResult(
        answers: [Int: Bool],
        completion: (@Sendable (MCHATScoreResult) -> Void)? = nil
    ) {
        let score = MCHATSurveyEngine.evaluate(answers: answers)

        researchQueue.sync {
            let profile = fetchEnrollmentDirect()
            // Persist structured survey score
            var storedSurveys = self.fetchStoredSurveyResults()
            storedSurveys.append(score)
            if let data = try? JSONEncoder().encode(storedSurveys) {
                self.userDefaults.set(data, forKey: self.surveyResultsKey)
            }

            // Also attach as ORKStepResult to research study history
            var dict: [String: AnyCodableValue] = [
                "totalFailed": .int(score.totalFailed),
                "riskLevel": .string(score.riskLevel.rawValue),
                "failedQuestionIds": .array(score.failedQuestionIds.map { .int($0) }),
                "clinicalRecommendation": .string(score.clinicalRecommendation),
                "participantId": .string(profile.participantId)
            ]
            for (qId, ans) in answers {
                dict["q_\(qId)"] = .bool(ans)
            }

            let surveyStep = ORKStepResult(
                identifier: "mchat_survey",
                stepType: "ORKQuestionStep",
                startDate: score.completionDate,
                endDate: score.completionDate,
                results: dict
            )

            var taskResult = ORKTaskResult(
                participantId: profile.participantId,
                startDate: score.completionDate,
                endDate: score.completionDate,
                stepResults: [surveyStep]
            )

            var results = self.fetchStoredTaskResults()
            results.append(taskResult)
            self.persistTaskResults(results)

            Self.logger.info("Recorded M-CHAT-R/F survey score: \(score.totalFailed) failed (\(score.riskLevel.rawValue, privacy: .public))")
            Self.dispatch(completion, value: score)
        }
    }

    // MARK: - Query & Study Bundle Compilation

    public func fetchCompletedTaskResults() -> [ORKTaskResult] {
        researchQueue.sync { fetchStoredTaskResults() }
    }

    public func fetchCompletedSurveys() -> [MCHATScoreResult] {
        researchQueue.sync { fetchStoredSurveyResults() }
    }

    /// Compile a de-identified ResearchKit study bundle ready for transmission
    /// to the doctor or research server, including SHA-256 cryptographic digest.
    public func compileStudyBundle() -> [String: Any] {
        researchQueue.sync { compileStudyBundleDirect() }
    }

    private func compileStudyBundleDirect() -> [String: Any] {
        let profile = fetchEnrollmentDirect()
        let tasks = fetchStoredTaskResults()
        let surveys = fetchStoredSurveyResults()

        let participant = StudyParticipantMetadata(
            participantId: profile.participantId,
            isConsented: profile.isConsented,
            childAgeMonths: profile.childAgeMonths,
            consentVersion: profile.consentSignature?.consentVersion ?? "unconsented"
        )

        var package = ResearchKitStudyPackage(
            participant: participant,
            taskResults: tasks,
            surveys: surveys
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        if let encodedData = try? encoder.encode(package) {
            let digest = SHA256.hash(data: encodedData)
            package.payloadSha256 = digest.compactMap { String(format: "%02x", $0) }.joined()
        }

        guard let finalData = try? encoder.encode(package),
              let jsonObject = try? JSONSerialization.jsonObject(with: finalData) as? [String: Any] else {
            return [:]
        }
        return jsonObject
    }

    /// Export study bundle as a pretty-printed JSON string.
    public func exportResearchKitJSON() -> String {
        let profile = fetchEnrollmentDirect()
        let tasks = fetchStoredTaskResults()
        let surveys = fetchStoredSurveyResults()

        let participant = StudyParticipantMetadata(
            participantId: profile.participantId,
            isConsented: profile.isConsented,
            childAgeMonths: profile.childAgeMonths,
            consentVersion: profile.consentSignature?.consentVersion ?? "unconsented"
        )

        var package = ResearchKitStudyPackage(
            participant: participant,
            taskResults: tasks,
            surveys: surveys
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        if let encodedData = try? encoder.encode(package) {
            let digest = SHA256.hash(data: encodedData)
            package.payloadSha256 = digest.compactMap { String(format: "%02x", $0) }.joined()
        }

        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? encoder.encode(package),
           let str = String(data: data, encoding: .utf8) {
            return str
        }
        return "{}"
    }

    // MARK: - Private Helpers

    private func fetchStoredTaskResults() -> [ORKTaskResult] {
        guard let data = userDefaults.data(forKey: taskResultsKey),
              let results = try? JSONDecoder().decode([ORKTaskResult].self, from: data) else {
            return []
        }
        return results
    }

    private func persistTaskResults(_ results: [ORKTaskResult]) {
        if let data = try? JSONEncoder().encode(results) {
            userDefaults.set(data, forKey: taskResultsKey)
        } else {
            Self.logger.error("Failed to encode [ORKTaskResult]")
        }
    }

    private func fetchStoredSurveyResults() -> [MCHATScoreResult] {
        guard let data = userDefaults.data(forKey: surveyResultsKey),
              let surveys = try? JSONDecoder().decode([MCHATScoreResult].self, from: data) else {
            return []
        }
        return surveys
    }

    private static func dispatch<T>(_ completion: (@Sendable (T) -> Void)?, value: T) {
        guard let completion else { return }
        DispatchQueue.main.async {
            completion(value)
        }
    }

    private static func dispatch(_ completion: (@Sendable () -> Void)?) {
        guard let completion else { return }
        DispatchQueue.main.async {
            completion()
        }
    }
}
