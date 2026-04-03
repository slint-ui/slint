// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// A simple Slint + Swift example using the interpreter API.
//
// Build and run:
//   cargo build --lib -p slint-swift --features interpreter
//   cd api/swift && swift build
//   swift -I api/swift/.build/debug -L api/swift/.build/debug \
//     -lSlint -lSlintInterpreter -lSlintCBridge \
//     examples/swift/hello-world/main.swift

import Slint
import SlintInterpreter

// Compile the .slint file
let compiler = SlintCompiler()
guard let definition = compiler.buildFromSource(
    source: """
    export component HelloWorld inherits Window {
        in-out property <int> counter: 0;
        callback button-clicked;

        VerticalLayout {
            alignment: center;

            Text {
                text: "Hello, Swift! Count: " + counter;
                horizontal-alignment: center;
                font-size: 24px;
            }

            Button {
                text: "Click me";
                clicked => {
                    root.counter += 1;
                    root.button-clicked();
                }
            }
        }
    }
    """,
    path: "hello.slint"
) else {
    print("Compilation failed:")
    for diagnostic in compiler.diagnostics {
        print("  \(diagnostic.level): \(diagnostic.message)")
    }
    exit(1)
}

print("Component: \(definition.name)")
print("Properties: \(definition.properties.map { "\($0.name): \($0.type)" }.joined(separator: ", "))")
print("Callbacks: \(definition.callbacks.joined(separator: ", "))")

// Create an instance and show it
guard let instance = definition.createInstance() else {
    print("Failed to create instance")
    exit(1)
}

instance.show()
SlintEventLoop.run()
