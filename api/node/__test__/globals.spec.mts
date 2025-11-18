// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";

import { private_api } from "../dist/index.js";

test("get/set global properties", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export global Global { in-out property <string> name: "Initial"; }
    export component App {}`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    expect(instance!.getGlobalProperty("Global", "name")).toBe("Initial");

    instance!.setGlobalProperty("Global", "name", "Hello");
    expect(instance!.getGlobalProperty("Global", "name")).toBe("Hello");

    {
        let thrownError: any;
        try {
            instance!.getGlobalProperty("MyGlobal", "name");
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe("Global MyGlobal not found");
    }

    {
        let thrownError: any;
        try {
            instance!.setGlobalProperty("MyGlobal", "name", "hello");
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe("Global MyGlobal not found");
    }

    {
        let thrownError: any;
        try {
            instance!.getGlobalProperty("Global", "age");
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe("no such property");
    }

    {
        let thrownError: any;
        try {
            instance!.setGlobalProperty("Global", "age", 42);
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe(
            "Property age of global Global not found in the component",
        );
    }
    {
        let thrownError: any;
        try {
            instance!.setGlobalProperty("Global", "name", 42);
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("InvalidArg");
        expect(thrownError.message).toBe("expect String, got: Number");
    }

    {
        let thrownError: any;
        try {
            instance!.setGlobalProperty("Global", "name", { blah: "foo" });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("InvalidArg");
        expect(thrownError.message).toBe("expect String, got: Object");
    }
});

test("invoke global callback", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    {
        let thrownError: any;
        try {
            instance!.setGlobalCallback("MyGlobal", "great", () => { });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe("Global MyGlobal not found");
    }

    {
        let thrownError: any;
        try {
            instance!.invokeGlobal("MyGlobal", "great", []);
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe("Global MyGlobal not found");
    }

    let speakTest: string;
    instance!.setGlobalCallback(
        "Global",
        "great",
        (a: string, b: string, c: string, d: string, e: string) => {
            speakTest =
                "hello " + a + ", " + b + ", " + c + ", " + d + " and " + e;
        },
    );

    {
        let thrownError: any;
        try {
            instance!.setGlobalCallback("Global", "bye", () => { });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe(
            "Callback bye of global Global not found in the component",
        );
    }

    {
        let thrownError: any;
        try {
            instance!.invokeGlobal("Global", "bye", []);
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe(
            "Callback bye of global Global not found in the component",
        );
    }

    instance!.invokeGlobal("Global", "great", [
        "simon",
        "olivier",
        "auri",
        "tobias",
        "florian",
    ]);
    expect(speakTest).toStrictEqual(
        "hello simon, olivier, auri, tobias and florian",
    );

    instance!.setGlobalCallback("Global", "great-person", (p: any) => {
        speakTest = "hello " + p.name;
    });

    instance!.invokeGlobal("Global", "great-person", [{ name: "simon" }]);
    expect(speakTest).toStrictEqual("hello simon");

    instance!.invokeGlobal("Global", "great-person", [{ hello: "simon" }]);
    expect(speakTest).toStrictEqual("hello ");

    expect(instance!.invokeGlobal("Global", "get-string", [])).toStrictEqual(
        "string",
    );
    expect(instance!.invokeGlobal("Global", "person", [])).toStrictEqual({
        name: "florian",
    });
});
