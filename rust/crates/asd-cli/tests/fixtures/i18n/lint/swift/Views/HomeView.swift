import SwiftUI

// Fixture for `asd-cli i18n lint` (WP-L.0, I18N-8). Exactly one finding: `Text("Hello")`.
struct HomeView: View {
    let name: String

    var body: some View {
        VStack {
            Text("Hello")
            Text(verbatim: "v1.2.3")
            Text(String(localized: "home.title"))
            Text(AppLocalizer.tr("home.subtitle", lang: .en))
            Text("Curv") // i18n-ok: brand name
            Text("\(name)")
            Text("·")
            Text("home.greeting")
            /* Text("in a comment") */
        }
    }
}
