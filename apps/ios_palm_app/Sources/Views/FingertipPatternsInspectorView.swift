import SwiftUI
import CurvCameraOverlay

/**
 * Inspection and fine-tuning view for captured fingertip dermatoglyphic patterns.
 * Displays each digit's captured pattern class (Arch, Loop, Whorl, Unknown),
 * ridge frequency metric, quality score, and allows manual fine-tuning/override.
 */
public struct FingertipPatternsInspectorView: View {
    public let session: FingertipScanSession
    public let onPatternOverride: ((FingertipDigit, DermatoglyphicPattern) -> Void)?

    public init(
        session: FingertipScanSession,
        onPatternOverride: ((FingertipDigit, DermatoglyphicPattern) -> Void)? = nil
    ) {
        self.session = session
        self.onPatternOverride = onPatternOverride
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                // Header summary card
                VStack(spacing: 8) {
                    HStack {
                        Image(systemName: "touchid")
                            .font(.system(size: 24))
                            .foregroundColor(OverlayPalette.emerald)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Dermatoglyphic Analysis")
                                .font(.system(size: 17, weight: .bold, design: .rounded))
                                .foregroundColor(Color.primary)
                            Text("\(session.scannedDigits.count) of 5 digits scanned")
                                .font(.system(size: 13, weight: .medium, design: .rounded))
                                .foregroundColor(Color.secondary)
                        }
                        Spacer()
                    }
                }
                .padding(16)
                .background(
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .fill(Color(uiColor: .secondarySystemGroupedBackground))
                        .overlay(
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .stroke(Color(uiColor: .separator).opacity(0.3), lineWidth: 0.8)
                        )
                )

                // Individual Digit Cards
                VStack(spacing: 10) {
                    ForEach(FingertipDigit.allCases, id: \.self) { digit in
                        digitRow(digit)
                    }
                }
            }
            .padding(16)
        }
        .background(Color(uiColor: .systemGroupedBackground).ignoresSafeArea())
    }

    private func digitRow(_ digit: FingertipDigit) -> some View {
        let result = session.results.first { $0.digit == digit }
        let pattern = result?.pattern ?? .unknown
        let quality = result?.quality ?? 0.0

        return VStack(alignment: .leading, spacing: 10) {
            HStack {
                HStack(spacing: 8) {
                    Circle()
                        .fill(result != nil ? OverlayPalette.emerald : Color.secondary.opacity(0.3))
                        .frame(width: 8, height: 8)

                    Text(digit.displayName)
                        .font(.system(size: 15, weight: .semibold, design: .rounded))
                        .foregroundColor(Color.primary)
                }

                Spacer()

                if result != nil {
                    HStack(spacing: 4) {
                        Text("Quality: \(Int(quality * 100))%")
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundColor(Color.secondary)
                    }
                } else {
                    Text("Pending Scan")
                        .font(.system(size: 12, weight: .regular, design: .rounded))
                        .foregroundColor(Color.secondary)
                }
            }

            // Pattern classification selector
            HStack(spacing: 8) {
                ForEach([DermatoglyphicPattern.arch, .loop, .whorl], id: \.self) { p in
                    let isSelected = (pattern == p)
                    Button {
                        onPatternOverride?(digit, p)
                    } label: {
                        HStack(spacing: 4) {
                            patternIcon(for: p)
                                .font(.system(size: 11))
                            Text(p.rawValue.capitalized)
                                .font(.system(size: 12, weight: isSelected ? .bold : .medium, design: .rounded))
                        }
                        .foregroundColor(isSelected ? .white : Color.secondary)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 8)
                        .background(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(isSelected ? OverlayPalette.emerald : Color(uiColor: .tertiarySystemGroupedBackground))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                                        .stroke(isSelected ? OverlayPalette.emerald : Color(uiColor: .separator).opacity(0.2), lineWidth: 0.8)
                                )
                        )
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(Color(uiColor: .secondarySystemGroupedBackground))
                .overlay(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .stroke(result != nil ? OverlayPalette.emerald.opacity(0.35) : Color(uiColor: .separator).opacity(0.25), lineWidth: 0.8)
                )
        )
    }

    @ViewBuilder
    private func patternIcon(for pattern: DermatoglyphicPattern) -> some View {
        switch pattern {
        case .arch:
            Image(systemName: "triangle")
        case .loop:
            Image(systemName: "arrow.triangle.capsulepath")
        case .whorl:
            Image(systemName: "circle.circle")
        case .unknown:
            Image(systemName: "questionmark")
        }
    }
}
