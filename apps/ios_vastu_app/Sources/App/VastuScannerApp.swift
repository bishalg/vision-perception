import SwiftUI

@main
struct VastuScannerApp: App {
    var body: some Scene {
        WindowGroup {
            // No preferredColorScheme: the app follows the system theme on
            // launch (camera canvas itself stays a dark media surface).
            MainScannerView()
        }
    }
}
