// swift-tools-version: 5.9
import PackageDescription

// The Rust core (rust/crates/asd-ffi) is linked as a prebuilt static xcframework (FFI-3,
// ADR-0006). SwiftPM requires a binary target's path to sit inside this package, so the
// build script writes its output here and the directory is git-ignored (`build/`, WS-4):
//
//   ASD_XCFRAMEWORK_OUT="$PWD/domains/asd_screening/ios/build/asd" \
//     scripts/build_asd_xcframework.sh --debug
//
// `scripts/ios_test.sh` does this before any kit or app step. Without it, package
// resolution fails with "binary target path ... does not exist".
//
// - `AsdCoreFFI`: the static library slices and the C module `asd_coreFFI`.
// - `AsdCore`: the UniFFI-generated Swift bindings (`asd_core.swift`), also a product so
//   the app's sync client can call `asd-sync` directly.
// - `CurvAsdGazeKit`: the engines keep their public Swift API and delegate to `AsdCore`.
let asdCoreOut = "build/asd"

let package = Package(
    name: "CurvAsdGazeKit",
    platforms: [
        .iOS(.v17),
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "CurvAsdGazeKit",
            targets: ["CurvAsdGazeKit"]
        ),
        .library(
            name: "AsdCore",
            targets: ["AsdCore"]
        )
    ],
    targets: [
        .binaryTarget(
            name: "AsdCoreFFI",
            path: "\(asdCoreOut)/AsdCore.xcframework"
        ),
        .target(
            name: "AsdCore",
            dependencies: ["AsdCoreFFI"],
            path: "\(asdCoreOut)/swift"
        ),
        .target(
            name: "CurvAsdGazeKit",
            dependencies: ["AsdCore"],
            path: "Sources/CurvAsdGazeKit",
            resources: [
                .process("Resources")
            ],
            swiftSettings: [
                // ADR-0001 / ADR-0004: Debug-only capabilities. `OTHER_SWIFT_FLAGS` in the
                // app's project.yml does not propagate into SwiftPM targets, so the kit
                // declares its own. Release builds compile neither simulation nor the
                // research classifier.
                .define("ASD_SIMULATION_ENABLED", .when(configuration: .debug)),
                .define("ASD_RESEARCH_CLASSIFIER", .when(configuration: .debug))
            ]
        ),
        .testTarget(
            name: "CurvAsdGazeKitTests",
            dependencies: ["CurvAsdGazeKit", "AsdCore"],
            path: "Tests/CurvAsdGazeKitTests",
            swiftSettings: [
                .define("ASD_SIMULATION_ENABLED", .when(configuration: .debug)),
                .define("ASD_RESEARCH_CLASSIFIER", .when(configuration: .debug))
            ]
        )
    ]
)
