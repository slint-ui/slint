// swift-tools-version: 6.0

// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import PackageDescription

let package = Package(
    name: "Slint",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [
        .library(name: "Slint", targets: ["Slint"]),
        .library(name: "SlintSwiftUI", targets: ["SlintSwiftUI"]),
        .library(name: "SlintInterpreter", targets: ["SlintInterpreter"]),
    ],
    targets: [
        // C bridging header (declares Rust extern "C" symbols) + links the Rust static library.
        // Build the Rust lib first: `cargo build --lib -p slint-swift --features interpreter`
        .target(
            name: "SlintCBridge",
            path: "Sources/SlintCBridge",
            publicHeadersPath: "include",
            linkerSettings: [
                .linkedLibrary("slint_swift"),
                .unsafeFlags(["-L../../target/debug"], .when(configuration: .debug)),
                .unsafeFlags(["-L../../target/release"], .when(configuration: .release)),
                // macOS system frameworks required by the Rust backend (winit, CoreFoundation, etc.)
                .linkedFramework("AppKit", .when(platforms: [.macOS])),
                .linkedFramework("Carbon", .when(platforms: [.macOS])),
                .linkedFramework("CoreFoundation", .when(platforms: [.macOS])),
                .linkedFramework("CoreGraphics", .when(platforms: [.macOS])),
                .linkedFramework("CoreServices", .when(platforms: [.macOS])),
                .linkedFramework("CoreText", .when(platforms: [.macOS])),
                .linkedFramework("CoreVideo", .when(platforms: [.macOS])),
                .linkedFramework("OpenGL", .when(platforms: [.macOS])),
                // Linux system libraries required by the Rust backend
                .linkedLibrary("fontconfig", .when(platforms: [.linux])),
            ]
        ),

        // Core Swift API (excludes SwiftUI wrapper to avoid compiler issues on CI)
        .target(
            name: "Slint",
            dependencies: ["SlintCBridge"],
            path: "Sources/Slint",
            exclude: ["SlintView.swift"]
        ),

        // SwiftUI wrapper (separate target to isolate SwiftUI dependency)
        .target(
            name: "SlintSwiftUI",
            dependencies: ["Slint"],
            path: "Sources/Slint",
            sources: ["SlintView.swift"]
        ),

        // Interpreter Swift API (requires `interpreter` feature in the Rust build)
        .target(
            name: "SlintInterpreter",
            dependencies: ["Slint"],
            path: "Sources/SlintInterpreter"
        ),

        // Unit tests
        .testTarget(
            name: "SlintTests",
            dependencies: ["Slint", "SlintInterpreter"],
            path: "Tests/SlintTests"
        ),
    ]
)
