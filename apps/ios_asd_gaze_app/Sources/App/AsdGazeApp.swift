import SwiftUI
import CurvAsdGazeKit

@main
struct AsdGazeApp: App {
    @State private var selectedTab: Int = 0
    @State private var showSplashScreen: Bool = true

    var body: some Scene {
        WindowGroup {
            ZStack {
                TabView(selection: $selectedTab) {
                    // Tab 0: Longitudinal CareKit Dashboard
                    CaregiverDashboardView {
                        selectedTab = 1
                    }
                    .tabItem {
                        Label(Localizer.tr("tab_dashboard"), systemImage: "chart.line.uptrend.xyaxis")
                    }
                    .tag(0)

                    // Tab 1: 3-Step Guided Clinical Screening Wizard
                    // Step 1 → Face Alignment & Distance
                    // Step 2 → 5-Point Bubble Calibration
                    // Step 3 → Full-Screen Stimulus + 1:1 Affect Logging
                    GazeScreeningWizardView(
                        onOpenHistory: {
                            selectedTab = 0
                        }
                    )
                    .tabItem {
                        Label(Localizer.tr("tab_gaze"), systemImage: "eye.circle.fill")
                    }
                    .tag(1)

                    // Tab 2: Settings (includes Screening & Calibration Tools section)
                    SettingsView()
                        .tabItem {
                            Label(Localizer.tr("tab_settings"), systemImage: "gearshape.fill")
                        }
                        .tag(2)
                }
                .tint(LiquidGlass.celestialTeal)

                // Animated App Icon Launch & Splash Screen
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
}
