import Foundation

/// One daily clock-time alert shared by medications and the sensory diary.
public struct CareReminderSlot: Identifiable, Codable, Sendable, Equatable {
    public var id: String
    /// `"HH:MM"` 24-hour clock.
    public var clockTime: String
    public var remindersEnabled: Bool
    public var alarmsEnabled: Bool
    public var liveActivityEnabled: Bool

    public init(
        id: String = UUID().uuidString,
        clockTime: String,
        remindersEnabled: Bool = true,
        alarmsEnabled: Bool = false,
        liveActivityEnabled: Bool = false
    ) {
        self.id = id
        self.clockTime = clockTime
        self.remindersEnabled = remindersEnabled
        self.alarmsEnabled = alarmsEnabled
        self.liveActivityEnabled = liveActivityEnabled
    }
}

/// Kind of care event driving a local notification / AlarmKit / Live Activity.
public enum CareReminderKind: String, Codable, Sendable {
    case medicationDose
    case diaryCheckIn
}

/// Normalized request consumed by `CareAlertScheduler`.
public struct CareAlertRequest: Identifiable, Sendable, Equatable {
    public let id: String
    public let kind: CareReminderKind
    public let title: String
    public let body: String
    public let clockTime: String
    public let remindersEnabled: Bool
    public let alarmsEnabled: Bool
    public let liveActivityEnabled: Bool
    /// When true, skip remaining fires for today (dose taken / diary already logged).
    public let skipToday: Bool

    public init(
        id: String,
        kind: CareReminderKind,
        title: String,
        body: String,
        clockTime: String,
        remindersEnabled: Bool,
        alarmsEnabled: Bool,
        liveActivityEnabled: Bool,
        skipToday: Bool
    ) {
        self.id = id
        self.kind = kind
        self.title = title
        self.body = body
        self.clockTime = clockTime
        self.remindersEnabled = remindersEnabled
        self.alarmsEnabled = alarmsEnabled
        self.liveActivityEnabled = liveActivityEnabled
        self.skipToday = skipToday
    }
}

public enum CareClockTime {
    /// Parse `"HH:MM"` into hour and minute, or nil.
    public static func parse(_ time: String) -> (hour: Int, minute: Int)? {
        let trimmed = time.trimmingCharacters(in: .whitespacesAndNewlines)
        let parts = trimmed.split(separator: ":")
        guard parts.count == 2,
              let hour = Int(parts[0].trimmingCharacters(in: .whitespacesAndNewlines)),
              let minute = Int(parts[1].trimmingCharacters(in: .whitespacesAndNewlines)),
              (0...23).contains(hour), (0...59).contains(minute) else { return nil }
        return (hour, minute)
    }

    public static func format(hour: Int, minute: Int) -> String {
        String(format: "%02d:%02d", hour, minute)
    }

    /// Next fire `Date` for a daily clock time, skipping today when `skipToday`.
    public static func nextDate(clockTime: String, from now: Date = Date(), skipToday: Bool) -> Date? {
        guard let (hour, minute) = parse(clockTime) else { return nil }
        let calendar = Calendar.current
        var components = calendar.dateComponents([.year, .month, .day], from: now)
        components.hour = hour
        components.minute = minute
        components.second = 0
        guard var candidate = calendar.date(from: components) else { return nil }
        if skipToday || candidate <= now {
            candidate = calendar.date(byAdding: .day, value: 1, to: candidate) ?? candidate
        }
        return candidate
    }
}

extension Array where Element == CareReminderSlot {
    /// One slot per clock time, preserving flags when times already existed.
    public func synced(to schedule: [String]) -> [CareReminderSlot] {
        schedule.map { time in
            first(where: { $0.clockTime == time })
                ?? CareReminderSlot(clockTime: time)
        }
    }
}
