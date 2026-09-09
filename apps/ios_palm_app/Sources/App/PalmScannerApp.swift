import SwiftUI

@main
struct PalmScannerApp: App {
    var body: some Scene {
        WindowGroup {
            MainScannerView()
                .preferredColorScheme(.dark)
        }
    }
}
