// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { Diagnostic } from "./snapshot";
import { SOURCE_MIXED, type SourceBytes, type SourceNode } from "./source";

export function normalizeAppearanceProperties(
    node: SourceNode<SourceBytes>,
    p: Record<string, unknown>,
    warnings: Diagnostic[],
): void {
    const nonnegative = (value: unknown): value is number =>
        typeof value === "number" && Number.isFinite(value) && value >= 0;
    const finite = (value: unknown): value is number =>
        typeof value === "number" && Number.isFinite(value);
    function fallback(key: string, value: unknown, action: string) {
        warnings.push({
            severity: "warning",
            code: "APPEARANCE_APPROXIMATED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: key,
            message: `${key} ${String(p[key])}; ${action}`,
        });
        p[key] = value;
    }
    for (const error of node.errors)
        warnings.push({
            severity: "warning",
            code: "PROPERTY_READ_FAILED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: error.property,
            message: `${error.message}; unavailable property uses its documented fallback`,
        });
    for (const key of ["fills", "strokes"]) {
        if (key in p && !Array.isArray(p[key]) && p[key] !== SOURCE_MIXED)
            fallback(key, [], "paint list omitted");
    }
    if ("cornerRadius" in p) {
        const radii =
            p.cornerRadius === SOURCE_MIXED
                ? [
                      p.topLeftRadius,
                      p.topRightRadius,
                      p.bottomRightRadius,
                      p.bottomLeftRadius,
                  ]
                : [p.cornerRadius];
        if (!radii.every(nonnegative))
            fallback("cornerRadius", 0, "using square corners");
    }
    if (Array.isArray(p.strokes) && p.strokes.length > 0) {
        const weights =
            p.strokeWeight === SOURCE_MIXED
                ? [
                      p.strokeTopWeight,
                      p.strokeRightWeight,
                      p.strokeBottomWeight,
                      p.strokeLeftWeight,
                  ]
                : [p.strokeWeight];
        if (!weights.every(nonnegative))
            fallback(
                "strokes",
                [],
                "stroke omitted because its width is unavailable",
            );
        if (Array.isArray(p.dashPattern) && !p.dashPattern.every(nonnegative))
            fallback("dashPattern", [], "using a solid stroke");
    }
    if (Array.isArray(p.effects))
        p.effects = p.effects.filter((effect, index) => {
            if (!effect || typeof effect !== "object") return false;
            if (
                !["DROP_SHADOW", "INNER_SHADOW"].includes(effect.type) ||
                effect.visible === false
            )
                return true;
            const valid =
                nonnegative(effect.radius) &&
                finite(effect.offset?.x) &&
                finite(effect.offset?.y) &&
                finite(effect.spread ?? 0) &&
                [
                    effect.color?.r,
                    effect.color?.g,
                    effect.color?.b,
                    effect.color?.a,
                ].every((v) => finite(v) && v >= 0 && v <= 1);
            if (!valid)
                warnings.push({
                    severity: "warning",
                    code: "EFFECT_OMITTED",
                    nodeId: node.id,
                    nodeName: node.name,
                    propertyPath: `effects[${index}]`,
                    message:
                        "Invalid shadow omitted; other effects and children are retained",
                });
            return valid;
        });
}

export function normalizeLayoutProperties(
    node: SourceNode<SourceBytes>,
    properties: Record<string, unknown>,
    warnings: Diagnostic[],
    parentAutoLayout = false,
): void {
    function warn(code: string, key: string, fallback: string) {
        warnings.push({
            severity: "warning",
            code,
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: key,
            message: `${key} ${String(properties[key])} ${fallback}`,
        });
    }
    const mode = properties.layoutMode;
    if (
        (mode !== undefined ||
            ["FRAME", "COMPONENT", "INSTANCE", "COMPONENT_SET"].includes(
                node.type,
            )) &&
        mode !== "NONE" &&
        mode !== "GRID"
    ) {
        if (
            !["HORIZONTAL", "VERTICAL"].includes(String(mode)) ||
            !["MIN", "CENTER", "MAX", "SPACE_BETWEEN"].includes(
                String(properties.primaryAxisAlignItems),
            ) ||
            !["MIN", "CENTER", "MAX", "BASELINE"].includes(
                String(properties.counterAxisAlignItems),
            )
        ) {
            warn(
                "LAYOUT_GEOMETRY_APPROXIMATED",
                "layoutMode",
                "uses captured absolute geometry because layout mode or alignment is unsupported",
            );
            properties.layoutMode = "NONE";
        } else {
            for (const key of [
                "paddingLeft",
                "paddingRight",
                "paddingTop",
                "paddingBottom",
                "itemSpacing",
                "counterAxisSpacing",
            ]) {
                if (key === "counterAxisSpacing" && properties[key] == null)
                    continue;
                const value = properties[key];
                if (
                    typeof value !== "number" ||
                    !Number.isFinite(value) ||
                    (key.startsWith("padding") && value < 0)
                ) {
                    warn(
                        key.startsWith("padding")
                            ? "LAYOUT_PADDING_APPROXIMATED"
                            : "LAYOUT_SPACING_APPROXIMATED",
                        key,
                        "was rendered as 0 px",
                    );
                    properties[key] = 0;
                }
            }
        }
    }
    for (const key of ["layoutSizingHorizontal", "layoutSizingVertical"]) {
        if (
            (key in properties ||
                parentAutoLayout ||
                mode === "HORIZONTAL" ||
                mode === "VERTICAL") &&
            !["FIXED", "HUG", "FILL"].includes(String(properties[key]))
        ) {
            warn(
                "LAYOUT_SIZING_APPROXIMATED",
                key,
                "uses the captured fixed dimension",
            );
            properties[key] = "FIXED";
        }
    }
}
