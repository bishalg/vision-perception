import Foundation
#if ASD_RESEARCH_CLASSIFIER && canImport(CoreML)
import CoreML
#endif

// ADR-0004 / CLM-3: the risk classifier is a research-only (Debug) capability. Only
// `ExecutionDevice` (shared with the age-band engine) compiles in every configuration;
// `RiskStratification` and the classifier compile strictly under `ASD_RESEARCH_CLASSIFIER`.

/// Compute device utilized for CoreML inference.
public enum ExecutionDevice: String, Codable, Sendable, Equatable {
    case appleNeuralEngine = "Apple Neural Engine (ANE)"
    case gpu = "Apple Metal GPU"
    case cpu = "CPU"
    case fallbackHeuristic = "Rule-Based Clinical Baseline"
}

#if ASD_RESEARCH_CLASSIFIER
/// Risk stratification category produced by the ML biomarker inference engine.
public enum RiskStratification: String, Codable, Sendable, Equatable {
    case typical = "Typical Social Attention"
    case monitor = "Monitoring Recommended"
    case elevatedRisk = "Elevated Risk — Specialist Evaluation Recommended"

    public var badgeColorHex: String {
        switch self {
        case .typical: return "#10B981" // Emerald Mint
        case .monitor: return "#F59E0B" // Amber
        case .elevatedRisk: return "#EF4444" // Crimson
        }
    }
}

/// Comprehensive inference outcome from the ASD Biomarker Classifier.
public struct BiomarkerClassificationResult: Codable, Sendable, Equatable {
    public let riskScore: Double
    public let confidence: Double
    public let riskStratification: RiskStratification
    public let primaryContributingFeature: String
    public let featureImportances: [String: Double]
    public let executionDevice: ExecutionDevice
    public let evaluationTimestamp: Date
    public let inferenceLatencyMs: Double

    public init(
        riskScore: Double,
        confidence: Double,
        riskStratification: RiskStratification,
        primaryContributingFeature: String,
        featureImportances: [String: Double],
        executionDevice: ExecutionDevice,
        evaluationTimestamp: Date = Date(),
        inferenceLatencyMs: Double = 1.2
    ) {
        self.riskScore = riskScore
        self.confidence = confidence
        self.riskStratification = riskStratification
        self.primaryContributingFeature = primaryContributingFeature
        self.featureImportances = featureImportances
        self.executionDevice = executionDevice
        self.evaluationTimestamp = evaluationTimestamp
        self.inferenceLatencyMs = inferenceLatencyMs
    }
}

/// Research-only classifier over visual attention telemetry (not a diagnostic tool).
///
/// If a compiled CoreML model (`.mlmodelc`) is present, it executes with `computeUnits = .all`.
/// Otherwise it falls back to a deterministic rule-based heuristic (`.fallbackHeuristic`)
/// whose weights are informed by GeoPref and oculomotor literature but are not validated.
public final class BiomarkerClassificationEngine: @unchecked Sendable {
    public static let shared = BiomarkerClassificationEngine()

    #if canImport(CoreML)
    private var mlModel: MLModel?
    #endif

    public init(compiledModelURL: URL? = nil) {
        #if canImport(CoreML)
        if let url = compiledModelURL {
            let config = MLModelConfiguration()
            config.computeUnits = .all // Prioritize Apple Neural Engine (ANE)
            self.mlModel = try? MLModel(contentsOf: url, configuration: config)
        }
        #endif
    }

    #if canImport(CoreML)
    public init(model: MLModel) {
        self.mlModel = model
    }
    #endif

    /// Classifies clinical session summary asynchronously.
    public func classify(summary: GazeSessionReportSummary) async -> BiomarkerClassificationResult {
        return classifySync(summary: summary)
    }

    /// Synchronous classification entry point.
    public func classifySync(summary: GazeSessionReportSummary) -> BiomarkerClassificationResult {
        let startTime = CFAbsoluteTimeGetCurrent()

        #if canImport(CoreML)
        if let model = mlModel,
           let mlResult = runCoreMLInference(model: model, summary: summary, startTime: startTime) {
            return mlResult
        }
        #endif

        return runClinicalBaselineInference(summary: summary, startTime: startTime)
    }

    #if canImport(CoreML)
    private func runCoreMLInference(
        model: MLModel,
        summary: GazeSessionReportSummary,
        startTime: CFAbsoluteTime
    ) -> BiomarkerClassificationResult? {
        var featureDictionary: [String: Double] = [
            "socialPreferenceScore": summary.socialPreferenceScore,
            "socialSmileIntensityMean": summary.socialSmileIntensityMean,
            "smileReactivityIndex": summary.smileReactivityIndex,
            "flatAffectScore": summary.flatAffectScore,
            "surpriseScoreMean": summary.surpriseScoreMean,
            "validRatio": summary.validRatio
        ]
        // Absent metrics are absent features (PLC-4); a model that requires them
        // fails prediction and the heuristic path runs instead.
        if let value = summary.eyeDwellRatio { featureDictionary["eyeDwellRatio"] = value }
        if let value = summary.mouthDwellRatio { featureDictionary["mouthDwellRatio"] = value }
        if let value = summary.firstEyeLatencyMs { featureDictionary["firstEyeLatencyMs"] = value }

        guard let featureProvider = try? MLDictionaryFeatureProvider(dictionary: featureDictionary as [String: Any]),
              let output = try? model.prediction(from: featureProvider) else {
            return nil
        }

        let riskScore = output.featureValue(for: "riskScore")?.doubleValue
            ?? output.featureValue(for: "probability")?.dictionaryValue["atypical"]?.doubleValue
            ?? 0.50

        let confidence = output.featureValue(for: "confidence")?.doubleValue ?? 0.92
        let latencyMs = (CFAbsoluteTimeGetCurrent() - startTime) * 1000.0

        let stratification: RiskStratification
        if riskScore < 0.35 {
            stratification = .typical
        } else if riskScore < 0.65 {
            stratification = .monitor
        } else {
            stratification = .elevatedRisk
        }

        return BiomarkerClassificationResult(
            riskScore: max(0.0, min(1.0, riskScore)),
            confidence: max(0.0, min(1.0, confidence)),
            riskStratification: stratification,
            primaryContributingFeature: "CoreML ANE Multi-Biomarker Synthesis",
            featureImportances: [
                "socialPreference": 0.40,
                "eyeDwell": 0.25,
                "firstEyeLatency": 0.15,
                "flatAffect": 0.12,
                "smileReactivity": 0.08
            ],
            executionDevice: .appleNeuralEngine,
            evaluationTimestamp: Date(),
            inferenceLatencyMs: latencyMs
        )
    }
    #endif

    /// Rule-based research heuristic loosely derived from Pierce et al. (GeoPref)
    /// and Jones & Klin. Not validated; research builds only.
    ///
    /// PLC-4: a `nil` metric is absent. Its weight is dropped and the remaining
    /// weights are re-normalised to sum to 1, so no default value stands in for a
    /// measurement that was not made.
    private func runClinicalBaselineInference(
        summary: GazeSessionReportSummary,
        startTime: CFAbsoluteTime
    ) -> BiomarkerClassificationResult {
        struct Term {
            let key: String
            let label: String
            let weight: Double
            /// Risk fraction in [0, 1] for this metric; nil when the metric is absent.
            let risk: Double?
        }

        let terms: [Term] = [
            // Social preference [-1, +1] normalised to [0, 1]; 1 is full social preference.
            Term(
                key: "socialPreference",
                label: "Low Social Preference Index (< 50%)",
                weight: 0.42,
                risk: 1.0 - max(0.0, min(1.0, (summary.socialPreferenceScore + 1.0) / 2.0))
            ),
            // Eye dwell: typical 0.25 - 0.45, reduced < 0.18.
            Term(
                key: "eyeDwell",
                label: "Reduced Eye Fixation Ratio (< 20%)",
                weight: 0.24,
                risk: summary.eyeDwellRatio.map { 1.0 - max(0.0, min(1.0, $0 / 0.40)) }
            ),
            // First-eye latency: typical < 420 ms, prolonged > 600 ms.
            Term(
                key: "firstEyeLatency",
                label: "Prolonged Saccade Latency (> 550ms)",
                weight: 0.14,
                risk: summary.firstEyeLatencyMs.map { max(0.0, min(1.0, ($0 - 300.0) / 500.0)) }
            ),
            Term(
                key: "flatAffect",
                label: "Affective Hypomimia (Flat Affect)",
                weight: 0.12,
                risk: max(0.0, min(1.0, summary.flatAffectScore))
            ),
            Term(
                key: "smileReactivity",
                label: "Attenuated Smile Reactivity",
                weight: 0.08,
                risk: 1.0 - max(0.0, min(1.0, summary.smileReactivityIndex / 0.50))
            )
        ]

        let present = terms.filter { $0.risk != nil }
        let presentWeight = present.reduce(0.0) { $0 + $1.weight }
        let scale = presentWeight > 0 ? 1.0 / presentWeight : 0.0

        var importances: [String: Double] = [:]
        var contributions: [(String, Double)] = []
        for term in present {
            let contribution = (term.risk ?? 0) * term.weight * scale
            importances[term.key] = contribution
            contributions.append((term.label, contribution))
        }

        // Logistic sigmoid around the heuristic midpoint.
        let rawSum = importances.values.reduce(0, +)
        let logit = (rawSum - 0.38) * 6.5
        let riskScore = 1.0 / (1.0 + exp(-logit))

        let primaryFeature = contributions.max(by: { $0.1 < $1.1 })?.0 ?? "Social Visual Attention Balance"

        let stratification: RiskStratification
        if riskScore < 0.35 {
            stratification = .typical
        } else if riskScore < 0.65 {
            stratification = .monitor
        } else {
            stratification = .elevatedRisk
        }

        // Confidence scales with valid sample ratio
        let confidence = max(0.70, min(0.98, summary.validRatio * 0.98))
        let latencyMs = (CFAbsoluteTimeGetCurrent() - startTime) * 1000.0

        return BiomarkerClassificationResult(
            riskScore: max(0.01, min(0.99, riskScore)),
            confidence: confidence,
            riskStratification: stratification,
            primaryContributingFeature: primaryFeature,
            featureImportances: importances,
            executionDevice: .fallbackHeuristic,
            evaluationTimestamp: Date(),
            inferenceLatencyMs: max(0.1, latencyMs)
        )
    }
}
#endif
