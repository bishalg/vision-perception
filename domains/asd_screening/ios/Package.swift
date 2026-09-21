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
            ]
        ),
        .testTarget(
            name: "CurvAsdGazeKitTests",
            dependencies: ["CurvAsdGazeKit"],
            path: "Tests/CurvAsdGazeKitTests"
        )
    ]
)
