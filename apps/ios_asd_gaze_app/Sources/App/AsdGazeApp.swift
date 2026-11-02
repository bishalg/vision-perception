import SwiftUI
import CurvAsdGazeKit

@main
struct AsdGazeApp: App {
    @UIApplicationDelegateAdaptor(AppOrientationDelegate.self) private var orientationDelegate
    @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding: Bool = false
    @State private var selectedTab: Int = 0
    @State private var showSplashScreen: Bool = true
    @State private var showMoodDiaryFromSuggestion: Bool = false
    @ObservedObject private var moodFusion = ARKitMoodFusionController.shared
    @ObservedObject private var appMode = AppModeStore.shared
    @Environment(\.scenePhase) private var scenePhase

    init() {
        // WP-A.5: the scoring configuration of this launch (a verified delivered bundle, else
        // the bundled default), installed before any session can start.
        ConfigBundleStore.standard.installAtLaunch()
    }

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
            // System controls (toggles, links, nav buttons) use the AA brand teal, not iOS blue.
            .tint(LiquidGlass.celestialTeal)
            .task {
                // SYNC-2/SYNC-4: flush the upload queue at launch (after `installAtLaunch`);
                // until the first network report the state is unknown and that report flushes.
                ResearchKitSyncService.shared?.flushAtLaunch()
                CareKitClinicalStore.shared.seedTasks(
                    CarePlanStore.shared.tasks,
                    medications: CarePlanStore.shared.medications
                )
                ARKitMoodFusionController.shared.startListening()
                // A newer bundle is persisted for the next launch (WP-A.5).
                await ResearchKitSyncService.shared?.refreshConfigBundle()
                #if ASD_ML_DATA_EXPORT
                // CNS-2: training-data logger exists only in ML data-export builds.
                BlendShapeCSVLogger.shared.startListening()
                #endif
            }
            .onChange(of: scenePhase) { _, phase in
                // SYNC-2: back in the foreground, send what was queued meanwhile.
                if phase == .active {
                    ResearchKitSyncService.shared?.requestFlush(.foreground)
                }
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

#if canImport(UIKit)
extension AppOrientationDelegate {
    /// SYNC-5: the system relaunches the app when background blob uploads finish. Wiring
    /// the sync service first means finished uploads mark their blob done in the queue;
    /// the system's completion handler runs once the session has delivered its events.
    @MainActor
    func application(
        _ application: UIApplication,
        handleEventsForBackgroundURLSession identifier: String,
        completionHandler: @escaping () -> Void
    ) {
        _ = ResearchKitSyncService.shared
        if !BackgroundBlobUploader.handleEventsForBackgroundURLSession(
            identifier: identifier,
            completionHandler: completionHandler
        ) {
            completionHandler()
        }
    }
}
#endif
