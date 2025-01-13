// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import test from "ava";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import { loadFile, loadSource, CompileError } from "../dist/index.js";

const dirname = path.dirname(
    fileURLToPath(import.meta.url).replace("build", "__test__"),
);

// loadFile api
test("loadFile", (t) => {
    const demo = loadFile(path.join(dirname, "resources/test.slint")) as any;
    const test = new demo.Test();
    t.is(test.check, "Test");

    const errorPath = path.join(dirname, "resources/error.slint");

    const error = t.throws(
        () => {
            loadFile(errorPath);
        },
        { instanceOf: CompileError },
    );

    const formattedDiagnostics = error?.diagnostics
        .map(
            (d) =>
                `[${d.fileName}:${d.lineNumber}:${d.columnNumber}] ${d.message}`,
        )
        .join("\n");
    t.is(
        error?.message,
        "Could not compile " +
            errorPath +
            `\nDiagnostics:\n${formattedDiagnostics}`,
    );
    t.deepEqual(error?.diagnostics, [
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

test("loadFile constructor parameters", (t) => {
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

    t.is(test.check, "test");
    t.is(hello, "hello");
});

test("loadFile component instances and modules are sealed", (t) => {
    const demo = loadFile(path.join(dirname, "resources/test.slint")) as any;

    t.throws(
        () => {
            demo.no_such_property = 42;
        },
        { instanceOf: TypeError },
    );

    const test = new demo.Test();
    t.is(test.check, "Test");

    t.throws(
        () => {
            test.no_such_callback = () => {};
        },
        { instanceOf: TypeError },
    );
});

// loadSource api
test("loadSource", (t) => {
    const source = `export component Test {
        out property <string> check: "Test";
    }`;
    const path = "api.spec.ts";
    const demo = loadSource(source, path) as any;
    const test = new demo.Test();
    t.is(test.check, "Test");

    const errorSource = `export component Error {
        out property bool> check: "Test";
    }`;

    const error = t.throws(
        () => {
            loadSource(errorSource, path);
        },
        { instanceOf: CompileError },
    );

    const formattedDiagnostics = error?.diagnostics
        .map(
            (d) =>
                `[${d.fileName}:${d.lineNumber}:${d.columnNumber}] ${d.message}`,
        )
        .join("\n");
    t.is(
        error?.message,
        "Could not compile " + path + `\nDiagnostics:\n${formattedDiagnostics}`,
    );
    // console.log(error?.diagnostics)
    t.deepEqual(error?.diagnostics, [
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

test("loadSource constructor parameters", (t) => {
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

    t.is(test.check, "test");
    t.is(hello, "hello");
});

test("loadSource component instances and modules are sealed", (t) => {
    const source = `export component Test {
        out property <string> check: "Test";
    }`;
    const path = "api.spec.ts";
    const demo = loadSource(source, path) as any;

    t.throws(
        () => {
            demo.no_such_property = 42;
        },
        { instanceOf: TypeError },
    );

    const test = new demo.Test();
    t.is(test.check, "Test");

    t.throws(
        () => {
            test.no_such_callback = () => {};
        },
        { instanceOf: TypeError },
    );
});

test("loadFile struct", (t) => {
    const demo = loadFile(
        path.join(dirname, "resources/test-struct.slint"),
    ) as any;

    const test = new demo.Test({
        check: new demo.TestStruct(),
    });

    t.deepEqual(test.check, { text: "", flag: false, value: 0 });
});

test("loadFile struct constructor parameters", (t) => {
    const demo = loadFile(
        path.join(dirname, "resources/test-struct.slint"),
    ) as any;

    const test = new demo.Test({
        check: new demo.TestStruct({ text: "text", flag: true, value: 12 }),
    });

    t.deepEqual(test.check, { text: "text", flag: true, value: 12 });

    test.check = new demo.TestStruct({
        text: "hello world",
        flag: false,
        value: 8,
    });
    t.deepEqual(test.check, { text: "hello world", flag: false, value: 8 });
});

test("loadFile struct constructor more parameters", (t) => {
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

    t.deepEqual(test.check, { text: "text", flag: true, value: 12 });
});

test("loadFile enum", (t) => {
    const demo = loadFile(
        path.join(dirname, "resources/test-enum.slint"),
    ) as any;

    const test = new demo.Test({
        check: demo.TestEnum.b,
    });

    t.deepEqual(test.check, "b");

    test.check = demo.TestEnum.c;

    t.deepEqual(test.check, "c");
});

test("file loader", (t) => {
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

    t.deepEqual(test.test_text, "Hello World");
});
