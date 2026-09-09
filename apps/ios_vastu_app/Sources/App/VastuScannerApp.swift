import SwiftUI

@main
struct VastuScannerApp: App {
    var body: some Scene {
        WindowGroup {
            MainScannerView()
                .preferredColorScheme(.dark)
        }
    }
}
