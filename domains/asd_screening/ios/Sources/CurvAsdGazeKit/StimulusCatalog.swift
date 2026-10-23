import Foundation

/// One place that decides which stimulus clips exist, which ages see them,
/// which session length plays them, and which clinical role each area has.
///
/// To add a video for a new age group, append a clip to `stimulus_catalog.json`:
/// age range, protocol membership (`express_30s` and/or `comprehensive_60s`),
/// area boxes, and roles (`social`, `geometric`, `eye`, `mouth`, `orienting`).
/// Add the same clip id to `normative_baselines.json` so scoring has a profile.
/// A new age pack listed in `agePacks` becomes the download tag `pack_<id>_<language>`
/// without a change to the player or the biomarker reducer.
public struct StimulusCatalog: Sendable, Equatable {
    public struct AgePack: Codable, Sendable, Equatable {
        public let id: String
        public let minMonths: Int
        public let maxMonths: Int
    }

    public struct Membership: Codable, Sendable, Equatable {
        public let id: String
        public let order: Int
    }

    public struct Area: Codable, Sendable, Equatable {
        public let name: String
        public let startSec: Double
        public let endSec: Double
        public let xMin: Double
        public let yMin: Double
        public let xMax: Double
        public let yMax: Double
        public let roles: [String]

        public var stimulusAOI: StimulusAOI {
            StimulusAOI(
                name: name,
                startTimeSec: startSec,
                endTimeSec: endSec,
                xMin: xMin,
                yMin: yMin,
                xMax: xMax,
                yMax: yMax
            )
        }
    }

    public struct Clip: Codable, Sendable, Equatable, Identifiable {
        public let id: String
        public let title: String
        public let category: String
        public let durationSec: Double
        public let minMonths: Int
        public let maxMonths: Int
        public let protocols: [Membership]
        public let areas: [Area]

        public func accepts(ageMonths: Int) -> Bool {
            ageMonths >= minMonths && ageMonths <= maxMonths
        }
    }

    public static let bundledResourceName = "stimulus_catalog"
    public static let expressProtocolId = "express_30s"
    public static let comprehensiveProtocolId = "comprehensive_60s"
    /// Session lengths at or above this value use the comprehensive protocol.
    public static let comprehensiveDurationThresholdSec: Double = 50

    public let schemaVersion: String
    public let languages: [String]
    public let fallbackLanguage: String
    public let agePacks: [AgePack]
    public let clips: [Clip]

    private let roleIndex: [String: Set<String>]

    public init(
        schemaVersion: String,
        languages: [String],
        fallbackLanguage: String,
        agePacks: [AgePack],
        clips: [Clip]
    ) {
        self.schemaVersion = schemaVersion
        self.languages = languages
        self.fallbackLanguage = fallbackLanguage
        self.agePacks = agePacks
        self.clips = clips
        var index: [String: Set<String>] = [:]
        for clip in clips {
            for area in clip.areas {
                index[area.name, default: []].formUnion(area.roles)
            }
        }
        self.roleIndex = index
    }

    private enum CodingKeys: String, CodingKey {
        case schemaVersion, languages, fallbackLanguage, agePacks, clips
    }
}

extension StimulusCatalog: Codable {
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            schemaVersion: try container.decode(String.self, forKey: .schemaVersion),
            languages: try container.decode([String].self, forKey: .languages),
            fallbackLanguage: try container.decode(String.self, forKey: .fallbackLanguage),
            agePacks: try container.decode([AgePack].self, forKey: .agePacks),
            clips: try container.decode([Clip].self, forKey: .clips)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(schemaVersion, forKey: .schemaVersion)
        try container.encode(languages, forKey: .languages)
        try container.encode(fallbackLanguage, forKey: .fallbackLanguage)
        try container.encode(agePacks, forKey: .agePacks)
        try container.encode(clips, forKey: .clips)
    }
}

extension StimulusCatalog {

    public static let current: StimulusCatalog = loadBundled() ?? .empty

    public static let empty = StimulusCatalog(
        schemaVersion: "0",
        languages: ["en"],
        fallbackLanguage: "en",
        agePacks: [],
        clips: []
    )

    public var clipIds: [String] { clips.map(\.id) }

    public func clip(id: String) -> Clip? {
        let clean = id.replacingOccurrences(of: ".mp4", with: "")
        return clips.first { $0.id == clean }
    }

    public func hasRole(_ areaName: String, _ role: String) -> Bool {
        roleIndex[areaName]?.contains(role) ?? false
    }

    public func tracker(forClipId id: String) -> StimulusAOITracker? {
        guard let clip = clip(id: id), !clip.areas.isEmpty else { return nil }
        return StimulusAOITracker(aois: clip.areas.map(\.stimulusAOI))
    }

    public func trackersByClipId() -> [String: StimulusAOITracker] {
        var map: [String: StimulusAOITracker] = [:]
        for clip in clips {
            if let tracker = tracker(forClipId: clip.id) {
                map[clip.id] = tracker
            }
        }
        return map
    }

    /// Clips assigned to the express or comprehensive protocol, optionally limited to an age.
    /// An age that matches nothing falls back to the full protocol so a session still has stimuli.
    public func playlist(forTargetDuration durationSec: Double, ageMonths: Int? = nil) -> [Clip] {
        let protocolId = durationSec >= Self.comprehensiveDurationThresholdSec
            ? Self.comprehensiveProtocolId
            : Self.expressProtocolId
        let selected = clips(in: protocolId, ageMonths: ageMonths)
        if selected.isEmpty, ageMonths != nil {
            return clips(in: protocolId, ageMonths: nil)
        }
        return selected
    }

    public func resourcePackTag(ageMonths: Int, localeCode: String) -> String? {
        guard let pack = agePacks.first(where: { ageMonths >= $0.minMonths && ageMonths <= $0.maxMonths })
                ?? agePacks.last else {
            return nil
        }
        let requested = String(localeCode.lowercased().prefix(2))
        let language = languages.contains(requested) ? requested : fallbackLanguage
        return "pack_\(pack.id)_\(language)"
    }

    public static func loadBundled() -> StimulusCatalog? {
        if let url = Bundle.module.url(forResource: bundledResourceName, withExtension: "json") {
            return load(from: url)
        }
        if let url = Bundle.main.url(forResource: bundledResourceName, withExtension: "json") {
            return load(from: url)
        }
        return nil
    }

    public static func load(from url: URL) -> StimulusCatalog? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return decode(data)
    }

    public static func decode(_ data: Data) -> StimulusCatalog? {
        try? JSONDecoder().decode(StimulusCatalog.self, from: data)
    }

    private func clips(in protocolId: String, ageMonths: Int?) -> [Clip] {
        clips
            .filter { clip in
                clip.protocols.contains { $0.id == protocolId }
                    && (ageMonths.map(clip.accepts) ?? true)
            }
            .sorted { lhs, rhs in
                let left = lhs.protocols.first { $0.id == protocolId }?.order ?? 0
                let right = rhs.protocols.first { $0.id == protocolId }?.order ?? 0
                return left < right
            }
    }
}
