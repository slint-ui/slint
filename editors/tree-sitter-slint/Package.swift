// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "TreeSitterSlint",
    products: [
        .library(name: "TreeSitterSlint", targets: ["TreeSitterSlint"]),
    ],
    dependencies: [
        .package(url: "https://github.com/ChimeHQ/SwiftTreeSitter", from: "0.8.0"),
    ],
    targets: [
        .target(
            name: "TreeSitterSlint",
            dependencies: [],
            path: ".",
            sources: [
                "src/parser.c",
                // NOTE: if your language has an external scanner, add it here.
            ],
            resources: [
                .copy("queries")
            ],
            publicHeadersPath: "bindings/swift",
            cSettings: [.headerSearchPath("src")]
        ),
        .testTarget(
            name: "TreeSitterSlintTests",
            dependencies: [
                "SwiftTreeSitter",
                "TreeSitterSlint",
            ],
            path: "bindings/swift/TreeSitterSlintTests"
        )
    ],
    cLanguageStandard: .c11
)
