// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export function solidVectorSvg(
    properties: Record<string, unknown>,
    paths: unknown,
): string | undefined {
    const { width, height, fills, strokes, effects } = properties;
    if (
        typeof width !== "number" ||
        !Number.isFinite(width) ||
        width <= 0 ||
        typeof height !== "number" ||
        !Number.isFinite(height) ||
        height <= 0 ||
        !Array.isArray(fills) ||
        !Array.isArray(strokes) ||
        strokes.length !== 0 ||
        !Array.isArray(effects) ||
        effects.length !== 0 ||
        [
            "cornerRadius",
            "topLeftRadius",
            "topRightRadius",
            "bottomLeftRadius",
            "bottomRightRadius",
        ].some(
            (key) => properties[key] !== undefined && properties[key] !== 0,
        ) ||
        !Array.isArray(paths) ||
        paths.length === 0 ||
        paths.some(
            (path) =>
                !path ||
                !["NONZERO", "EVENODD"].includes(path.windingRule) ||
                typeof path.data !== "string" ||
                !/^[Mm][MmLlHhVvCcSsQqTtAaZz0-9eE+.,\s-]+$/.test(path.data),
        )
    )
        return undefined;
    const visible = fills.filter((paint) => paint?.visible !== false);
    if (visible.length !== 1) return undefined;
    const paint = visible[0];
    if (
        !paint ||
        paint.type !== "SOLID" ||
        (paint.blendMode !== undefined && paint.blendMode !== "NORMAL") ||
        !paint.color ||
        ![paint.color.r, paint.color.g, paint.color.b].every(
            (value) =>
                typeof value === "number" &&
                Number.isFinite(value) &&
                value >= 0 &&
                value <= 1,
        )
    )
        return undefined;
    const opacity = properties.opacity ?? 1;
    const paintOpacity = paint.opacity ?? 1;
    if (
        ![opacity, paintOpacity].every(
            (value) =>
                typeof value === "number" &&
                Number.isFinite(value) &&
                value >= 0 &&
                value <= 1,
        )
    )
        return undefined;
    const color = [paint.color.r, paint.color.g, paint.color.b]
        .map((value) =>
            Math.round(value * 255)
                .toString(16)
                .padStart(2, "0"),
        )
        .join("");
    return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><g opacity="${opacity}" fill="#${color}" fill-opacity="${paintOpacity}">${paths.map((path) => `<path d="${path.data}" fill-rule="${path.windingRule === "EVENODD" ? "evenodd" : "nonzero"}"/>`).join("")}</g></svg>`;
}

export function unpaintedVectorSvg(
    properties: Record<string, unknown>,
    network: unknown,
): string | undefined {
    const { width, height, fills, strokes, effects, fillStyleId } = properties;
    if (
        typeof width !== "number" ||
        !Number.isFinite(width) ||
        width <= 0 ||
        typeof height !== "number" ||
        !Number.isFinite(height) ||
        height <= 0 ||
        !Array.isArray(fills) ||
        fills.length !== 0 ||
        !Array.isArray(strokes) ||
        strokes.length !== 0 ||
        !Array.isArray(effects) ||
        effects.length !== 0 ||
        properties.isMask === true ||
        (fillStyleId !== undefined && fillStyleId !== "") ||
        !network ||
        typeof network !== "object"
    )
        return undefined;
    const regions = (network as { regions?: unknown }).regions;
    if (
        !Array.isArray(regions) ||
        regions.some(
            (region) =>
                !region ||
                !Array.isArray(region.fills) ||
                region.fills.length !== 0 ||
                (region.fillStyleId !== undefined && region.fillStyleId !== ""),
        )
    )
        return undefined;
    return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"/>`;
}
