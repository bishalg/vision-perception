import SwiftUI

struct DiaryHistoryEditorView: View {
    @Environment(\.dismiss) private var dismiss
    @ObservedObject var store: CarePlanStore

    @State private var selectedDate: Date = Date()
    @State private var draft = DailyDiaryEntry()
    @State private var reminderSlots: [CareReminderSlot] = []

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    DatePicker("Day", selection: $selectedDate, displayedComponents: .date)
                        .onChange(of: selectedDate) { _, newDate in
                            draft = store.diary(for: newDate)
                        }

                    previewStrip

                    Text("Mood")
                        .font(.system(size: 15, weight: .bold, design: .rounded))
                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 8) {
                        ForEach(ChildMood.allCases) { mood in
                            Button {
                                draft.mood = mood
                            } label: {
                                HStack {
                                    Text(mood.emoji)
                                    Text(mood.localizedLabel)
                                        .font(.system(size: 12, weight: .semibold))
                                        .lineLimit(1)
                                    Spacer()
                                    if draft.mood == mood {
                                        Image(systemName: "checkmark")
                                    }
                                }
                                .padding(10)
                                .background(
                                    draft.mood == mood
                                        ? LiquidGlass.celestialTeal.opacity(LiquidGlass.tintOpacity)
                                        : LiquidGlass.cardBackground
                                )
                                .cornerRadius(10)
                            }
                            .buttonStyle(.plain)
                            .foregroundColor(.primary)
                        }
                    }

                    stepperRow("Eye contact", value: $draft.eyeContactRating, range: 1...5)
                    stepperRow("Joint attention", value: $draft.jointAttentionCount, range: 0...20)
                    stepperRow("Sensory load", value: $draft.sensoryLoad, range: 1...5)

                    TextField("Notes", text: $draft.notes, axis: .vertical)
                        .lineLimit(3...6)
                        .textFieldStyle(.roundedBorder)

                    CareReminderSlotsEditor(
                        slots: $reminderSlots,
                        allowAlarmKit: CareAlertPreferences.isAlarmKitAvailable
                    )
                }
                .padding(20)
            }
            .background(
                LinearGradient(
                    colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .ignoresSafeArea()
            )
            .navigationTitle("Sensory diary")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                }
                ToolbarItem(placement: .bottomBar) {
                    Button("Clear this day", role: .destructive) {
                        store.deleteDiary(on: selectedDate)
                        draft = store.diary(for: selectedDate)
                    }
                }
            }
            .onAppear {
                draft = store.diary(for: selectedDate)
                reminderSlots = store.diaryReminderSlots
            }
        }
    }

    private var previewStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(store.recentDiaryEntries.prefix(14)) { entry in
                    Button {
                        selectedDate = entry.date
                        draft = store.diary(for: entry.date)
                    } label: {
                        VStack(spacing: 4) {
                            Text(shortDay(entry.date))
                                .font(.system(size: 11, weight: .bold))
                            Text(entry.mood.emoji)
                            Text(entry.hasCaregiverInput ? "Logged" : "—")
                                .font(.system(size: 9, weight: .semibold))
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                        .background(
                            Calendar.current.isDate(entry.date, inSameDayAs: selectedDate)
                                ? LiquidGlass.celestialTeal.opacity(LiquidGlass.tintOpacity)
                                : LiquidGlass.cardBackground
                        )
                        .cornerRadius(10)
                    }
                    .buttonStyle(.plain)
                    .foregroundColor(.primary)
                }
            }
        }
    }

    private func stepperRow(_ title: String, value: Binding<Int>, range: ClosedRange<Int>) -> some View {
        Stepper(value: value, in: range) {
            Text("\(title): \(value.wrappedValue)")
                .font(.system(size: 14, weight: .semibold))
        }
    }

    private func shortDay(_ date: Date) -> String {
        let fmt = DateFormatter()
        fmt.dateFormat = "d MMM"
        return fmt.string(from: date)
    }

    private func save() {
        var entry = draft
        entry.date = selectedDate
        store.upsertDiary(entry)
        let previousIds = Set(store.diaryReminderSlots.map(\.id))
        let nextIds = Set(reminderSlots.map(\.id))
        for slot in reminderSlots {
            store.upsertDiaryReminderSlot(slot)
        }
        for id in previousIds.subtracting(nextIds) {
            store.deleteDiaryReminderSlot(id: id)
        }
        dismiss()
        Task { await CareAlertScheduler.shared.requestAuthorizationIfNeeded() }
    }
}
