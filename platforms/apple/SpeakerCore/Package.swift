// swift-tools-version:5.9
// 由 scripts/build-apple.sh 生成 XCFramework 与 SpeakerCore.swift 后可用。
import PackageDescription

let package = Package(
    name: "SpeakerCore",
    platforms: [.iOS(.v16), .macOS("13.4")],
    products: [
        .library(name: "SpeakerCore", targets: ["SpeakerCore"]),
    ],
    targets: [
        .binaryTarget(name: "SpeakerCoreFFI", path: "SpeakerCoreFFI.xcframework"),
        .target(
            name: "SpeakerCore",
            dependencies: ["SpeakerCoreFFI"],
            // 静态链接的 ONNX Runtime（CoreML）与 llama.cpp（Metal / Accelerate）的系统依赖
            linkerSettings: [
                .linkedFramework("CoreML"), .linkedFramework("Foundation"),
                .linkedFramework("Metal"), .linkedFramework("MetalKit"), .linkedFramework("Accelerate"),
                .linkedLibrary("c++"),
            ]
        ),
        .testTarget(name: "SpeakerCoreTests", dependencies: ["SpeakerCore"]),
    ]
)
