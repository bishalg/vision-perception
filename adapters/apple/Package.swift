// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "CurvApple",
    platforms: [
        .iOS(.v16),
        .macOS(.v13)
    ],
    products: [
        .library(
            name: "CurvApple",
            targets: ["CurvApple"]
        ),
    ],
    targets: [
        .target(
            name: "CurvApple",
            path: ".",
            exclude: [
                "README.md"
            ],
            sources: [
                "CurvEngine.swift"
            ]
        ),
    ]
)
