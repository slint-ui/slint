import test from 'ava'
var Jimp = require("jimp");

import { ComponentCompiler, Brush, Model, Color, ImageData } from '../index'

test('get/set include paths', (t) => {
  let compiler = new ComponentCompiler;

  t.is(compiler.includePaths.length, 0);

  compiler.includePaths = ["path/one/", "path/two/", "path/three/"];

  t.deepEqual(compiler.includePaths, ["path/one/", "path/two/", "path/three/"]);
})

test('get/set style', (t) => {
  let compiler = new ComponentCompiler;

  t.is(compiler.style, null);

  compiler.style = "fluent";
  t.is(compiler.style, "fluent");
})

test('get/set build from source', (t) => {
  let compiler = new ComponentCompiler;

  let definition = compiler.buildFromSource(`export component App {}`, "");
  t.is(definition?.name, "App");
})


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

  instance.setProperty("player", {
    "name": "Simon",
    "age": 22,
  });

  t.deepEqual(instance.getProperty("player"), {
    "name": "Simon",
    "age": 22,
  });
})

test('get/set image properties', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
  export component App {
    in-out property <image> image: @image-url("__test__/resources/rgb.png");
  }
  `, "");
  let instance = definition?.create()!;

  let slintImage = instance.getProperty("image");
  if (t.true((slintImage instanceof ImageData))) {
    t.deepEqual((slintImage as ImageData).width, 64);
    t.deepEqual((slintImage as ImageData).height, 64);

    Jimp.read("__test__/resources/rgb.png")
      .then((image) => {
        t.deepEqual(image.bitmap.data, (slintImage as ImageData).data);
      })
      .catch((err) => {
        console.error(err);
      });
  }
})

test('get/set brush properties', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
  export component App {
    in-out property <brush> black: #000000;
    in-out property <brush> trans: transparent;
    in-out property <brush> ref: transparent;
  }
  `, "");
  let instance = definition?.create()!;

  let black = instance.getProperty("black");

  if (t.true((black instanceof Brush))) {
    let blackColor = (black as Brush).color;
    t.deepEqual(blackColor.red, 0);
    t.deepEqual(blackColor.green, 0);
    t.deepEqual(blackColor.blue, 0);
  }

  let transparent = instance.getProperty("trans");

  if (t.true((black instanceof Brush))) {
    t.assert((transparent as Brush).isTransparent);
  }

  let ref = Brush.fromColor(Color.fromRgb(100, 110, 120));
  instance.setProperty("ref", ref);

  let instance_ref = instance.getProperty("ref");

  if (t.true((instance_ref instanceof Brush))) {
    let ref_color = (instance_ref as Brush).color;
    t.deepEqual(ref_color.red, 100);
    t.deepEqual(ref_color.green, 110);
    t.deepEqual(ref_color.blue, 120);
  }
})

test('get/set model properties', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
  export component App {
    in-out property <[string]> model: ["Florian", "Simon"];
  }
  `, "");
  let instance = definition?.create()!;

  let model = instance.getProperty("model");

  if (t.true((model instanceof Model))) {
    t.deepEqual((model as Model).rowCount, 2);
  }
})

test('invoke callback', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
  export struct Person {
    name: string
  }
  export component App {
    callback great(string, string);
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
  `, "");
  let instance = definition?.create()!;
  let speakTest;

  instance.setCallback("great", (a: string, b: string) => {
    speakTest = "hello " + a + " and " + b;
  });

  instance.invoke("great", ["simon", "florian"]);
  t.deepEqual(speakTest, "hello simon and florian");

  instance.setCallback("great-person", (p: any) => {
    speakTest = "hello " + p.name;
  });

  instance.invoke("great-person", [{ "name": "simon" }]);
  t.deepEqual(speakTest, "hello simon");

  t.deepEqual( instance.invoke("get-string", []), "string");
  t.deepEqual(instance.invoke("person", []), { "name": "florian" });

})