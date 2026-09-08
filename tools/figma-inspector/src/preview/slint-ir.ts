// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/** Typed visual IR. The renderer builds this directly; no generated Slint is parsed. */
export type Expression =
    | { kind: "literal"; type: string; code: string }
    | { kind: "reference"; type: string; code: string }
    | { kind: "raw"; type: string; code: string };
export type Binding = {
    kind: "binding";
    name: string;
    value: Expression;
    depth: number;
};
export type ElementStart = {
    kind: "open";
    type: string;
    depth: number;
    condition?: string;
    role?: string;
    origin?: { id: string; name: string };
};
export type SlintLine =
    | Binding
    | ElementStart
    | { kind: "close"; depth: number };
export type Element = {
    condition?: string;
    role?: string;
    type: string;
    origin?: { id: string; name: string };
    bindings: Binding[];
    children: Element[];
};
export const openElement = (
    type: string,
    depth: number,
    origin?: ElementStart["origin"],
): ElementStart => ({ kind: "open", type, depth, origin });
export const closeElement = (depth: number): SlintLine => ({
    kind: "close",
    depth,
});
export const literal = (type: string, code: string): Expression => ({
    kind: "literal",
    type,
    code,
});
export const reference = (type: string, code: string): Expression => ({
    kind: "reference",
    type,
    code,
});

function propertyType(name: string): string {
    if (
        [
            "alignment",
            "cross-axis-alignment",
            "cross-axis-self-alignment",
            "flex-direction",
            "flex-wrap",
            "horizontal-alignment",
            "vertical-alignment",
            "wrap",
            "overflow",
            "image-fit",
            "image-rendering",
        ].includes(name)
    )
        return "enum";
    if (["text", "font-family", "commands"].includes(name)) return "string";
    if (
        [
            "background",
            "color",
            "default-color",
            "border-color",
            "fill",
            "stroke",
            "drop-shadow-color",
        ].includes(name)
    )
        return "brush";
    if (name === "source") return "image";
    if (["clip", "font-italic"].includes(name)) return "bool";
    if (
        [
            "font-weight",
            "max-lines",
            "layout-order",
            "source-clip-x",
            "source-clip-y",
            "source-clip-width",
            "source-clip-height",
        ].includes(name)
    )
        return "int";
    if (["opacity", "horizontal-stretch", "vertical-stretch"].includes(name))
        return "float";
    if (name === "transform-rotation") return "angle";
    if (
        [
            "x",
            "y",
            "width",
            "height",
            "spacing",
            "spacing-horizontal",
            "spacing-vertical",
            "letter-spacing",
            "font-size",
        ].includes(name) ||
        name.startsWith("padding") ||
        name.startsWith("min-") ||
        name.startsWith("max-") ||
        name.startsWith("preferred-") ||
        name.includes("radius") ||
        name === "border-width" ||
        name === "stroke-width" ||
        name.startsWith("drop-shadow-")
    )
        return "length";
    return "opaque";
}
export function binding(
    name: string,
    value: string | number | boolean | Expression,
    depth = 2,
): Binding {
    if (typeof value === "object")
        return { kind: "binding", name, value, depth };
    const code =
        typeof value === "number"
            ? String(Number(value.toFixed(3)))
            : String(value);
    const type = propertyType(name);
    const constant =
        type !== "opaque" &&
        (typeof value !== "string" ||
            code.startsWith('"') ||
            code.startsWith("#") ||
            code.startsWith("@image-url(") ||
            code.startsWith("@linear-gradient(") ||
            code === "true" ||
            code === "false" ||
            code === "transparent" ||
            (type === "enum" && /^[a-z][a-z-]*$/.test(code)) ||
            Number.isFinite(Number(code)) ||
            ((code.endsWith("px") || code.endsWith("deg")) &&
                Number.isFinite(
                    Number(code.slice(0, -(code.endsWith("px") ? 2 : 3))),
                )));
    return {
        kind: "binding",
        name,
        value: { kind: constant ? "literal" : "raw", type, code },
        depth,
    };
}
export function printLine(line: SlintLine | string): string {
    if (typeof line === "string") return line;
    const indent = "    ".repeat(line.depth);
    if (line.kind === "open")
        return `${indent}${line.condition ? `if ${line.condition}: ` : ""}${line.type} {`;
    if (line.kind === "close") return `${indent}}`;
    return `${indent}${line.name}: ${line.value.code};`;
}
export function elementTree(lines: SlintLine[]): Element {
    const stack: Element[] = [];
    let root: Element | undefined;
    for (const line of lines) {
        if (line.kind === "open") {
            const element: Element = {
                type: line.type,
                condition: line.condition,
                role: line.role,
                origin: line.origin,
                bindings: [],
                children: [],
            };
            if (stack.length) stack[stack.length - 1].children.push(element);
            else if (root) throw Error("Multiple component roots");
            else root = element;
            stack.push(element);
        } else if (line.kind === "close") {
            if (!stack.pop()) throw Error("Unbalanced element tree");
        } else {
            if (!stack.length) throw Error("Binding outside element");
            stack[stack.length - 1].bindings.push(line);
        }
    }
    if (!root || stack.length) throw Error("Incomplete element tree");
    return root;
}
export function treeLines(tree: Element, depth = 1): SlintLine[] {
    return [
        {
            ...openElement(tree.type, depth, tree.origin),
            condition: tree.condition,
            role: tree.role,
        },
        ...tree.bindings.map((b) => ({ ...b, depth: depth + 1 })),
        ...tree.children.flatMap((c) => treeLines(c, depth + 1)),
        closeElement(depth),
    ];
}

export function requireValue<T>(value: T | undefined | null): T {
    if (value === undefined || value === null)
        throw Error("Missing validated component data");
    return value;
}

/** Preserve helper purpose before lowering hides the authored structure. */
export function roleLines(lines: SlintLine[], role: string): SlintLine[] {
    const depth = lines.find((line) => line.kind === "open")?.depth;
    let index = 0;
    return lines.map((line) =>
        line.kind === "open" && line.depth === depth
            ? { ...line, role: `${role}-${++index}` }
            : line,
    );
}
