import SwiftUI

@main
struct PalmScannerApp: App {
    @State private var isShowingSplash: Bool = true

    var body: some Scene {
        WindowGroup {
            ZStack {
                MainScannerView()

                if isShowingSplash {
                    ZStack {
                        Color.white.ignoresSafeArea()

                        Image("LaunchLogo")
                            .resizable()
                            .aspectRatio(contentMode: .fit)
                            .frame(width: 128, height: 128)
                    }
                    .transition(.opacity)
                    .zIndex(999)
                    .allowsHitTesting(false)
                }
            }
            .onAppear {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
                    withAnimation(.easeOut(duration: 0.35)) {
                        isShowingSplash = false
                    }
                }
            }
            // NOTE: no global preferredColorScheme here. The viewfinder, review,
            // and editor force dark themselves (media surfaces, like Camera/Photos);
            // the reading sheet follows the system so it feels native in both modes.
        }
    }
}

