// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava';
const path = require('node:path');

import { ComponentCompiler } from '../index'

test('get/set global properties', (t) => {

  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
    export global Global { in-out property <string> name: "Initial"; }
    export component App {}`, "");
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  t.is(instance!.getGlobalProperty("Global", "name"), "Initial");

  instance!.setGlobalProperty("Global", "name", "Hello");
  t.is(instance!.getGlobalProperty("Global", "name"), "Hello");

  t.throws(() => {
    instance!.getGlobalProperty("MyGlobal", "name")
  },
    {
      code: "GenericFailure",
      message: "Global MyGlobal not found"
    }
  );

  t.throws(() => {
    instance!.setGlobalProperty("MyGlobal", "name", "hello")
  },
    {
      code: "GenericFailure",
      message: "Global MyGlobal not found"
    }
  );

  t.throws(() => {
    instance!.setGlobalProperty("Global", "name", 42)
  },
    {
      code: "InvalidArg",
      message: "expect String, got: Number"
    }
  );

  t.throws(() => {
    instance!.setGlobalProperty("Global", "name", { "blah": "foo" })
  },
    {
      code: "InvalidArg",
      message: "expect String, got: Object"
    }
  );

})

test('invoke global callback', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
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
  `, "");
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);
  let speakTest;

  t.throws(() => {
    instance!.setGlobalCallback("MyGlobal", "great", () => {})
  },
    {
      code: "GenericFailure",
      message: "Global MyGlobal not found"
    }
  );

  t.throws(() => {
    instance!.invokeGlobal("MyGlobal", "great", [])
  },
    {
      code: "GenericFailure",
      message: "Global MyGlobal not found"
    }
  );

  instance!.setGlobalCallback("Global", "great", (a: string, b: string, c: string, d: string, e: string) => {
    speakTest = "hello " + a + ", " + b + ", " + c + ", " + d + " and " + e;
  });

  instance!.invokeGlobal("Global", "great", ["simon", "olivier", "auri", "tobias", "florian"]);
  t.deepEqual(speakTest, "hello simon, olivier, auri, tobias and florian");

  instance!.setGlobalCallback("Global", "great-person", (p: any) => {
    speakTest = "hello " + p.name;
  });

  instance!.invokeGlobal("Global", "great-person", [{ "name": "simon" }]);
  t.deepEqual(speakTest, "hello simon");

  t.throws(() => {
    instance!.invokeGlobal("Global", "great-person", [{ "hello": "simon" }]);
  },
    {
      code: "InvalidArg",
      message: "expect String, got: Undefined"
    }
  );

  t.deepEqual(instance!.invokeGlobal("Global", "get-string", []), "string");
  t.deepEqual(instance!.invokeGlobal("Global", "person", []), { "name": "florian" });
})
