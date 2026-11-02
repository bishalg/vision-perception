import Foundation

/// Test-only mutable cell for values written inside `@Sendable` completion closures and
/// read after `wait(for:)`. This is the single `@unchecked Sendable` type CONC-5 allows
/// (ADR-0010 d2); it lives under `Tests/` and never in `Sources/**`. Every use pairs a
/// write in the callback with a read after the expectation was fulfilled.
final class Box<T>: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: T

    init(_ value: T) { stored = value }

    var value: T {
        get { lock.lock(); defer { lock.unlock() }; return stored }
        set { lock.lock(); defer { lock.unlock() }; stored = newValue }
    }
}
