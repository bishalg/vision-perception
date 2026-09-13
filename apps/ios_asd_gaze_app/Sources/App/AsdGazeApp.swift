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
                    CaregiverDashboardView {
                        if GazeCalibrationStore.load().isCalibrated {
                            selectedTab = 1
                        } else {
                            selectedTab = 2
                        }
                    }
                    .tabItem {
                        Label(Localizer.tr("tab_dashboard"), systemImage: "chart.line.uptrend.xyaxis")
                    }
                    .tag(0)

                    MainGazeScannerView(
                        onClose: nil,
                        onComplete: nil,
                        onOpenHistory: {
                            selectedTab = 0
                        }
                    )
                    .tabItem {
                        Label(Localizer.tr("tab_gaze"), systemImage: "eye.circle.fill")
                    }
                    .tag(1)

                    BubbleCalibrationView(
                        onClose: nil,
                        onCalibrationComplete: {
                            selectedTab = 1
                        }
                    )
                    .tabItem {
                        Label(Localizer.tr("tab_calibrate"), systemImage: "target")
                    }
                    .tag(2)

                    SettingsView()
                        .tabItem {
                            Label(Localizer.tr("tab_settings"), systemImage: "gearshape.fill")
                        }
                        .tag(3)
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
                // Seed the on-device CareKit store once per launch (idempotent):
                // care-plan tasks + caregiver diary + medication tasks.
                CareKitClinicalStore.shared.seedTasks(
                    CarePlanStore.shared.tasks,
                    medications: CarePlanStore.shared.medications
                )
            }
            // No preferredColorScheme: the app follows the system theme on
            // launch. Camera/stimulus fields stay dark media surfaces; all
            // chrome uses the adaptive LiquidGlass tokens + semantic text.
        }
    }
}

