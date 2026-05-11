// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";

import { loadSource, private_api, ArrayModel, Model } from "../dist/index.js";

private_api.initTesting();

/// Yield to the event loop and run a full GC pass. V8 needs a microtask
/// checkpoint before weak references are cleared.
function gcAndYield(): Promise<void> {
    return new Promise((resolve) => {
        setTimeout(() => {
            global.gc!({ type: "major", execution: "sync" });
            resolve();
        }, 0);
    });
}

// --- Tests that callbacks DO NOT prevent GC ---

test("callback closure does not prevent GC", async () => {
    function makeInstance() {
        const demo = loadSource(
            `export component Test {
            callback say_hello();
            in-out property <string> check: "initial value";
        }`,
            "gc.slint",
        ) as any;
        const instance = new demo.Test();

        // Set a callback that captures the instance, creating a cycle.
        instance.say_hello = function () {
            instance.check = "hello from callback";
        };

        return new WeakRef(instance);
    }
    const weakRef = makeInstance();
    await gcAndYield();
    expect(weakRef.deref()).toBeUndefined();
});

test("callback with return value does not prevent GC", async () => {
    function makeInstance() {
        const source = `export component Test {
            callback compute(int, int) -> int;
            in-out property <int> result;
        }`;
        const instance = new (loadSource(source, "gc.slint") as any).Test();

        instance.compute = function (a: number, b: number) {
            return a + b;
        };
        expect(instance.compute(3, 4)).toBe(7);

        return new WeakRef(instance);
    }
    const weakRef = makeInstance();
    await gcAndYield();
    expect(weakRef.deref()).toBeUndefined();
});

test("multiple callbacks do not prevent GC", async () => {
    function makeInstance() {
        const source = `export component Test {
            callback on_click();
            callback on_hover();
            callback on_key();
            in-out property <string> status: "idle";
        }`;
        const instance = new (
            loadSource(source, "gc-multi.slint") as any
        ).Test();

        instance.on_click = function () {
            instance.status = "clicked";
        };
        instance.on_hover = function () {
            instance.status = "hovered";
        };
        instance.on_key = function () {
            instance.status = "key pressed";
        };

        return new WeakRef(instance);
    }
    const weakRef = makeInstance();
    await gcAndYield();
    expect(weakRef.deref()).toBeUndefined();
});

test("global callback does not prevent GC", async () => {
    function makeInstance() {
        const compiler = new private_api.ComponentCompiler();
        const definition = compiler.buildFromSource(
            `
            export global Logic {
                callback do_something();
            }
            export component App {
                in-out property <string> label: "app";
            }
            `,
            "",
        );
        expect(definition.App).not.toBeNull();

        const instance = definition.App!.create()!;

        instance.setGlobalCallback("Logic", "do_something", function () {
            instance.setProperty("label", "updated");
        });

        return new WeakRef(instance);
    }
    const weakRef = makeInstance();
    await gcAndYield();
    expect(weakRef.deref()).toBeUndefined();
});

test("constructor callback does not prevent GC", async () => {
    function makeInstance() {
        const demo = loadSource(
            `export component Test {
            callback say_hello();
            in-out property <string> check: "initial value";
        }`,
            "gc.slint",
        ) as any;
        const instance = new demo.Test({
            say_hello: function () {
                instance.check = "constructed callback";
            },
            check: "from constructor",
        });

        expect(instance.check).toBe("from constructor");
        return new WeakRef(instance);
    }
    const weakRef = makeInstance();
    await gcAndYield();
    expect(weakRef.deref()).toBeUndefined();
});

// --- Tests that callbacks SURVIVE GC while instance is alive ---

test("callback survives GC while instance is alive", async () => {
    const demo = loadSource(
        `export component Test {
        callback say_hello();
        in-out property <string> check: "initial value";
    }`,
        "gc.slint",
    ) as any;
    const instance = new demo.Test();

    let callCount = 0;
    instance.say_hello = function () {
        callCount++;
    };

    instance.say_hello();
    expect(callCount).toBe(1);

    // GC must not collect the callback because the instance is still alive.
    await gcAndYield();

    instance.say_hello();
    expect(callCount).toBe(2);
});

test("callback with return value survives GC", async () => {
    const source = `export component Test {
        callback compute(int, int) -> int;
    }`;
    const instance = new (loadSource(source, "gc-surv.slint") as any).Test();

    instance.compute = function (a: number, b: number) {
        return a + b;
    };

    expect(instance.compute(2, 3)).toBe(5);

    await gcAndYield();

    expect(instance.compute(10, 20)).toBe(30);
});

test("constructor callback survives GC", async () => {
    const demo = loadSource(
        `export component Test {
        callback say_hello();
        in-out property <string> check: "initial value";
    }`,
        "gc.slint",
    ) as any;
    const instance = new demo.Test({
        say_hello: function () {
            instance.check = "from constructor";
        },
    });

    await gcAndYield();

    instance.say_hello();
    expect(instance.check).toBe("from constructor");
});

test("replacing a callback works after GC", async () => {
    const source = `export component Test {
        callback action();
        in-out property <string> log: "";
    }`;
    const instance = new (loadSource(source, "gc-replace.slint") as any).Test();

    instance.action = function () {
        instance.log = "first";
    };
    instance.action = function () {
        instance.log = "second";
    };

    instance.action();
    expect(instance.log).toBe("second");

    await gcAndYield();

    instance.action();
    expect(instance.log).toBe("second");
});

// --- Model GC tests ---

test("model without JS reference survives GC", async () => {
    const source = `export component Test {
        in-out property <[int]> items;
    }`;
    const instance = new (
        loadSource(source, "gc-model-surv.slint") as any
    ).Test();

    // Assign a model without keeping a JS reference to it.
    instance.items = new ArrayModel([10, 20, 30]);

    await gcAndYield();

    // The model must still be alive and readable.
    const items = instance.items;
    expect(items.rowCount()).toBe(3);
    expect(items.rowData(0)).toBe(10);
    expect(items.rowData(1)).toBe(20);
    expect(items.rowData(2)).toBe(30);
});

test("custom model without JS reference survives GC", async () => {
    const source = `export component Test {
        in-out property <[string]> items;
    }`;
    const instance = new (
        loadSource(source, "gc-model-custom.slint") as any
    ).Test();

    class MyModel extends ArrayModel<string> {
        greeting(row: number): string {
            return "hello " + (this.rowData(row) ?? "");
        }
    }

    // Assign without keeping a reference.
    instance.items = new MyModel(["alice", "bob"]);

    await gcAndYield();

    const items = instance.items;
    expect(items.rowCount()).toBe(2);
    expect(items.rowData(0)).toBe("alice");
});

test("model returned by callback survives GC", async () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export component Test {
            callback get_items() -> [string];
            in-out property <[string]> items;
        }`,
        "",
    );
    const instance = definition.Test!.create()!;

    // The callback returns a model as a temporary — no JS variable holds it.
    instance.setCallback("get_items", function () {
        return new ArrayModel(["x", "y", "z"]);
    });

    // Invoke from the Slint side — the return value goes through to_value
    // in the Rust closure, not through JS set_property.
    const result = instance.invoke("get_items", []);

    await gcAndYield();

    // The model must still be alive after GC.
    expect(result.rowCount()).toBe(3);
    expect(result.rowData(0)).toBe("x");
});

test("multiple models returned by callback survive GC", async () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export component Test {
            callback get_items() -> [string];
            in-out property <[string]> items1;
            in-out property <[string]> items2;
        }`,
        "",
    );
    const instance = definition.Test!.create()!;

    // Each call returns a NEW model — both must survive GC.
    let callCount = 0;
    instance.setCallback("get_items", function () {
        callCount++;
        return new ArrayModel(["item_" + callCount]);
    });

    // Invoke the callback twice and store the results in separate properties.
    instance.setProperty("items1", instance.invoke("get_items", []));
    instance.setProperty("items2", instance.invoke("get_items", []));

    const items1 = instance.getProperty("items1");
    const items2 = instance.getProperty("items2");

    expect(items1.rowCount()).toBe(1);
    expect(items2.rowCount()).toBe(1);
    expect(items1.rowData(0)).not.toBe(items2.rowData(0));

    await gcAndYield();

    // Both models must still be alive after GC.
    expect(items1.rowCount()).toBe(1);
    expect(items1.rowData(0)).toBe("item_1");
    expect(items2.rowCount()).toBe(1);
    expect(items2.rowData(0)).toBe("item_2");
});

test("model in struct field survives GC", async () => {
    const source = `
        export struct Data {
            items: [string],
            label: string,
        }
        export component Test {
            in-out property <Data> data;
        }
    `;
    const instance = new (
        loadSource(source, "gc-struct-model.slint") as any
    ).Test();

    // The model inside the struct has no JS variable holding it.
    instance.data = {
        items: new ArrayModel(["one", "two", "three"]),
        label: "test",
    };

    await gcAndYield();

    const data = instance.data;
    expect(data.label).toBe("test");
    expect(data.items.rowCount()).toBe(3);
    expect(data.items.rowData(0)).toBe("one");
});

test("model passed as callback argument survives GC", async () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export component Test {
            callback receive_items([string]);
            in-out property <[string]> stored_items;
        }`,
        "",
    );
    const instance = definition.Test!.create()!;

    instance.setCallback("receive_items", function (items: any) {
        instance.setProperty("stored_items", items);
    });

    // Pass a model as a callback argument — no JS variable keeps it.
    instance.invoke("receive_items", [new ArrayModel(["a", "b"])]);

    await gcAndYield();

    const items = instance.getProperty("stored_items");
    expect(items.rowCount()).toBe(2);
    expect(items.rowData(0)).toBe("a");
});

test("model passed to public function survives GC", async () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export component Test {
            in-out property <[string]> stored;
            public function set_model(m: [string]) {
                stored = m;
            }
        }`,
        "",
    );
    const instance = definition.Test!.create()!;

    // Pass a model to a public function — no JS variable keeps it.
    instance.invoke("set_model", [new ArrayModel(["fn_a", "fn_b"])]);

    await gcAndYield();

    const stored = instance.getProperty("stored");
    expect(stored.rowCount()).toBe(2);
    expect(stored.rowData(0)).toBe("fn_a");
    expect(stored.rowData(1)).toBe("fn_b");
});

test("nested model in struct returned by rowData survives GC", async () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
        export struct Row {
            label: string,
            tags: [string],
        }
        export component Test {
            in-out property <[Row]> rows;
        }
        `,
        "",
    );
    const instance = definition.Test!.create()!;

    // The outer model's rowData returns structs that contain nested models.
    // Neither the outer nor inner models are held in JS variables.
    class RowModel extends Model<{ label: string; tags: any }> {
        private data: Array<{ label: string; tags: any }>;
        constructor() {
            super();
            this.data = [
                { label: "first", tags: new ArrayModel(["a", "b"]) },
                { label: "second", tags: new ArrayModel(["x", "y", "z"]) },
            ];
        }
        rowCount(): number {
            return this.data.length;
        }
        rowData(row: number): { label: string; tags: any } | undefined {
            return this.data[row];
        }
    }

    instance.setProperty("rows", new RowModel() as any);

    await gcAndYield();

    // Both the outer model and the nested ArrayModels must survive.
    const rows = instance.getProperty("rows");
    expect(rows.rowCount()).toBe(2);

    const first = rows.rowData(0);
    expect(first.label).toBe("first");
    expect(first.tags.rowCount()).toBe(2);
    expect(first.tags.rowData(0)).toBe("a");
    expect(first.tags.rowData(1)).toBe("b");

    const second = rows.rowData(1);
    expect(second.label).toBe("second");
    expect(second.tags.rowCount()).toBe(3);
    expect(second.tags.rowData(0)).toBe("x");
});

// --- Two-phase tests: survive while alive, then collected when dropped ---

test("callback and instance are both collected together", async () => {
    let callbackRef: WeakRef<Function>;
    function makeInstance() {
        const demo = loadSource(
            `export component Test {
            callback say_hello();
            in-out property <string> check: "initial";
        }`,
            "gc-2phase-cb.slint",
        ) as any;
        const instance = new demo.Test();

        const cb = function () {
            instance.check = "called";
        };
        callbackRef = new WeakRef(cb);
        instance.say_hello = cb;

        // Verify callback works.
        instance.say_hello();
        expect(instance.check).toBe("called");

        return new WeakRef(instance);
    }
    const instanceRef = makeInstance();

    // GC should NOT collect — makeInstance's WeakRef is the only ref but
    // we haven't yielded yet. Actually the instance went out of scope in
    // makeInstance, so both should be collectible.
    await gcAndYield();

    expect(instanceRef.deref()).toBeUndefined();
    expect(callbackRef!.deref()).toBeUndefined();
});

test("model survives while instance alive, then both collected", async () => {
    let modelRef: WeakRef<object>;

    async function phase1() {
        const source = `export component Test {
            in-out property <[string]> items;
        }`;
        const instance = new (
            loadSource(source, "gc-2phase-model.slint") as any
        ).Test();

        const model = new ArrayModel(["a", "b", "c"]);
        modelRef = new WeakRef(model);
        instance.items = model;

        // GC while instance alive — model must survive.
        await gcAndYield();

        expect(modelRef.deref()).toBeDefined();
        expect(instance.items.rowCount()).toBe(3);
        expect(instance.items.rowData(0)).toBe("a");

        return new WeakRef(instance);
    }

    const instanceRef = await phase1();

    // Instance went out of scope — both should be collected.
    await gcAndYield();

    expect(instanceRef.deref()).toBeUndefined();
    expect(modelRef!.deref()).toBeUndefined();
});

test("model in struct field: survives then collected", async () => {
    let modelRef: WeakRef<object>;

    async function phase1() {
        const source = `
            export struct Data {
                items: [string],
                label: string,
            }
            export component Test {
                in-out property <Data> data;
            }
        `;
        const instance = new (
            loadSource(source, "gc-2phase-struct.slint") as any
        ).Test();

        const model = new ArrayModel(["one", "two"]);
        modelRef = new WeakRef(model);
        instance.data = { items: model, label: "test" };

        await gcAndYield();

        // Model must survive while instance is alive.
        expect(modelRef.deref()).toBeDefined();
        expect(instance.data.items.rowCount()).toBe(2);

        return new WeakRef(instance);
    }

    const instanceRef = await phase1();
    await gcAndYield();

    expect(instanceRef.deref()).toBeUndefined();
    expect(modelRef!.deref()).toBeUndefined();
});

test("nested model in rowData: survives then collected", async () => {
    let outerModelRef: WeakRef<object>;
    let innerModelRef: WeakRef<object>;

    async function phase1() {
        const compiler = new private_api.ComponentCompiler();
        const definition = compiler.buildFromSource(
            `
            export struct Row {
                label: string,
                tags: [string],
            }
            export component Test {
                in-out property <[Row]> rows;
            }
            `,
            "",
        );
        const instance = definition.Test!.create()!;

        const innerModel = new ArrayModel(["tag1", "tag2"]);
        innerModelRef = new WeakRef(innerModel);

        class OuterModel extends Model<{ label: string; tags: any }> {
            rowCount() {
                return 1;
            }
            rowData(row: number) {
                return { label: "row0", tags: innerModel };
            }
        }

        const outer = new OuterModel();
        outerModelRef = new WeakRef(outer);
        instance.setProperty("rows", outer as any);

        await gcAndYield();

        // Both models must survive while instance is alive.
        expect(outerModelRef.deref()).toBeDefined();
        expect(innerModelRef.deref()).toBeDefined();

        const rows = instance.getProperty("rows");
        expect(rows.rowCount()).toBe(1);
        const row = rows.rowData(0);
        expect(row.label).toBe("row0");
        expect(row.tags.rowCount()).toBe(2);

        return new WeakRef(instance);
    }

    const instanceRef = await phase1();
    await gcAndYield();

    expect(instanceRef.deref()).toBeUndefined();
    expect(outerModelRef!.deref()).toBeUndefined();
    expect(innerModelRef!.deref()).toBeUndefined();
});

test("multiple callbacks capturing instance: all collected", async () => {
    const cbRefs: WeakRef<Function>[] = [];

    function makeInstance() {
        const source = `export component Test {
            callback alpha();
            callback beta();
            callback gamma();
            in-out property <string> log: "";
        }`;
        const instance = new (
            loadSource(source, "gc-2phase-multi-cb.slint") as any
        ).Test();

        for (const name of ["alpha", "beta", "gamma"]) {
            const cb = function () {
                instance.log += name + ",";
            };
            cbRefs.push(new WeakRef(cb));
            (instance as any)[name] = cb;
        }

        return new WeakRef(instance);
    }

    const instanceRef = makeInstance();
    await gcAndYield();

    expect(instanceRef.deref()).toBeUndefined();
    for (const ref of cbRefs) {
        expect(ref.deref()).toBeUndefined();
    }
});

test("model replaced multiple times: old models collected", async () => {
    const source = `export component Test {
        in-out property <[int]> items;
    }`;
    const instance = new (
        loadSource(source, "gc-replace-model.slint") as any
    ).Test();

    const oldModels: WeakRef<object>[] = [];

    // Replace the model several times.
    for (let i = 0; i < 5; i++) {
        const model = new ArrayModel([i]);
        oldModels.push(new WeakRef(model));
        instance.items = model;
    }

    // The last model should survive, old ones should be collectible
    // (they're no longer used by the property, though they may still
    // be registered on `this` if Drop hasn't cleaned them up yet).
    const lastModel = new ArrayModel([99]);
    instance.items = lastModel;

    await gcAndYield();

    // Last model must survive.
    expect(instance.items.rowCount()).toBe(1);
    expect(instance.items.rowData(0)).toBe(99);
});

test("custom model capturing instance does not prevent GC", async () => {
    function makeInstance() {
        const compiler = new private_api.ComponentCompiler();
        const definition = compiler.buildFromSource(
            `export component App {
                in-out property <[string]> items;
                in-out property <string> label: "app";
            }`,
            "",
        );
        const instance = definition.App!.create()!;

        class CapturingModel extends ArrayModel<string> {
            rowData(row: number): string | undefined {
                void instance.getProperty("label");
                return super.rowData(row);
            }
        }

        const model = new CapturingModel(["a", "b", "c"]);
        instance.setProperty("items", model);

        return new WeakRef(instance);
    }
    const weakRef = makeInstance();
    await gcAndYield();
    expect(weakRef.deref()).toBeUndefined();
});
