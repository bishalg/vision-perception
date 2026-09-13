import SwiftUI
import UIKit

@main
struct PalmScannerApp: App {
    @State private var isShowingSplash: Bool = true

    var body: some Scene {
        WindowGroup {
            ZStack {
                MainScannerView()

                if isShowingSplash {
                    ZStack {
                        // System backdrop (adapts); brand mark sits on a fixed
                        // dark tile since the logo art is white-on-transparent.
                        Color(uiColor: .systemBackground).ignoresSafeArea()

                        Image("LaunchLogo")
                            .resizable()
                            .aspectRatio(contentMode: .fit)
                            .frame(width: 112, height: 112)
                            .padding(20)
                            .background(
                                RoundedRectangle(cornerRadius: 30, style: .continuous)
                                    .fill(Color(red: 0.04, green: 0.06, blue: 0.10))
                            )
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
            // NOTE: no global preferredColorScheme here — the app follows the
            // system theme on launch. Camera/editor canvases stay dark media
            // surfaces (contrast for tracing); all chrome uses semantic
            // colors. The reading sheet follows the system so it feels
            // native in both modes.
        }
    }
}

