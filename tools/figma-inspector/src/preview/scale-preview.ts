// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { FigmaSnapshot, SnapshotNode } from "../plugin/snapshot";

const lengths = new Set([
    "x",
    "y",
    "width",
    "height",
    "fontSize",
    "letterSpacing",
    "paddingLeft",
    "paddingRight",
    "paddingTop",
    "paddingBottom",
    "itemSpacing",
    "counterAxisSpacing",
    "strokeTopWeight",
    "strokeRightWeight",
    "strokeBottomWeight",
    "strokeLeftWeight",
    "offsetX",
    "offsetY",
    "blur",
    "spread",
]);

/** Scale logical preview geometry without changing source snapshots or asset pixels. */
export function scalePreview(
    snapshot: FigmaSnapshot,
    scale: number,
): FigmaSnapshot {
    const dimensions = <T extends object>(value: T): T =>
        Object.fromEntries(
            Object.entries(value).map(([key, value]) => [
                key,
                typeof value === "number" && lengths.has(key)
                    ? value * scale
                    : value,
            ]),
        ) as T;
    const node = (original: SnapshotNode): SnapshotNode =>
        ({
            ...dimensions(original),
            ...("children" in original
                ? { children: original.children.map(node) }
                : {}),
            ...("autoLayout" in original && original.autoLayout
                ? { autoLayout: dimensions(original.autoLayout) }
                : {}),
            ...("cornerRadii" in original
                ? {
                      cornerRadii: original.cornerRadii.map(
                          (value) => value * scale,
                      ),
                  }
                : {}),
            ...("strokes" in original
                ? {
                      strokes: original.strokes.map((stroke) => ({
                          ...dimensions(stroke),
                          dashPattern: stroke.dashPattern.map(
                              (value) => value * scale,
                          ),
                      })),
                  }
                : {}),
            ...("shadows" in original
                ? { shadows: original.shadows.map(dimensions) }
                : {}),
            ...("fills" in original
                ? {
                      fills: original.fills.map((paint) =>
                          paint.kind === "image" &&
                          paint.tileScale !== undefined
                              ? { ...paint, tileScale: paint.tileScale * scale }
                              : paint,
                      ),
                  }
                : {}),
            ...("paintBounds" in original && original.paintBounds
                ? { paintBounds: dimensions(original.paintBounds) }
                : {}),
            ...("svgBounds" in original && original.svgBounds
                ? { svgBounds: dimensions(original.svgBounds) }
                : {}),
            ...("raster" in original && original.raster?.bounds
                ? {
                      raster: {
                          ...original.raster,
                          bounds: dimensions(original.raster.bounds),
                      },
                  }
                : {}),
        }) as SnapshotNode;
    // Preview geometry uses resolved values. Native export keeps the variable library.
    const { variables: _variables, ...resolved } = snapshot;
    return {
        ...resolved,
        root: node(snapshot.root),
        ...(snapshot.components
            ? {
                  components: {
                      ...snapshot.components,
                      definitions: snapshot.components.definitions.map(
                          (definition) => ({
                              ...definition,
                              variants: definition.variants.map((variant) => ({
                                  ...variant,
                                  root: node(variant.root),
                              })),
                          }),
                      ),
                  },
              }
            : {}),
    };
}
