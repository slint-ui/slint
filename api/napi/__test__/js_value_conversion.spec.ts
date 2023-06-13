import test from 'ava'

import { ComponentCompiler, ComponentDefinition, JsComponentInstance } from '../index'

test('get/set string properties', (t) => {

  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`export component App { in-out property <string> name: "Initial"; }`, "");
  let instance = definition?.create()!;

  t.is(instance.getProperty("name"), "Initial");

  instance.setProperty("name", "Hello");
  t.is(instance.getProperty("name"), "Hello");

  t.throws(() => {
    instance.setProperty("name", 42)
  },
    {
      code: "InvalidArg",
      message: "expect String, got: Number"
    }
  );

  t.throws(() => {
    instance.setProperty("name", { "blah": "foo" })
  },
    {
      code: "InvalidArg",
      message: "expect String, got: Object"
    }
  );

})


test('get/set number properties', (t) => {

  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`export component App { in-out property <float> age: 42; }`, "");
  let instance = definition?.create()!;

  t.is(instance.getProperty("age"), 42);

  instance.setProperty("age", 100);
  t.is(instance.getProperty("age"), 100);

  t.throws(() => {
    instance.setProperty("age", "Hello")
  },
    {
      code: "InvalidArg",
      message: "expect Number, got: String"
    }
  );

  t.throws(() => {
    instance.setProperty("age", { "blah": "foo" })
  },
    {
      code: "InvalidArg",
      message: "expect Number, got: Object"
    }
  );

})

test('get/set bool properties', (t) => {

  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`export component App { in-out property <bool> ready: true; }`, "");
  let instance = definition?.create()!;

  t.is(instance.getProperty("ready"), true);

  instance.setProperty("ready", false);
  t.is(instance.getProperty("ready"), false);

  t.throws(() => {
    instance.setProperty("ready", "Hello")
  },
    {
      code: "InvalidArg",
      message: "expect Boolean, got: String"
    }
  );

  t.throws(() => {
    instance.setProperty("ready", { "blah": "foo" })
  },
    {
      code: "InvalidArg",
      message: "expect Boolean, got: Object"
    }
  );

})

test('set struct properties', (t) => {

  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
  export struct Player {
    name: string,
    age: int
  }
  export component App {
    in-out property <Player> player: {
      name: "Florian",
      age: 20,
    }; 
  }
  `, "");
  let instance = definition?.create()!;

  t.deepEqual(instance.getProperty("player"), {
    "name": "Florian",
    "age": 20,
  });
})
