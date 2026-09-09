// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "CurvCameraOverlay",
    platforms: [
        .iOS(.v16)
    ],
    products: [
        .library(name: "CurvCameraOverlay", targets: ["CurvCameraOverlay"]),
        .library(name: "CurvPalmOverlay", targets: ["CurvPalmOverlay"]),
        .library(name: "CurvVastuOverlay", targets: ["CurvVastuOverlay"])
    ],
    targets: [
        .target(
            name: "CurvCameraOverlay",
            path: "Sources/CurvCameraOverlay"
        ),
        .target(
            name: "CurvPalmOverlay",
            dependencies: ["CurvCameraOverlay"],
            path: "Sources/CurvPalmOverlay"
        ),
        .target(
            name: "CurvVastuOverlay",
            dependencies: ["CurvCameraOverlay"],
            path: "Sources/CurvVastuOverlay"
        )
    ]
)
