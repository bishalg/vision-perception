// swift-tools-version: 5.9
import PackageDescription

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
        )
    ],
    targets: [
        .target(
            name: "CurvAsdGazeKit",
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
            dependencies: ["CurvAsdGazeKit"],
            path: "Tests/CurvAsdGazeKitTests",
            swiftSettings: [
                .define("ASD_SIMULATION_ENABLED", .when(configuration: .debug)),
                .define("ASD_RESEARCH_CLASSIFIER", .when(configuration: .debug))
            ]
        )
    ]
)
