import SwiftUI

/// Liquid Glass implementation of the Apple ResearchKit Informed e-Consent flow
/// (`ORKVisualConsentStep` & `ORKConsentReviewStep`).
public struct ResearchStudyConsentView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var guardianName: String = ""
    @State private var childAgeMonths: Int = 24
    @State private var activeSectionIndex: Int = 0
    @State private var hasSigned: Bool = false
    @State private var drawnPaths: [Path] = []
    @State private var currentPath: Path = Path()
    @State private var isCompleted: Bool = false

    private let document = ORKConsentDocument()
    private let onConsentCompleted: ((ResearchStudyEnrollment) -> Void)?

    public init(onConsentCompleted: ((ResearchStudyEnrollment) -> Void)? = nil) {
        self.onConsentCompleted = onConsentCompleted
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        headerSection
                        protocolBadgeSection
                        sectionTabs
                        currentSectionContent
                        Divider().background(LiquidGlass.dividerColor)
                        signatureReviewSection
                        actionButtons
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Research Study e-Consent")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .foregroundColor(.secondary)
                }
            }
        }
    }

    // MARK: - Header & Badges

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(document.title)
                .font(.system(size: 20, weight: .bold, design: .rounded))
                .foregroundColor(.primary)

            Text("Administered under Apple ResearchKit guidelines for pediatric observational studies.")
                .font(.system(size: 13))
                .foregroundColor(.secondary)
        }
    }

    private var protocolBadgeSection: some View {
        HStack(spacing: 12) {
            Label(document.irbProtocolId, systemImage: "checkmark.shield.fill")
                .font(.system(size: 11, weight: .bold))
                .foregroundColor(LiquidGlass.celestialTeal)
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(0.12)))

            Label("Zero Raw Video", systemImage: "eye.slash.fill")
                .font(.system(size: 11, weight: .bold))
                .foregroundColor(LiquidGlass.emeraldMint)
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(Capsule().fill(LiquidGlass.emeraldMint.opacity(0.12)))
        }
    }

    // MARK: - Section Tabs & Carousel

    private var sectionTabs: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(document.sections.indices, id: \.self) { idx in
                    let sec = document.sections[idx]
                    Button {
                        activeSectionIndex = idx
                    } label: {
                        Text(sec.title)
                            .font(.system(size: 12, weight: activeSectionIndex == idx ? .bold : .medium))
                            .padding(.horizontal, 12)
                            .padding(.vertical, 7)
                            .background(
                                Capsule().fill(
                                    activeSectionIndex == idx
                                        ? LiquidGlass.celestialTeal.opacity(0.20)
                                        : LiquidGlass.cardBackground
                                )
                            )
                            .foregroundColor(activeSectionIndex == idx ? LiquidGlass.celestialTeal : .primary)
                    }
                }
            }
        }
    }

    private var currentSectionContent: some View {
        let sec = document.sections[activeSectionIndex]
        return VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(sec.title)
                    .font(.system(size: 16, weight: .bold))
                    .foregroundColor(.primary)
                Spacer()
                Text("\(activeSectionIndex + 1) of \(document.sections.count)")
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundColor(.secondary)
            }

            Text(sec.summary)
                .font(.system(size: 13, weight: .semibold))
                .foregroundColor(LiquidGlass.warmAmber)

            Text(sec.content)
                .font(.system(size: 13))
                .foregroundColor(.secondary)
                .lineSpacing(4)
        }
        .glassCard(cornerRadius: 16, padding: 16)
    }

    // MARK: - Signature Capture & Review

    private var signatureReviewSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Legal Guardian Digital Signature")
                .font(.system(size: 16, weight: .bold, design: .rounded))
                .foregroundColor(.primary)

            VStack(alignment: .leading, spacing: 6) {
                Text("Legal Guardian Full Name")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(.secondary)

                TextField("e.g. Jane Doe", text: $guardianName)
                    .textFieldStyle(.plain)
                    .padding(12)
                    .background(RoundedRectangle(cornerRadius: 10).fill(LiquidGlass.cardBackground))
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(LiquidGlass.cardBorder, lineWidth: 1))
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("Child Age (Months)")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(.secondary)

                Stepper("\(childAgeMonths) months", value: $childAgeMonths, in: 12...60)
                    .padding(10)
                    .background(RoundedRectangle(cornerRadius: 10).fill(LiquidGlass.cardBackground))
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(LiquidGlass.cardBorder, lineWidth: 1))
            }

            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("Digital Signature")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(.secondary)
                    Spacer()
                    if !drawnPaths.isEmpty {
                        Button("Clear") {
                            drawnPaths.removeAll()
                            hasSigned = false
                        }
                        .font(.system(size: 11, weight: .bold))
                        .foregroundColor(LiquidGlass.warmAmber)
                    }
                }

                // Interactive touch signature canvas
                ZStack {
                    RoundedRectangle(cornerRadius: 12)
                        .fill(LiquidGlass.cardBackground)
                        .overlay(RoundedRectangle(cornerRadius: 12).stroke(LiquidGlass.cardBorder, lineWidth: 1))

                    if drawnPaths.isEmpty && currentPath.isEmpty {
                        Text("Sign your name here with your finger")
                            .font(.system(size: 12))
                            .foregroundColor(.secondary.opacity(0.6))
                    }

                    ForEach(drawnPaths.indices, id: \.self) { i in
                        drawnPaths[i].stroke(LiquidGlass.celestialTeal, lineWidth: 2.5)
                    }
                    currentPath.stroke(LiquidGlass.celestialTeal, lineWidth: 2.5)
                }
                .frame(height: 120)
                .gesture(
                    DragGesture(minimumDistance: 0.1)
                        .onChanged { value in
                            if currentPath.isEmpty {
                                currentPath.move(to: value.location)
                            } else {
                                currentPath.addLine(to: value.location)
                            }
                            hasSigned = true
                        }
                        .onEnded { _ in
                            drawnPaths.append(currentPath)
                            currentPath = Path()
                        }
                )
            }
        }
        .glassCard(cornerRadius: 16, padding: 16)
    }

    // MARK: - Action Buttons

    private var actionButtons: some View {
        VStack(spacing: 10) {
            Button(action: submitConsent) {
                HStack {
                    Image(systemName: "signature")
                    Text("Provide Digital Consent & Enroll")
                        .fontWeight(.semibold)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .background(canSubmit ? LiquidGlass.celestialTeal : Color.gray.opacity(0.3))
                .foregroundColor(canSubmit ? Color.black : Color.white.opacity(0.5)) // theme:fixed-media
                .clipShape(RoundedRectangle(cornerRadius: 14))
            }
            .disabled(!canSubmit)

            Text("By tapping provide digital consent, you agree to study participation. Consent can be revoked anytime in settings.")
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
    }

    private var canSubmit: Bool {
        !guardianName.trimmingCharacters(in: .whitespaces).isEmpty && hasSigned
    }

    private func submitConsent() {
        let signatureToken = "SIG-" + UUID().uuidString.prefix(12).uppercased()
        let sig = ORKConsentSignature(
            guardianFullName: guardianName.trimmingCharacters(in: .whitespaces),
            signatureToken: signatureToken
        )

        ResearchKitStudyStore.shared.saveConsent(
            signature: sig,
            childAgeMonths: childAgeMonths
        ) { enrollment in
            onConsentCompleted?(enrollment)
            dismiss()
        }
    }
}
