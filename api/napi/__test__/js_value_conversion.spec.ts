// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava';
const path = require('node:path');
var Jimp = require("jimp");

import { ComponentCompiler, Brush, Model, Color, ImageData } from '../index'

test('get/set string properties', (t) => {

  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`export component App { in-out property <string> name: "Initial"; }`, "");
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  t.is(instance!.getProperty("name"), "Initial");

  instance!.setProperty("name", "Hello");
  t.is(instance!.getProperty("name"), "Hello");

  t.throws(() => {
    instance!.setProperty("name", 42)
  },
    {
      code: "InvalidArg",
      message: "expect String, got: Number"
    }
  );

  t.throws(() => {
    instance!.setProperty("name", { "blah": "foo" })
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
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  t.is(instance!.getProperty("age"), 42);

  instance!.setProperty("age", 100);
  t.is(instance!.getProperty("age"), 100);

  t.throws(() => {
    instance!.setProperty("age", "Hello")
  },
    {
      code: "InvalidArg",
      message: "expect Number, got: String"
    }
  );

  t.throws(() => {
    instance!.setProperty("age", { "blah": "foo" })
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
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  t.is(instance!.getProperty("ready"), true);

  instance!.setProperty("ready", false);
  t.is(instance!.getProperty("ready"), false);

  t.throws(() => {
    instance!.setProperty("ready", "Hello")
  },
    {
      code: "InvalidArg",
      message: "expect Boolean, got: String"
    }
  );

  t.throws(() => {
    instance!.setProperty("ready", { "blah": "foo" })
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
    in-out property <bool> bool-prop;
    in-out property <Player> player: {
      name: "Florian",
      age: 20,
    };
  }
  `, "");
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  t.false(instance!.getProperty("bool-prop"));
  instance!.setProperty("bool-prop", true);
  t.true(instance!.getProperty("bool-prop"));

  t.deepEqual(instance!.getProperty("player"), {
    "name": "Florian",
    "age": 20,
  });

  instance!.setProperty("player", {
    "name": "Simon",
    "age": 22,
  });

  t.deepEqual(instance!.getProperty("player"), {
    "name": "Simon",
    "age": 22,
  });
})

test('get/set image properties', async (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
  export component App {
    callback image-width() -> float;
    callback image-height() -> float;

    in-out property <image> image: @image-url("resources/rgb.png");
    in property <image> alt-image;

    image-width => {
        alt-image.width
    }

    image-height => {
      alt-image.width
    }
  }
  `, __filename);
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  let slintImage = instance!.getProperty("image");
  if (t.true((slintImage instanceof ImageData))) {
    t.deepEqual((slintImage as ImageData).width, 64);
    t.deepEqual((slintImage as ImageData).height, 64);

    let image = await Jimp.read(path.join(__dirname, "resources/rgb.png"));
    t.is(image.bitmap.data.length, (slintImage as ImageData).data.length);
    t.deepEqual(image.bitmap.data, (slintImage as ImageData).data);

    instance!.invoke("image-width", [0]);
    instance!.invoke("image-height", [0]);
    instance!.setProperty("alt-image", new ImageData(image.bitmap.data, 64));
    instance!.invoke("image-width", [64]);
    instance!.invoke("image-height", [64]);
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
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  let black = instance!.getProperty("black");

  if (t.true((black instanceof Brush))) {
    let blackColor = (black as Brush).color;
    t.deepEqual(blackColor.red, 0);
    t.deepEqual(blackColor.green, 0);
    t.deepEqual(blackColor.blue, 0);
  }

  let transparent = instance!.getProperty("trans");

  if (t.true((black instanceof Brush))) {
    t.assert((transparent as Brush).isTransparent);
  }

  let ref = Brush.fromColor(Color.fromRgb(100, 110, 120));
  instance!.setProperty("ref", ref);

  let instance_ref = instance!.getProperty("ref");

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
  t.not(definition, null);
  let instance = definition!.create();
  t.not(instance, null);

  let model = instance!.getProperty("model");

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
  `, "");
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);
  let speakTest;

  instance!.setCallback("great", (a: string, b: string, c: string, d: string, e: string) => {
    speakTest = "hello " + a + ", " + b + ", " + c + ", " + d + " and " + e;
  });

  instance!.invoke("great", ["simon", "olivier", "auri", "tobias", "florian"]);
  t.deepEqual(speakTest, "hello simon, olivier, auri, tobias and florian");

  instance!.setCallback("great-person", (p: any) => {
    speakTest = "hello " + p.name;
  });

  instance!.invoke("great-person", [{ "name": "simon" }]);
  t.deepEqual(speakTest, "hello simon");

  t.throws(() => {
    instance!.invoke("great-person", [{ "hello": "simon" }]);
  },
    {
      code: "InvalidArg",
      message: "expect String, got: Undefined"
    }
  );

  t.deepEqual(instance!.invoke("get-string", []), "string");
  t.deepEqual(instance!.invoke("person", []), { "name": "florian" });
})
