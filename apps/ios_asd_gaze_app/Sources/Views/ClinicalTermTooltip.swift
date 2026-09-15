import SwiftUI

public struct ClinicalTermTooltip: View {
    public let term: ClinicalTerm
    @State private var show = false

    public init(term: ClinicalTerm) {
        self.term = term
    }

    public var body: some View {
        Button {
            show.toggle()
        } label: {
            HStack(spacing: 4) {
                Text(term.term)
                    .underline()
                Image(systemName: "info.circle")
                    .font(.caption)
            }
            .font(.subheadline.weight(.semibold))
        }
        .buttonStyle(.plain)
        .foregroundStyle(.tint)
        .popover(isPresented: $show) {
            VStack(alignment: .leading, spacing: 8) {
                Text(term.term).font(.headline)
                Text(term.explanation).font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .padding()
            .frame(maxWidth: 280)
            .presentationCompactAdaptation(.popover)
        }
        .accessibilityHint("Shows clinical explanation")
    }
}
