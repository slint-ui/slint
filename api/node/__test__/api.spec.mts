// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import {
    loadFile,
    loadSource,
    CompileError,
    StyledText,
} from "../dist/index.js";

const dirname = path.dirname(
    fileURLToPath(import.meta.url).replace("build", "__test__"),
);

// loadFile api
test("loadFile", () => {
    // Test the URL variant here, to ensure that it works (esp. on Windows)
    const demo = loadFile(
        new URL(
            "resources/test.slint",
            import.meta.url.replace("build", "__test__"),
        ),
    ) as any;
    const test = new demo.Test();
    expect(test.check).toBe("Test");

    const errorPath = path.join(dirname, "resources/error.slint");

    let error: any;
    try {
        loadFile(errorPath);
    } catch (e) {
        error = e;
    }
    expect(error).toBeDefined();
    expect(error).toBeInstanceOf(CompileError);

    const formattedDiagnostics = error.diagnostics
        .map(
            (d) =>
                `[${d.fileName}:${d.lineNumber}:${d.columnNumber}] ${d.message}`,
        )
        .join("\n");
    expect(error.message).toBe(
        "Could not compile " +
            errorPath +
            `\nDiagnostics:\n${formattedDiagnostics}`,
    );
    expect(error.diagnostics).toStrictEqual([
        {
            columnNumber: 18,
            level: 0,
            lineNumber: 5,
            message:
                "Missing type. The syntax to declare a property is `property <type> name;`. Only two way bindings can omit the type",
            fileName: errorPath,
        },
        {
            columnNumber: 22,
            level: 0,
            lineNumber: 5,
            message: "Syntax error: expected ';'",
            fileName: errorPath,
        },
        {
            columnNumber: 22,
            level: 0,
            lineNumber: 5,
            message: "Parse error",
            fileName: errorPath,
        },
    ]);
});

test("loadFile constructor parameters", () => {
    const demo = loadFile(
        path.join(dirname, "resources/test-constructor.slint"),
    ) as any;
    let hello = "";
    const test = new demo.Test({
        say_hello: function () {
            hello = "hello";
        },
        check: "test",
    });

    test.say_hello();

    expect(test.check).toBe("test");
    expect(hello).toBe("hello");
});

test("loadFile component instances and modules are sealed", () => {
    const demo = loadFile(path.join(dirname, "resources/test.slint")) as any;

    {
        let thrownError: any;
        try {
            demo.no_such_property = 42;
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError).toBeInstanceOf(TypeError);
    }

    const test = new demo.Test();
    expect(test.check).toBe("Test");

    {
        let thrownError: any;
        try {
            test.no_such_callback = () => {};
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError).toBeInstanceOf(TypeError);
    }
});

// loadSource api
test("loadSource", () => {
    const source = `export component Test {
        out property <string> check: "Test";
    }`;
    const path = "api.spec.ts";
    const demo = loadSource(source, path) as any;
    const test = new demo.Test();
    expect(test.check).toBe("Test");

    const errorSource = `export component Error {
        out property bool> check: "Test";
    }`;

    let error: any;
    try {
        loadSource(errorSource, path);
    } catch (e) {
        error = e;
    }
    expect(error).toBeDefined();
    expect(error).toBeInstanceOf(CompileError);

    const formattedDiagnostics = error.diagnostics
        .map(
            (d) =>
                `[${d.fileName}:${d.lineNumber}:${d.columnNumber}] ${d.message}`,
        )
        .join("\n");
    expect(error.message).toBe(
        "Could not compile " + path + `\nDiagnostics:\n${formattedDiagnostics}`,
    );
    // console.log(error?.diagnostics)
    expect(error.diagnostics).toStrictEqual([
        {
            columnNumber: 22,
            level: 0,
            lineNumber: 2,
            message:
                "Missing type. The syntax to declare a property is `property <type> name;`. Only two way bindings can omit the type",
            fileName: path,
        },
        {
            columnNumber: 26,
            level: 0,
            lineNumber: 2,
            message: "Syntax error: expected ';'",
            fileName: path,
        },
        {
            columnNumber: 26,
            level: 0,
            lineNumber: 2,
            message: "Parse error",
            fileName: path,
        },
    ]);
});

test("non-windowed components have no `window` property", () => {
    const source = `
        export component Win inherits Window {
            in-out property <string> name: "world";
        }
        export component Tray inherits SystemTrayIcon {
            callback do-something();
        }
    `;
    const mod = loadSource(source, "api.spec.ts") as any;

    const win = new mod.Win();
    const tray = new mod.Tray();

    expect("window" in win).toBe(true);
    expect("window" in tray).toBe(false);
});

test("loadSource constructor parameters", () => {
    const source = `export component Test {
        callback say_hello();
        in-out property <string> check;
    }`;
    const path = "api.spec.ts";
    const demo = loadSource(source, path) as any;
    let hello = "";
    const test = new demo.Test({
        say_hello: function () {
            hello = "hello";
        },
        check: "test",
    });

    test.say_hello();

    expect(test.check).toBe("test");
    expect(hello).toBe("hello");
});

test("loadSource component instances and modules are sealed", () => {
    const source = `export component Test {
        out property <string> check: "Test";
    }`;
    const path = "api.spec.ts";
    const demo = loadSource(source, path) as any;

    {
        let thrownError: any;
        try {
            demo.no_such_property = 42;
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError).toBeInstanceOf(TypeError);
    }

    const test = new demo.Test();
    expect(test.check).toBe("Test");

    {
        let thrownError: any;
        try {
            test.no_such_callback = () => {};
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError).toBeInstanceOf(TypeError);
    }
});

test("loadFile struct", () => {
    const demo = loadFile(
        path.join(dirname, "resources/test-struct.slint"),
    ) as any;

    const test = new demo.Test({
        check: new demo.TestStruct(),
    });

    expect(test.check).toStrictEqual({ text: "", flag: false, value: 0 });
});

test("loadFile struct constructor parameters", () => {
    const demo = loadFile(
        path.join(dirname, "resources/test-struct.slint"),
    ) as any;

    const test = new demo.Test({
        check: new demo.TestStruct({ text: "text", flag: true, value: 12 }),
    });

    expect(test.check).toStrictEqual({ text: "text", flag: true, value: 12 });

    test.check = new demo.TestStruct({
        text: "hello world",
        flag: false,
        value: 8,
    });
    expect(test.check).toStrictEqual({
        text: "hello world",
        flag: false,
        value: 8,
    });
});

test("loadFile struct constructor more parameters", () => {
    const demo = loadFile(
        path.join(dirname, "resources/test-struct.slint"),
    ) as any;

    const test = new demo.Test({
        check: new demo.TestStruct({
            text: "text",
            flag: true,
            value: 12,
            noProp: "hello",
        }),
    });

    expect(test.check).toStrictEqual({ text: "text", flag: true, value: 12 });
});

test("loadFile enum", () => {
    const demo = loadFile(
        path.join(dirname, "resources/test-enum.slint"),
    ) as any;

    const test = new demo.Test({
        check: demo.TestEnum.b,
    });

    expect(test.check).toStrictEqual("b");

    test.check = demo.TestEnum.c;

    expect(test.check).toStrictEqual("c");
});

test("loadSource styled-text property get/set", () => {
    const source = `export component App {
        in-out property <styled-text> content;
    }`;
    const demo = loadSource(source, "api.spec.ts") as any;
    const app = new demo.App();

    const st = StyledText.fromPlainText("hello world");
    app.content = st;

    const result = app.content;
    expect(result).toBeInstanceOf(StyledText);
    expect(result.equals(st)).toBe(true);
});

test("loadSource styled-text property with markdown", () => {
    const source = `export component App {
        in-out property <styled-text> content;
    }`;
    const demo = loadSource(source, "api.spec.ts") as any;
    const app = new demo.App();

    const st = StyledText.fromMarkdown("**bold** and *italic*");
    app.content = st;

    const result = app.content;
    expect(result).toBeInstanceOf(StyledText);
    expect(result.equals(st)).toBe(true);
});

test("loadSource styled-text default is returned as StyledText", () => {
    const source = `export component App {
        in-out property <styled-text> content;
    }`;
    const demo = loadSource(source, "api.spec.ts") as any;
    const app = new demo.App();

    const result = app.content;
    expect(result).toBeInstanceOf(StyledText);
});

test("loadSource styled-text in callback argument", () => {
    const source = `export component App {
        in-out property <styled-text> content;
        callback format(styled-text) -> styled-text;
    }`;
    const demo = loadSource(source, "api.spec.ts") as any;
    const app = new demo.App({
        format: (st: InstanceType<typeof StyledText>) => {
            expect(st).toBeInstanceOf(StyledText);
            return StyledText.fromPlainText("formatted");
        },
    });

    const input = StyledText.fromPlainText("input");
    const result = app.format(input);
    expect(result).toBeInstanceOf(StyledText);
    expect(result.equals(StyledText.fromPlainText("formatted"))).toBe(true);
});

test("loadSource styled-text constructor parameter", () => {
    const source = `export component App {
        in-out property <styled-text> content;
    }`;
    const demo = loadSource(source, "api.spec.ts") as any;
    const st = StyledText.fromPlainText("initial");
    const app = new demo.App({ content: st });

    const result = app.content;
    expect(result).toBeInstanceOf(StyledText);
    expect(result.equals(st)).toBe(true);
});

test("loadSource styled-text with inline markdown expression", () => {
    const source = `export component App {
        out property <styled-text> content: @markdown("hello **world**");
    }`;
    const demo = loadSource(source, "api.spec.ts") as any;
    const app = new demo.App();

    const result = app.content;
    expect(result).toBeInstanceOf(StyledText);

    const expected = StyledText.fromMarkdown("hello **world**");
    expect(result.equals(expected)).toBe(true);
});

test("StyledText.fromMarkdown throws on unsupported HTML tag", () => {
    let thrownError: any;
    try {
        StyledText.fromMarkdown("<span>text</span>");
    } catch (error) {
        thrownError = error;
    }
    expect(thrownError).toBeDefined();
    expect(thrownError).toBeInstanceOf(Error);
    expect(thrownError.message).toBe("HTML tag <span> is not supported");
});

test("StyledText.fromMarkdown throws on unsupported markdown syntax", () => {
    let thrownError: any;
    try {
        StyledText.fromMarkdown("![alt](image.png)");
    } catch (error) {
        thrownError = error;
    }
    expect(thrownError).toBeDefined();
    expect(thrownError).toBeInstanceOf(Error);
    expect(thrownError.message).toBe("Markdown images are not supported");
});

test("StyledText.fromMarkdown throws on invalid color", () => {
    let thrownError: any;
    try {
        StyledText.fromMarkdown('<font color="notacolor">text</font>');
    } catch (error) {
        thrownError = error;
    }
    expect(thrownError).toBeDefined();
    expect(thrownError).toBeInstanceOf(Error);
    expect(thrownError.message).toBe("Invalid color value 'notacolor'");
});

test("StyledText.fromMarkdown reports multiple errors", () => {
    let thrownError: any;
    try {
        StyledText.fromMarkdown('<div>block</div>\n<img src="x">');
    } catch (error) {
        thrownError = error;
    }
    expect(thrownError).toBeDefined();
    expect(thrownError).toBeInstanceOf(Error);
    expect(thrownError.message).toContain("are not supported");
    // The message contains multiple errors separated by newlines
    const lines = thrownError.message.split("\n");
    expect(lines.length).toBeGreaterThanOrEqual(2);
});

test("file loader", () => {
    const testSource = `export component Test {
       in-out property <string> text: "Hello World";
    }`;
    const demo = loadFile(
        path.join(dirname, "resources/test-fileloader.slint"),
        {
            fileLoader: (path) => {
                if (path.includes("lib.slint")) {
                    return testSource;
                }
                return "";
            },
        },
    ) as any;

    const test = new demo.App();

    expect(test.test_text).toStrictEqual("Hello World");
});
