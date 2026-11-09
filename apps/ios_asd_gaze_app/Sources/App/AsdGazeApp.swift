import SwiftUI
import CurvAsdGazeKit

@main
struct AsdGazeApp: App {
    @UIApplicationDelegateAdaptor(AppOrientationDelegate.self) private var orientationDelegate
    @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding: Bool = false
    @State private var selectedTab: RootTab = RootTab.visibleTabs().first ?? .capture
    @State private var showSplashScreen: Bool = true
    #if !ASD_STORE_BUILD
    @State private var showMoodDiaryFromSuggestion: Bool = false
    @ObservedObject private var moodFusion = ARKitMoodFusionController.shared
    #endif
    @ObservedObject private var appMode = AppModeStore.shared
    /// S-10: a new consent document version (or an upgrade from the old consent switch)
    /// asks the guardian again after onboarding. "Not now" defers to the capture gate.
    @ObservedObject private var consentRecords = ConsentRecordStore.shared
    @State private var showConsentReview: Bool = false
    @Environment(\.scenePhase) private var scenePhase

    init() {
        // WP-A.5: the scoring configuration of this launch (a verified delivered bundle, else
        // the bundled default), installed before any session can start.
        ConfigBundleStore.standard.installAtLaunch()
        #if ASD_STORE_BUILD
        // S-2: the store build has no Parent/Clinic picker; the mode is set silently.
        if !AppModeStore.shared.hasSelectedMode {
            AppModeStore.shared.select(.clinic)
        }
        #endif
    }

    var body: some Scene {
        WindowGroup {
            ZStack {
                // Main Application Layer (routes Mode Selection → Onboarding → TabView)
                #if ASD_STORE_BUILD && ASD_STORE_SCREENSHOTS
                // StoreSim only (WP-S.7): example sessions for the App Store screenshot flows.
                rootContent
                    .modifier(StoreScreenshotFixtureModifier())
                #else
                rootContent
                #endif

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
                #if !ASD_STORE_BUILD
                // S-6: care plan and mood diary belong to the clinic platform in the store build.
                CareKitClinicalStore.shared.seedTasks(
                    CarePlanStore.shared.tasks,
                    medications: CarePlanStore.shared.medications
                )
                ARKitMoodFusionController.shared.startListening()
                #endif
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

    // MARK: - Root Routing

    @ViewBuilder
    private var rootContent: some View {
        #if ASD_STORE_BUILD
        // S-2: no mode picker in the store build (the mode is set to `.clinic` in `init`).
        if hasCompletedOnboarding {
            mainTabView
                .transition(.opacity)
        } else {
            onboarding
        }
        #else
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
            onboarding
        }
        #endif
    }

    private var onboarding: some View {
        OnboardingFlowView {
            withAnimation(.easeInOut(duration: 0.4)) {
                hasCompletedOnboarding = true
            }
        }
        .transition(.opacity)
    }

    // MARK: - Main Tab Bar View

    /// Tabs come from `RootTab.visibleTabs`: Dashboard, Capture, Settings, or in the store
    /// build Capture, Sessions, Settings (S-2).
    private var mainTabView: some View {
        TabView(selection: $selectedTab) {
            ForEach(RootTab.visibleTabs(), id: \.self) { tab in
                tabContent(tab)
                    .tabItem {
                        tabLabel(tab)
                            // Maestro flows select tabs by id in every language (WP-S.7).
                            .accessibilityIdentifier("tab_\(tab)")
                    }
                    .tag(tab)
            }
        }
        .tint(LiquidGlass.celestialTeal)
        // Evaluated when the tabs appear and when the splash ends, not on every change, so
        // withdrawing consent in Privacy & Your Data does not pop the flow up immediately.
        .onAppear { evaluateConsentReview() }
        .onChange(of: showSplashScreen) { _, _ in evaluateConsentReview() }
        .fullScreenCover(isPresented: $showConsentReview) {
            ConsentFlowView(
                store: consentRecords,
                onAgreed: { showConsentReview = false },
                onCancel: { showConsentReview = false }
            )
        }
        #if !ASD_STORE_BUILD
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
        #endif
    }

    private func evaluateConsentReview() {
        guard hasCompletedOnboarding, !showSplashScreen else { return }
        if !consentRecords.isCurrent { showConsentReview = true }
    }

    @ViewBuilder
    private func tabContent(_ tab: RootTab) -> some View {
        switch tab {
        case .dashboard:
            #if ASD_STORE_BUILD
            // Never listed in the store build (S-2): the dashboard holds every verdict surface.
            EmptyView()
            #else
            // Longitudinal CareKit Dashboard (label reflects the chosen mode)
            CaregiverDashboardView {
                selectedTab = .capture
            }
            #endif
        case .capture:
            // Guided observation wizard
            GazeScreeningWizardView(
                onOpenHistory: {
                    selectedTab = StoreBuild.isStoreBuild ? .sessions : .dashboard
                }
            )
        case .sessions:
            StoreSessionsView()
        case .settings:
            SettingsView()
        }
    }

    @ViewBuilder
    private func tabLabel(_ tab: RootTab) -> some View {
        switch tab {
        case .dashboard:
            Label(
                appMode.mode == .clinic ? "Clinic Dashboard" : Localizer.tr("tab_dashboard"),
                systemImage: "chart.line.uptrend.xyaxis"
            )
        case .capture:
            if StoreBuild.isStoreBuild {
                Label(Localizer.tr("store_tab_capture"), systemImage: "eye.circle.fill")
            } else {
                Label(Localizer.tr("tab_gaze"), systemImage: "eye.circle.fill")
            }
        case .sessions:
            Label(Localizer.tr("store_tab_sessions"), systemImage: "list.bullet.rectangle")
        case .settings:
            if StoreBuild.isStoreBuild {
                Label(Localizer.tr("tab_settings"), systemImage: "gearshape.fill")
            } else {
                Label(Localizer.tr("tab_settings"), systemImage: "gearshape.fill")
            }
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
