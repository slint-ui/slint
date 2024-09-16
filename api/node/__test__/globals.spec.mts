// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import test from "ava";

import { private_api } from "../dist/index.js";

test("get/set global properties", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export global Global { in-out property <string> name: "Initial"; }
    export component App {}`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.is(instance!.getGlobalProperty("Global", "name"), "Initial");

    instance!.setGlobalProperty("Global", "name", "Hello");
    t.is(instance!.getGlobalProperty("Global", "name"), "Hello");

    t.throws(
        () => {
            instance!.getGlobalProperty("MyGlobal", "name");
        },
        {
            code: "GenericFailure",
            message: "Global MyGlobal not found",
        },
    );

    t.throws(
        () => {
            instance!.setGlobalProperty("MyGlobal", "name", "hello");
        },
        {
            code: "GenericFailure",
            message: "Global MyGlobal not found",
        },
    );

    t.throws(
        () => {
            instance!.getGlobalProperty("Global", "age");
        },
        {
            code: "GenericFailure",
            message: "no such property",
        },
    );

    t.throws(
        () => {
            instance!.setGlobalProperty("Global", "age", 42);
        },
        {
            code: "GenericFailure",
            message: "Property age of global Global not found in the component",
        },
    );

    t.throws(
        () => {
            instance!.setGlobalProperty("Global", "name", 42);
        },
        {
            code: "InvalidArg",
            message: "expect String, got: Number",
        },
    );

    t.throws(
        () => {
            instance!.setGlobalProperty("Global", "name", { blah: "foo" });
        },
        {
            code: "InvalidArg",
            message: "expect String, got: Object",
        },
    );
});

test("invoke global callback", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export struct Person {
    name: string
  }

  export global Global {
    callback great(string, string, string, string, string);
    callback great-person(Person);
    callback person() -> Person;
    callback get-string() -> string;

    person => {
      {
        name: "florian"
      }
    }

    get-string => {
      "string"
    }
  }
  export component App {}
  `,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.throws(
        () => {
            instance!.setGlobalCallback("MyGlobal", "great", () => {});
        },
        {
            code: "GenericFailure",
            message: "Global MyGlobal not found",
        },
    );

    t.throws(
        () => {
            instance!.invokeGlobal("MyGlobal", "great", []);
        },
        {
            code: "GenericFailure",
            message: "Global MyGlobal not found",
        },
    );

    let speakTest: string;
    instance!.setGlobalCallback(
        "Global",
        "great",
        (a: string, b: string, c: string, d: string, e: string) => {
            speakTest =
                "hello " + a + ", " + b + ", " + c + ", " + d + " and " + e;
        },
    );

    t.throws(
        () => {
            instance!.setGlobalCallback("Global", "bye", () => {});
        },
        {
            code: "GenericFailure",
            message: "Callback bye of global Global not found in the component",
        },
    );

    t.throws(
        () => {
            instance!.invokeGlobal("Global", "bye", []);
        },
        {
            code: "GenericFailure",
            message: "Callback bye of global Global not found in the component",
        },
    );

    instance!.invokeGlobal("Global", "great", [
        "simon",
        "olivier",
        "auri",
        "tobias",
        "florian",
    ]);
    t.deepEqual(speakTest, "hello simon, olivier, auri, tobias and florian");

    instance!.setGlobalCallback("Global", "great-person", (p: any) => {
        speakTest = "hello " + p.name;
    });

    instance!.invokeGlobal("Global", "great-person", [{ name: "simon" }]);
    t.deepEqual(speakTest, "hello simon");

    instance!.invokeGlobal("Global", "great-person", [{ hello: "simon" }]);
    t.deepEqual(speakTest, "hello ");

    t.deepEqual(instance!.invokeGlobal("Global", "get-string", []), "string");
    t.deepEqual(instance!.invokeGlobal("Global", "person", []), {
        name: "florian",
    });
});
