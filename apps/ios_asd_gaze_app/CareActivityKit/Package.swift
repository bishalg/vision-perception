// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "CareActivityKit",
    platforms: [.iOS(.v18)],
    products: [
        .library(name: "CareActivityKit", targets: ["CareActivityKit"]),
    ],
    targets: [
        .target(name: "CareActivityKit"),
    ]
)
