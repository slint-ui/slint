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
    const groups = new Map<string, Record<string, string>[]>();
    for (const item of cases) {
        if (item.code === fallback) continue;
        const group = groups.get(item.code) ?? [];
        group.push(item.values);
        groups.set(item.code, group);
    }
    return [...groups].reduceRight((tail, [code, values]) => {
        const condition = predicate(values, relevant);
        return condition === "true"
            ? code
            : `${condition} ? ${code} : (${tail})`;
    }, fallback);
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

/** Partition whole appearances, not individual predicates: states on one element
 * are exclusive in Slint. Project away an axis only if all captured rows agree. */
export function appearanceStates(
    samples: Appearance[],
    axes: VariantAxis[],
    depth: number,
): string[] {
    const base = Object.fromEntries(
        Object.keys(samples[0].properties).map((key) => [
            key,
            appearanceDefault(samples.map((sample) => sample.properties[key])),
        ]),
    );
    let relevant = [...axes];
    for (const axis of axes) {
        const candidate = relevant.filter((a) => a !== axis);
        const seen = new Map<string, string>();
        if (
            samples.every((s) => {
                const key = JSON.stringify(
                    candidate.map((a) => s.values[a.key]),
                );
                const value = JSON.stringify(s.properties);
                if (seen.has(key) && seen.get(key) !== value) return false;
                seen.set(key, value);
                return true;
            })
        )
            relevant = candidate;
    }
    const groups = new Map<string, Appearance[]>();
    for (const sample of samples) {
        const key = JSON.stringify(sample.properties);
        const group = groups.get(key) ?? [];
        group.push(sample);
        groups.set(key, group);
    }
    const indent = "    ".repeat(depth);
    const allocate = nameAllocator(enumReservedNames);
    const lines: string[] = [];
    for (const group of groups.values()) {
        const changes = Object.entries(group[0].properties).filter(
            ([key, value]) => base[key] !== value,
        );
        if (!changes.length) continue;
        const name = allocate(
            relevant
                .flatMap((a) => {
                    const values = [
                        ...new Set(group.map((s) => s.values[a.key])),
                    ].sort();
                    if (values.length === a.options.size || values.length > 3)
                        return [];
                    return [
                        values
                            .map((value) =>
                                a.type === "int"
                                    ? `${identifier(a.key)}-${value.replace("-", "minus-")}`
                                    : identifier(value),
                            )
                            .join("-or-"),
                    ];
                })
                .join("-") || "appearance",
        );
        lines.push(
            ...declaration(
                `${indent}    ${name} when `,
                predicate(
                    group.map((s) => s.values),
                    relevant,
                ),
                ": {",
            ),
        );
        for (const [key, value] of changes)
            lines.push(...declaration(`${indent}        ${key}: `, value));
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
        let best: string | undefined;
        for (const index of remaining) {
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
                    const excluded = axis.options.filter(
                        (v) => !options.includes(v),
                    );
                    const negative = excluded
                        .map((v) => axis.test(v).replace(" == ", " != "))
                        .join(" && ");
                    if (!excluded.length) condition = "true";
                    else if (negative.length < condition.length)
                        condition = negative;
                }
                if (condition === "true") return suffix;
                if (suffix === "true") return condition;
                return `(${condition}) && (${suffix})`;
            });
            let expression = branches.includes("true")
                ? "true"
                : branches.length === 1
                  ? branches[0]
                  : branches.map((s) => `(${s})`).join(" || ") || "false";
            // A || (!A && B) == A || B. The branches partition the whole
            // enum domain, so the non-default branch need not repeat !A.
            if (
                axis.finite &&
                groups.size === 2 &&
                groups.has("true") &&
                [...groups.values()].flat().length === axis.options.length
            ) {
                const other = [...groups.keys()].find(
                    (value) => value !== "true",
                );
                const trueIndex = [...groups.keys()].indexOf("true");
                if (other !== undefined) {
                    const reduced = `(${branches[trueIndex]}) || (${other})`;
                    if (reduced.length < expression.length)
                        expression = reduced;
                }
            }
            if (best === undefined || expression.length < best.length)
                best = expression;
        }
        const result = best ?? "false";
        memo.set(key, result);
        return result;
    }
    return solve(
        unique.map((_, i) => i),
        axes.map((_, i) => i),
    );
}
