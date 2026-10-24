// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import capabilities from "./runtime-fonts.json";

export function embeddedFontSupports(
    font: { family: string; style: string },
    weight: unknown,
    variations: unknown,
    characters: string,
): boolean {
    const axes = variations === undefined ? {} : variations;
    if (!axes || typeof axes !== "object" || Array.isArray(axes)) return false;
    const entries = Object.entries(axes);
    const effectiveWeight =
        entries.find(([tag]) => tag === "wght")?.[1] ?? weight;
    const italic =
        entries.find(([tag]) => tag === "ital")?.[1] ??
        /italic|oblique/i.test(font.style);
    return capabilities.fonts.some((available) => {
        if (
            !available.families.some(
                (family) =>
                    family.toLowerCase() === font.family.trim().toLowerCase(),
            ) ||
            Boolean(italic) !== available.italic ||
            typeof effectiveWeight !== "number" ||
            !Number.isInteger(effectiveWeight)
        )
            return false;
        const range = available.axes.wght;
        if (
            range
                ? effectiveWeight < range.min || effectiveWeight > range.max
                : effectiveWeight !== available.weight
        )
            return false;
        if (
            entries.some(
                ([tag, value]) =>
                    !(
                        tag === "wght" &&
                        typeof value === "number" &&
                        Number.isInteger(value)
                    ) &&
                    !(tag === "ital" && value === Number(available.italic)),
            )
        )
            return false;
        return Array.from(characters).every((character) => {
            if (/^[\p{Cc}\p{Cf}]$/u.test(character)) return true;
            const cp = character.codePointAt(0);
            return (
                cp !== undefined &&
                available.ranges.some(
                    ([start, end]) => cp >= start && cp <= end,
                )
            );
        });
    });
}
