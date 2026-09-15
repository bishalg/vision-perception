#if canImport(ActivityKit) && !os(macOS)
import Foundation
import ActivityKit

/// Shared Live Activity payload for medication doses and diary check-ins.
public struct CareCountdownAttributes: ActivityAttributes {
    public struct ContentState: Codable, Hashable {
        public var title: String
        public var subtitle: String
        public var endsAt: Date

        public init(title: String, subtitle: String, endsAt: Date) {
            self.title = title
            self.subtitle = subtitle
            self.endsAt = endsAt
        }
    }

    public var kind: String
    public var itemId: String

    public init(kind: String, itemId: String) {
        self.kind = kind
        self.itemId = itemId
    }
}
#else
import Foundation

public struct CareCountdownAttributes: Sendable {
    public struct ContentState: Codable, Hashable {
        public var title: String
        public var subtitle: String
        public var endsAt: Date

        public init(title: String, subtitle: String, endsAt: Date) {
            self.title = title
            self.subtitle = subtitle
            self.endsAt = endsAt
        }
    }

    public var kind: String
    public var itemId: String

    public init(kind: String, itemId: String) {
        self.kind = kind
        self.itemId = itemId
    }
}
#endif
