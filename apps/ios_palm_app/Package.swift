// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "PalmScannerApp",
    platforms: [
        .iOS(.v16)
    ],
    products: [
        .library(
            name: "PalmScannerAppCore",
            targets: ["PalmScannerAppCore"]
        )
    ],
    dependencies: [
        .package(path: "../../adapters/apple")
    ],
    targets: [
        .target(
            name: "PalmScannerAppCore",
            dependencies: [
                .product(name: "CurvApple", package: "apple")
            ],
            path: "Sources"
        )
    ]
)
