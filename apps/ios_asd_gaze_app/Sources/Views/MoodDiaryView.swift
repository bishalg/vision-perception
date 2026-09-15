import SwiftUI

/// HIG-aligned grouped list for caregiver mood logging (Health app style).
public struct MoodDiaryView: View {
    @StateObject private var model: MoodDiaryViewModel
    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var diaryStore = MoodDiaryStore.shared

    public init(suggestion: MoodDiarySuggestion? = nil) {
        _model = StateObject(wrappedValue: MoodDiaryViewModel(prefilling: suggestion))
    }

    public var body: some View {
        NavigationStack {
            List {
                if let seconds = model.suggestionSeconds {
                    Section {
                        Text("We noticed some distress earlier. Would you like to log this?")
                            .font(.body)
                        Text("Camera-based estimate: \(Int(seconds.rounded())) seconds of distress signature (brow raise + frown). This is not a diagnosis.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }

                Section {
                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                        ForEach(PediatricEmotion.allCases) { emotion in
                            emotionButton(emotion)
                        }
                    }
                    .listRowInsets(EdgeInsets(top: 12, leading: 16, bottom: 12, trailing: 16))
                    .listRowBackground(Color.clear)
                } header: {
                    Text("Emotion")
                } footer: {
                    Text("Labels follow CDC social-emotional themes (joy, engagement, overload, distress). Choose what the caregiver observed.")
                }

                Section("Intensity") {
                    VStack(alignment: .leading, spacing: 8) {
                        Slider(value: $model.intensity, in: 0...1, step: 0.05)
                            .accessibilityLabel("Intensity")
                            .accessibilityValue("\(Int(model.intensity * 100)) percent")
                        Text(intensityCaption)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }

                Section("Duration") {
                    VStack(alignment: .leading, spacing: 8) {
                        Slider(value: $model.durationMinutes, in: 0.5...60, step: 0.5)
                            .accessibilityLabel("Duration in minutes")
                        Text("\(model.durationMinutes, specifier: "%.1f") minutes")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }

                Section("Context tags") {
                    ForEach(MoodContextTag.allCases) { tag in
                        Button {
                            model.toggle(tag)
                        } label: {
                            HStack {
                                Text(tag.rawValue)
                                    .foregroundStyle(.primary)
                                Spacer()
                                if model.selectedTags.contains(tag) {
                                    Image(systemName: "checkmark.circle.fill")
                                        .foregroundStyle(Color.accentColor)
                                }
                            }
                        }
                        .accessibilityAddTraits(model.selectedTags.contains(tag) ? [.isSelected] : [])
                    }
                }

                Section("Notes") {
                    TextField("Optional caregiver notes", text: $model.notes, axis: .vertical)
                        .lineLimit(3...6)
                }

                if !diaryStore.recentEntries.isEmpty {
                    Section("Recent logs") {
                        ForEach(diaryStore.recentEntries.prefix(8)) { entry in
                            VStack(alignment: .leading, spacing: 4) {
                                Text(entry.emotion.title)
                                    .font(.headline)
                                Text(entry.loggedAt.formatted(date: .abbreviated, time: .shortened))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                if !entry.tags.isEmpty {
                                    Text(entry.tags.map(\.rawValue).joined(separator: " · "))
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Mood & Emotion")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        Task {
                            await model.save()
                            dismiss()
                        }
                    }
                    .fontWeight(.semibold)
                }
            }
        }
    }

    private var intensityCaption: String {
        switch model.intensity {
        case ..<0.34: return "Mild"
        case ..<0.67: return "Moderate"
        default: return "Intense"
        }
    }

    private func emotionButton(_ emotion: PediatricEmotion) -> some View {
        let selected = model.emotion == emotion
        return Button {
            model.emotion = emotion
        } label: {
            VStack(alignment: .leading, spacing: 6) {
                Image(systemName: emotion.systemImage)
                    .font(.title2)
                Text(emotion.title)
                    .font(.headline)
                Text(emotion.subtitle)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(selected ? Color.accentColor.opacity(0.15) : Color(.secondarySystemGroupedBackground))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .strokeBorder(selected ? Color.accentColor : Color.clear, lineWidth: 2)
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(emotion.title)
        .accessibilityAddTraits(selected ? [.isSelected] : [])
    }
}
