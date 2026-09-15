import SwiftUI

/// Shared reminder-slot editor used by medication and diary sheets.
struct CareReminderSlotsEditor: View {
    @Binding var slots: [CareReminderSlot]
    var allowAlarmKit: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Reminders & alerts")
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                Spacer()
                Button {
                    slots.append(CareReminderSlot(clockTime: "20:00"))
                } label: {
                    Label("Add time", systemImage: "plus.circle.fill")
                        .font(.system(size: 13, weight: .semibold))
                }
                .buttonStyle(.plain)
                .foregroundColor(LiquidGlass.celestialTeal)
            }

            if slots.isEmpty {
                Text("No reminder times yet.")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
            }

            ForEach($slots) { $slot in
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        DatePicker(
                            "Time",
                            selection: Binding(
                                get: { date(from: slot.clockTime) },
                                set: { slot.clockTime = CareClockTime.format(hour: Calendar.current.component(.hour, from: $0), minute: Calendar.current.component(.minute, from: $0)) }
                            ),
                            displayedComponents: .hourAndMinute
                        )
                        .labelsHidden()
                        Spacer()
                        Button(role: .destructive) {
                            slots.removeAll { $0.id == slot.id }
                        } label: {
                            Image(systemName: "trash")
                        }
                        .buttonStyle(.plain)
                    }
                    Toggle("Notification reminder", isOn: $slot.remindersEnabled)
                    Toggle("Alarm (AlarmKit)", isOn: $slot.alarmsEnabled)
                        .disabled(!allowAlarmKit)
                    if !allowAlarmKit {
                        Text("AlarmKit requires iOS 26 on a device that supports Clock alarms.")
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                    }
                    Toggle("Dynamic Island countdown", isOn: $slot.liveActivityEnabled)
                }
                .padding(10)
                .background(LiquidGlass.cardBackground)
                .cornerRadius(10)
            }
        }
    }

    private func date(from clock: String) -> Date {
        let parts = CareClockTime.parse(clock) ?? (20, 0)
        return Calendar.current.date(bySettingHour: parts.hour, minute: parts.minute, second: 0, of: Date()) ?? Date()
    }
}
