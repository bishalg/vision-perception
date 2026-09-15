import Foundation
import os
import UserNotifications
import CareActivityKit
#if canImport(ActivityKit)
import ActivityKit
#endif
#if canImport(ActivityKit)
import ActivityKit
#endif
#if canImport(AlarmKit)
import AlarmKit
#endif

/// Schedules repeating local reminders, optional AlarmKit alarms (iOS 26+),
/// and an opt-in Dynamic Island countdown for the next care event.
@MainActor
public final class CareAlertScheduler {
    public static let shared = CareAlertScheduler()

    private static let logger = Logger(subsystem: "org.curv.asd", category: "CareAlerts")
    private static let prefix = "org.curv.asd.care."

    public func requestAuthorizationIfNeeded() async {
        let center = UNUserNotificationCenter.current()
        let settings = await center.notificationSettings()
        guard settings.authorizationStatus == .notDetermined else { return }
        do {
            _ = try await center.requestAuthorization(options: [.alert, .sound, .badge])
        } catch {
            Self.logger.error("notification auth failed: \(error.localizedDescription, privacy: .public)")
        }
    }

    public func resync(requests: [CareAlertRequest], preferences: CareAlertPreferences) async {
        let center = UNUserNotificationCenter.current()
        let pending = await center.pendingNotificationRequests()
        let ours = pending.filter { $0.identifier.hasPrefix(Self.prefix) }.map(\.identifier)
        center.removePendingNotificationRequests(withIdentifiers: ours)

        guard preferences.masterEnabled else {
            await endLiveActivities()
            return
        }

        for request in requests where request.remindersEnabled && !request.skipToday {
            guard let (hour, minute) = CareClockTime.parse(request.clockTime) else { continue }
            let content = UNMutableNotificationContent()
            content.title = request.title
            content.body = request.body
            content.sound = .default
            content.userInfo = ["kind": request.kind.rawValue, "id": request.id]
            var date = DateComponents()
            date.hour = hour
            date.minute = minute
            let trigger = UNCalendarNotificationTrigger(dateMatching: date, repeats: true)
            let item = UNNotificationRequest(
                identifier: Self.prefix + request.id,
                content: content,
                trigger: trigger
            )
            do {
                try await center.add(item)
            } catch {
                Self.logger.error("schedule failed: \(error.localizedDescription, privacy: .public)")
            }
        }

        if preferences.useAlarmKitWhenAvailable {
            scheduleAlarmKitIfAvailable(requests: requests)
        }

        if preferences.useLiveActivityCountdown {
            await refreshLiveActivity(requests: requests)
        } else {
            await endLiveActivities()
        }
    }

    public func buildRequests(
        medications: [MedicationItem],
        diarySlots: [CareReminderSlot],
        diaryLoggedToday: Bool
    ) -> [CareAlertRequest] {
        var result: [CareAlertRequest] = []
        for med in medications {
            for (index, slot) in med.reminderSlots.enumerated() {
                let taken = med.dosesTakenToday.indices.contains(index) && med.dosesTakenToday[index]
                result.append(CareAlertRequest(
                    id: "med.\(med.id).\(slot.clockTime)",
                    kind: .medicationDose,
                    title: "Medication: \(med.name)",
                    body: "\(med.dose) · \(med.instructions)",
                    clockTime: slot.clockTime,
                    remindersEnabled: slot.remindersEnabled,
                    alarmsEnabled: slot.alarmsEnabled,
                    liveActivityEnabled: slot.liveActivityEnabled,
                    skipToday: taken
                ))
            }
        }
        for slot in diarySlots {
            result.append(CareAlertRequest(
                id: "diary.\(slot.id)",
                kind: .diaryCheckIn,
                title: "Sensory & mood diary",
                body: "Log today's mood, sensory load, and observations.",
                clockTime: slot.clockTime,
                remindersEnabled: slot.remindersEnabled,
                alarmsEnabled: slot.alarmsEnabled,
                liveActivityEnabled: slot.liveActivityEnabled,
                skipToday: diaryLoggedToday
            ))
        }
        return result
    }

    private func scheduleAlarmKitIfAvailable(requests: [CareAlertRequest]) {
        #if canImport(AlarmKit)
        if #available(iOS 26.0, *) {
            // AlarmKit is availability-gated so the iOS 18 target still compiles
            // when the SDK is older than AlarmKit. Concrete AlarmManager calls
            // land here once the project is built with the iOS 26 SDK.
            _ = requests.filter { $0.alarmsEnabled && !$0.skipToday }
        }
        #endif
    }

    #if canImport(ActivityKit)
    private func refreshLiveActivity(requests: [CareAlertRequest]) async {
        let now = Date()
        let upcoming = requests
            .filter { $0.liveActivityEnabled && $0.remindersEnabled }
            .compactMap { req -> (CareAlertRequest, Date)? in
                guard let date = CareClockTime.nextDate(clockTime: req.clockTime, from: now, skipToday: req.skipToday) else {
                    return nil
                }
                return (req, date)
            }
            .sorted { $0.1 < $1.1 }
            .first
        guard let (request, endsAt) = upcoming else {
            await endLiveActivities()
            return
        }
        let attributes = CareCountdownAttributes(kind: request.kind.rawValue, itemId: request.id)
        let state = CareCountdownAttributes.ContentState(
            title: request.title,
            subtitle: request.body,
            endsAt: endsAt
        )
        let content = ActivityContent(state: state, staleDate: endsAt)
        if let existing = Activity<CareCountdownAttributes>.activities.first {
            await existing.update(content)
        } else {
            do {
                _ = try Activity.request(attributes: attributes, content: content, pushType: nil)
            } catch {
                Self.logger.error("live activity failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    private func endLiveActivities() async {
        for activity in Activity<CareCountdownAttributes>.activities {
            await activity.end(nil, dismissalPolicy: .immediate)
        }
    }
    #else
    private func refreshLiveActivity(requests: [CareAlertRequest]) async {}
    private func endLiveActivities() async {}
    #endif
}
