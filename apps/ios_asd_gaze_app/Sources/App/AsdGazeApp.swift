import SwiftUI
import CurvAsdGazeKit

@main
struct AsdGazeApp: App {
    @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding: Bool = false
    @State private var selectedTab: Int = 0
    @State private var showSplashScreen: Bool = true
    @State private var showMoodDiaryFromSuggestion: Bool = false
    @ObservedObject private var moodFusion = ARKitMoodFusionController.shared
    @ObservedObject private var appMode = AppModeStore.shared

    var body: some Scene {
        WindowGroup {
            ZStack {
                // Main Application Layer (routes Mode Selection → Onboarding → TabView)
                if !appMode.hasSelectedMode {
                    AppModeSelectionView { chosenMode in
                        withAnimation(.easeInOut(duration: 0.4)) {
                            appMode.select(chosenMode)
                        }
                    }
                    .transition(.opacity)
                } else if hasCompletedOnboarding {
                    mainTabView
                        .transition(.opacity)
                } else {
                    OnboardingFlowView {
                        withAnimation(.easeInOut(duration: 0.4)) {
                            hasCompletedOnboarding = true
                        }
                    }
                    .transition(.opacity)
                }

                // Branded Launch Splash Screen with Custom Liquid Glass Spinner
                if showSplashScreen {
                    SplashScreenView {
                        withAnimation(.easeInOut(duration: 0.35)) {
                            showSplashScreen = false
                        }
                    }
                    .transition(.opacity)
                    .zIndex(999)
                }
            }
            .task {
                CareKitClinicalStore.shared.seedTasks(
                    CarePlanStore.shared.tasks,
                    medications: CarePlanStore.shared.medications
                )
                ARKitMoodFusionController.shared.startListening()
                BlendShapeCSVLogger.shared.startListening()
            }
        }
    }

    // MARK: - Main Tab Bar View

    private var mainTabView: some View {
        TabView(selection: $selectedTab) {
            // Tab 0: Longitudinal CareKit Dashboard (label reflects the chosen mode)
            CaregiverDashboardView {
                selectedTab = 1
            }
            .tabItem {
                Label(
                    appMode.mode == .clinic ? "Clinic Dashboard" : Localizer.tr("tab_dashboard"),
                    systemImage: "chart.line.uptrend.xyaxis"
                )
            }
            .tag(0)

            // Tab 1: Guided Clinical Screening Wizard
            GazeScreeningWizardView(
                onOpenHistory: {
                    selectedTab = 0
                }
            )
            .tabItem {
                Label(Localizer.tr("tab_gaze"), systemImage: "eye.circle.fill")
            }
            .tag(1)

            // Tab 2: Settings & Diagnostics
            SettingsView()
                .tabItem {
                    Label(Localizer.tr("tab_settings"), systemImage: "gearshape.fill")
                }
                .tag(2)
        }
        .tint(LiquidGlass.celestialTeal)
        .alert(
            "Mood diary",
            isPresented: Binding(
                get: { moodFusion.pendingSuggestion != nil && !showMoodDiaryFromSuggestion },
                set: { if !$0 { moodFusion.dismissSuggestion() } }
            )
        ) {
            Button("Log this") { showMoodDiaryFromSuggestion = true }
            Button("Not now", role: .cancel) { moodFusion.dismissSuggestion() }
        } message: {
            Text(moodFusion.pendingSuggestion?.prompt
                 ?? "We noticed some distress earlier. Would you like to log this?")
        }
        .sheet(isPresented: $showMoodDiaryFromSuggestion, onDismiss: {
            moodFusion.dismissSuggestion()
        }) {
            MoodDiaryView(suggestion: moodFusion.pendingSuggestion)
        }
    }
}
