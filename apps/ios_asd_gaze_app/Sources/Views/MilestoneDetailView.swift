import SwiftUI

public struct MilestoneDetailView: View {
    public let milestoneId: String
    @ObservedObject var store: CarePlanStore
    @Environment(\.dismiss) private var dismiss

    public init(milestoneId: String, store: CarePlanStore = .shared) {
        self.milestoneId = milestoneId
        self._store = ObservedObject(wrappedValue: store)
    }

    private var milestone: Milestone? {
        store.milestones.first { $0.id == milestoneId }
    }

    public var body: some View {
        NavigationStack {
            if let milestone {
                List {
                    Section {
                        LabeledContent("Surveillance age", value: "\(milestone.ageMonths) months")
                        LabeledContent("Domain", value: milestone.domain.rawValue)
                        LabeledContent("Window", value: milestone.dueDateString)
                        ProgressView(value: milestone.completionRatio)
                        ClinicalTermTooltip(term: ClinicalGlossary.pointingContrast)
                    } header: {
                        Text("Risk context")
                    } footer: {
                        Text("These checks support well-child surveillance. They are not a diagnosis.")
                    }

                    Section("Educational checklist") {
                        ForEach(milestone.items) { item in
                            DisclosureGroup {
                                VStack(alignment: .leading, spacing: 10) {
                                    labeled("What to look for", item.whatToLookFor)
                                    labeled("How to prompt this at home", item.howToPrompt)
                                    labeled("When to consult your pediatrician", item.whenToConsult)
                                }
                                .padding(.vertical, 4)
                            } label: {
                                Button {
                                    store.toggleMilestoneItem(milestoneId: milestone.id, itemId: item.id)
                                } label: {
                                    Label(item.title, systemImage: item.isCompleted ? "checkmark.circle.fill" : "circle")
                                        .foregroundStyle(.primary)
                                        .multilineTextAlignment(.leading)
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                }
                .listStyle(.insetGrouped)
                .navigationTitle(milestone.title)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") { dismiss() }
                    }
                }
            }
        }
    }

    private func labeled(_ title: String, _ body: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            Text(body).font(.subheadline)
        }
    }
}

public struct DevelopmentalMilestonesSectionView: View {
    public let milestones: [Milestone]
    public let onSelect: (Milestone) -> Void

    public init(milestones: [Milestone], onSelect: @escaping (Milestone) -> Void) {
        self.milestones = milestones
        self.onSelect = onSelect
    }

    public var body: some View {
        ForEach(CDCSurveillanceAge.allCases) { age in
            let group = milestones.filter { $0.ageMonths == age.rawValue }
            if !group.isEmpty {
                Section("\(age.rawValue)-month CDC / AAP") {
                    ForEach(group) { milestone in
                        Button {
                            onSelect(milestone)
                        } label: {
                            HStack {
                                Image(systemName: milestone.domain.systemImage)
                                VStack(alignment: .leading) {
                                    Text(milestone.domain.shortLabel)
                                    Text("\(Int(milestone.completionRatio * 100))% of checks")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Image(systemName: milestone.isCompleted ? "checkmark.circle.fill" : "chevron.right")
                                    .foregroundStyle(milestone.isCompleted ? .green : .secondary)
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Compatibility wrapper for the previous modal name.
public struct MilestoneDetailModalView: View {
    public let milestone: Milestone

    public init(milestone: Milestone, onToggle: @escaping (String) -> Void = { _ in }) {
        self.milestone = milestone
    }

    public var body: some View {
        MilestoneDetailView(milestoneId: milestone.id)
    }
}
