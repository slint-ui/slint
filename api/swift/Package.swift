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
        // C bridging header (declares Rust extern "C" symbols)
        .target(
            name: "SlintCBridge",
            path: "Sources/SlintCBridge",
            publicHeadersPath: "include"
        ),

        // Core Swift API
        .target(
            name: "Slint",
            dependencies: ["SlintCBridge"],
            path: "Sources/Slint"
        ),
    ]
)
