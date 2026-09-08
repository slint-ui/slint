// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { requireValue } from "./slint-ir";
import {
    identifier,
    nameAllocator,
    enumReservedNames,
} from "./component-names";

/** Preserve numeric and boolean variant domains as values, rather than inventing
 * enums such as Option1 or TrueFalse. Other authored labels remain enums. */
export function axisScalar(options: string[]): "int" | "bool" | undefined {
    if (
        options.length === 2 &&
        new Set(options.map((value) => value.toLowerCase())).size === 2 &&
        options.every((value) =>
            ["true", "false"].includes(value.toLowerCase()),
        )
    )
        return "bool";
    if (
        options.length &&
        options.every(
            (value) =>
                Number.isSafeInteger(Number(value)) &&
                String(Number(value)) === value &&
                Number(value) >= -2147483648 &&
                Number(value) <= 2147483647,
        )
    )
        return "int";
}

/** Exact finite-domain predicate reduction. Merging cubes never adds tuples. */
export type VariantAxis = {
    key: string;
    property: string;
    type: string;
    options: Map<string, string>;
    test?: (value: string) => string;
};
export function optionValue(axis: VariantAxis, value: string): string {
    return axis.type === "int" || axis.type === "bool"
        ? requireValue(axis.options.get(value))
        : `${axis.type}.${axis.options.get(value)}`;
}
export function predicate(
    values: Record<string, string>[],
    axes: VariantAxis[],
): string {
    return variantDecision(
        values,
        axes.map((axis) => ({
            key: axis.key,
            options: [...axis.options.keys()].sort(),
            finite: axis.type !== "int",
            test: (value) =>
                axis.test?.(value) ??
                `root.${axis.property} == ${optionValue(axis, value)}`,
        })),
    );
}
export function selectValues(
    cases: { values: Record<string, string>; code: string }[],
    axes: VariantAxis[],
    fallback: string,
): string {
    // Drop an axis only when every captured case agrees on the result after
    // projection. Unsupported tuples are guarded by the private variant validity check.
    let relevant = [...axes];
    for (const axis of axes) {
        const candidate = relevant.filter((a) => a !== axis);
        const results = new Map<string, string>();
        let conflict = false;
        for (const item of cases) {
            const key = JSON.stringify(
                candidate.map((a) => item.values[a.key]),
            );
            if (results.has(key) && results.get(key) !== item.code) {
                conflict = true;
                break;
            }
            results.set(key, item.code);
        }
        if (!conflict) relevant = candidate;
    }
    if (cases.every((c) => c.code === "true" || c.code === "false"))
        return predicate(
            cases.filter((c) => c.code === "true").map((c) => c.values),
            relevant,
        );
    // Factor values by authored axes instead of enumerating every combination
    // producing the same value. Shared suffixes collapse before printing.
    function choose(rows: typeof cases, remaining: VariantAxis[]): string {
        if (!rows.length) return fallback;
        if (rows.every((row) => row.code === rows[0].code)) return rows[0].code;
        const [axis, ...rest] = remaining;
        if (!axis) return fallback;
        const groups = new Map<string, string[]>();
        for (const option of [...axis.options.keys()].sort()) {
            const selected = rows.filter(
                (row) => row.values[axis.key] === option,
            );
            if (!selected.length) continue;
            const code = choose(selected, rest);
            const options = groups.get(code) ?? [];
            options.push(option);
            groups.set(code, options);
        }
        const branches = [...groups];
        let result = branches.pop()?.[0] ?? fallback;
        for (const [code, options] of branches.reverse()) {
            const condition = options
                .map(
                    (option) =>
                        axis.test?.(option) ??
                        `root.${axis.property} == ${optionValue(axis, option)}`,
                )
                .join(" || ");
            result = `(${condition}) ? (${code}) : (${result})`;
        }
        return result;
    }
    return choose(cases, relevant);
}
/** Break generated expressions only between tokens, never inside literal strings. */
export function declaration(
    prefix: string,
    expression: string,
    suffix = ";",
    width = 116,
): string[] {
    if (prefix.length + expression.length + suffix.length <= width)
        return [prefix + expression + suffix];
    const tokens: string[] = [];
    let part = "",
        quoted = false,
        escaped = false;
    for (const char of expression) {
        part += char;
        if (escaped) {
            escaped = false;
            continue;
        }
        if (char === "\\" && quoted) {
            escaped = true;
            continue;
        }
        if (char === '"') quoted = !quoted;
        if (char === " " && !quoted) {
            tokens.push(part);
            part = "";
        }
    }
    if (part) tokens.push(part);
    const lines = [prefix.trimEnd()];
    const continuation = " ".repeat(
        prefix.length - prefix.trimStart().length + 4,
    );
    let line = continuation;
    for (const token of tokens) {
        if (
            line.length > continuation.length &&
            line.length + token.length > width
        ) {
            lines.push(line.trimEnd());
            line = continuation;
        }
        line += token;
    }
    lines.push(line.trimEnd() + suffix);
    return lines;
}

export type Appearance = {
    values: Record<string, string>;
    properties: Record<string, string>;
};

/** Use the most common authored value as the normal binding. This avoids
 * repeating shared border/layout defaults in every interaction state. */
export function appearanceDefault(values: string[]): string {
    const counts = new Map<string, number>();
    for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1);
    return [...counts].sort((a, b) => b[1] - a[1])[0][0];
}

/** Only an explicitly authored state axis produces Slint states. Independent
 * layout/style axes are handled by ordinary property bindings in the caller. */
export function appearanceStates(
    samples: Appearance[],
    axis: VariantAxis,
    depth: number,
): string[] {
    const fields = Object.keys(samples[0].properties);
    const base = Object.fromEntries(
        fields.map((field) => [
            field,
            appearanceDefault(
                samples.map((sample) => sample.properties[field]),
            ),
        ]),
    );
    const indent = "    ".repeat(depth);
    const allocate = nameAllocator(enumReservedNames);
    const lines: string[] = [];
    for (const option of [...axis.options.keys()].sort()) {
        const sample = samples.find(
            (sample) => sample.values[axis.key] === option,
        );
        if (!sample) continue;
        const changes = Object.entries(sample.properties).filter(
            ([field, value]) => base[field] !== value,
        );
        if (!changes.length) continue;
        const name = allocate(
            axis.type === "int"
                ? `state-${option.replace("-", "minus-")}`
                : identifier(option),
        );
        lines.push(
            ...declaration(
                `${indent}    ${name} when `,
                axis.test?.(option) ??
                    `root.${axis.property} == ${optionValue(axis, option)}`,
                ": {",
            ),
        );
        for (const [field, value] of changes)
            lines.push(...declaration(`${indent}        ${field}: `, value));
        lines.push(`${indent}    }`);
    }
    return lines.length ? [`${indent}states [`, ...lines, `${indent}]`] : [];
}

/** A reduced finite-domain decision tree. Each branch is an exact subset of
 * observed tuples; unlike greedy cube merging, shared suffixes are factored. */
export type DecisionAxis = {
    key: string;
    options: string[];
    finite: boolean;
    test: (value: string) => string;
};
export function variantDecision(
    rows: Record<string, string>[],
    axes: DecisionAxis[],
): string {
    const memo = new Map<string, string>();
    const unique = [
        ...new Map(
            rows.map((row) => [
                JSON.stringify(axes.map((axis) => row[axis.key])),
                row,
            ]),
        ).values(),
    ];
    function solve(indices: number[], remaining: number[]): string {
        if (!indices.length) return "false";
        if (!remaining.length) return "true";
        const key = `${indices.join(",")}:${remaining.join(",")}`;
        const cached = memo.get(key);
        if (cached !== undefined) return cached;
        // Authored axis order is stable. Do not search permutations or choose
        // an opaque negative complement merely because its spelling is shorter.
        const index = remaining[0];
        {
            const axis = axes[index];
            const rest = remaining.filter((i) => i !== index);
            const groups = new Map<string, string[]>();
            for (const option of axis.options) {
                const suffix = solve(
                    indices.filter((i) => unique[i][axis.key] === option),
                    rest,
                );
                if (suffix === "false") continue;
                const options = groups.get(suffix) ?? [];
                options.push(option);
                groups.set(suffix, options);
            }
            const branches = [...groups].map(([suffix, options]) => {
                let condition = options.map(axis.test).join(" || ");
                if (axis.finite) {
                    if (options.length === axis.options.length)
                        condition = "true";
                }
                if (condition === "true") return suffix;
                if (suffix === "true") return condition;
                return `(${condition}) && (${suffix})`;
            });
            const expression = branches.includes("true")
                ? "true"
                : branches.length === 1
                  ? branches[0]
                  : branches.map((s) => `(${s})`).join(" || ") || "false";
            memo.set(key, expression);
            return expression;
        }
    }
    return solve(
        unique.map((_, i) => i),
        axes.map((_, i) => i),
    );
}
