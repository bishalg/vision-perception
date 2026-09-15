import SwiftUI

struct MedicationEditorView: View {
    @Environment(\.dismiss) private var dismiss
    @ObservedObject var store: CarePlanStore
    @ObservedObject var profileStore = ChildProfileStore.shared

    let existing: MedicationItem?

    @State private var name: String = ""
    @State private var dose: String = ""
    @State private var instructions: String = ""
    @State private var notes: String = ""
    @State private var scheduleDates: [Date] = []
    @State private var reminderSlots: [CareReminderSlot] = []
    @State private var linkedConditionId: String = ""
    @State private var newConditionTitle: String = ""

    init(store: CarePlanStore, existing: MedicationItem?) {
        self.store = store
        self.existing = existing
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    labeledField("Name", text: $name)
                    labeledField("Dose", text: $dose)
                    labeledField("Instructions", text: $instructions)
                    labeledField("Notes", text: $notes)

                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Dose times")
                                .font(.system(size: 15, weight: .bold, design: .rounded))
                            Spacer()
                            Button {
                                scheduleDates.append(Date())
                            } label: {
                                Label("Add", systemImage: "plus.circle.fill")
                                    .font(.system(size: 13, weight: .semibold))
                            }
                            .buttonStyle(.plain)
                            .foregroundColor(LiquidGlass.celestialTeal)
                        }
                        ForEach(scheduleDates.indices, id: \.self) { index in
                            HStack {
                                DatePicker("Dose", selection: $scheduleDates[index], displayedComponents: .hourAndMinute)
                                    .labelsHidden()
                                Button(role: .destructive) {
                                    scheduleDates.remove(at: index)
                                } label: {
                                    Image(systemName: "trash")
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Text("Linked medical condition")
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                        Picker("Condition", selection: $linkedConditionId) {
                            Text("None").tag("")
                            ForEach(profileStore.profile.medicalConditions.filter(\.isActive)) { condition in
                                Text(condition.title).tag(condition.id)
                            }
                        }
                        HStack {
                            TextField("Add condition", text: $newConditionTitle)
                                .textFieldStyle(.roundedBorder)
                            Button("Add") {
                                let title = newConditionTitle.trimmingCharacters(in: .whitespacesAndNewlines)
                                guard !title.isEmpty else { return }
                                var profile = profileStore.profile
                                let condition = MedicalCondition(title: title)
                                profile.medicalConditions.append(condition)
                                profileStore.profile = profile
                                linkedConditionId = condition.id
                                newConditionTitle = ""
                            }
                        }
                        Text("Conditions are caregiver notes, not a diagnosis from this app.")
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                    }

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
            .navigationTitle(existing == nil ? "Add medicine" : "Edit medicine")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                        .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || scheduleDates.isEmpty)
                }
                if existing != nil {
                    ToolbarItem(placement: .bottomBar) {
                        Button("Delete medicine", role: .destructive) {
                            if let existing {
                                store.deleteMedication(id: existing.id)
                            }
                            dismiss()
                        }
                    }
                }
            }
            .onAppear { hydrate() }
        }
    }

    private func labeledField(_ title: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundColor(.secondary)
            TextField(title, text: text)
                .textFieldStyle(.roundedBorder)
        }
    }

    private func hydrate() {
        let item = existing ?? MedicationItem(
            id: UUID().uuidString,
            name: "",
            dose: "",
            instructions: "",
            schedule: ["08:00"]
        )
        name = item.name
        dose = item.dose
        instructions = item.instructions
        notes = item.notes
        linkedConditionId = item.linkedConditionId ?? ""
        scheduleDates = item.schedule.compactMap { clock in
            guard let parsed = CareClockTime.parse(clock) else { return nil }
            return Calendar.current.date(bySettingHour: parsed.hour, minute: parsed.minute, second: 0, of: Date())
        }
        if scheduleDates.isEmpty {
            scheduleDates = [Date()]
        }
        reminderSlots = item.reminderSlots
    }

    private func save() {
        let times = scheduleDates.map {
            CareClockTime.format(
                hour: Calendar.current.component(.hour, from: $0),
                minute: Calendar.current.component(.minute, from: $0)
            )
        }
        var item = existing ?? MedicationItem(
            id: UUID().uuidString, name: name, dose: dose, instructions: instructions, schedule: times
        )
        item.name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        item.dose = dose
        item.instructions = instructions
        item.notes = notes
        item.linkedConditionId = linkedConditionId.isEmpty ? nil : linkedConditionId
        item.setSchedule(times)
        item.reminderSlots = reminderSlots.synced(to: times)
        store.upsertMedication(item)
        dismiss()
        Task { await CareAlertScheduler.shared.requestAuthorizationIfNeeded() }
    }
}
