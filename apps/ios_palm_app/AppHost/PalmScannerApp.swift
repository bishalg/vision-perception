import SwiftUI

@main
struct PalmScannerApp: App {
    var body: some Scene {
        WindowGroup {
            MainScannerView()
            // NOTE: no global preferredColorScheme here. The viewfinder, review,
            // and editor force dark themselves (media surfaces, like Camera/Photos);
            // the reading sheet follows the system so it feels native in both modes.
        }
    }
}
