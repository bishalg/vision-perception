import SwiftUI
import CurvAsdGazeKit

@main
struct AsdGazeApp: App {
    @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding: Bool = false
    @State private var selectedTab: Int = 0
    @State private var showSplashScreen: Bool = true

    var body: some Scene {
        WindowGroup {
            ZStack {
                // Main Application Layer (routes between Onboarding and TabView)
                if hasCompletedOnboarding {
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
            }
        }
    }

    // MARK: - Main Tab Bar View

    private var mainTabView: some View {
        TabView(selection: $selectedTab) {
            // Tab 0: Longitudinal CareKit Dashboard
            CaregiverDashboardView {
                selectedTab = 1
            }
            .tabItem {
                Label(Localizer.tr("tab_dashboard"), systemImage: "chart.line.uptrend.xyaxis")
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
    }
}
