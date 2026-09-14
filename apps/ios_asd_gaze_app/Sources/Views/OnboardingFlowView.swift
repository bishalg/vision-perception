import SwiftUI

/// 3-Step Liquid Glass onboarding experience for pediatric ASD gaze screening:
/// Step 1 → Privacy Impact Assessment (PIA), HIPAA/COPPA Security, Zero-Video Consent
/// Step 2 → Child Profile Setup (Name, 5-Tier Granular Age Bracket, Language)
/// Step 3 → Environmental Guidance (Distance sweet spot 45–55 cm, ambient lighting)
public struct OnboardingFlowView: View {
    @ObservedObject private var profileStore = ChildProfileStore.shared
    @State private var currentStep: Int = 1
    @State private var childNameInput: String = ""
    @State private var selectedAgeBracket: PediatricAgeBracket = .months18_23
    @State private var selectedLanguage: String = "en"
    @State private var consentAgreed: Bool = false
    @State private var showLanguagePickerSheet: Bool = false

    public let onCompleted: () -> Void

    public init(onCompleted: @escaping () -> Void) {
        self.onCompleted = onCompleted
    }

    public var body: some View {
        ZStack {
            LinearGradient(
                colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()

            VStack(spacing: 0) {
                // Top Step Progress Indicator Bar
                stepProgressBar
                    .padding(.horizontal, 24)
                    .padding(.top, 16)
                    .padding(.bottom, 12)

                // Main Content Carousel
                ScrollView {
                    VStack(alignment: .leading, spacing: 20) {
                        switch currentStep {
                        case 1:
                            step1PrivacyAndConsentView
                        case 2:
                            step2ChildProfileView
                        case 3:
                            step3EnvironmentGuidanceView
                        default:
                            EmptyView()
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 24)
                    .padding(.vertical, 8)
                }

                // Anchored Bottom Navigation Controls
                bottomActionBar
                    .padding(.horizontal, 24)
                    .padding(.vertical, 20)
            }
        }
        .sheet(isPresented: $showLanguagePickerSheet) {
            LanguageSelectionSheetView(selectedLanguage: $selectedLanguage)
        }
        .onAppear {
            childNameInput = profileStore.profile.childName
            selectedAgeBracket = profileStore.profile.ageBracket
            selectedLanguage = profileStore.profile.preferredLanguage
            consentAgreed = profileStore.profile.hasConsentedPIA
        }
    }

    // MARK: - Top Step Progress Bar

    private var stepProgressBar: some View {
        HStack(spacing: 8) {
            ForEach(1...3, id: \.self) { step in
                Capsule()
                    .fill(
                        step <= currentStep
                            ? LiquidGlass.celestialTeal
                            : Color.white.opacity(0.15)
                    )
                    .frame(height: 5)
                    .animation(.easeInOut(duration: 0.25), value: currentStep)
            }
        }
    }

    // MARK: - Step 1: Privacy Impact Assessment (PIA) & Security

    private var step1PrivacyAndConsentView: some View {
        VStack(alignment: .leading, spacing: 18) {
            // Header
            VStack(alignment: .leading, spacing: 6) {
                Text("Privacy & Clinical Security")
                    .font(.system(size: 24, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textHeadline)

                Text("Your child’s privacy and biometrics are protected by hardware encryption and zero-video storage.")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundColor(LiquidGlass.textSecondary)
                    .lineLimit(nil)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            // Security Cards (Equal Width)
            VStack(spacing: 12) {
                piaFeatureRow(
                    icon: "video.slash.fill",
                    color: LiquidGlass.emeraldMint,
                    title: "Zero Raw Video Recording",
                    description: "The TrueDepth infrared camera processes facial landmarks exclusively in volatile RAM. No videos or photos of your child are ever recorded or stored."
                )

                piaFeatureRow(
                    icon: "lock.shield.fill",
                    color: LiquidGlass.celestialTeal,
                    title: "Hardware-Bound Encryption",
                    description: "All gaze telemetry is encrypted with Apple CryptoKit (AES-GCM-256) tied directly to this device's Secure Enclave."
                )

                piaFeatureRow(
                    icon: "person.badge.shield.checkmark.fill",
                    color: LiquidGlass.warmAmber,
                    title: "De-Identified Clinical Tokens",
                    description: "Screening sessions use randomized UUID hashes. Your child's personal identity is never attached to exported telemetry."
                )
            }

            // Consent Agreement Toggle
            Toggle(isOn: $consentAgreed) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Informed Consent Agreement")
                        .font(.system(size: 14, weight: .bold, design: .rounded))
                        .foregroundColor(LiquidGlass.textPrimary)

                    Text("I understand and authorize digital gaze tracking for developmental observation.")
                        .font(.system(size: 12))
                        .foregroundColor(LiquidGlass.textSecondary)
                        .lineLimit(nil)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .tint(LiquidGlass.emeraldMint)
            .frame(maxWidth: .infinity, alignment: .leading)
            .glassCard(cornerRadius: 16, padding: 14)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: - Step 2: Child Profile & Developmental Tier

    private var step2ChildProfileView: some View {
        VStack(alignment: .leading, spacing: 20) {
            // Header
            VStack(alignment: .leading, spacing: 6) {
                Text("Child Profile Setup")
                    .font(.system(size: 24, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textHeadline)

                Text("Personalizing age-calibrated visual stimuli and language prompts.")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundColor(LiquidGlass.textSecondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            // Child Name Input Card
            VStack(alignment: .leading, spacing: 8) {
                Text("Child's First Name or Nickname")
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .foregroundColor(LiquidGlass.textPrimary)

                HStack {
                    Image(systemName: "person.fill")
                        .foregroundColor(LiquidGlass.celestialTeal)
                    TextField("e.g. Aarav, Maya, Leo", text: $childNameInput)
                        .font(.system(size: 16, weight: .medium))
                        .foregroundColor(.primary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .glassCard(cornerRadius: 14, padding: 12)

                Text("Used for future voice-synthesized 'Response to Name' audio tests.")
                    .font(.system(size: 11, weight: .regular))
                    .foregroundColor(LiquidGlass.textSecondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            // Granular Age Tier Selection Cards
            VStack(alignment: .leading, spacing: 8) {
                Text("Developmental Age Bracket")
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .foregroundColor(LiquidGlass.textPrimary)

                VStack(spacing: 8) {
                    ForEach(PediatricAgeBracket.allCases) { bracket in
                        Button {
                            selectedAgeBracket = bracket
                        } label: {
                            HStack(spacing: 12) {
                                Image(systemName: selectedAgeBracket == bracket ? "checkmark.circle.fill" : "circle")
                                    .foregroundColor(selectedAgeBracket == bracket ? LiquidGlass.emeraldMint : LiquidGlass.textTertiary)
                                    .font(.system(size: 18))

                                VStack(alignment: .leading, spacing: 2) {
                                    HStack {
                                        Text(bracket.rawValue)
                                            .font(.system(size: 15, weight: .bold, design: .rounded))
                                            .foregroundColor(LiquidGlass.textPrimary)
                                        Text("• \(bracket.developmentalStage)")
                                            .font(.system(size: 12, weight: .medium))
                                            .foregroundColor(LiquidGlass.textSecondary)
                                    }
                                    Text(bracket.clinicalFocus)
                                        .font(.system(size: 11))
                                        .foregroundColor(LiquidGlass.textSecondary)
                                }

                                Spacer(minLength: 0)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .glassCard(cornerRadius: 12, padding: 12)
                            .overlay(
                                RoundedRectangle(cornerRadius: 12)
                                    .stroke(
                                        selectedAgeBracket == bracket ? LiquidGlass.celestialTeal.opacity(0.6) : Color.clear,
                                        lineWidth: 1.5
                                    )
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            // Multi-Language Selection Section
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Primary Language Spoken at Home")
                        .font(.system(size: 13, weight: .semibold, design: .rounded))
                        .foregroundColor(LiquidGlass.textPrimary)

                    Spacer()

                    Button {
                        showLanguagePickerSheet = true
                    } label: {
                        HStack(spacing: 4) {
                            Text("All Languages")
                                .font(.system(size: 12, weight: .semibold, design: .rounded))
                            Image(systemName: "chevron.right")
                                .font(.system(size: 10, weight: .bold))
                        }
                        .foregroundColor(LiquidGlass.celestialTeal)
                    }
                }

                // Quick Top 4 Language Chips (Equal Width)
                HStack(spacing: 8) {
                    ForEach(ScreeningLanguages.primaryCodes, id: \.self) { code in
                        let item = ScreeningLanguages.item(for: code)
                        languageChip(code: code, label: "\(item.flag) \(item.nativeName)")
                    }
                }
                .frame(maxWidth: .infinity)

                // Active Language Details & Sheet Navigation Card
                Button {
                    showLanguagePickerSheet = true
                } label: {
                    HStack(spacing: 12) {
                        ZStack {
                            Circle()
                                .fill(LiquidGlass.celestialTeal.opacity(0.15))
                                .frame(width: 36, height: 36)
                            Image(systemName: "globe")
                                .font(.system(size: 16, weight: .semibold))
                                .foregroundColor(LiquidGlass.celestialTeal)
                        }

                        VStack(alignment: .leading, spacing: 2) {
                            HStack(spacing: 6) {
                                Text(ScreeningLanguages.item(for: selectedLanguage).flag)
                                Text(ScreeningLanguages.item(for: selectedLanguage).displayName)
                                    .font(.system(size: 14, weight: .bold, design: .rounded))
                                    .foregroundColor(LiquidGlass.textPrimary)
                            }
                            Text("\(ScreeningLanguages.item(for: selectedLanguage).region) • Tap to select from 18 languages")
                                .font(.system(size: 11, weight: .regular))
                                .foregroundColor(LiquidGlass.textSecondary)
                        }

                        Spacer(minLength: 0)

                        HStack(spacing: 4) {
                            Text("Change")
                                .font(.system(size: 12, weight: .semibold, design: .rounded))
                                .foregroundColor(LiquidGlass.celestialTeal)
                            Image(systemName: "chevron.right")
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundColor(LiquidGlass.celestialTeal)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .glassCard(cornerRadius: 12, padding: 12)
                }
                .buttonStyle(.plain)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: - Step 3: Screening Environment Guidance

    private var step3EnvironmentGuidanceView: some View {
        VStack(alignment: .leading, spacing: 20) {
            // Header
            VStack(alignment: .leading, spacing: 6) {
                Text("Screening Environment")
                    .font(.system(size: 24, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textHeadline)

                Text("Set up optimal conditions before seating your child for visual tracking.")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundColor(LiquidGlass.textSecondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            // Equal-Width Clinical Guidance Cards
            VStack(spacing: 12) {
                piaFeatureRow(
                    icon: "ruler.fill",
                    color: LiquidGlass.celestialTeal,
                    title: "Viewing Distance: 45–55 cm",
                    description: "Hold the iPhone at child eye-level approximately an arm's length away. The app automatically verifies face distance before testing."
                )

                piaFeatureRow(
                    icon: "sun.max.fill",
                    color: LiquidGlass.warmAmber,
                    title: "Diffused Room Lighting",
                    description: "Ensure the room is well-lit with soft ambient light. Avoid bright windows or lamps directly behind the child."
                )

                piaFeatureRow(
                    icon: "speaker.wave.2.fill",
                    color: LiquidGlass.emeraldMint,
                    title: "Balanced Audio Volume",
                    description: "Set speaker volume to a comfortable, clear level for synchronized nursery rhymes and name orienting cues."
                )
            }

            // Upgraded Full-Width Configured Profile Card
            configuredProfileCard
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: - Configured Profile Card (Redesigned)

    private var configuredProfileCard: some View {
        let languageItem = ScreeningLanguages.item(for: selectedLanguage)
        let childName = childNameInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "Participant" : childNameInput

        return VStack(alignment: .leading, spacing: 14) {
            // Header with status indicator
            HStack {
                Label("CONFIGURED PROFILE PREVIEW", systemImage: "person.text.rectangle.fill")
                    .font(.system(size: 11, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.celestialTeal)

                Spacer(minLength: 0)

                HStack(spacing: 5) {
                    Circle()
                        .fill(LiquidGlass.emeraldMint)
                        .frame(width: 7, height: 7)
                    Text("Ready for Gaze")
                        .font(.system(size: 11, weight: .bold, design: .rounded))
                        .foregroundColor(LiquidGlass.emeraldMint)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(Capsule().fill(LiquidGlass.emeraldMint.opacity(0.15)))
            }

            // Child Profile Hero Row
            HStack(spacing: 14) {
                ZStack {
                    Circle()
                        .fill(
                            LinearGradient(
                                colors: [LiquidGlass.celestialTeal.opacity(0.25), LiquidGlass.emeraldMint.opacity(0.18)],
                                startPoint: .topLeading,
                                endPoint: .bottomTrailing
                            )
                        )
                        .frame(width: 50, height: 50)
                    Circle()
                        .stroke(LiquidGlass.celestialTeal.opacity(0.5), lineWidth: 1.5)
                        .frame(width: 50, height: 50)
                    Image(systemName: "person.crop.circle.fill")
                        .font(.system(size: 36))
                        .foregroundColor(LiquidGlass.celestialTeal)
                }

                VStack(alignment: .leading, spacing: 3) {
                    HStack(spacing: 8) {
                        Text(childName)
                            .font(.system(size: 17, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.textPrimary)

                        Text(selectedAgeBracket.rawValue)
                            .font(.system(size: 11, weight: .bold, design: .rounded))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(0.2)))
                            .foregroundColor(LiquidGlass.celestialTeal)
                    }

                    Text("Cohort: \(selectedAgeBracket.developmentalStage)")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(LiquidGlass.textSecondary)

                    Text("Clinical Focus: \(selectedAgeBracket.clinicalFocus)")
                        .font(.system(size: 11, weight: .regular))
                        .foregroundColor(LiquidGlass.textTertiary)
                        .lineLimit(2)
                }

                Spacer(minLength: 0)
            }

            GlassDivider()

            // Metadata Badges (Language, Privacy, Distance)
            HStack(spacing: 8) {
                HStack(spacing: 5) {
                    Text(languageItem.flag)
                    Text(languageItem.nativeName)
                        .font(.system(size: 11, weight: .bold, design: .rounded))
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.white.opacity(0.06)))
                .foregroundColor(LiquidGlass.textPrimary)

                HStack(spacing: 5) {
                    Image(systemName: "lock.shield.fill")
                        .foregroundColor(LiquidGlass.emeraldMint)
                        .font(.system(size: 11))
                    Text("Zero Raw Video")
                        .font(.system(size: 11, weight: .semibold, design: .rounded))
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.white.opacity(0.06)))
                .foregroundColor(LiquidGlass.textSecondary)

                HStack(spacing: 5) {
                    Image(systemName: "ruler.fill")
                        .foregroundColor(LiquidGlass.warmAmber)
                        .font(.system(size: 11))
                    Text("45–55 cm")
                        .font(.system(size: 11, weight: .semibold, design: .rounded))
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.white.opacity(0.06)))
                .foregroundColor(LiquidGlass.textSecondary)

                Spacer(minLength: 0)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard(cornerRadius: 16, padding: 16)
    }

    // MARK: - Bottom Action Bar

    private var bottomActionBar: some View {
        HStack(spacing: 12) {
            if currentStep > 1 {
                Button {
                    withAnimation(.easeInOut(duration: 0.25)) {
                        currentStep -= 1
                    }
                } label: {
                    Text("Back")
                        .font(.system(size: 15, weight: .semibold, design: .rounded))
                        .foregroundColor(LiquidGlass.textSecondary)
                        .padding(.horizontal, 20)
                        .padding(.vertical, 14)
                        .glassCard(cornerRadius: 14, padding: 0)
                }
                .buttonStyle(.plain)
            }

            Button {
                if currentStep < 3 {
                    withAnimation(.easeInOut(duration: 0.25)) {
                        currentStep += 1
                    }
                } else {
                    completeOnboarding()
                }
            } label: {
                HStack(spacing: 6) {
                    Text(currentStep == 3 ? "Complete Setup & Enter App" : "Continue")
                        .font(.system(size: 16, weight: .bold, design: .rounded))
                    Image(systemName: currentStep == 3 ? "checkmark.circle.fill" : "arrow.right.circle.fill")
                        .font(.system(size: 17))
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 15)
                .background(
                    isStepValid
                        ? LinearGradient(
                            colors: [LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                            startPoint: .leading,
                            endPoint: .trailing
                        )
                        : LinearGradient(
                            colors: [Color.gray.opacity(0.3), Color.gray.opacity(0.2)],
                            startPoint: .leading,
                            endPoint: .trailing
                        )
                )
                .foregroundColor(isStepValid ? .black : Color.white.opacity(0.4))
                .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                .shadow(color: isStepValid ? LiquidGlass.emeraldMint.opacity(0.3) : Color.clear, radius: 8, y: 3)
            }
            .disabled(!isStepValid)
        }
    }

    // MARK: - Helpers

    private var isStepValid: Bool {
        switch currentStep {
        case 1:
            return consentAgreed
        case 2:
            return !childNameInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        default:
            return true
        }
    }

    private func piaFeatureRow(icon: String, color: Color, title: String, description: String) -> some View {
        HStack(alignment: .top, spacing: 14) {
            ZStack {
                Circle()
                    .fill(color.opacity(0.15))
                    .frame(width: 40, height: 40)
                Image(systemName: icon)
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundColor(color)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textPrimary)

                Text(description)
                    .font(.system(size: 12, weight: .regular))
                    .foregroundColor(LiquidGlass.textSecondary)
                    .lineLimit(nil)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard(cornerRadius: 14, padding: 14)
    }

    private func languageChip(code: String, label: String) -> some View {
        Button {
            selectedLanguage = code
        } label: {
            Text(label)
                .font(.system(size: 12, weight: selectedLanguage == code ? .bold : .medium))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 9)
                .background(
                    selectedLanguage == code
                        ? LiquidGlass.celestialTeal.opacity(0.25)
                        : Color.white.opacity(0.06)
                )
                .foregroundColor(selectedLanguage == code ? LiquidGlass.celestialTeal : LiquidGlass.textSecondary)
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(
                            selectedLanguage == code ? LiquidGlass.celestialTeal : Color.clear,
                            lineWidth: 1.2
                        )
                )
        }
        .buttonStyle(.plain)
    }

    private func completeOnboarding() {
        var profile = profileStore.profile
        profile.childName = childNameInput.trimmingCharacters(in: .whitespacesAndNewlines)
        profile.ageBracket = selectedAgeBracket
        profile.preferredLanguage = selectedLanguage
        profile.hasConsentedPIA = true
        profile.consentTimestamp = Date()
        profileStore.profile = profile
        onCompleted()
    }
}

// MARK: - Language Selection Sheet / Popover

public struct LanguageSelectionSheetView: View {
    @Binding public var selectedLanguage: String
    @Environment(\.dismiss) private var dismiss
    @State private var searchText: String = ""

    private var filteredLanguages: [ScreeningLanguageItem] {
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if query.isEmpty {
            return ScreeningLanguages.all
        }
        return ScreeningLanguages.all.filter { item in
            item.englishName.lowercased().contains(query) ||
            item.nativeName.lowercased().contains(query) ||
            item.region.lowercased().contains(query) ||
            item.code.lowercased().contains(query)
        }
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LinearGradient(
                    colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .ignoresSafeArea()

                VStack(spacing: 12) {
                    // Search Bar
                    HStack(spacing: 10) {
                        Image(systemName: "magnifyingglass")
                            .foregroundColor(LiquidGlass.celestialTeal)
                        TextField("Search language, country, or script...", text: $searchText)
                            .font(.system(size: 15, weight: .medium))
                            .foregroundColor(.primary)
                        if !searchText.isEmpty {
                            Button {
                                searchText = ""
                            } label: {
                                Image(systemName: "xmark.circle.fill")
                                    .foregroundColor(LiquidGlass.textTertiary)
                            }
                        }
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 10)
                    .glassCard(cornerRadius: 12, padding: 0)
                    .padding(.horizontal, 16)
                    .padding(.top, 12)

                    // Language List
                    ScrollView {
                        LazyVStack(spacing: 8) {
                            ForEach(filteredLanguages) { item in
                                Button {
                                    selectedLanguage = item.code
                                    dismiss()
                                } label: {
                                    HStack(spacing: 14) {
                                        Text(item.flag)
                                            .font(.system(size: 26))

                                        VStack(alignment: .leading, spacing: 2) {
                                            HStack {
                                                Text(item.nativeName)
                                                    .font(.system(size: 16, weight: .bold, design: .rounded))
                                                    .foregroundColor(LiquidGlass.textPrimary)
                                                Text("(\(item.englishName))")
                                                    .font(.system(size: 13, weight: .medium))
                                                    .foregroundColor(LiquidGlass.textSecondary)
                                            }

                                            Text(item.region)
                                                .font(.system(size: 11, weight: .regular))
                                                .foregroundColor(LiquidGlass.textTertiary)
                                        }

                                        Spacer(minLength: 0)

                                        if selectedLanguage == item.code {
                                            Image(systemName: "checkmark.circle.fill")
                                                .font(.system(size: 18, weight: .bold))
                                                .foregroundColor(LiquidGlass.emeraldMint)
                                        }
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .glassCard(cornerRadius: 12, padding: 12)
                                    .overlay(
                                        RoundedRectangle(cornerRadius: 12)
                                            .stroke(
                                                selectedLanguage == item.code ? LiquidGlass.emeraldMint.opacity(0.6) : Color.clear,
                                                lineWidth: 1.2
                                            )
                                    )
                                }
                                .buttonStyle(.plain)
                            }
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                    }
                }
            }
            .navigationTitle("Select Language")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") {
                        dismiss()
                    }
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }
}
