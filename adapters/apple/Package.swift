// swift-tools-version: 5.9
import PackageDescription
import Foundation

let buildDir = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()
    .appendingPathComponent("../../build")
    .standardizedFileURL

let xcframeworkPath = buildDir.appendingPathComponent("CurvEngine.xcframework").path
let opencvPath = buildDir.appendingPathComponent("opencv2.xcframework").path

let hasXCFramework = FileManager.default.fileExists(atPath: xcframeworkPath)
let hasOpenCV = FileManager.default.fileExists(atPath: opencvPath)

// The native engine is only usable when BOTH frameworks are present: the thin
// C API archive (CurvEngine.xcframework) has unresolved cv::* symbols that
// opencv2.xcframework satisfies at link time.
let nativeAvailable = hasXCFramework && hasOpenCV

var curvAppleDependencies: [Target.Dependency] = []
if nativeAvailable {
    curvAppleDependencies = ["curv_capi", "opencv2"]
}

var packageTargets: [Target] = [
    .target(
        name: "CurvApple",
        dependencies: curvAppleDependencies,
        path: ".",
        exclude: [
            "README.md"
        ],
        sources: [
            "CurvEngine.swift"
        ]
    )
]

if nativeAvailable {
    packageTargets.append(
        .binaryTarget(
            name: "curv_capi",
            path: "../../build/CurvEngine.xcframework"
        )
    )
    packageTargets.append(
        .binaryTarget(
            name: "opencv2",
            path: "../../build/opencv2.xcframework"
        )
    )
}

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
    targets: packageTargets
)
