import Foundation
import CoreVideo
#if canImport(CoreML)
import CoreML
#endif

// MARK: - Facial Age Band

/// Coarse facial age band used for **assistive subject verification** — telling an
/// adult face from a toddler/child face so setup guidance and session telemetry can
/// react. This is deliberately NOT exact age estimation: public models overestimate
/// young children by years, which is finer than the app's 12/18/24/36/48-month
/// bracket boundaries, so the output is only ever consumed as a coarse band.
public enum FacialAgeBand: String, Codable, Sendable, CaseIterable, Equatable {
    /// 12–36 months
    case toddler
    /// 37–72 months
    case preschool
    /// 7–12 years
    case child
    /// 13+ years (adolescent or adult — intentionally merged, the classifier only
    /// needs to separate them from the young-child bands)
    case teenAdult

    public var label: String {
        switch self {
        case .toddler: return "Toddler (1–3 yrs)"
        case .preschool: return "Preschool (3–6 yrs)"
        case .child: return "Child (7–12 yrs)"
        case .teenAdult: return "Teen / Adult (13+ yrs)"
        }
    }

    public var monthRange: ClosedRange<Int> {
        switch self {
        case .toddler: return 12...36
        case .preschool: return 37...72
        case .child: return 73...156
        case .teenAdult: return 157...1200
        }
    }

    /// Ordinal index for band-distance comparisons (bands are ordered youngest → oldest).
    public var ordinal: Int {
        FacialAgeBand.allCases.firstIndex(of: self) ?? 0
    }

    /// Number of band steps between two bands (0 = same band, 3 = toddler vs teen/adult).
    public func ordinalDistance(to other: FacialAgeBand) -> Int {
        abs(ordinal - other.ordinal)
    }

    /// Maps an age in months to the nearest coarse band — used to cross-check the
    /// caregiver-entered `PediatricAgeBracket` against the on-camera estimate.
    public static func approximateBand(months: Int) -> FacialAgeBand {
        switch months {
        case ..<37: return .toddler
        case 37...72: return .preschool
        case 73...156: return .child
        default: return .teenAdult
        }
    }
}

// MARK: - Estimation Result

/// Single-frame (or windowed) age-band estimate. Never persisted as an image; only
/// the band and confidence floats flow into session metadata.
public struct AgeBandEstimation: Codable, Sendable, Equatable {
    public let band: FacialAgeBand
    public let confidence: Double
    public let executionDevice: ExecutionDevice
    public let evaluationTimestamp: Date
    public let inferenceLatencyMs: Double

    public init(
        band: FacialAgeBand,
        confidence: Double,
        executionDevice: ExecutionDevice,
        evaluationTimestamp: Date = Date(),
        inferenceLatencyMs: Double = 1.0
    ) {
        self.band = band
        self.confidence = max(0.0, min(1.0, confidence))
        self.executionDevice = executionDevice
        self.evaluationTimestamp = evaluationTimestamp
        self.inferenceLatencyMs = inferenceLatencyMs
    }
}

// MARK: - Inference Abstraction

/// Abstraction over the age-band model so windowing/aggregation logic can be
/// unit-tested without a bundled CoreML model.
public protocol AgeBandInferring: Sendable {
    /// False when no model is loaded — callers skip cropping entirely to avoid
    /// spending CoreImage work on frames that cannot produce an estimate.
    var isModelAvailable: Bool { get }
    /// Runs inference on an upright, square face crop. Returns nil when no model is
    /// loaded or prediction fails. Callers must treat age output as assistive only.
    func estimate(crop: CVPixelBuffer) -> AgeBandEstimation?
}

// MARK: - CoreML Engine

/// CoreML inference engine mapping a 224×224 face crop to a coarse `FacialAgeBand`.
///
/// Mirrors the `BiomarkerClassificationEngine` optional-model pattern: when no
/// compiled model (`.mlmodelc`) is present the engine is inert (`estimate` returns
/// nil, `isModelAvailable` is false) and the whole subject-age pipeline degrades to
/// a no-op instead of fabricating age data.
///
/// Expected model interface (produced by `tools/asd_age_model/`):
/// - Input:  `image` — 224×224 color image
/// - Output: `ageBandProbabilities` — dictionary keyed by `FacialAgeBand.rawValue`
///           (falls back to `probabilities`, then `classLabel`)
public final class AgeBandEstimationEngine: @unchecked Sendable, AgeBandInferring {
    public static let shared = AgeBandEstimationEngine()

    #if canImport(CoreML)
    private var mlModel: MLModel?
    #endif

    /// Looks for a compiled model in the main bundle, then in
    /// `Application Support/CurvAsdGazeKit/Models/AgeBand.mlmodelc` (sideloaded
    /// without an app update). See `tools/asd_age_model/` for the producer.
    public static func discoverCompiledModelURL() -> URL? {
        let fileName = "AgeBand"
        if let bundled = Bundle.main.url(forResource: fileName, withExtension: "mlmodelc") {
            return bundled
        }
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        let candidate = appSupport?
            .appendingPathComponent("CurvAsdGazeKit", isDirectory: true)
            .appendingPathComponent("Models", isDirectory: true)
            .appendingPathComponent("\(fileName).mlmodelc", isDirectory: true)
        if let candidate, FileManager.default.fileExists(atPath: candidate.path) {
            return candidate
        }
        return nil
    }

    public init(compiledModelURL: URL? = nil) {
        #if canImport(CoreML)
        let url = compiledModelURL ?? Self.discoverCompiledModelURL()
        if let url {
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

    public var isModelAvailable: Bool {
        #if canImport(CoreML)
        return mlModel != nil
        #else
        return false
        #endif
    }

    public func estimate(crop: CVPixelBuffer) -> AgeBandEstimation? {
        let startTime = CFAbsoluteTimeGetCurrent()

        #if canImport(CoreML)
        guard let model = mlModel else { return nil }
        guard let featureProvider = try? MLDictionaryFeatureProvider(dictionary: ["image": MLFeatureValue(pixelBuffer: crop)]),
              let output = try? model.prediction(from: featureProvider) else {
            return nil
        }

        let latencyMs = (CFAbsoluteTimeGetCurrent() - startTime) * 1000.0

        if let band = Self.bestBand(from: output) {
            return AgeBandEstimation(
                band: band,
                confidence: Self.confidence(from: output, for: band),
                executionDevice: .appleNeuralEngine,
                evaluationTimestamp: Date(),
                inferenceLatencyMs: latencyMs
            )
        }
        #endif

        return nil
    }

    #if canImport(CoreML)
    private static func probabilityDictionary(from output: MLFeatureProvider) -> [AnyHashable: NSNumber]? {
        output.featureValue(for: "ageBandProbabilities")?.dictionaryValue
            ?? output.featureValue(for: "probabilities")?.dictionaryValue
    }

    private static func bestBand(from output: MLFeatureProvider) -> FacialAgeBand? {
        guard let probabilities = probabilityDictionary(from: output) else {
            if let label = output.featureValue(for: "classLabel")?.stringValue {
                return FacialAgeBand(rawValue: label)
            }
            return nil
        }

        let scored: [(band: FacialAgeBand, value: Double)] = probabilities.compactMap { key, value in
            guard let keyString = key as? String, let band = FacialAgeBand(rawValue: keyString) else { return nil }
            return (band, value.doubleValue)
        }
        guard let best = scored.max(by: { $0.value < $1.value }) else { return nil }
        return best.band
    }

    private static func confidence(from output: MLFeatureProvider, for band: FacialAgeBand) -> Double {
        if let probabilities = probabilityDictionary(from: output),
           let raw = probabilities.first(where: { ($0.key as? String) == band.rawValue })?.value.doubleValue {
            return max(0.0, min(1.0, raw))
        }
        return 0.5
    }
    #endif
}
