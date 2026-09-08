// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type {
    VariableLibrary,
    VariableValue,
} from "../plugin/variable-library";
import type { Diagnostic } from "../plugin/snapshot";
import {
    identifier,
    nameAllocator,
    typeName,
    enumReservedNames,
} from "./component-names";
import { declaration } from "./component-variants";
import { requireValue, reference, type Element } from "./slint-ir";

function valueCode(value: VariableValue): string {
    if (typeof value !== "object") return JSON.stringify(value);
    if ("type" in value) throw Error("Unresolved variable alias");
    const channels = [
        value.r,
        value.g,
        value.b,
        ...((value.a ?? 1) < 1 ? [value.a ?? 1] : []),
    ];
    return `#${channels
        .map((c) =>
            Math.round(c * 255)
                .toString(16)
                .padStart(2, "0")
                .toUpperCase(),
        )
        .join("")}`;
}
/** Figma bindings are explicit token mappings; incompatible local modes stay literal. */
export function componentTokens(
    library: VariableLibrary | undefined,
    allocateType: (name: string) => string,
) {
    const source: string[] = [];
    const used = new Set<string>();
    const warnings: Diagnostic[] = [];
    if (!library) return { source, warnings, apply: (tree: Element) => tree };
    const graph = library;
    const global = allocateType("DesignTokens");
    const propertyName = nameAllocator();
    const properties = new Map(
        Object.keys(library.variables)
            .sort()
            .map((id) => [
                id,
                propertyName(identifier(library.variables[id].name)),
            ]),
    );
    const collections = new Map(
        Object.keys(library.collections)
            .sort()
            .map((id) => {
                const collection = library.collections[id],
                    optionName = nameAllocator(enumReservedNames);
                const modes = new Map(
                    [...collection.modes]
                        .sort((a, b) => (a.id < b.id ? -1 : 1))
                        .map((m) => [m.id, optionName(identifier(m.name))]),
                );
                const observed = new Set(
                    Object.values(library.bindings)
                        .map((b) => b.modes[id])
                        .filter(Boolean),
                );
                return [
                    id,
                    {
                        type: allocateType(`${typeName(collection.name)}Mode`),
                        property: propertyName(
                            `${identifier(collection.name)}-mode`,
                        ),
                        modes,
                        initial:
                            observed.size === 1
                                ? [...observed][0]
                                : collection.defaultModeId,
                    },
                ];
            }),
    );
    const declarations: string[] = [];
    const emittedCollections = new Set<string>();
    function emitCollection(id: string) {
        const c = requireValue(collections.get(id));
        if (c.modes.size === 1 || emittedCollections.has(id)) return;
        emittedCollections.add(id);
        source.push(
            ...declaration(
                `export enum ${c.type} { `,
                [...c.modes.values()].join(", "),
                " }",
            ),
        );
        declarations.push(
            `    in-out property <${c.type}> ${c.property}: ${c.type}.${c.modes.get(c.initial)};`,
        );
    }
    const active = new Set<string>(),
        done = new Set<string>();
    const resolve = (
        id: string,
        modes: Record<string, string>,
        visiting = new Set<string>(),
    ): VariableValue => {
        if (visiting.has(id)) throw Error(`Cyclic variable alias: ${id}`);
        visiting.add(id);
        const v = library.variables[id];
        const mode =
            modes[v.collectionId] ??
            requireValue(collections.get(v.collectionId)).initial;
        const value = v.values[mode];
        if (value === undefined)
            throw Error(`Missing variable mode value: ${v.name} (${mode})`);
        return typeof value === "object" && "type" in value
            ? resolve(value.id, modes, visiting)
            : value;
    };
    function emit(id: string) {
        if (active.has(id)) throw Error(`Cyclic variable alias: ${id}`);
        if (done.has(id)) return;
        active.add(id);
        const v = graph.variables[id];
        const c = requireValue(collections.get(v.collectionId));
        emitCollection(v.collectionId);
        for (const value of Object.values(v.values))
            if (typeof value === "object" && "type" in value) emit(value.id);
        const entries = [...c.modes].map(([mode, option]) => {
            const value = v.values[mode];
            if (value === undefined)
                throw Error(`Missing variable mode value: ${v.name} (${mode})`);
            return {
                condition: `${global}.${c.property} == ${c.type}.${option}`,
                code:
                    typeof value === "object" && "type" in value
                        ? `${global}.${properties.get(value.id)}`
                        : valueCode(value),
            };
        });
        const values = new Map<string, string[]>();
        for (const entry of entries) {
            const conditions = values.get(entry.code) ?? [];
            conditions.push(entry.condition);
            values.set(entry.code, conditions);
        }
        // Modes with the same value share a branch. Globals cannot own Slint
        // states, so keep a small conditional expression for genuine mode changes.
        const groups = [...values].sort((a, b) => a[1].length - b[1].length);
        const fallback = requireValue(groups.pop())[0];
        const code = groups.reduceRight(
            (tail, [value, conditions]) =>
                `(${conditions.join(" || ")}) ? ${value} : (${tail})`,
            fallback,
        );
        const type = {
            COLOR: "brush",
            FLOAT: "float",
            STRING: "string",
            BOOLEAN: "bool",
        }[v.type];
        declarations.push(
            ...declaration(
                `    out property <${type}> ${properties.get(id)}: `,
                code,
            ),
        );
        active.delete(id);
        done.add(id);
    }

    const warned = new Set<string>();
    function apply(tree: Element): Element {
        const mapping = graph.bindings[tree.origin?.id ?? ""];
        return {
            ...tree,
            bindings: tree.bindings.map((b) => {
                const id = mapping?.fields[b.name];
                if (!id || b.value.kind !== "literal") return b;
                const v = graph.variables[id];
                const explicit = Object.entries(mapping.modes).some(
                    ([id, mode]) => collections.get(id)?.initial !== mode,
                );
                const captured = valueCode(resolve(id, mapping.modes));
                const expected =
                    v.type === "FLOAT" && b.value.type === "length"
                        ? `${Number(captured)}px`
                        : captured;
                if (explicit || expected !== b.value.code) {
                    const key = `${tree.origin?.id ?? ""}:${b.name}`;
                    if (!warned.has(key)) {
                        warned.add(key);
                        warnings.push({
                            severity: "warning",
                            code: "COMPONENT_TOKEN_STATIC",
                            nodeId: tree.origin?.id ?? "",
                            message: `${tree.origin?.name ?? "element"}.${b.name}: retained captured value because its local mode or value differs from the shared token context`,
                        });
                    }
                    return b;
                }
                used.add(id);
                const code = `${global}.${properties.get(id)}${v.type === "FLOAT" && b.value.type === "length" ? " * 1px" : ""}`;
                return { ...b, value: reference(b.value.type, code) };
            }),
            children: tree.children.map(apply),
        };
    }
    return {
        get source() {
            if (!used.size) return [];
            [...used].sort().forEach(emit);
            return [
                ...source,
                `export global ${global} {`,
                ...declarations,
                "}",
                "",
            ];
        },
        warnings,
        apply,
    };
}
