import SwiftUI
import CurvAsdGazeKit

/// Main caregiver dashboard adhering to Apple CareKit paradigms for longitudinal
/// pediatric developmental tracking, daily care plans, mood diary, and gaze analytics.
public struct CaregiverDashboardView: View {
    public let onStartScreening: () -> Void

    @StateObject private var carePlanStore = CarePlanStore.shared
    @State private var showingExportSheet = false
    @State private var showingClinicalDetailsSheet = false
    @State private var isAdvisoryDismissed = false
    @State private var selectedMilestone: MilestoneItem?
    @State private var diaryNotesText: String = ""

    public init(onStartScreening: @escaping () -> Void) {
        self.onStartScreening = onStartScreening
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                // Liquid Glass twilight gradient background
                LinearGradient(
                    colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        headerSection

                        // Apple Health Style Clinical Advisory Alert
                        if !isAdvisoryDismissed {
                            clinicalAdvisoryBanner
                        }

                        // Apple Health Style 3-Column Activity & Care Protocol Card
                        careKitActivityCard

                        // Standardized Screening Action Card
                        screeningActionCard

                        // Pinned Care Plan Interventions
                        todayCarePlanSection

                        // Caregiver-Logged Medication Schedule
                        medicationsSection

                        // Daily Sensory & Mood Diary (2-Item-Per-Row Grid)
                        dailyDiarySection

                        // 7-Day Attention Trend with Bar Chart & Highlights
                        longitudinalTrendSection

                        // CDC / AAP Developmental Milestones
                        milestonesSection
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 14)
                }
            }
            .navigationTitle(Localizer.tr("dashboard_title"))
            .navigationBarTitleDisplayMode(.large)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    HStack(spacing: 12) {
                        // Export Report
                        Button {
                            showingExportSheet = true
                        } label: {
                            Image(systemName: "square.and.arrow.up")
                                .font(.system(size: 15, weight: .semibold))
                                .foregroundColor(LiquidGlass.celestialTeal)
                        }
                        .accessibilityLabel(Localizer.tr("carekit_export_title"))

                        // Apple Health Profile Avatar Button
                        Button {
                            showingExportSheet = true
                        } label: {
                            ZStack {
                                Circle()
                                    .fill(LiquidGlass.celestialTeal.opacity(0.2))
                                    .frame(width: 32, height: 32)
                                Image(systemName: "person.crop.circle.fill")
                                    .font(.system(size: 26))
                                    .foregroundColor(LiquidGlass.celestialTeal)
                            }
                        }
                        .accessibilityLabel("Caregiver Profile & Export")
                    }
                }
            }
            .sheet(isPresented: $showingExportSheet) {
                CareKitExportModalView(store: carePlanStore)
            }
            .sheet(isPresented: $showingClinicalDetailsSheet) {
                ClinicalDetailsModalView()
            }
            .sheet(item: $selectedMilestone) { milestone in
                MilestoneDetailModalView(milestone: milestone)
            }
            .onAppear {
                diaryNotesText = carePlanStore.diaryEntry.notes
            }
        }
    }

    // MARK: - Header & Notice

    private var headerSection: some View {
        HStack {
            Text(Localizer.tr("dashboard_subtitle"))
                .font(.system(size: 14, weight: .medium))
                .foregroundColor(.secondary)
            Spacer()
        }
    }

    // MARK: - 1. Clinical Advisory Alert Banner (Apple Health Style)

    private var clinicalAdvisoryBanner: some View {
        VStack(alignment: .leading, spacing: 10) {
            // Alert Header Row
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(LiquidGlass.warmAmber)
                    .font(.system(size: 12))

                Text(Localizer.tr("dashboard_clinical_advisory"))
                    .font(.system(size: 11, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.warmAmber)

                Spacer()

                Text("13 Sep")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(.secondary)

                Button {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        isAdvisoryDismissed = true
                    }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 16))
                        .foregroundColor(.secondary.opacity(0.8))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Dismiss Alert")
            }

            // Body: Pediatric heart/clinical icon + Title + Description
            HStack(alignment: .top, spacing: 12) {
                ZStack {
                    Circle()
                        .fill(LiquidGlass.warmAmber.opacity(0.2))
                        .frame(width: 40, height: 40)
                    Image(systemName: "heart.text.square.fill")
                        .font(.system(size: 20))
                        .foregroundColor(LiquidGlass.warmAmber)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text(Localizer.tr("dashboard_clinical_title"))
                        .font(.system(size: 14, weight: .bold))
                        .foregroundColor(.primary)

                    Text(Localizer.tr("dashboard_clinical_desc"))
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            // More Details Action Button
            HStack {
                Button {
                    showingClinicalDetailsSheet = true
                } label: {
                    Text(Localizer.tr("dashboard_more_details"))
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundColor(.black) // theme:fixed-media
                        .padding(.horizontal, 14)
                        .padding(.vertical, 7)
                        .background(Capsule().fill(LiquidGlass.warmAmber))
                }
                .buttonStyle(.plain)

                Spacer()
            }
            .padding(.top, 2)
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(LiquidGlass.warmAmber.opacity(0.12))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(LiquidGlass.warmAmber.opacity(0.35), lineWidth: 1)
        )
    }

    // MARK: - 2. Care Protocol Activity Ring Card (Apple Health 3-Column + Ring Style)

    private var careKitActivityCard: some View {
        VStack(alignment: .leading, spacing: 14) {
            // Card Header: Icon + Title + Streak + Chevron
            HStack {
                Image(systemName: "flame.fill")
                    .foregroundColor(LiquidGlass.warmAmber)
                    .font(.system(size: 15))

                Text(Localizer.tr("dashboard_care_activity"))
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.warmAmber)

                Spacer()

                HStack(spacing: 4) {
                    Text("\(carePlanStore.streakDays) \(Localizer.tr("carekit_streak"))")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(.secondary)
                    Image(systemName: "chevron.right")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundColor(.secondary.opacity(0.6))
                }
            }

            // 3-Column Stats + Activity Ring
            HStack(spacing: 0) {
                // Column 1: Tasks
                VStack(alignment: .leading, spacing: 2) {
                    Text("Care Plan")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.secondary)
                    HStack(alignment: .firstTextBaseline, spacing: 2) {
                        Text("\(carePlanStore.completedTaskCount)")
                            .font(.system(size: 22, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                        Text("/\(carePlanStore.tasks.count)")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundColor(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                Divider()
                    .frame(height: 32)
                    .background(Color.primary.opacity(0.12))

                // Column 2: Streak
                VStack(alignment: .leading, spacing: 2) {
                    Text("Streak")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.secondary)
                    HStack(alignment: .firstTextBaseline, spacing: 2) {
                        Text("\(carePlanStore.streakDays)")
                            .font(.system(size: 22, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                        Text("days")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundColor(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.leading, 12)

                Divider()
                    .frame(height: 32)
                    .background(Color.primary.opacity(0.12))

                // Column 3: Social Focus
                VStack(alignment: .leading, spacing: 2) {
                    Text(Localizer.tr("dashboard_social_focus"))
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.secondary)
                    HStack(alignment: .firstTextBaseline, spacing: 2) {
                        Text("\(Int(carePlanStore.completionRatio * 100))")
                            .font(.system(size: 22, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                        Text("%")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundColor(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.leading, 12)

                Spacer()

                // Apple Health Activity Ring
                ZStack {
                    Circle()
                        .stroke(Color.primary.opacity(0.1), lineWidth: 7)
                        .frame(width: 56, height: 56)

                    Circle()
                        .trim(from: 0, to: CGFloat(carePlanStore.completionRatio))
                        .stroke(
                            LinearGradient(
                                colors: [LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                                startPoint: .topLeading,
                                endPoint: .bottomTrailing
                            ),
                            style: StrokeStyle(lineWidth: 7, lineCap: .round)
                        )
                        .rotationEffect(.degrees(-90))
                        .frame(width: 56, height: 56)
                        .animation(.spring(response: 0.6, dampingFraction: 0.8), value: carePlanStore.completionRatio)

                    Text("\(Int(carePlanStore.completionRatio * 100))%")
                        .font(.system(size: 11, weight: .bold, design: .rounded))
                        .foregroundColor(.primary)
                }
            }
        }
        .glassCard(cornerRadius: 16, padding: 16)
    }

    // MARK: - 3. Standardized Screening Quick Action Card

    private var screeningActionCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: "eyes")
                    .font(.system(size: 15))
                    .foregroundColor(LiquidGlass.celestialTeal)

                Text("Daily Eye Tracking")
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)

                Spacer()

                Text("Ready")
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundColor(LiquidGlass.emeraldMint)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 3)
                    .background(LiquidGlass.emeraldMint.opacity(0.18))
                    .clipShape(Capsule())

                Image(systemName: "chevron.right")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundColor(.secondary.opacity(0.6))
            }

            Text("Interactive calibration followed by standardized pediatric visual attention screening.")
                .font(.system(size: 13))
                .foregroundColor(.secondary)

            GlassButton(
                title: Localizer.tr("start_screening"),
                systemImage: "play.fill",
                isPrimary: true,
                action: onStartScreening
            )
        }
        .glassCard(cornerRadius: 16, padding: 16)
    }

    // MARK: - 4. Today's Care Plan Interventions (Pinned Section)

    private var todayCarePlanSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(Localizer.tr("dashboard_pinned"))
                    .font(.system(size: 18, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)

                Spacer()

                Text("\(carePlanStore.completedTaskCount) of \(carePlanStore.tasks.count)")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundColor(.secondary)
            }

            VStack(spacing: 8) {
                ForEach(carePlanStore.tasks) { task in
                    carePlanTaskRow(task: task)
                }
            }
        }
    }

    private func carePlanTaskRow(task: CarePlanTask) -> some View {
        HStack(spacing: 12) {
            // Circular Checkbox with Tactile Haptic Feedback
            Button {
                let generator = UIImpactFeedbackGenerator(style: .light)
                generator.impactOccurred()
                carePlanStore.toggleTask(id: task.id)
            } label: {
                Image(systemName: task.isCompleted ? "checkmark.circle.fill" : "circle")
                    .font(.system(size: 22))
                    .foregroundColor(task.isCompleted ? LiquidGlass.emeraldMint : Color.primary.opacity(0.35))
            }
            .buttonStyle(.plain)

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(task.title)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundColor(task.isCompleted ? .secondary : .primary)
                        .strikethrough(task.isCompleted, color: .primary.opacity(0.4))

                    Spacer()

                    Label(task.category.rawValue, systemImage: task.category.systemIcon)
                        .font(.system(size: 10, weight: .medium))
                        .foregroundColor(LiquidGlass.celestialTeal)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(LiquidGlass.celestialTeal.opacity(0.12))
                        .clipShape(Capsule())
                }

                Text(task.subtitle)
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .glassCard(cornerRadius: 12, padding: 12)
    }

    // MARK: - 5. Caregiver-Logged Medication Schedule

    private var medicationsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(Localizer.tr("carekit_meds_title"))
                    .font(.system(size: 18, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)

                Spacer()

                Text("\(Int(carePlanStore.medicationCompletionRatio * 100))% today")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }

            VStack(spacing: 8) {
                ForEach(carePlanStore.medications) { medication in
                    medicationRow(medication)
                }
            }
        }
    }

    private func medicationRow(_ medication: MedicationItem) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 12) {
                Image(systemName: "pills.fill")
                    .font(.system(size: 18))
                    .foregroundColor(LiquidGlass.celestialTeal)

                VStack(alignment: .leading, spacing: 2) {
                    Text(medication.name)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundColor(.primary)

                    Text("\(medication.dose) · \(medication.instructions)")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }

                Spacer()

                Text("\(medication.dosesTakenToday.filter { $0 }.count)/\(medication.schedule.count)")
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .foregroundColor(LiquidGlass.emeraldMint)
            }

            HStack(spacing: 10) {
                ForEach(Array(medication.schedule.enumerated()), id: \.offset) { index, time in
                    let isTaken = medication.dosesTakenToday[index]
                    Button {
                        let generator = UIImpactFeedbackGenerator(style: .light)
                        generator.impactOccurred()
                        carePlanStore.toggleMedicationTaken(id: medication.id, doseIndex: index)
                    } label: {
                        HStack(spacing: 5) {
                            Image(systemName: isTaken ? "checkmark.circle.fill" : "circle")
                                .font(.system(size: 15))
                            Text(time)
                                .font(.system(size: 12, weight: .semibold, design: .monospaced))
                        }
                        .foregroundColor(isTaken ? LiquidGlass.emeraldMint : .secondary)
                        .padding(.horizontal, 11)
                        .padding(.vertical, 7)
                        .background(
                            isTaken ? LiquidGlass.emeraldMint.opacity(0.16) : LiquidGlass.cardBackground
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 9, style: .continuous)
                                .stroke(
                                    isTaken ? LiquidGlass.emeraldMint.opacity(0.6) : LiquidGlass.cardBorder.opacity(0.3),
                                    lineWidth: isTaken ? 1.2 : 0.8
                                )
                        )
                        .cornerRadius(9)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .glassCard(cornerRadius: 12, padding: 12)
    }

    // MARK: - 6. Daily Sensory & Mood Diary (2 Items Per Row Grid)

    private var dailyDiarySection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(Localizer.tr("carekit_diary_title"))
                .font(.system(size: 18, weight: .bold, design: .rounded))
                .foregroundColor(.primary)

            VStack(alignment: .leading, spacing: 14) {
                // Diary Header
                HStack {
                    Image(systemName: "brain.head.profile")
                        .foregroundColor(LiquidGlass.celestialTeal)
                        .font(.system(size: 15))

                    Text(Localizer.tr("dashboard_child_state"))
                        .font(.system(size: 14, weight: .bold))
                        .foregroundColor(.primary)

                    Spacer()

                    Text("Logged Today")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundColor(LiquidGlass.emeraldMint)
                }

                // 2 ITEMS PER ROW GRID FOR CHILD MOOD OPTIONS
                LazyVGrid(columns: [GridItem(.flexible(), spacing: 10), GridItem(.flexible(), spacing: 10)], spacing: 10) {
                    ForEach(ChildMood.allCases) { mood in
                        Button {
                            let gen = UIImpactFeedbackGenerator(style: .light)
                            gen.impactOccurred()
                            carePlanStore.updateMood(mood)
                        } label: {
                            HStack(spacing: 8) {
                                Text(mood.emoji)
                                    .font(.system(size: 20))

                                Text(mood.rawValue)
                                    .font(.system(size: 13, weight: .semibold))
                                    .lineLimit(1)

                                Spacer()

                                if carePlanStore.diaryEntry.mood == mood {
                                    Image(systemName: "checkmark")
                                        .font(.system(size: 11, weight: .bold))
                                        .foregroundColor(LiquidGlass.emeraldMint)
                                }
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 10)
                            .frame(maxWidth: .infinity, minHeight: 44)
                            .background(
                                carePlanStore.diaryEntry.mood == mood
                                    ? LiquidGlass.celestialTeal.opacity(0.22)
                                    : LiquidGlass.cardBackground
                            )
                            .foregroundColor(
                                carePlanStore.diaryEntry.mood == mood
                                    ? .primary
                                    : .secondary
                            )
                            .overlay(
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .stroke(
                                        carePlanStore.diaryEntry.mood == mood
                                            ? LiquidGlass.celestialTeal
                                            : LiquidGlass.cardBorder.opacity(0.3),
                                        lineWidth: carePlanStore.diaryEntry.mood == mood ? 1.5 : 0.8
                                    )
                            )
                            .cornerRadius(10)
                        }
                        .buttonStyle(.plain)
                    }
                }

                Divider()
                    .background(Color.primary.opacity(0.08))

                // Spontaneous eye contact rating (1 to 5 scale)
                HStack {
                    Text(Localizer.tr("dashboard_eye_contact"))
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(.primary.opacity(0.85))

                    Spacer()

                    HStack(spacing: 8) {
                        ForEach(1...5, id: \.self) { star in
                            Button {
                                let gen = UIImpactFeedbackGenerator(style: .light)
                                gen.impactOccurred()
                                carePlanStore.updateEyeContactRating(star)
                            } label: {
                                Image(systemName: star <= carePlanStore.diaryEntry.eyeContactRating ? "star.fill" : "star")
                                    .font(.system(size: 17))
                                    .foregroundColor(star <= carePlanStore.diaryEntry.eyeContactRating ? LiquidGlass.warmAmber : Color.primary.opacity(0.25))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }

                // Observation notes input field
                HStack(spacing: 8) {
                    Image(systemName: "pencil.and.ellipsis.rectangle")
                        .foregroundColor(.secondary)
                        .font(.system(size: 14))

                    TextField(Localizer.tr("carekit_diary_notes_placeholder"), text: $diaryNotesText)
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .onChange(of: diaryNotesText) { _, newText in
                            carePlanStore.updateNotes(newText)
                        }
                }
                .padding(10)
                .background(LiquidGlass.cardBackground)
                .cornerRadius(10)
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    // MARK: - 6. 7-Day Longitudinal Attention Trend (Apple Health Style)

    private var longitudinalTrendSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(Localizer.tr("carekit_trend_title"))
                .font(.system(size: 18, weight: .bold, design: .rounded))
                .foregroundColor(.primary)

            VStack(alignment: .leading, spacing: 14) {
                // Header: Metric Name + Date Range + Chevron
                HStack {
                    Image(systemName: "chart.bar.xaxis")
                        .foregroundColor(LiquidGlass.celestialTeal)
                        .font(.system(size: 14))

                    Text("Social Attention Distribution")
                        .font(.system(size: 14, weight: .bold))
                        .foregroundColor(.primary)

                    Spacer()

                    HStack(spacing: 4) {
                        Text("7–13 Sep")
                            .font(.system(size: 12))
                            .foregroundColor(.secondary)
                        Image(systemName: "chevron.right")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundColor(.secondary.opacity(0.6))
                    }
                }

                // Big Headline Value + Target Status Badge
                HStack(alignment: .firstTextBaseline) {
                    HStack(alignment: .firstTextBaseline, spacing: 3) {
                        Text("68%")
                            .font(.system(size: 28, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                        Text("Average")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundColor(.secondary)
                    }

                    Spacer()

                    Text("Target: >50%")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundColor(LiquidGlass.emeraldMint)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(Capsule().fill(LiquidGlass.emeraldMint.opacity(0.18)))
                }

                // Apple Health Bar Chart with 50% Target Reference Line
                ZStack(alignment: .bottom) {
                    // Dashed 50% target reference line
                    VStack {
                        Spacer()
                        HStack {
                            Rectangle()
                                .stroke(style: StrokeStyle(lineWidth: 1, dash: [4, 4]))
                                .foregroundColor(LiquidGlass.emeraldMint.opacity(0.4))
                                .frame(height: 1)
                            Text("50%")
                                .font(.system(size: 9, weight: .bold, design: .monospaced))
                                .foregroundColor(LiquidGlass.emeraldMint.opacity(0.7))
                        }
                        .padding(.bottom, 50)
                    }

                    // 7 Daily Bars
                    HStack(alignment: .bottom, spacing: 10) {
                        ForEach(Array(carePlanStore.weeklyHistory.enumerated()), id: \.element.id) { index, point in
                            let isLatest = index == carePlanStore.weeklyHistory.count - 1

                            VStack(spacing: 6) {
                                // Daily % Label
                                Text("\(Int(point.socialPreferenceScore * 100))%")
                                    .font(.system(size: 9, weight: .bold))
                                    .foregroundColor(isLatest ? LiquidGlass.emeraldMint : .secondary)

                                // Bar with rounded caps
                                RoundedRectangle(cornerRadius: 5, style: .continuous)
                                    .fill(
                                        isLatest
                                            ? LinearGradient(
                                                colors: [LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                                                startPoint: .top,
                                                endPoint: .bottom
                                            )
                                            : LinearGradient(
                                                colors: [Color.primary.opacity(0.25), Color.primary.opacity(0.12)],
                                                startPoint: .top,
                                                endPoint: .bottom
                                            )
                                    )
                                    .frame(height: max(16, CGFloat(point.socialPreferenceScore) * 80))

                                // Day Label
                                Text(point.dayLabel)
                                    .font(.system(size: 11, weight: isLatest ? .bold : .medium))
                                    .foregroundColor(isLatest ? .primary : .secondary)
                            }
                            .frame(maxWidth: .infinity)
                        }
                    }
                }
                .frame(height: 130)
                .padding(.top, 4)

                Divider()
                    .background(Color.primary.opacity(0.08))

                // Apple Health Style "Highlights" Callout Card
                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: "sparkles")
                        .foregroundColor(LiquidGlass.warmAmber)
                        .font(.system(size: 16))

                    VStack(alignment: .leading, spacing: 2) {
                        Text(Localizer.tr("dashboard_highlights"))
                            .font(.system(size: 12, weight: .bold))
                            .foregroundColor(.primary)

                        Text(Localizer.tr("dashboard_highlight_note"))
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    // MARK: - 7. CDC / AAP Developmental Milestones

    private var milestonesSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(Localizer.tr("carekit_milestones_title"))
                .font(.system(size: 18, weight: .bold, design: .rounded))
                .foregroundColor(.primary)

            VStack(spacing: 8) {
                ForEach(carePlanStore.milestones) { milestone in
                    Button {
                        selectedMilestone = milestone
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: milestone.isCompleted ? "checkmark.circle.fill" : "circle.dashed")
                                .foregroundColor(milestone.isCompleted ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                                .font(.system(size: 20))

                            VStack(alignment: .leading, spacing: 2) {
                                Text(milestone.title)
                                    .font(.system(size: 14, weight: .semibold))
                                    .foregroundColor(.primary)

                                Text(milestone.dueDateString)
                                    .font(.system(size: 12))
                                    .foregroundColor(.secondary)
                            }

                            Spacer()

                            Image(systemName: "chevron.right")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundColor(.secondary.opacity(0.5))
                        }
                        .glassCard(cornerRadius: 12, padding: 12)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

