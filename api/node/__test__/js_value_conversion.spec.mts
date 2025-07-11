// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import test from "ava";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { Jimp } from "jimp";
import { captureStderr } from "capture-console";

import {
    private_api,
    type ImageData,
    ArrayModel,
    type Model,
} from "../dist/index.js";

const filename = fileURLToPath(import.meta.url).replace("build", "__test__");
const dirname = path.dirname(filename);

test("get/set string properties", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export component App { in-out property <string> name: "Initial"; }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.is(instance!.getProperty("name"), "Initial");

    instance!.setProperty("name", "Hello");
    t.is(instance!.getProperty("name"), "Hello");

    t.throws(
        () => {
            instance!.setProperty("name", 42);
        },
        {
            code: "InvalidArg",
            message: "expect String, got: Number",
        },
    );

    t.throws(
        () => {
            instance!.setProperty("name", { blah: "foo" });
        },
        {
            code: "InvalidArg",
            message: "expect String, got: Object",
        },
    );
});

test("get/set number properties", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export component App {
        in-out property <float> age: 42;
    }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.is(instance!.getProperty("age"), 42);

    instance!.setProperty("age", 100);
    t.is(instance!.getProperty("age"), 100);

    t.throws(
        () => {
            instance!.setProperty("age", "Hello");
        },
        {
            code: "InvalidArg",
            message: "expect Number, got: String",
        },
    );

    t.throws(
        () => {
            instance!.setProperty("age", { blah: "foo" });
        },
        {
            code: "InvalidArg",
            message: "expect Number, got: Object",
        },
    );
});

test("get/set bool properties", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export component App { in-out property <bool> ready: true; }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.is(instance!.getProperty("ready"), true);

    instance!.setProperty("ready", false);
    t.is(instance!.getProperty("ready"), false);

    t.throws(
        () => {
            instance!.setProperty("ready", "Hello");
        },
        {
            code: "InvalidArg",
            message: "expect Boolean, got: String",
        },
    );

    t.throws(
        () => {
            instance!.setProperty("ready", { blah: "foo" });
        },
        {
            code: "InvalidArg",
            message: "expect Boolean, got: Object",
        },
    );
});

test("set struct properties", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export struct Player {
    name: string,
    age: int,
    energy_level: float
  }
  export component App {
    in-out property <Player> player: {
      name: "Florian",
      age: 20,
      energy_level: 40%
    };
  }
  `,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.deepEqual(instance!.getProperty("player"), {
        name: "Florian",
        age: 20,
        energy_level: 0.4,
    });

    instance!.setProperty("player", {
        name: "Simon",
        age: 22,
        energy_level: 0.8,
    });

    t.deepEqual(instance!.getProperty("player"), {
        name: "Simon",
        age: 22,
        energy_level: 0.8,
    });

    // Extra properties are thrown away
    instance!.setProperty("player", {
        name: "Excessive Player",
        age: 100,
        energy_level: 0.8,
        weight: 200,
    });
    t.deepEqual(instance!.getProperty("player"), {
        name: "Excessive Player",
        age: 100,
        energy_level: 0.8,
    });

    // Missing properties are defaulted
    instance!.setProperty("player", { age: 39 });
    t.deepEqual(instance!.getProperty("player"), {
        name: "",
        age: 39,
        energy_level: 0.0,
    });
});

test("get/set image properties", async (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    in-out property <image> image: @image-url("resources/rgb.png");
    in property <image> external-image;
    out property <bool> external-image-ok: self.external-image.width == 64 && self.external-image.height == 64;
  }`,
        filename,
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    const slintImage = instance!.getProperty("image");
    if (t.true(slintImage instanceof private_api.SlintImageData)) {
        t.deepEqual((slintImage as private_api.SlintImageData).width, 64);
        t.deepEqual((slintImage as private_api.SlintImageData).height, 64);
        t.true((slintImage as ImageData).path.endsWith("rgb.png"));

        const image = await Jimp.read(path.join(dirname, "resources/rgb.png"));

        // Sanity check: setProperty fails when passed definitely a non-image
        t.throws(
            () => {
                instance!.setProperty("external-image", 42);
            },
            {
                message:
                    "Cannot convert object to image, because the provided object does not have an u32 `width` property",
            },
        );
        t.throws(
            () => {
                instance!.setProperty("external-image", { garbage: true });
            },
            {
                message:
                    "Cannot convert object to image, because the provided object does not have an u32 `width` property",
            },
        );
        t.throws(
            () => {
                instance!.setProperty("external-image", { width: [1, 2, 3] });
            },
            {
                message:
                    "Cannot convert object to image, because the provided object does not have an u32 `height` property",
            },
        );
        t.throws(
            () => {
                instance!.setProperty("external-image", {
                    width: 1,
                    height: 1,
                    data: new Uint8ClampedArray(1),
                });
            },
            {
                message:
                    "data property does not have the correct size; expected 1 (width) * 1 (height) * 4 = 1; got 4",
            },
        );

        t.is(image.bitmap.width, 64);
        t.is(image.bitmap.height, 64);
        // Duck typing: The `image.bitmap` object that Jump returns, has the shape of the official ImageData, so
        // it should be possible to use it with Slint:
        instance!.setProperty("external-image", image.bitmap);
        t.is(instance!.getProperty("external-image-ok"), true);

        t.is(image.bitmap.data.length, (slintImage as ImageData).data.length);
        t.deepEqual(image.bitmap.data, (slintImage as ImageData).data);

        t.deepEqual(
            (instance!.getProperty("external-image") as ImageData).path,
            undefined,
        );
    }
});

test("get/set brush properties", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    in-out property <brush> black: #000000;
    in-out property <brush> trans: transparent;
    in-out property <brush> ref: transparent;
    in-out property <brush> linear-gradient: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%);
    in-out property <brush> radial-gradient: @radial-gradient(circle, #f00 0%, #0f0 50%, #00f 100%);
    in-out property <color> ref-color;
  }
  `,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    const black = instance!.getProperty("black");

    t.is((black as private_api.SlintBrush).toString(), "#000000ff");

    if (t.true(black instanceof private_api.SlintBrush)) {
        const blackSlintRgbaColor = (black as private_api.SlintBrush).color;
        t.deepEqual(blackSlintRgbaColor.red, 0);
        t.deepEqual(blackSlintRgbaColor.green, 0);
        t.deepEqual(blackSlintRgbaColor.blue, 0);
    }

    instance?.setProperty("black", "#ffffff");
    const white = instance!.getProperty("black");

    if (t.true(white instanceof private_api.SlintBrush)) {
        const whiteSlintRgbaColor = (white as private_api.SlintBrush).color;
        t.deepEqual(whiteSlintRgbaColor.red, 255);
        t.deepEqual(whiteSlintRgbaColor.green, 255);
        t.deepEqual(whiteSlintRgbaColor.blue, 255);
    }

    const transparent = instance!.getProperty("trans");

    if (t.true(black instanceof private_api.SlintBrush)) {
        t.assert((transparent as private_api.SlintBrush).isTransparent);
    }

    const ref = new private_api.SlintBrush({
        red: 100,
        green: 110,
        blue: 120,
        alpha: 255,
    });
    instance!.setProperty("ref", ref);

    let instance_ref = instance!.getProperty("ref");

    if (t.true(instance_ref instanceof private_api.SlintBrush)) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        t.deepEqual(ref_color.red, 100);
        t.deepEqual(ref_color.green, 110);
        t.deepEqual(ref_color.blue, 120);
        t.deepEqual(ref_color.alpha, 255);
    }

    instance!.setProperty("ref", {
        color: { red: 110, green: 120, blue: 125, alpha: 255 },
    });

    instance_ref = instance!.getProperty("ref");

    if (t.true(instance_ref instanceof private_api.SlintBrush)) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        t.deepEqual(ref_color.red, 110);
        t.deepEqual(ref_color.green, 120);
        t.deepEqual(ref_color.blue, 125);
        t.deepEqual(ref_color.alpha, 255);
    }

    instance!.setProperty("ref", {
        red: 110,
        green: 120,
        blue: 125,
        alpha: 255,
    });

    instance_ref = instance!.getProperty("ref");

    if (t.true(instance_ref instanceof private_api.SlintBrush)) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        t.deepEqual(ref_color.red, 110);
        t.deepEqual(ref_color.green, 120);
        t.deepEqual(ref_color.blue, 125);
        t.deepEqual(ref_color.alpha, 255);
    }

    instance!.setProperty("ref", {});

    instance_ref = instance!.getProperty("ref");

    if (t.true(instance_ref instanceof private_api.SlintBrush)) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        t.deepEqual(ref_color.red, 0);
        t.deepEqual(ref_color.green, 0);
        t.deepEqual(ref_color.blue, 0);
        t.deepEqual(ref_color.alpha, 0);
    }

    const radialGradient = instance!.getProperty("radial-gradient");

    if (t.true(radialGradient instanceof private_api.SlintBrush)) {
        t.is(
            (radialGradient as private_api.SlintBrush).toString(),
            "radial-gradient(circle, rgba(255, 0, 0, 255) 0%, rgba(0, 255, 0, 255) 50%, rgba(0, 0, 255, 255) 100%)",
        );
    }

    const linearGradient = instance!.getProperty("linear-gradient");

    if (t.true(linearGradient instanceof private_api.SlintBrush)) {
        t.is(
            (linearGradient as private_api.SlintBrush).toString(),
            "linear-gradient(90deg, rgba(63, 135, 166, 255) 0%, rgba(235, 248, 225, 255) 50%, rgba(246, 157, 60, 255) 100%)",
        );
    }

    t.throws(
        () => {
            instance.setProperty("ref-color", {
                red: "abc",
                blue: 0,
                green: 0,
                alpha: 0,
            });
        },
        {
            code: "NumberExpected",
            message: "Failed to convert napi value String into rust type `f64`",
        },
    );

    t.throws(
        () => {
            instance.setProperty("ref-color", {
                red: 0,
                blue: true,
                green: 0,
                alpha: 0,
            });
        },
        {
            code: "NumberExpected",
            message:
                "Failed to convert napi value Boolean into rust type `f64`",
        },
    );

    t.throws(
        () => {
            instance.setProperty("ref-color", {
                red: 0,
                blue: 0,
                green: true,
                alpha: 0,
            });
        },
        {
            code: "NumberExpected",
            message:
                "Failed to convert napi value Boolean into rust type `f64`",
        },
    );

    t.throws(
        () => {
            instance.setProperty("ref-color", {
                red: 0,
                blue: 0,
                green: 0,
                alpha: new private_api.SlintRgbaColor(),
            });
        },
        {
            code: "NumberExpected",
            message: "Failed to convert napi value Object into rust type `f64`",
        },
    );

    t.throws(
        () => {
            instance.setProperty("ref-color", { blue: 0, green: 0, alpha: 0 });
        },
        {
            code: "GenericFailure",
            message: "Property red is missing",
        },
    );

    t.throws(
        () => {
            instance.setProperty("ref-color", { red: 0, green: 0, alpha: 0 });
        },
        {
            code: "GenericFailure",
            message: "Property blue is missing",
        },
    );

    instance.setProperty("ref-color", { red: 0, green: 0, blue: 0 });
    instance_ref = instance!.getProperty("ref-color");

    if (t.true(instance_ref instanceof private_api.SlintBrush)) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        t.deepEqual(ref_color.red, 0);
        t.deepEqual(ref_color.green, 0);
        t.deepEqual(ref_color.blue, 0);
        t.deepEqual(ref_color.alpha, 255);
    }

    // ref is a brush, but setting to a color should not throw, but take the brush's color.
    instance!.setProperty("ref-color", ref);
    instance_ref = instance!.getProperty("ref-color");
    if (t.true(instance_ref instanceof private_api.SlintBrush)) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        t.deepEqual(ref_color.red, ref.color.red);
        t.deepEqual(ref_color.green, ref.color.green);
        t.deepEqual(ref_color.blue, ref.color.blue);
        t.deepEqual(ref_color.alpha, ref.color.alpha);
    }
});

test("get/set enum properties", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export enum Direction { up, down }
         export component App { in-out property <Direction> direction: up; }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.is(instance!.getProperty("direction"), "up");

    instance!.setProperty("direction", "down");
    t.is(instance!.getProperty("direction"), "down");

    t.throws(
        () => {
            instance!.setProperty("direction", 42);
        },
        {
            code: "InvalidArg",
            message: "expect String, got: Number",
        },
    );

    t.throws(
        () => {
            instance!.setProperty("direction", { blah: "foo" });
        },
        {
            code: "InvalidArg",
            message: "expect String, got: Object",
        },
    );

    t.throws(
        () => {
            instance!.setProperty("direction", "left");
        },
        {
            code: "GenericFailure",
            message: "left is not a value of enum Direction",
        },
    );
});

test("ArrayModel", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export struct Player {
    name: string,
    age: int
  }

  export component App {
    in-out property <[int]> int-model;
    in-out property <[string]> string-model;
    in-out property <[Player]> struct-model;
  }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.deepEqual(Array.from(new ArrayModel([3, 2, 1])), [3, 2, 1]);

    instance!.setProperty("int-model", new ArrayModel([10, 9, 8]));

    const intArrayModel = instance!.getProperty(
        "int-model",
    ) as ArrayModel<number>;
    t.deepEqual(intArrayModel.rowCount(), 3);
    t.deepEqual(intArrayModel.values(), new ArrayModel([10, 9, 8]).values());

    instance!.setProperty(
        "string-model",
        new ArrayModel(["Simon", "Olivier", "Auri", "Tobias", "Florian"]),
    );

    const stringArrayModel = instance!.getProperty(
        "string-model",
    ) as ArrayModel<number>;
    t.deepEqual(
        stringArrayModel.values(),
        new ArrayModel([
            "Simon",
            "Olivier",
            "Auri",
            "Tobias",
            "Florian",
        ]).values(),
    );

    instance!.setProperty(
        "struct-model",
        new ArrayModel([
            { name: "simon", age: 22 },
            { name: "florian", age: 22 },
        ]),
    );

    const structArrayModel = instance!.getProperty(
        "struct-model",
    ) as ArrayModel<object>;
    t.deepEqual(
        structArrayModel.values(),
        new ArrayModel([
            { name: "simon", age: 22 },
            { name: "florian", age: 22 },
        ]).values(),
    );
});

test("MapModel", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export component App {
      in-out property <[string]> model;
    }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    interface Name {
        first: string;
        last: string;
    }

    const nameModel: ArrayModel<Name> = new ArrayModel([
        { first: "Hans", last: "Emil" },
        { first: "Max", last: "Mustermann" },
        { first: "Roman", last: "Tisch" },
    ]);

    const mapModel = new private_api.MapModel(nameModel, (data) => {
        return data.last + ", " + data.first;
    });

    instance!.setProperty("model", mapModel);

    nameModel.setRowData(0, { first: "Simon", last: "Hausmann" });
    nameModel.setRowData(1, { first: "Olivier", last: "Goffart" });

    const checkModel = instance!.getProperty("model") as Model<string>;
    t.is(checkModel.rowData(0), "Hausmann, Simon");
    t.is(checkModel.rowData(1), "Goffart, Olivier");
    t.is(checkModel.rowData(2), "Tisch, Roman");
});

test("MapModel undefined rowData sourcemodel", (t) => {
    const nameModel: ArrayModel<number> = new ArrayModel([1, 2, 3]);

    let mapFunctionCallCount = 0;
    const mapModel = new private_api.MapModel<number, string>(
        nameModel,
        (data) => {
            mapFunctionCallCount++;
            return data.toString();
        },
    );

    for (let i = 0; i < mapModel.rowCount(); ++i) {
        mapModel.rowData(i);
    }
    t.deepEqual(mapFunctionCallCount, mapModel.rowCount());
    mapFunctionCallCount = 0;
    t.is(nameModel.rowData(nameModel.rowCount()), undefined);
    t.deepEqual(mapFunctionCallCount, 0);
    t.is(mapModel.rowData(mapModel.rowCount()), undefined);
    t.deepEqual(mapFunctionCallCount, 0);
});

test("ArrayModel rowCount", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    out property <int> model-length: model.length;
    in-out property <[int]> model;
  }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    const model = new ArrayModel([10, 9, 8]);

    instance!.setProperty("model", model);
    t.is(3, model.rowCount());
    t.is(3, instance?.getProperty("model-length") as number);
});

test("ArrayModel rowData/setRowData", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    callback data(int) -> int;

    in-out property <[int]> model;

    data(row) => {
      model[row]
    }
  }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    const model = new ArrayModel([10, 9, 8]);

    instance!.setProperty("model", model);
    t.is(9, model.rowData(1));
    t.deepEqual(instance!.invoke("data", [1]), 9);

    model.setRowData(1, 4);
    t.is(4, model.rowData(1));
    t.deepEqual(instance!.invoke("data", [1]), 4);
});

test("Model notify", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    width: 300px;
    height: 300px;

    out property<length> layout-height: layout.height;
    in-out property<[length]> fixed-height-model;

    VerticalLayout {
      alignment: start;

      layout := VerticalLayout {
        for fixed-height in fixed-height-model: Rectangle {
            background: blue;
            height: fixed-height;
        }
      }
    }

  }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    const model = new ArrayModel([100, 0]);

    instance!.setProperty("fixed-height-model", model);
    t.is(100, instance!.getProperty("layout-height") as number);
    model.setRowData(1, 50);
    t.is(150, instance!.getProperty("layout-height") as number);
    model.push(75);
    t.is(225, instance!.getProperty("layout-height") as number);
    model.remove(1, 2);
    t.is(100, instance!.getProperty("layout-height") as number);
});

test("model from array", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    in-out property <[int]> int-array;
    in-out property <[string]> string-array;
  }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    instance!.setProperty("int-array", [10, 9, 8]);
    const wrapped_int_model = instance!.getProperty(
        "int-array",
    ) as Model<number>;
    t.deepEqual(Array.from(wrapped_int_model), [10, 9, 8]);
    t.deepEqual(wrapped_int_model.rowCount(), 3);
    t.deepEqual(wrapped_int_model.rowData(0), 10);
    t.deepEqual(wrapped_int_model.rowData(1), 9);
    t.deepEqual(wrapped_int_model.rowData(2), 8);
    t.deepEqual(Array.from(wrapped_int_model), [10, 9, 8]);

    instance!.setProperty("string-array", [
        "Simon",
        "Olivier",
        "Auri",
        "Tobias",
        "Florian",
    ]);
    const wrapped_string_model = instance!.getProperty(
        "string-array",
    ) as Model<string>;
    t.deepEqual(wrapped_string_model.rowCount(), 5);
    t.deepEqual(wrapped_string_model.rowData(0), "Simon");
    t.deepEqual(wrapped_string_model.rowData(1), "Olivier");
    t.deepEqual(wrapped_string_model.rowData(2), "Auri");
    t.deepEqual(wrapped_string_model.rowData(3), "Tobias");
    t.deepEqual(wrapped_string_model.rowData(4), "Florian");
});

test("invoke callback", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
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
  `,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);
    let speakTest: string;

    instance!.setCallback(
        "great",
        (a: string, b: string, c: string, d: string, e: string) => {
            speakTest =
                "hello " + a + ", " + b + ", " + c + ", " + d + " and " + e;
        },
    );

    instance!.invoke("great", [
        "simon",
        "olivier",
        "auri",
        "tobias",
        "florian",
    ]);
    t.deepEqual(speakTest, "hello simon, olivier, auri, tobias and florian");

    instance!.setCallback("great-person", (p: any) => {
        speakTest = "hello " + p.name;
    });

    instance!.invoke("great-person", [{ name: "simon" }]);
    t.deepEqual(speakTest, "hello simon");

    instance!.invoke("great-person", [{ hello: "simon" }]);
    t.deepEqual(speakTest, "hello ");

    t.deepEqual(instance!.invoke("get-string", []), "string");
    t.deepEqual(instance!.invoke("person", []), { name: "florian" });
});

test("wrong callback return type ", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export struct Person {
    name: string,
    age: int,

  }
  export component App {
    callback get-string() -> string;
    callback get-int() -> int;
    callback get-bool() -> bool;
    callback get-person() -> Person;
  }
  `,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);
    let speakTest: string;

    instance!.setCallback("get-string", () => {
        return 20;
    });

    const string = instance!.invoke("get-string", []);
    t.deepEqual(string, "");

    instance!.setCallback("get-int", () => {
        return "string";
    });

    const int = instance!.invoke("get-int", []);
    t.deepEqual(int, 0);

    instance!.setCallback("get-bool", () => {
        return "string";
    });

    const bool = instance!.invoke("get-bool", []);
    t.deepEqual(bool, false);

    instance!.setCallback("get-person", () => {
        return "string";
    });

    const person = instance!.invoke("get-person", []);
    t.deepEqual(person, { name: "", age: 0 });
});

test("wrong global callback return type ", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
        export struct Person {
            name: string,
            age: int,
        }
        export global Global {
            callback get-string() -> string;
            callback get-int() -> int;
            callback get-bool() -> bool;
            callback get-person() -> Person;
        }
        export component App {
        }
  `,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);
    let speakTest: string;

    instance!.setGlobalCallback("Global", "get-string", () => {
        return 20;
    });

    const string = instance!.invokeGlobal("Global", "get-string", []);
    t.deepEqual(string, "");

    instance!.setGlobalCallback("Global", "get-bool", () => {
        return "string";
    });

    const bool = instance!.invokeGlobal("Global", "get-bool", []);
    t.deepEqual(bool, false);

    instance!.setGlobalCallback("Global", "get-person", () => {
        return "string";
    });

    const person = instance!.invokeGlobal("Global", "get-person", []);
    t.deepEqual(person, { name: "", age: 0 });
});

test("throw exception in callback", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    callback throw-something();
  }
  `,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);
    let speakTest: string;

    instance!.setCallback("throw-something", () => {
        throw new Error("I'm an error");
    });

    const output = captureStderr(() => {
        instance!.invoke("throw-something", []);
    });
    t.assert(
        output.includes("Node.js: Invoking callback 'throw-something' failed:"),
        `Output was ${output}`,
    );
    t.assert(output.includes("I'm an error"), `Output was ${output}`);
});

test("throw exception set color", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    in-out property <color> test;
  }
  `,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    t.throws(
        () => {
            instance!.setProperty("test", { garbage: true });
        },
        {
            code: "GenericFailure",
            message: "Property red is missing",
        },
    );
});
