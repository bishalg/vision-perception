// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "VastuScannerApp",
    platforms: [
        .iOS(.v16)
    ],
    products: [
        .library(
            name: "VastuScannerAppCore",
            targets: ["VastuScannerAppCore"]
        )
    ],
    dependencies: [
        .package(path: "../../adapters/apple"),
        .package(path: "../mobile_overlay/ios")
    ],
    targets: [
        .target(
            name: "VastuScannerAppCore",
            dependencies: [
                .product(name: "CurvApple", package: "apple"),
                .product(name: "CurvCameraOverlay", package: "ios"),
                .product(name: "CurvVastuOverlay", package: "ios")
            ],
            path: "Sources"
        )
    ]
)
