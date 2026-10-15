// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export const REST_UNAVAILABLE = Symbol("REST field unavailable");
const excluded = new Set([
    "rotation",
    "boundVariables",
    "componentPropertyReferences",
    "resolvedVariableModes",
    "fontName",
    "fontWeight",
    "fontStyle",
    "characters",
    "strokeWeight",
    "layoutAlign",
]);
const defaults: Record<string, unknown> = {
    visible: true,
    isMask: false,
    clipsContent: false,
    opacity: 1,
    effects: [],
    fills: [],
    strokes: [],
    minWidth: null,
    maxWidth: null,
    minHeight: null,
    maxHeight: null,
    layoutPositioning: "AUTO",
    rotation: 0,
    layoutMode: "NONE",
    layoutWrap: "NO_WRAP",
    primaryAxisAlignItems: "MIN",
    counterAxisAlignItems: "MIN",
    counterAxisAlignContent: "AUTO",
    itemSpacing: 0,
    counterAxisSpacing: 0,
    paddingLeft: 0,
    paddingRight: 0,
    paddingTop: 0,
    paddingBottom: 0,
    itemReverseZIndex: false,
    strokesIncludedInLayout: false,
    maskType: "ALPHA",
};
export function restProperty(
    node: Record<string, unknown>,
    key: string,
): unknown {
    if (key === "rotation" && !(key in node)) return 0;
    const corners = [
        "topLeftRadius",
        "topRightRadius",
        "bottomRightRadius",
        "bottomLeftRadius",
    ];
    const corner = corners.indexOf(key);
    if (node.type === "VECTOR" && (corner !== -1 || key === "cornerRadius"))
        return REST_UNAVAILABLE;
    if (corner !== -1) {
        const radii = node.rectangleCornerRadii;
        if (Array.isArray(radii) && radii.length === 4) return radii[corner];
        return node.cornerRadius ?? 0;
    }
    if (key === "cornerRadius" && !node.rectangleCornerRadii)
        return node.cornerRadius ?? 0;
    if (key === "dashPattern") return node.strokeDashes ?? [];
    if (key === "boundVariables" && !(key in node)) return {};
    if (excluded.has(key)) return REST_UNAVAILABLE;
    const value =
        key in node
            ? node[key]
            : key in defaults
              ? defaults[key]
              : REST_UNAVAILABLE;
    if (key === "fills" || key === "strokes") {
        if (
            !Array.isArray(value) ||
            value.some(
                (paint) =>
                    !paint ||
                    paint.type !== "SOLID" ||
                    (paint.opacity !== undefined &&
                        (typeof paint.opacity !== "number" ||
                            !Number.isFinite(paint.opacity) ||
                            paint.opacity < 0 ||
                            paint.opacity > 1)) ||
                    (paint.color?.a !== undefined && paint.color.a !== 1) ||
                    (paint.boundVariables &&
                        Object.keys(paint.boundVariables).length !== 0),
            )
        )
            return REST_UNAVAILABLE;
        return value.some((paint) => paint.opacity === undefined)
            ? value.map((paint) =>
                  paint.opacity === undefined
                      ? { ...paint, opacity: 1 }
                      : paint,
              )
            : value;
    }
    if (key === "effects" && (!Array.isArray(value) || value.length !== 0))
        return REST_UNAVAILABLE;
    return value;
}
