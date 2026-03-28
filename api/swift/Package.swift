// swift-tools-version: 6.2

// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import PackageDescription

let package = Package(
    name: "Slint",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [
        .library(name: "Slint", targets: ["Slint"]),
    ],
    targets: [
        // C bridging header (declares Rust extern "C" symbols) + links the Rust static library.
        // Build the Rust lib first: `cargo build --lib -p slint-swift` (or --release)
        .target(
            name: "SlintCBridge",
            path: "Sources/SlintCBridge",
            publicHeadersPath: "include",
            linkerSettings: [
                .linkedLibrary("slint_swift"),
                .unsafeFlags(["-L../../target/debug"], .when(configuration: .debug)),
                .unsafeFlags(["-L../../target/release"], .when(configuration: .release)),
                // macOS system frameworks required by the Rust backend (winit, CoreFoundation, etc.)
                .linkedFramework("AppKit"),
                .linkedFramework("Carbon"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("CoreServices"),
                .linkedFramework("CoreText"),
                .linkedFramework("CoreVideo"),
                .linkedFramework("OpenGL"),
            ]
        ),

        // Core Swift API
        .target(
            name: "Slint",
            dependencies: ["SlintCBridge"],
            path: "Sources/Slint"
        ),

        // Unit tests
        .testTarget(
            name: "SlintTests",
            dependencies: ["Slint"],
            path: "Tests/SlintTests"
        ),
    ]
)
