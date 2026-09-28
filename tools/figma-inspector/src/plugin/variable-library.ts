// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export type VariableValue =
    | boolean
    | number
    | string
    | { r: number; g: number; b: number; a?: number }
    | { type: "VARIABLE_ALIAS"; id: string };
export type VariableLibrary = {
    version: 1;
    variables: Record<
        string,
        {
            name: string;
            type: "COLOR" | "FLOAT" | "STRING" | "BOOLEAN";
            collectionId: string;
            values: Record<string, VariableValue>;
        }
    >;
    collections: Record<
        string,
        {
            name: string;
            defaultModeId: string;
            modes: { id: string; name: string }[];
        }
    >;
    bindings: Record<
        string,
        { fields: Record<string, string>; modes: Record<string, string> }
    >;
};
export function validateVariableLibrary(
    value: unknown,
): asserts value is VariableLibrary {
    const object = (v: unknown): v is Record<string, unknown> =>
        !!v && typeof v === "object" && !Array.isArray(v);
    const fail = () => {
        throw Error("Invalid variable library");
    };
    if (
        !object(value) ||
        value.version !== 1 ||
        !object(value.variables) ||
        !object(value.collections) ||
        !object(value.bindings)
    )
        fail();
    const library = value as VariableLibrary;
    for (const c of Object.values(library.collections)) {
        if (
            !object(c) ||
            typeof c.name !== "string" ||
            typeof c.defaultModeId !== "string" ||
            !Array.isArray(c.modes) ||
            !c.modes.length
        )
            fail();
        if (
            c.modes.some(
                (m) =>
                    !object(m) ||
                    typeof m.id !== "string" ||
                    typeof m.name !== "string",
            ) ||
            new Set(c.modes.map((m) => m.id)).size !== c.modes.length ||
            !c.modes.some((m) => m.id === c.defaultModeId)
        )
            fail();
    }
    for (const v of Object.values(library.variables)) {
        if (
            !object(v) ||
            typeof v.name !== "string" ||
            !["COLOR", "FLOAT", "STRING", "BOOLEAN"].includes(v.type) ||
            !library.collections[v.collectionId] ||
            !object(v.values)
        )
            fail();
        for (const val of Object.values(v.values)) {
            if (object(val) && "type" in val && val.type === "VARIABLE_ALIAS") {
                if (
                    typeof val.id !== "string" ||
                    !library.variables[val.id] ||
                    library.variables[val.id].type !== v.type
                )
                    fail();
            } else if (v.type === "COLOR") {
                if (
                    !object(val) ||
                    "type" in val ||
                    ![val.r, val.g, val.b, val.a ?? 1].every(
                        (n) =>
                            typeof n === "number" &&
                            Number.isFinite(n) &&
                            n >= 0 &&
                            n <= 1,
                    )
                )
                    fail();
            } else if (
                typeof val !==
                    (
                        {
                            FLOAT: "number",
                            STRING: "string",
                            BOOLEAN: "boolean",
                        } as Record<string, string>
                    )[v.type] ||
                (typeof val === "number" && !Number.isFinite(val))
            )
                fail();
        }
    }
    for (const b of Object.values(library.bindings)) {
        if (
            !object(b) ||
            !object(b.fields) ||
            !object(b.modes) ||
            Object.values(b.fields).some(
                (id) => typeof id !== "string" || !library.variables[id],
            )
        )
            fail();
        for (const [id, mode] of Object.entries(b.modes))
            if (
                typeof mode !== "string" ||
                !library.collections[id]?.modes.some((m) => m.id === mode)
            )
                fail();
    }
}
