// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { Jimp } from "jimp";
import { captureAsyncStderr } from "./helpers/utils.js";
import {
    private_api,
    type ImageData,
    ArrayModel,
    type Model,
} from "../dist/index.js";

const filename = fileURLToPath(import.meta.url).replace("build", "__test__");
const dirname = path.dirname(filename);

test("get/set string properties", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export component App { in-out property <string> name: "Initial"; }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    expect(instance!.getProperty("name")).toBe("Initial");

    instance!.setProperty("name", "Hello");
    expect(instance!.getProperty("name")).toBe("Hello");

    {
        let thrownError: any;
        try {
            instance!.setProperty("name", 42);
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
            instance!.setProperty("name", { blah: "foo" });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("InvalidArg");
        expect(thrownError.message).toBe("expect String, got: Object");
    }
});

test("get/set number properties", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export component App {
        in-out property <float> age: 42;
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    expect(instance!.getProperty("age")).toBe(42);

    instance!.setProperty("age", 100);
    expect(instance!.getProperty("age")).toBe(100);

    {
        let thrownError: any;
        try {
            instance!.setProperty("age", "Hello");
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("InvalidArg");
        expect(thrownError.message).toBe("expect Number, got: String");
    }

    {
        let thrownError: any;
        try {
            instance!.setProperty("age", { blah: "foo" });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("InvalidArg");
        expect(thrownError.message).toBe("expect Number, got: Object");
    }
});

test("get/set bool properties", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export component App { in-out property <bool> ready: true; }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    expect(instance!.getProperty("ready")).toBe(true);

    instance!.setProperty("ready", false);
    expect(instance!.getProperty("ready")).toBe(false);

    {
        let thrownError: any;
        try {
            instance!.setProperty("ready", "Hello");
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("InvalidArg");
        expect(thrownError.message).toBe("expect Boolean, got: String");
    }

    {
        let thrownError: any;
        try {
            instance!.setProperty("ready", { blah: "foo" });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("InvalidArg");
        expect(thrownError.message).toBe("expect Boolean, got: Object");
    }
});

test("set struct properties", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    expect(instance!.getProperty("player")).toStrictEqual({
        name: "Florian",
        age: 20,
        energy_level: 0.4,
    });

    instance!.setProperty("player", {
        name: "Simon",
        age: 22,
        energy_level: 0.8,
    });

    expect(instance!.getProperty("player")).toStrictEqual({
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
    expect(instance!.getProperty("player")).toStrictEqual({
        name: "Excessive Player",
        age: 100,
        energy_level: 0.8,
    });

    // Missing properties are defaulted
    instance!.setProperty("player", { age: 39 });
    expect(instance!.getProperty("player")).toStrictEqual({
        name: "",
        age: 39,
        energy_level: 0.0,
    });
});

test("get/set image properties", async () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const slintImage = instance!.getProperty("image");
    if (slintImage instanceof private_api.SlintImageData) {
        expect((slintImage as private_api.SlintImageData).width).toStrictEqual(
            64,
        );
        expect((slintImage as private_api.SlintImageData).height).toStrictEqual(
            64,
        );
        expect((slintImage as ImageData).path.endsWith("rgb.png")).toBe(true);

        const image = await Jimp.read(path.join(dirname, "resources/rgb.png"));

        // Sanity check: setProperty fails when passed definitely a non-image
        {
            let thrownError: any;
            try {
                instance!.setProperty("external-image", 42);
            } catch (error) {
                thrownError = error;
            }
            expect(thrownError).toBeDefined();
            expect(thrownError.message).toBe(
                "Cannot convert object to image, because the provided object does not have an u32 `width` property",
            );
        }
        {
            let thrownError: any;
            try {
                instance!.setProperty("external-image", { garbage: true });
            } catch (error) {
                thrownError = error;
            }
            expect(thrownError).toBeDefined();
            expect(thrownError.message).toBe(
                "Cannot convert object to image, because the provided object does not have an u32 `width` property",
            );
        }
        {
            let thrownError: any;
            try {
                instance!.setProperty("external-image", { width: [1, 2, 3] });
            } catch (error) {
                thrownError = error;
            }
            expect(thrownError).toBeDefined();
            expect(thrownError.message).toBe(
                "Cannot convert object to image, because the provided object does not have an u32 `height` property",
            );
        }
        {
            let thrownError: any;
            try {
                instance!.setProperty("external-image", {
                    width: 1,
                    height: 1,
                    data: new Uint8ClampedArray(1),
                });
            } catch (error) {
                thrownError = error;
            }
            expect(thrownError).toBeDefined();
            expect(thrownError.message).toBe(
                "data property does not have the correct size; expected 1 (width) * 1 (height) * 4 = 1; got 4",
            );
        }

        expect(image.bitmap.width).toBe(64);
        expect(image.bitmap.height).toBe(64);
        // Duck typing: The `image.bitmap` object that Jump returns, has the shape of the official ImageData, so
        // it should be possible to use it with Slint:
        instance!.setProperty("external-image", image.bitmap);
        expect(instance!.getProperty("external-image-ok")).toBe(true);

        expect(image.bitmap.data.length).toBe(
            (slintImage as ImageData).data.length,
        );
        expect(image.bitmap.data).toStrictEqual((slintImage as ImageData).data);

        expect(
            (instance!.getProperty("external-image") as ImageData).path,
        ).toStrictEqual(undefined);
    }
});

test("get/set brush properties", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const black = instance!.getProperty("black");

    expect((black as private_api.SlintBrush).toString()).toBe("#000000");

    if (black instanceof private_api.SlintBrush) {
        const blackSlintRgbaColor = (black as private_api.SlintBrush).color;
        expect(blackSlintRgbaColor.red).toStrictEqual(0);
        expect(blackSlintRgbaColor.green).toStrictEqual(0);
        expect(blackSlintRgbaColor.blue).toStrictEqual(0);
    }

    instance?.setProperty("black", "#ffffff");
    const white = instance!.getProperty("black");

    if (white instanceof private_api.SlintBrush) {
        const whiteSlintRgbaColor = (white as private_api.SlintBrush).color;
        expect(whiteSlintRgbaColor.red).toStrictEqual(255);
        expect(whiteSlintRgbaColor.green).toStrictEqual(255);
        expect(whiteSlintRgbaColor.blue).toStrictEqual(255);
    }

    const transparent = instance!.getProperty("trans");

    if (black instanceof private_api.SlintBrush) {
        expect((transparent as private_api.SlintBrush).isTransparent).toBe(
            true,
        );
    }

    const ref = new private_api.SlintBrush({
        red: 100,
        green: 110,
        blue: 120,
        alpha: 255,
    });
    instance!.setProperty("ref", ref);

    let instance_ref = instance!.getProperty("ref");

    if (instance_ref instanceof private_api.SlintBrush) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        expect(ref_color.red).toStrictEqual(100);
        expect(ref_color.green).toStrictEqual(110);
        expect(ref_color.blue).toStrictEqual(120);
        expect(ref_color.alpha).toStrictEqual(255);
    }

    instance!.setProperty("ref", {
        color: { red: 110, green: 120, blue: 125, alpha: 255 },
    });

    instance_ref = instance!.getProperty("ref");

    if (instance_ref instanceof private_api.SlintBrush) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        expect(ref_color.red).toStrictEqual(110);
        expect(ref_color.green).toStrictEqual(120);
        expect(ref_color.blue).toStrictEqual(125);
        expect(ref_color.alpha).toStrictEqual(255);
    }

    instance!.setProperty("ref", {
        red: 110,
        green: 120,
        blue: 125,
        alpha: 255,
    });

    instance_ref = instance!.getProperty("ref");

    if (instance_ref instanceof private_api.SlintBrush) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        expect(ref_color.red).toStrictEqual(110);
        expect(ref_color.green).toStrictEqual(120);
        expect(ref_color.blue).toStrictEqual(125);
        expect(ref_color.alpha).toStrictEqual(255);
    }

    instance!.setProperty("ref", {});

    instance_ref = instance!.getProperty("ref");

    if (instance_ref instanceof private_api.SlintBrush) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        expect(ref_color.red).toStrictEqual(0);
        expect(ref_color.green).toStrictEqual(0);
        expect(ref_color.blue).toStrictEqual(0);
        expect(ref_color.alpha).toStrictEqual(0);
    }

    const radialGradient = instance!.getProperty("radial-gradient");

    if (radialGradient instanceof private_api.SlintBrush) {
        expect((radialGradient as private_api.SlintBrush).toString()).toBe(
            "radial-gradient(circle, rgba(255, 0, 0, 255) 0%, rgba(0, 255, 0, 255) 50%, rgba(0, 0, 255, 255) 100%)",
        );
    }

    const linearGradient = instance!.getProperty("linear-gradient");

    if (linearGradient instanceof private_api.SlintBrush) {
        expect((linearGradient as private_api.SlintBrush).toString()).toBe(
            "linear-gradient(90deg, rgba(63, 135, 166, 255) 0%, rgba(235, 248, 225, 255) 50%, rgba(246, 157, 60, 255) 100%)",
        );
    }

    {
        let thrownError: any;
        try {
            instance.setProperty("ref-color", {
                red: "abc",
                blue: 0,
                green: 0,
                alpha: 0,
            });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("NumberExpected");
        expect(thrownError.message).toBe(
            "Failed to convert napi value String into rust type `f64`",
        );
    }

    {
        let thrownError: any;
        try {
            instance.setProperty("ref-color", {
                red: 0,
                blue: true,
                green: 0,
                alpha: 0,
            });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("NumberExpected");
        expect(thrownError.message).toBe(
            "Failed to convert napi value Boolean into rust type `f64`",
        );
    }

    {
        let thrownError: any;
        try {
            instance.setProperty("ref-color", {
                red: 0,
                blue: 0,
                green: true,
                alpha: 0,
            });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("NumberExpected");
        expect(thrownError.message).toBe(
            "Failed to convert napi value Boolean into rust type `f64`",
        );
    }

    {
        let thrownError: any;
        try {
            instance.setProperty("ref-color", {
                red: 0,
                blue: 0,
                green: 0,
                alpha: new private_api.SlintRgbaColor(),
            });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("NumberExpected");
        expect(thrownError.message).toBe(
            "Failed to convert napi value Object into rust type `f64`",
        );
    }

    {
        let thrownError: any;
        try {
            instance.setProperty("ref-color", { blue: 0, green: 0, alpha: 0 });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe("Property red is missing");
    }

    {
        let thrownError: any;
        try {
            instance.setProperty("ref-color", { red: 0, green: 0, alpha: 0 });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe("Property blue is missing");
    }

    instance.setProperty("ref-color", { red: 0, green: 0, blue: 0 });
    instance_ref = instance!.getProperty("ref-color");

    if (instance_ref instanceof private_api.SlintBrush) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        expect(ref_color.red).toStrictEqual(0);
        expect(ref_color.green).toStrictEqual(0);
        expect(ref_color.blue).toStrictEqual(0);
        expect(ref_color.alpha).toStrictEqual(255);
    }

    // ref is a brush, but setting to a color should not throw, but take the brush's color.
    instance!.setProperty("ref-color", ref);
    instance_ref = instance!.getProperty("ref-color");
    if (instance_ref instanceof private_api.SlintBrush) {
        const ref_color = (instance_ref as private_api.SlintBrush).color;
        expect(ref_color.red).toStrictEqual(ref.color.red);
        expect(ref_color.green).toStrictEqual(ref.color.green);
        expect(ref_color.blue).toStrictEqual(ref.color.blue);
        expect(ref_color.alpha).toStrictEqual(ref.color.alpha);
    }
});

test("get/set enum properties", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export enum Direction { up, down }
         export component App { in-out property <Direction> direction: up; }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    expect(instance!.getProperty("direction")).toBe("up");

    instance!.setProperty("direction", "down");
    expect(instance!.getProperty("direction")).toBe("down");

    {
        let thrownError: any;
        try {
            instance!.setProperty("direction", 42);
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
            instance!.setProperty("direction", { blah: "foo" });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("InvalidArg");
        expect(thrownError.message).toBe("expect String, got: Object");
    }

    {
        let thrownError: any;
        try {
            instance!.setProperty("direction", "left");
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe(
            "left is not a value of enum Direction",
        );
    }
});

test("ArrayModel", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    expect(Array.from(new ArrayModel([3, 2, 1]))).toStrictEqual([3, 2, 1]);

    instance!.setProperty("int-model", new ArrayModel([10, 9, 8]));

    const intArrayModel = instance!.getProperty(
        "int-model",
    ) as ArrayModel<number>;
    expect(intArrayModel.rowCount()).toStrictEqual(3);
    expect(intArrayModel.values()).toStrictEqual(
        new ArrayModel([10, 9, 8]).values(),
    );

    instance!.setProperty(
        "string-model",
        new ArrayModel(["Simon", "Olivier", "Auri", "Tobias", "Florian"]),
    );

    const stringArrayModel = instance!.getProperty(
        "string-model",
    ) as ArrayModel<number>;
    expect(stringArrayModel.values()).toStrictEqual(
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
    expect(structArrayModel.values()).toStrictEqual(
        new ArrayModel([
            { name: "simon", age: 22 },
            { name: "florian", age: 22 },
        ]).values(),
    );
});

test("MapModel", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export component App {
      in-out property <[string]> model;
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

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
    expect(checkModel.rowData(0)).toBe("Hausmann, Simon");
    expect(checkModel.rowData(1)).toBe("Goffart, Olivier");
    expect(checkModel.rowData(2)).toBe("Tisch, Roman");
});

test("MapModel undefined rowData sourcemodel", () => {
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
    expect(mapFunctionCallCount).toStrictEqual(mapModel.rowCount());
    mapFunctionCallCount = 0;
    expect(nameModel.rowData(nameModel.rowCount())).toBeUndefined();
    expect(mapFunctionCallCount).toStrictEqual(0);
    expect(mapModel.rowData(mapModel.rowCount())).toBeUndefined();
    expect(mapFunctionCallCount).toStrictEqual(0);
});

test("ArrayModel rowCount", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    out property <int> model-length: model.length;
    in-out property <[int]> model;
  }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const model = new ArrayModel([10, 9, 8]);

    instance!.setProperty("model", model);
    expect(model.rowCount()).toBe(3);
    expect(instance?.getProperty("model-length") as number).toBe(3);
});

test("ArrayModel rowData/setRowData", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const model = new ArrayModel([10, 9, 8]);

    instance!.setProperty("model", model);
    expect(model.rowData(1)).toBe(9);
    expect(instance!.invoke("data", [1])).toStrictEqual(9);

    model.setRowData(1, 4);
    expect(model.rowData(1)).toBe(4);
    expect(instance!.invoke("data", [1])).toStrictEqual(4);
});

test("Model notify", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const model = new ArrayModel([100, 0]);

    instance!.setProperty("fixed-height-model", model);
    expect(instance!.getProperty("layout-height") as number).toBe(100);
    model.setRowData(1, 50);
    expect(instance!.getProperty("layout-height") as number).toBe(150);
    model.push(75);
    expect(instance!.getProperty("layout-height") as number).toBe(225);
    model.remove(1, 2);
    expect(instance!.getProperty("layout-height") as number).toBe(100);
});

test("model from array", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    in-out property <[int]> int-array;
    in-out property <[string]> string-array;
  }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    instance!.setProperty("int-array", [10, 9, 8]);
    const wrapped_int_model = instance!.getProperty(
        "int-array",
    ) as Model<number>;
    expect(Array.from(wrapped_int_model)).toStrictEqual([10, 9, 8]);
    expect(wrapped_int_model.rowCount()).toStrictEqual(3);
    expect(wrapped_int_model.rowData(0)).toStrictEqual(10);
    expect(wrapped_int_model.rowData(1)).toStrictEqual(9);
    expect(wrapped_int_model.rowData(2)).toStrictEqual(8);
    expect(Array.from(wrapped_int_model)).toStrictEqual([10, 9, 8]);

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
    expect(wrapped_string_model.rowCount()).toStrictEqual(5);
    expect(wrapped_string_model.rowData(0)).toStrictEqual("Simon");
    expect(wrapped_string_model.rowData(1)).toStrictEqual("Olivier");
    expect(wrapped_string_model.rowData(2)).toStrictEqual("Auri");
    expect(wrapped_string_model.rowData(3)).toStrictEqual("Tobias");
    expect(wrapped_string_model.rowData(4)).toStrictEqual("Florian");
});

test("invoke callback", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();
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
    expect(speakTest).toStrictEqual(
        "hello simon, olivier, auri, tobias and florian",
    );

    instance!.setCallback("great-person", (p: any) => {
        speakTest = "hello " + p.name;
    });

    instance!.invoke("great-person", [{ name: "simon" }]);
    expect(speakTest).toStrictEqual("hello simon");

    instance!.invoke("great-person", [{ hello: "simon" }]);
    expect(speakTest).toStrictEqual("hello ");

    expect(instance!.invoke("get-string", [])).toStrictEqual("string");
    expect(instance!.invoke("person", [])).toStrictEqual({ name: "florian" });
});

test("wrong callback return type ", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();
    let speakTest: string;

    instance!.setCallback("get-string", () => {
        return 20;
    });

    const string = instance!.invoke("get-string", []);
    expect(string).toStrictEqual("");

    instance!.setCallback("get-int", () => {
        return "string";
    });

    const int = instance!.invoke("get-int", []);
    expect(int).toStrictEqual(0);

    instance!.setCallback("get-bool", () => {
        return "string";
    });

    const bool = instance!.invoke("get-bool", []);
    expect(bool).toStrictEqual(false);

    instance!.setCallback("get-person", () => {
        return "string";
    });

    const person = instance!.invoke("get-person", []);
    expect(person).toStrictEqual({ name: "", age: 0 });
});

test("wrong global callback return type ", () => {
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
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();
    let speakTest: string;

    instance!.setGlobalCallback("Global", "get-string", () => {
        return 20;
    });

    const string = instance!.invokeGlobal("Global", "get-string", []);
    expect(string).toStrictEqual("");

    instance!.setGlobalCallback("Global", "get-bool", () => {
        return "string";
    });

    const bool = instance!.invokeGlobal("Global", "get-bool", []);
    expect(bool).toStrictEqual(false);

    instance!.setGlobalCallback("Global", "get-person", () => {
        return "string";
    });

    const person = instance!.invokeGlobal("Global", "get-person", []);
    expect(person).toStrictEqual({ name: "", age: 0 });
});

test("throw exception in callback", async () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    callback throw-something();
  }
  `,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    instance!.setCallback("throw-something", () => {
        throw new Error("I'm an error");
    });

    const stderrCapture = captureAsyncStderr();
    try {
        instance!.invoke("throw-something", []);
        // Vitest runs these tests in workers and the native binding writes to
        // stderr on the next macrotask, so yield once before restoring writers.
        await new Promise((resolve) => setTimeout(resolve, 0));
    } finally {
        stderrCapture.restore();
    }
    const output = stderrCapture.output();
    expect(
        output.includes("Node.js: Invoking callback 'throw-something' failed"),
    ).toBe(true);
    expect(output.includes("I'm an error")).toBe(true);
});

test("throw exception set color", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    in-out property <color> test;
  }
  `,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    {
        let thrownError: any;
        try {
            instance!.setProperty("test", { garbage: true });
        } catch (error) {
            thrownError = error;
        }
        expect(thrownError).toBeDefined();
        expect(thrownError.code).toBe("GenericFailure");
        expect(thrownError.message).toBe("Property red is missing");
    }
});
