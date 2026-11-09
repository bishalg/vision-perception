import Foundation

/// WP-LN.3 (I-5): locale for machine output (CSV, log lines, hashes, wire values). Written
/// explicitly at every machine `String(format:)` so the `swift-format-locale` lint rule can tell
/// intent apart. User-visible numbers use `Locale.current` instead.
enum FormatLocale {
    static let posix = Locale(identifier: "en_US_POSIX")
}
