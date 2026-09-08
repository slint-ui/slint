// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    color,
    isMixed,
    number,
    problem,
    CompatibilityError,
    type MaterializedNode,
    type NormalizationContext,
    type TraversalOptions,
    type NodeResult,
} from "./normalization-context";
import {
    normalizeText,
    classifyNonInterText,
    classifyIconText,
    captureTableCellText,
} from "./normalization-text";
import {
    exportVisual,
    visiblePaints,
    normalizeVisual,
} from "./normalization-visual";
import { NormalizationAssets } from "../images";
import { normalizeComponents } from "./components";
import { uniqueDiagnostics } from "./snapshot";
import { normalizeAppearanceProperties } from "./normalization-properties";
import { maskComposition } from "./mask-composition";
import { normalizeLayoutProperties } from "./normalization-properties";
import {
    SOURCE_MIXED,
    decodeValue,
    validateSource,
    type SourceCapture as JsonSourceCapture,
    type SourceBytes,
    type SourceNode as JsonSourceNode,
} from "./source";
import {
    type CaptureResult,
    type Diagnostic,
    type FigmaSnapshot,
    SNAPSHOT_SCHEMA_VERSION,
    type SnapshotAppearance,
    type SnapshotAutoLayout,
    type SnapshotNode,
    type SnapshotPaint,
    type SnapshotShadow,
    type SnapshotSizing,
    type SnapshotStroke,
} from "./snapshot";

type SourceCapture = JsonSourceCapture<SourceBytes>;
type SourceNode = JsonSourceNode<SourceBytes>;
type MixedValue = unknown;

type ImageData = {
    readonly base64?: string;
    readonly bytes: Uint8Array;
    readonly width?: number;
    readonly height?: number;
    readonly sizeError?: string;
};
export type ImageResolver = (hash: string) => Promise<ImageData | undefined>;
export type SvgExporter = (
    node: VectorNode | BooleanOperationNode | TextNode,
) => Promise<string>;
export type PngExporter = (
    node: VectorNode | BooleanOperationNode | TextNode,
    exportScale: number,
) => Promise<Uint8Array>;

export type CaptureInstrumentation = {
    readonly measureFontToImageConversion: <T>(
        operation: () => Promise<T>,
    ) => Promise<T>;
};

type MutableCaptureMetrics = {
    durationMs: number;
    requests: number;
    exports: number;
    cacheHits: number;
};

export type GenerationTarget = "preview" | "export";

function zeroCaptureMetrics(): MutableCaptureMetrics {
    return { durationMs: 0, requests: 0, exports: 0, cacheHits: 0 };
}

async function captureAppearance(
    node: MaterializedNode,
    mixedValue: MixedValue,
    imageResolver: ImageResolver,
): Promise<
    | { appearance: SnapshotAppearance; warnings: Diagnostic[] }
    | { error: Diagnostic }
> {
    const nodeWithPaints = node;
    const rawFills = "fills" in node ? node.fills : [];
    const rawStrokes = "strokes" in node ? node.strokes : [];
    const fills = await visiblePaints(
        rawFills,
        node,
        mixedValue,
        "fills",
        imageResolver,
    );
    // Slint borders accept brushes but not image data. Image strokes are a
    // valid Figma paint, so skip them before paint decoding and keep the
    // node renderable with an explicit warning instead of treating the
    // unsupported representation as a capture failure.
    const rawStrokeArray = Array.isArray(rawStrokes) ? rawStrokes : undefined;
    const skippedImageStrokes =
        rawStrokeArray?.filter(
            (item) => item.visible !== false && item.type === "IMAGE",
        ).length ?? 0;
    const strokePaints =
        rawStrokeArray === undefined
            ? rawStrokes
            : rawStrokeArray.filter((item) => item.type !== "IMAGE");
    const strokes = await visiblePaints(
        strokePaints,
        node,
        mixedValue,
        "strokes",
        imageResolver,
    );
    const cornerRadius =
        "cornerRadius" in node ? (node.cornerRadius as unknown) : 0;
    let cornerRadii: [number, number, number, number];
    if (isMixed(cornerRadius, mixedValue)) {
        if (
            !("topLeftRadius" in node) ||
            !("topRightRadius" in node) ||
            !("bottomRightRadius" in node) ||
            !("bottomLeftRadius" in node)
        )
            return {
                error: problem(
                    "NON_UNIFORM_CORNERS",
                    node,
                    "Individual corner radii are unavailable",
                    "cornerRadius",
                ),
            };
        cornerRadii = [
            node.topLeftRadius,
            node.topRightRadius,
            node.bottomRightRadius,
            node.bottomLeftRadius,
        ];
    } else {
        if (!number(cornerRadius) || cornerRadius < 0)
            return {
                error: problem(
                    "INVALID_CORNER_RADIUS",
                    node,
                    "Corner radii must be non-negative",
                    "cornerRadius",
                ),
            };
        cornerRadii = [cornerRadius, cornerRadius, cornerRadius, cornerRadius];
    }
    if (cornerRadii.some((radius) => !number(radius) || radius < 0))
        return {
            error: problem(
                "INVALID_CORNER_RADIUS",
                node,
                "Corner radii must be non-negative",
                "cornerRadii",
            ),
        };
    const warnings: Diagnostic[] = [...fills.warnings, ...strokes.warnings];
    if (skippedImageStrokes > 0)
        warnings.push({
            ...problem(
                "UNSUPPORTED_STROKE_PAINT",
                node,
                "Image stroke paints are unsupported and were omitted",
                "strokes",
            ),
            severity: "warning",
        });
    for (const item of Array.isArray(rawFills) ? rawFills : []) {
        if (item.visible === false || item.type !== "IMAGE") continue;
        if (
            item.scaleMode === "TILE" &&
            item.scalingFactor !== undefined &&
            (!number(item.scalingFactor) || item.scalingFactor <= 0)
        )
            warnings.push({
                ...problem(
                    "IMAGE_TILE_SCALE_APPROXIMATED",
                    node,
                    "Invalid image tile scale uses the intrinsic image size",
                    "fills",
                ),
                severity: "warning",
            });
        if ((item.rotation ?? 0) !== 0)
            warnings.push({
                ...problem(
                    "IMAGE_ROTATION_IGNORED",
                    node,
                    "Rotated image fills are rendered without rotation",
                    "fills",
                ),
                severity: "warning",
            });
        if (
            item.filters !== undefined &&
            Object.entries(item.filters).some(
                ([name, filter]) =>
                    filter !== 0 && !(name === "saturation" && filter === -1),
            )
        )
            warnings.push({
                ...problem(
                    "IMAGE_FILTERS_IGNORED",
                    node,
                    "Image adjustments other than full desaturation are not supported and were ignored",
                    "fills",
                ),
                severity: "warning",
            });
        if (
            item.scaleMode === "CROP" &&
            item.imageTransform !== undefined &&
            (Math.abs(item.imageTransform[0][1]) >= 1e-6 ||
                Math.abs(item.imageTransform[1][0]) >= 1e-6)
        )
            warnings.push({
                ...problem(
                    "IMAGE_CROP_APPROXIMATED",
                    node,
                    "Rotated image crops fall back to cover",
                    "fills",
                ),
                severity: "warning",
            });
    }
    const visibleStrokes = strokes.paints;
    const strokeData: SnapshotStroke[] = [];
    if (visibleStrokes.length > 0) {
        const strokeWeight = nodeWithPaints.strokeWeight;
        let weights: [number, number, number, number];
        if (isMixed(strokeWeight, mixedValue)) {
            const sides = [
                "strokeTopWeight",
                "strokeRightWeight",
                "strokeBottomWeight",
                "strokeLeftWeight",
            ] as const;
            const values = sides.map((side) =>
                side in node ? node[side] : undefined,
            );
            if (values.some((value) => !number(value) || value < 0))
                return {
                    error: problem(
                        "INVALID_STROKE_WEIGHT",
                        node,
                        "Individual stroke weights must be non-negative finite values",
                        "strokeWeight",
                    ),
                };
            weights = values as [number, number, number, number];
        } else if (number(strokeWeight) && strokeWeight >= 0) {
            weights = [strokeWeight, strokeWeight, strokeWeight, strokeWeight];
        } else
            return {
                error: problem(
                    "INVALID_STROKE_WEIGHT",
                    node,
                    "Stroke weight must be non-negative and finite",
                    "strokeWeight",
                ),
            };
        const strokeAlign = nodeWithPaints.strokeAlign;
        const align =
            strokeAlign === "CENTER"
                ? ("center" as const)
                : strokeAlign === "OUTSIDE"
                  ? ("outside" as const)
                  : undefined;
        if (
            strokeAlign !== undefined &&
            strokeAlign !== "INSIDE" &&
            strokeAlign !== "CENTER" &&
            strokeAlign !== "OUTSIDE"
        )
            warnings.push({
                ...problem(
                    "STROKE_ALIGNMENT_APPROXIMATED",
                    node,
                    "Unknown stroke alignment uses an inside border",
                    "strokeAlign",
                ),
                severity: "warning",
            });
        const dashPattern =
            "dashPattern" in node && Array.isArray(node.dashPattern)
                ? [...node.dashPattern]
                : [];
        if (dashPattern.some((item) => !number(item) || item < 0))
            return {
                error: problem(
                    "INVALID_STROKE_DASHES",
                    node,
                    "Dash pattern must contain non-negative finite values",
                    "dashPattern",
                ),
            };
        if (dashPattern.length > 0)
            warnings.push({
                ...problem(
                    "DASHED_STROKE_APPROXIMATED",
                    node,
                    "Dashed strokes are rendered as solid borders",
                    "dashPattern",
                ),
                severity: "warning",
            });
        const strokePaint = visibleStrokes.find(
            (item): item is SnapshotPaint => item.kind !== "image",
        );
        if (strokePaint !== undefined)
            strokeData.push({
                paint: strokePaint,
                ...(align === undefined ? {} : { align }),
                strokeTopWeight: weights[0],
                strokeRightWeight: weights[1],
                strokeBottomWeight: weights[2],
                strokeLeftWeight: weights[3],
                dashPattern,
            });
    }
    const shadows: SnapshotShadow[] = [];
    if ("effects" in node && Array.isArray(node.effects)) {
        const visibleEffects = node.effects.filter(
            (effect) => effect.visible !== false,
        );
        for (const effect of visibleEffects) {
            if (
                effect.type === "DROP_SHADOW" ||
                effect.type === "INNER_SHADOW"
            ) {
                const shadow = effect as DropShadowEffect;
                if (
                    !number(shadow.radius) ||
                    shadow.radius < 0 ||
                    !number(shadow.offset.x) ||
                    !number(shadow.offset.y) ||
                    !number(shadow.spread ?? 0)
                )
                    return {
                        error: problem(
                            "INVALID_EFFECT",
                            node,
                            "Shadow geometry must be finite and non-negative where applicable",
                            "effects",
                        ),
                    };
                shadows.push({
                    ...(effect.type === "INNER_SHADOW"
                        ? { kind: "inner" as const }
                        : {}),
                    color: color(shadow.color, node),
                    offsetX: shadow.offset.x,
                    offsetY: shadow.offset.y,
                    blur: shadow.radius,
                    spread: shadow.spread ?? 0,
                });
                if (shadow.blendMode !== "NORMAL")
                    warnings.push({
                        ...problem(
                            "SHADOW_BLEND_MODE_IGNORED",
                            node,
                            "Drop shadow blend mode is not supported and was ignored",
                            "effects",
                        ),
                        severity: "warning",
                    });
                if (effect.type === "INNER_SHADOW")
                    warnings.push({
                        ...problem(
                            "INNER_SHADOW_APPROXIMATED",
                            node,
                            "Inner shadow rendered using native Gaussian edge gradients; rounded-corner shadow profiles are approximated for the WASM renderer",
                            "effects",
                        ),
                        severity: "warning",
                    });
            } else if (
                effect.type === "LAYER_BLUR" ||
                effect.type === "BACKGROUND_BLUR"
            )
                warnings.push({
                    ...problem(
                        "UNSUPPORTED_EFFECT_IGNORED",
                        node,
                        "Layer and background blur effects were ignored",
                        "effects",
                    ),
                    severity: "warning",
                });
            else
                warnings.push({
                    ...problem(
                        "UNSUPPORTED_EFFECT_IGNORED",
                        node,
                        "Unsupported effects were ignored",
                        "effects",
                    ),
                    severity: "warning",
                });
        }
    }
    return {
        appearance: {
            fills: fills.paints,
            strokes: strokeData,
            cornerRadii,
            clipsContent: "clipsContent" in node ? node.clipsContent : false,
            shadows,
        },
        warnings,
    };
}

function captureAutoLayout(
    node: MaterializedNode,
    mixedValue: MixedValue,
):
    | {
          autoLayout: SnapshotAutoLayout | null;
          layoutFallback: "flex" | "freeform";
          warnings: Diagnostic[];
      }
    | { error: Diagnostic } {
    const mode = node.layoutMode;
    if (mode === "NONE")
        return { autoLayout: null, layoutFallback: "freeform", warnings: [] };
    if (mode === "GRID")
        return {
            autoLayout: null,
            layoutFallback: "freeform",
            warnings: [
                {
                    ...problem(
                        "GRID_LAYOUT_APPROXIMATED",
                        node,
                        "Grid auto-layout is rendered using captured absolute geometry",
                        "layoutMode",
                    ),
                    severity: "warning",
                },
            ],
        };
    if (mode !== "HORIZONTAL" && mode !== "VERTICAL")
        return {
            error: problem(
                "UNSUPPORTED_LAYOUT_MODE",
                node,
                `Auto-layout mode ${String(mode)} is not supported`,
                "layoutMode",
            ),
        };
    const warnings: Diagnostic[] = [];
    if (node.strokesIncludedInLayout)
        warnings.push({
            ...problem(
                "STROKES_IN_LAYOUT_APPROXIMATED",
                node,
                "Strokes included in layout are approximated using the uninflated geometry",
                "strokesIncludedInLayout",
            ),
            severity: "warning",
        });
    const padding = [
        ["paddingLeft", node.paddingLeft],
        ["paddingRight", node.paddingRight],
        ["paddingTop", node.paddingTop],
        ["paddingBottom", node.paddingBottom],
    ] as const;
    for (const [key, value] of padding) {
        if (isMixed(value, mixedValue) || !number(value) || value < 0)
            return {
                error: problem(
                    "INVALID_LAYOUT_PADDING",
                    node,
                    `${key} must be a non-negative finite number`,
                    key,
                ),
            };
    }
    if (isMixed(node.itemSpacing, mixedValue) || !number(node.itemSpacing))
        return {
            error: problem(
                "INVALID_LAYOUT_SPACING",
                node,
                "itemSpacing must be a finite number",
                "itemSpacing",
            ),
        };
    const primary: SnapshotAutoLayout["primaryAlignment"] | undefined =
        node.primaryAxisAlignItems === "MIN"
            ? "start"
            : node.primaryAxisAlignItems === "CENTER"
              ? "center"
              : node.primaryAxisAlignItems === "MAX"
                ? "end"
                : node.primaryAxisAlignItems === "SPACE_BETWEEN"
                  ? "space-between"
                  : undefined;
    if (primary === undefined)
        return {
            error: problem(
                "INVALID_LAYOUT_ALIGNMENT",
                node,
                "Unsupported primary auto-layout alignment",
                "primaryAxisAlignItems",
            ),
        };
    const counter: SnapshotAutoLayout["counterAlignment"] | undefined =
        node.counterAxisAlignItems === "MIN"
            ? "start"
            : node.counterAxisAlignItems === "CENTER"
              ? "center"
              : node.counterAxisAlignItems === "MAX"
                ? "end"
                : undefined;
    const counterAlignment = counter;
    if (node.counterAxisAlignItems === "BASELINE") {
        warnings.push({
            ...problem(
                "BASELINE_ALIGNMENT_APPROXIMATED",
                node,
                "Baseline alignment uses captured child positions because Slint does not support baseline layout",
                "counterAxisAlignItems",
            ),
            severity: "warning",
        });
        return { autoLayout: null, layoutFallback: "freeform", warnings };
    }
    if (counterAlignment === undefined)
        return {
            error: problem(
                "INVALID_LAYOUT_ALIGNMENT",
                node,
                "Unsupported counter auto-layout alignment",
                "counterAxisAlignItems",
            ),
        };
    const counterAxisSpacing =
        ("counterAxisSpacing" in node ? node.counterAxisSpacing : undefined) ??
        node.itemSpacing;
    const counterAxisAlignContent =
        node.layoutWrap === "WRAP"
            ? "counterAxisAlignContent" in node &&
              node.counterAxisAlignContent === "SPACE_BETWEEN"
                ? "space-between"
                : (node.children ?? []).length > 0 &&
                    (node.children ?? []).every(
                        (child) =>
                            "layoutAlign" in child &&
                            child.layoutAlign === "STRETCH",
                    )
                  ? "stretch"
                  : counterAlignment
            : "start";
    if (
        node.itemReverseZIndex &&
        (node.children ?? []).some(
            (child) =>
                "layoutPositioning" in child &&
                child.layoutPositioning === "ABSOLUTE",
        )
    )
        return {
            autoLayout: null,
            layoutFallback: "freeform",
            warnings: [
                ...warnings,
                {
                    ...problem(
                        "REVERSE_Z_ORDER_GEOMETRY_APPROXIMATED",
                        node,
                        "Reverse paint order with absolute children uses captured geometry",
                        "itemReverseZIndex",
                    ),
                    severity: "warning",
                },
            ],
        };
    return {
        autoLayout: {
            direction: mode === "HORIZONTAL" ? "horizontal" : "vertical",
            paddingLeft: node.paddingLeft,
            paddingRight: node.paddingRight,
            paddingTop: node.paddingTop,
            paddingBottom: node.paddingBottom,
            itemSpacing: node.itemSpacing,
            wrap: node.layoutWrap === "WRAP",
            counterAxisSpacing,
            counterAxisAlignContent,
            primaryAlignment: primary,
            counterAlignment: counterAlignment,
            ...(node.itemReverseZIndex ? { reversePaintOrder: true } : {}),
        },
        layoutFallback: "flex",
        warnings,
    };
}

function captureSizing(
    node: MaterializedNode,
    parentLayout: SnapshotAutoLayout | undefined,
    isAutoLayoutFrame: boolean,
    mixedValue: MixedValue,
):
    | {
          layoutSizingHorizontal: SnapshotSizing | null;
          layoutSizingVertical: SnapshotSizing | null;
      }
    | { error: Diagnostic } {
    if (!isAutoLayoutFrame && parentLayout === undefined)
        return { layoutSizingHorizontal: null, layoutSizingVertical: null };
    const horizontal = node.layoutSizingHorizontal;
    const vertical = node.layoutSizingVertical;
    const toSizing = (value: unknown): SnapshotSizing | undefined => {
        if (isMixed(value, mixedValue)) return undefined;
        if (value === "FIXED") return "fixed";
        if (value === "HUG") return "hug";
        if (value === "FILL") return "fill";
        return undefined;
    };
    const sizingHorizontal = toSizing(horizontal);
    const sizingVertical = toSizing(vertical);
    if (sizingHorizontal === undefined)
        return {
            error: problem(
                "INVALID_LAYOUT_SIZING",
                node,
                "layoutSizingHorizontal must be FIXED, HUG, or FILL",
                "layoutSizingHorizontal",
            ),
        };
    if (sizingVertical === undefined)
        return {
            error: problem(
                "INVALID_LAYOUT_SIZING",
                node,
                "layoutSizingVertical must be FIXED, HUG, or FILL",
                "layoutSizingVertical",
            ),
        };
    return {
        layoutSizingHorizontal: sizingHorizontal,
        layoutSizingVertical: sizingVertical,
    };
}

function geometry(node: MaterializedNode): {
    x: number;
    y: number;
    width: number;
    height: number;
    opacity: number;
    visible: boolean;
    rotation: number;
    layoutPositioning: "auto" | "absolute";
} {
    return {
        x: node.x,
        y: node.y,
        width: node.width,
        height: node.height,
        opacity: node.opacity,
        visible: node.visible,
        rotation: node.rotation,
        layoutPositioning:
            "layoutPositioning" in node && node.layoutPositioning === "ABSOLUTE"
                ? "absolute"
                : "auto",
    };
}

function collectNodeIds(node: MaterializedNode, nodeIds: string[]): void {
    nodeIds.push(node.id);
    if (node.children !== undefined) {
        for (const child of node.children) collectNodeIds(child, nodeIds);
    }
}

type TableCellCapture = {
    readonly node: SnapshotNode;
    readonly warnings: Diagnostic[];
};

function sectionAppearance(node: MaterializedNode): {
    appearance: SnapshotAppearance;
    warnings: Diagnostic[];
} {
    const hasPaints =
        (Array.isArray(node.fills) &&
            node.fills.some((paint) => paint.visible !== false)) ||
        (Array.isArray(node.strokes) &&
            node.strokes.some((paint) => paint.visible !== false)) ||
        ("effects" in node &&
            Array.isArray(node.effects) &&
            node.effects.some((effect) => effect.visible !== false));
    return {
        appearance: {
            fills: [],
            strokes: [],
            cornerRadii: [0, 0, 0, 0],
            clipsContent: false,
            shadows: [],
        },
        warnings: hasPaints
            ? [
                  {
                      ...problem(
                          "SECTION_APPEARANCE_IGNORED",
                          node,
                          "Section appearance is omitted because sections are transparent preview containers",
                          "appearance",
                      ),
                      severity: "warning",
                  },
              ]
            : [],
    };
}

function measureTable(table: MaterializedNode) {
    const rows = table.numRows;
    const columns = table.numColumns;
    const cellsData = table.sourceCells;
    if (
        !number(rows) ||
        !number(columns) ||
        !Number.isSafeInteger(rows) ||
        !Number.isSafeInteger(columns) ||
        rows < 0 ||
        columns < 0 ||
        !Array.isArray(cellsData)
    )
        return {
            error: problem(
                "INVALID_TABLE_STRUCTURE",
                table,
                "Table dimensions and cellAt() are required for table capture",
                "table",
            ),
        };
    const cells: {
        readonly row: number;
        readonly column: number;
        readonly value: Record<string, unknown>;
        readonly width: number;
        readonly height: number;
    }[] = [];
    const columnWidths = Array.from({ length: columns }, () => 0);
    const rowHeights = Array.from({ length: rows }, () => 0);
    for (let row = 0; row < rows; row += 1) {
        for (let column = 0; column < columns; column += 1) {
            const cell: unknown = cellsData[row * columns + column];
            if (typeof cell !== "object" || cell === null) {
                return {
                    error: problem(
                        "TABLE_CELL_CAPTURE_FAILED",
                        table,
                        "cellAt() did not return a table cell",
                        `cellAt(${row},${column})`,
                    ),
                };
            }
            const value = cell as Record<string, unknown>;
            const width = value.width;
            const height = value.height;
            if (!number(width) || !number(height) || width < 0 || height < 0)
                return {
                    error: problem(
                        "INVALID_TABLE_CELL_GEOMETRY",
                        table,
                        "Table cell dimensions must be finite and non-negative",
                        `cellAt(${row},${column})`,
                    ),
                };
            cells.push({ row, column, value, width, height });
            columnWidths[column] = Math.max(columnWidths[column], width);
            rowHeights[row] = Math.max(rowHeights[row], height);
        }
    }
    function offsets(sizes: readonly number[]) {
        let total = 0;
        const positions = sizes.map((size) => {
            const position = total;
            total += size;
            return position;
        });
        return { positions, total };
    }
    const columnsMeasured = offsets(columnWidths);
    const rowsMeasured = offsets(rowHeights);
    return {
        cells,
        columnWidths,
        rowHeights,
        columnOffsets: columnsMeasured.positions,
        rowOffsets: rowsMeasured.positions,
        width: columnsMeasured.total,
        height: rowsMeasured.total,
    };
}

async function captureTableCells(
    table: MaterializedNode,
    mixedValue: MixedValue,
    imageResolver: ImageResolver,
    measurement = measureTable(table),
): Promise<
    | {
          readonly cells: readonly SnapshotNode[];
          readonly warnings: Diagnostic[];
      }
    | { readonly error: Diagnostic }
> {
    if (measurement.error !== undefined) return { error: measurement.error };
    const { cells, columnOffsets, rowOffsets } = measurement;
    const captured: TableCellCapture[] = [];
    for (const cell of cells) {
        const id = `${table.id}:cell:${cell.row}:${cell.column}`;
        const name = `${table.name} cell ${cell.row + 1},${cell.column + 1}`;
        const appearanceNode = {
            id,
            name,
            fills: Array.isArray(cell.value.fills) ? cell.value.fills : [],
            strokes: [],
            cornerRadius: 0,
            clipsContent: false,
        } as unknown as MaterializedNode;
        const appearance = await captureAppearance(
            appearanceNode,
            mixedValue,
            imageResolver,
        );
        if ("error" in appearance) return { error: appearance.error };
        const warnings = [...appearance.warnings];
        const textRecord =
            typeof cell.value.text === "object" && cell.value.text !== null
                ? (cell.value.text as Record<string, unknown>)
                : undefined;
        const characters =
            typeof textRecord?.characters === "string"
                ? textRecord.characters
                : "";
        const textNode =
            characters.length === 0 || textRecord === undefined
                ? undefined
                : await captureTableCellText(
                      textRecord,
                      id,
                      name,
                      cell.width,
                      cell.height,
                      mixedValue,
                      imageResolver,
                      warnings,
                  );
        captured.push({
            node: {
                kind: "container",
                id,
                name,
                x: columnOffsets[cell.column],
                y: rowOffsets[cell.row],
                width: cell.width,
                height: cell.height,
                opacity: 1,
                visible: true,
                rotation: 0,
                layoutPositioning: "absolute",
                layoutSizingHorizontal: null,
                layoutSizingVertical: null,
                containerKind: "generic",
                layoutFallback: "freeform",
                autoLayout: null,
                ...appearance.appearance,
                children: textNode === undefined ? [] : [textNode],
            },
            warnings,
        });
    }
    return {
        cells: captured.map((item) => item.node),
        warnings: captured.flatMap((item) => item.warnings),
    };
}

async function captureChildren(
    source: readonly MaterializedNode[],
    context: NormalizationContext,
    children: SnapshotNode[],
    warnings: Diagnostic[],
    options: TraversalOptions = {},
): Promise<NodeResult | undefined> {
    for (const child of source) {
        const result = await captureNode(child, context, options);
        if (result.error !== undefined) return result;
        if (result.node !== undefined) children.push(result.node);
        if (result.warnings !== undefined) warnings.push(...result.warnings);
    }
}

// Only structured compatibility failures are recoverable. Exceptions are deliberately
// not caught here: programming errors must still fail the revision.
async function captureNode(
    node: MaterializedNode,
    context: NormalizationContext,
    options: TraversalOptions = {},
): Promise<NodeResult> {
    const result = await captureNodeStrict(node, context, options);
    if (!result.error) return result;
    const { isRoot } = options;
    if (isRoot) return result;
    const warning: Diagnostic = {
        ...result.error,
        severity: "warning",
        code: "NODE_PLACEHOLDER",
        message: `${result.error.code}: ${result.error.message}; retained a transparent geometry placeholder`,
    };
    const bounds = [node.x, node.y, node.width, node.height];
    if (!bounds.every(Number.isFinite) || node.width < 0 || node.height < 0) {
        return {
            warnings: [
                {
                    ...warning,
                    code: "NODE_OMITTED",
                    message: `${result.error.message}; omitted node because usable bounds are unavailable`,
                },
            ],
        };
    }
    const children: SnapshotNode[] = [];
    const warnings = [warning];
    if (node.children !== undefined)
        for (const child of node.children) {
            const captured = await captureNode(child, context, {
                parentLayout: options.parentLayout,
            });
            if (captured.node) children.push(captured.node);
            warnings.push(...(captured.warnings ?? []));
        }
    return {
        node: {
            id: node.id,
            name: node.name,
            kind: "group",
            containerKind: "generic",
            layoutFallback: "freeform",
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            opacity:
                "opacity" in node && Number.isFinite(node.opacity)
                    ? node.opacity
                    : 1,
            rotation:
                "rotation" in node && Number.isFinite(node.rotation)
                    ? node.rotation
                    : 0,
            visible: node.visible !== false,
            layoutPositioning: "auto",
            layoutSizingHorizontal: "fixed",
            layoutSizingVertical: "fixed",
            children,
        },
        warnings,
    };
}

async function captureNodeStrict(
    sourceNode: MaterializedNode,
    context: NormalizationContext,
    { parentLayout, isRoot = false }: TraversalOptions = {},
): Promise<NodeResult> {
    const { mixedValue, imageResolver, exportPngNode } = context;
    const runtimeNodeType = sourceNode.type;
    // Figma can expose hidden descendants when a component set or instance is
    // selected. They are not renderable, and exportAsync rejects them with
    // "This node may not have any visible layers". Skip them before reading
    // geometry or attempting any visual export. Keep the root so selecting a
    // hidden container still produces the same empty preview semantics.
    if (!isRoot && sourceNode.visible === false) return {};
    if (
        sourceNode.type !== "GROUP" &&
        sourceNode.type !== "FRAME" &&
        sourceNode.type !== "COMPONENT" &&
        sourceNode.type !== "INSTANCE" &&
        sourceNode.type !== "RECTANGLE" &&
        sourceNode.type !== "VECTOR" &&
        sourceNode.type !== "BOOLEAN_OPERATION" &&
        sourceNode.type !== "ELLIPSE" &&
        sourceNode.type !== "LINE" &&
        sourceNode.type !== "POLYGON" &&
        sourceNode.type !== "STAR" &&
        sourceNode.type !== "COMPONENT_SET" &&
        sourceNode.type !== "TEXT" &&
        sourceNode.type !== "SECTION" &&
        sourceNode.type !== "TABLE" &&
        runtimeNodeType !== "TABLE_CELL" &&
        sourceNode.type !== "CONNECTOR" &&
        sourceNode.type !== "SHAPE_WITH_TEXT"
    ) {
        const structural = sourceNode;
        if (Array.isArray(structural.children)) {
            // Keep this runtime fallback deliberately narrow. New structural
            // Figma nodes can still be represented without spreading their
            // proprietary types into the snapshot contract.
        } else if (sourceNode.type === "SLICE") {
            // It is represented as an empty transparent container below so a
            // helper node never turns an otherwise valid selection into an
            // error.
            if (!isRoot)
                return {
                    warnings: [
                        {
                            ...problem(
                                "NON_VISUAL_NODE_SKIPPED",
                                sourceNode,
                                "Non-visual helper node was skipped",
                                "type",
                            ),
                            severity: "warning",
                        },
                    ],
                };
        } else {
            const exported = await captureNodeStrict(
                { ...sourceNode, type: "VECTOR" },
                context,
                { parentLayout, isRoot },
            );
            if (exported.node)
                return {
                    ...exported,
                    warnings: [
                        ...(exported.warnings ?? []),
                        {
                            ...problem(
                                "NODE_IMAGE_SUBSTITUTED",
                                sourceNode,
                                "Unsupported visual node rendered using its captured export",
                                "type",
                            ),
                            severity: "warning",
                        },
                    ],
                };
            return exported;
        }
    }
    let node = sourceNode;
    if (runtimeNodeType === "TABLE_CELL") {
        const cell = node;
        const cellId =
            "id" in cell && typeof cell.id === "string"
                ? cell.id
                : `table-cell:${String(cell.rowIndex ?? 0)}:${String(cell.columnIndex ?? 0)}`;
        const cellName =
            "name" in cell && typeof cell.name === "string"
                ? cell.name
                : `Table cell ${String(cell.rowIndex ?? 0)},${String(cell.columnIndex ?? 0)}`;
        node = {
            type: "TABLE_CELL",
            id: cellId,
            name: cellName,
            x: number(cell.x) ? cell.x : 0,
            y: number(cell.y) ? cell.y : 0,
            width: cell.width,
            height: cell.height,
            opacity: 1,
            visible: true,
            rotation: 0,
            fills: Array.isArray(cell.fills) ? cell.fills : [],
            strokes: [],
            strokeWeight: 0,
            strokeAlign: "INSIDE",
            cornerRadius: 0,
            clipsContent: false,
            text: "text" in cell ? cell.text : undefined,
        } as unknown as MaterializedNode;
    }
    const captureWarnings: Diagnostic[] = [];
    if (
        "blendMode" in node &&
        node.blendMode !== "NORMAL" &&
        node.blendMode !== "PASS_THROUGH"
    )
        captureWarnings.push({
            ...problem(
                "BLEND_MODE_APPROXIMATED",
                node,
                "Non-normal blend modes are rendered with normal compositing",
                "blendMode",
            ),
            severity: "warning",
        });
    if (node.type === "SLICE")
        captureWarnings.push({
            ...problem(
                "NON_VISUAL_NODE_SKIPPED",
                node,
                "Non-visual helper node was skipped",
                "type",
            ),
            severity: "warning",
        });
    if ("isMask" in node && node.isMask)
        captureWarnings.push({
            ...problem(
                "MASK_APPROXIMATED",
                node,
                "Mask semantics are not represented in the Slint preview",
                "isMask",
            ),
            severity: "warning",
        });
    if (
        node.type === "INSTANCE" &&
        "scaleFactor" in node &&
        (!number(node.scaleFactor) || node.scaleFactor <= 0)
    ) {
        captureWarnings.push({
            ...problem(
                "INVALID_INSTANCE_SCALE_IGNORED",
                node,
                "Invalid instance scale metadata was ignored; captured descendant geometry is already resolved",
                "scaleFactor",
            ),
            severity: "warning",
        });
    }
    const rawCapturable = node;
    const usesStructuralGeometryDefaults =
        runtimeNodeType === "SECTION" ||
        runtimeNodeType === "TABLE" ||
        runtimeNodeType === "TABLE_CELL";
    const tableMeasurement =
        runtimeNodeType === "TABLE" ? measureTable(node) : undefined;
    const tableGeometry =
        tableMeasurement && !("error" in tableMeasurement)
            ? tableMeasurement
            : undefined;
    // SECTION and newer structural node typings do not expose the common
    // opacity/rotation/visibility fields even though the runtime node is
    // still renderable. Keep those absent fields deterministic without
    // weakening validation when a field is present but malformed.
    let capturable = usesStructuralGeometryDefaults
        ? {
              ...rawCapturable,
              x: rawCapturable.x === undefined ? 0 : rawCapturable.x,
              y: rawCapturable.y === undefined ? 0 : rawCapturable.y,
              width:
                  rawCapturable.width === undefined
                      ? (tableGeometry?.width ?? 0)
                      : rawCapturable.width,
              height:
                  rawCapturable.height === undefined
                      ? (tableGeometry?.height ?? 0)
                      : rawCapturable.height,
              opacity: "opacity" in rawCapturable ? rawCapturable.opacity : 1,
              rotation:
                  "rotation" in rawCapturable ? rawCapturable.rotation : 0,
              visible:
                  "visible" in rawCapturable ? rawCapturable.visible : true,
          }
        : rawCapturable;
    // Nearly axis-aligned Figma instances can carry a negative zero-sized
    // extent from float32 transform arithmetic. Keep this absolute tolerance
    // tiny (1/100,000 pixel), and never repair genuinely negative geometry.
    for (const dimension of ["width", "height"] as const) {
        const value = capturable[dimension];
        if (number(value) && value < 0 && value >= -0.00001) {
            capturable = { ...capturable, [dimension]: 0 };
            captureWarnings.push({
                ...problem(
                    "GEOMETRY_ROUNDOFF_APPROXIMATED",
                    node,
                    `Rounded near-zero negative ${dimension} to zero`,
                    dimension,
                ),
                severity: "warning",
            });
        }
    }
    if (
        !number(capturable.x) ||
        !number(capturable.y) ||
        !number(capturable.width) ||
        !number(capturable.height) ||
        capturable.width < 0 ||
        capturable.height < 0 ||
        !number(capturable.opacity) ||
        capturable.opacity < 0 ||
        capturable.opacity > 1 ||
        !number(capturable.rotation)
    )
        return {
            error: problem(
                "INVALID_GEOMETRY",
                node,
                `Node geometry must contain finite non-negative sizes and opacity between 0 and 1 (x=${String(capturable.x)}, y=${String(capturable.y)}, width=${String(capturable.width)}, height=${String(capturable.height)}, opacity=${String(capturable.opacity)}, rotation=${String(capturable.rotation)})`,
                "geometry",
            ),
        };
    let autoLayout: SnapshotAutoLayout | null = null;
    let layoutFallback: "flex" | "freeform" = "freeform";
    if (
        node.type === "FRAME" ||
        node.type === "COMPONENT" ||
        node.type === "INSTANCE" ||
        node.type === "COMPONENT_SET"
    ) {
        const capturedLayout = captureAutoLayout(node, mixedValue);
        if ("error" in capturedLayout) return { error: capturedLayout.error };
        autoLayout = capturedLayout.autoLayout;
        layoutFallback = capturedLayout.layoutFallback;
        captureWarnings.push(...capturedLayout.warnings);
    }
    const sizing = captureSizing(
        capturable,
        parentLayout,
        autoLayout !== null,
        mixedValue,
    );
    if ("error" in sizing) return { error: sizing.error };
    const base = {
        ...geometry(capturable),
        ...sizing,
        id: node.id,
        name: node.name,
    };
    if (node.sourceMaskRaster) {
        const exported = await exportVisual(
            node,
            async () => "",
            exportPngNode,
            context.exportScale,
            context.assets?.encode,
        );
        const warning: Diagnostic = {
            ...problem(
                exported.raster
                    ? "MASK_IMAGE_SUBSTITUTED"
                    : "MASK_COMPOSITION_OMITTED",
                node,
                exported.raster
                    ? "Mask composition rendered as a PNG of its containing node"
                    : `Mask composition omitted because PNG export failed: ${String(exported.pngError ?? "PNG unavailable")}`,
                "children",
            ),
            severity: "warning",
        };
        return exported.raster
            ? {
                  node: {
                      ...base,
                      kind: "svg",
                      sourceType: "MASK_COMPOSITION",
                      raster: exported.raster,
                  },
                  warnings: [...captureWarnings, warning],
              }
            : {
                  node: {
                      ...base,
                      kind: "group",
                      containerKind: "generic",
                      layoutFallback: "freeform",
                      children: [],
                  },
                  warnings: [...captureWarnings, warning],
              };
    }
    if (node.type === "TEXT") {
        const textNode = node;
        const nonInterText =
            context.target === "export"
                ? false
                : classifyNonInterText(textNode, mixedValue);
        if (typeof nonInterText !== "boolean") return nonInterText;
        const iconInfo = classifyIconText(textNode, mixedValue);
        if (
            context.target !== "export" &&
            (nonInterText || iconInfo !== undefined)
        ) {
            context.metrics.requests += 1;
            context.metrics.exports += 1;
            return normalizeVisual(
                node,
                base,
                context,
                captureWarnings,
                "text",
            );
        }
    }
    if (
        [
            "VECTOR",
            "BOOLEAN_OPERATION",
            "ELLIPSE",
            "LINE",
            "POLYGON",
            "STAR",
            "CONNECTOR",
            "SHAPE_WITH_TEXT",
        ].includes(node.type)
    )
        return normalizeVisual(node, base, context, captureWarnings, "vector");
    if (node.type === "GROUP") {
        const container = node;
        const children: SnapshotNode[] = [];
        const warnings: Diagnostic[] = [...captureWarnings];
        const failedChild = await captureChildren(
            container.children ?? [],
            context,
            children,
            warnings,
        );
        if (failedChild) return failedChild;
        return {
            node: {
                ...base,
                kind: "group",
                containerKind: "group",
                layoutFallback: "freeform",
                children,
            },
            warnings,
        };
    }
    if (
        node.type === "FRAME" ||
        node.type === "COMPONENT" ||
        node.type === "INSTANCE" ||
        node.type === "COMPONENT_SET" ||
        node.type === "SECTION"
    ) {
        const frame = node;
        const appearance =
            node.type === "SECTION"
                ? sectionAppearance(node)
                : await captureAppearance(frame, mixedValue, imageResolver);
        if ("error" in appearance) return { error: appearance.error };
        const children: SnapshotNode[] = [];
        const warnings = [...captureWarnings, ...appearance.warnings];
        const failedChild = await captureChildren(
            frame.children ?? [],
            context,
            children,
            warnings,
            { parentLayout: autoLayout ?? undefined },
        );
        if (failedChild) return failedChild;
        const kind =
            node.type === "FRAME"
                ? "frame"
                : node.type === "COMPONENT"
                  ? "component"
                  : node.type === "INSTANCE"
                    ? "instance"
                    : node.type === "COMPONENT_SET"
                      ? "component-set"
                      : "section";
        return {
            node: {
                ...base,
                kind,
                containerKind: node.type === "SECTION" ? "section" : "native",
                layoutFallback,
                ...appearance.appearance,
                autoLayout,
                children:
                    "itemReverseZIndex" in node &&
                    node.itemReverseZIndex &&
                    autoLayout === null
                        ? children.reverse()
                        : children,
            } as SnapshotNode,
            warnings,
        };
    }
    if (node.type === "RECTANGLE") {
        const rectangle = node;
        const appearance = await captureAppearance(
            rectangle,
            mixedValue,
            imageResolver,
        );
        if ("error" in appearance) return { error: appearance.error };
        return {
            node: {
                ...base,
                kind: "rectangle",
                ...appearance.appearance,
            },
            warnings: [...captureWarnings, ...appearance.warnings],
        };
    }
    const structural = node;
    const runtimeType = node.type;
    const hasStructuralChildren =
        "children" in structural && Array.isArray(structural.children);
    if (
        runtimeType === "TABLE" ||
        runtimeType === "TABLE_CELL" ||
        (runtimeType === "SLICE" && isRoot) ||
        hasStructuralChildren
    ) {
        const appearance = await captureAppearance(
            node,
            mixedValue,
            imageResolver,
        );
        if ("error" in appearance) return { error: appearance.error };
        const children: SnapshotNode[] = [];
        const warnings = [
            ...captureWarnings,
            ...appearance.warnings,
            {
                ...problem(
                    "CONTAINER_TYPE_APPROXIMATED",
                    node,
                    "This structural node is rendered as a transparent generic container",
                    "type",
                ),
                severity: "warning" as const,
            },
        ];
        if (runtimeType === "TABLE_CELL") {
            const cellRecord = node;
            const textRecord =
                typeof cellRecord.text === "object" && cellRecord.text !== null
                    ? (cellRecord.text as Record<string, unknown>)
                    : undefined;
            const characters =
                typeof textRecord?.characters === "string"
                    ? textRecord.characters
                    : "";
            if (textRecord !== undefined && characters.length > 0) {
                children.push(
                    await captureTableCellText(
                        textRecord,
                        node.id,
                        node.name,
                        node.width,
                        node.height,
                        mixedValue,
                        imageResolver,
                        warnings,
                    ),
                );
            }
        }
        if (runtimeType === "TABLE" && !hasStructuralChildren) {
            const table = await captureTableCells(
                node,
                mixedValue,
                imageResolver,
                tableMeasurement,
            );
            if ("error" in table) return { error: table.error };
            children.push(...table.cells);
            warnings.push({
                ...problem(
                    "TABLE_LAYOUT_APPROXIMATED",
                    node,
                    "Table cells are rendered as positioned structural containers",
                    "cellAt",
                ),
                severity: "warning",
            });
            warnings.push(...table.warnings);
        }
        const failedChild = await captureChildren(
            structural.children ?? [],
            context,
            children,
            warnings,
        );
        if (failedChild) return failedChild;
        return {
            node: {
                ...base,
                kind: "container",
                containerKind: "generic",
                layoutFallback: "freeform",
                ...appearance.appearance,
                autoLayout: null,
                children,
            } as SnapshotNode,
            warnings,
        };
    }
    if (node.type === "TEXT")
        return normalizeText(node, base, context, captureWarnings);
    return {
        error: problem(
            "UNSUPPORTED_NODE",
            node,
            "Node type is not supported",
            "type",
        ),
    };
}

// Selection cardinality is handled by capture; replay always has one validated root.
async function normalizeRoot(
    root: MaterializedNode,
    input: NormalizationContext,
): Promise<CaptureResult> {
    const { assets, imageResolver } = input;
    const nodeIds: string[] = [];
    collectNodeIds(root, nodeIds);
    const imageCache = new Map<string, Promise<ImageData | undefined>>();
    const cachedImageResolver: ImageResolver = (hash) => {
        const cached = imageCache.get(hash);
        if (cached !== undefined) return cached;
        const pending = imageResolver(hash).then((image) => {
            if (!image) return undefined;
            let encoded: string | undefined;
            return {
                ...image,
                // The resolver cache belongs to this normalization only. Paint
                // placement/crop remains per node; only immutable bytes are shared.
                get base64() {
                    encoded ??= assets.encode(image.bytes);
                    return encoded;
                },
            };
        });
        imageCache.set(hash, pending);
        return pending;
    };
    const context = { ...input, imageResolver: cachedImageResolver };
    const result = await captureNode(root, context, { isRoot: true });
    if (result.error !== undefined || result.node === undefined) {
        return {
            ok: false,
            nodeIds,
            captureMetrics: context.metrics,
            diagnostics: result.error === undefined ? [] : [result.error],
        };
    }
    const degraded = new Set(
        (result.warnings ?? [])
            .filter(
                (w) =>
                    w.code === "NODE_PLACEHOLDER" ||
                    w.code === "MASK_COMPOSITION_OMITTED",
            )
            .map((w) => w.nodeId),
    );
    function hasContent(node: SnapshotNode): boolean {
        if (degraded.has(node.id))
            return "children" in node && node.children.some(hasContent);
        if ("children" in node)
            return (
                node.children.some(hasContent) ||
                ("fills" in node && node.fills.length > 0)
            );
        return true;
    }
    if (
        (result.warnings ?? []).some((w) =>
            [
                "NODE_OMITTED",
                "NODE_PLACEHOLDER",
                "MASK_COMPOSITION_OMITTED",
            ].includes(w.code),
        ) &&
        !hasContent(result.node)
    )
        return {
            ok: false,
            nodeIds,
            captureMetrics: context.metrics,
            diagnostics: [
                ...(result.warnings ?? []),
                problem(
                    "UNRENDERABLE_SELECTION",
                    root,
                    "No renderable content remains after compatibility recovery",
                    "type",
                ),
            ],
        };
    const snapshot: FigmaSnapshot = {
        schemaVersion: SNAPSHOT_SCHEMA_VERSION,
        selection: {
            nodeId: root.id,
            nodeName: root.name,
        },
        root: result.node,
    };
    const warnings = uniqueDiagnostics(result.warnings ?? []);
    return {
        ok: true,
        snapshot,
        nodeIds,
        warnings,
        captureMetrics: context.metrics,
    };
}

export function requiresVisualExport(
    properties: Record<string, unknown>,
    segments: unknown,
): boolean {
    const node = {
        ...properties,
        sourceSegments: segments,
    } as unknown as MaterializedNode;
    if (node.type !== "TEXT")
        return (
            [
                "VECTOR",
                "BOOLEAN_OPERATION",
                "ELLIPSE",
                "LINE",
                "POLYGON",
                "STAR",
                "CONNECTOR",
                "SHAPE_WITH_TEXT",
            ].includes(node.type) ||
            ![
                "GROUP",
                "FRAME",
                "COMPONENT",
                "INSTANCE",
                "RECTANGLE",
                "COMPONENT_SET",
                "SECTION",
                "TABLE",
                "TABLE_CELL",
                "SLICE",
                "PAGE",
            ].includes(node.type)
        );
    return (
        classifyNonInterText(node, SOURCE_MIXED) === true ||
        classifyIconText(node, SOURCE_MIXED) !== undefined
    );
}

function groupChildPosition(
    node: SourceNode,
    parent: SourceNode | undefined,
): { x: number; y: number } | undefined {
    if (parent?.type !== "GROUP") return undefined;
    const matrix = (value: unknown): value is number[][] =>
        Array.isArray(value) &&
        value.length === 2 &&
        value.every(
            (row) =>
                Array.isArray(row) &&
                row.length === 3 &&
                row.every(
                    (entry) =>
                        typeof entry === "number" && Number.isFinite(entry),
                ),
        );
    const group = parent.properties.absoluteTransform;
    const child = node.properties.absoluteTransform;
    if (!matrix(group) || !matrix(child)) return undefined;
    const [[a, c, tx], [b, d, ty]] = group;
    const determinant = a * d - b * c;
    if (!Number.isFinite(determinant) || determinant === 0) return undefined;
    // Figma group children can retain coordinates in the enclosing frame's
    // space. Slint creates a local space for the group, so derive the child
    // origin from the captured world transforms instead of adding it twice.
    // Use the original parent transform even when a captured root was moved.
    const dx = child[0][2] - tx;
    const dy = child[1][2] - ty;
    const x = (d * dx - c * dy) / determinant;
    const y = (a * dy - b * dx) / determinant;
    return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : undefined;
}

/** The caller owns this captured input and must keep it immutable. Internal
 * variants and output targets share asset encodings after one boundary check. */
export function createSourceNormalizer(source: SourceCapture) {
    validateSource(source);
    const assets = new NormalizationAssets();
    return {
        assets,
        normalize: (target: GenerationTarget = "preview") =>
            normalizeValidatedSource(source, target, assets),
    };
}
export type SourceNormalizer = ReturnType<typeof createSourceNormalizer>;
export async function normalizeSource(
    source: SourceCapture,
    target: GenerationTarget = "preview",
): Promise<CaptureResult> {
    return createSourceNormalizer(source).normalize(target);
}
async function normalizeValidatedSource(
    source: SourceCapture,
    target: GenerationTarget,
    assets: NormalizationAssets,
): Promise<CaptureResult> {
    const nodes = new Map<string, SourceNode>();
    const warnings: Diagnostic[] = [];
    function materialize(
        node: SourceNode,
        parentAutoLayout = false,
        parent?: SourceNode,
        flexDepth = 0,
        hiddenAncestor = false,
    ): MaterializedNode {
        nodes.set(node.id, node);
        const properties = decodeValue(node.properties);
        const hidden = hiddenAncestor || properties.visible === false;
        Object.assign(properties, groupChildPosition(node, parent));
        normalizeLayoutProperties(node, properties, warnings, parentAutoLayout);
        let autoLayout =
            properties.layoutMode === "HORIZONTAL" ||
            properties.layoutMode === "VERTICAL";
        // The current Slint interpreter repeatedly measures deeply nested flex
        // descendants. The real Select fixture has ten consecutive layouts and
        // stalls even with fixed sizes. Break the measurement chain locally;
        // preserve native layouts below the boundary and Figma's child geometry.
        if (target === "preview" && autoLayout && flexDepth >= 3 && !hidden) {
            warnings.push({
                severity: "warning",
                code: "LAYOUT_DEPTH_GEOMETRY_APPROXIMATED",
                nodeId: node.id,
                nodeName: node.name,
                propertyPath: "layoutMode",
                message:
                    "Deeply nested auto layout uses captured child positions to bound Slint layout evaluation; descendant layouts are retained",
            });
            properties.layoutMode = "NONE";
            autoLayout = false;
        }
        normalizeAppearanceProperties(node, properties, warnings);
        const materialized = {
            ...properties,
            id: node.id,
            name: node.name,
            type: node.type,
            sourceSvgOmitted: node.exports?.svgOmitted === "png",
            sourceRasterBounds: node.exports?.rasterBounds,
            sourceSvgBounds: node.exports?.svgBounds,
            ...(node.children === undefined
                ? {}
                : {
                      children: node.children.map((child) =>
                          materialize(
                              child,
                              autoLayout,
                              node,
                              autoLayout ? flexDepth + 1 : 0,
                              hidden,
                          ),
                      ),
                  }),
            ...(node.segments === undefined
                ? {}
                : {
                      sourceSegments: node.segments.error
                          ? { error: new Error(node.segments.error) }
                          : { segments: decodeValue(node.segments.value) },
                  }),
            ...(node.cells === undefined
                ? {}
                : { sourceCells: node.cells.map((cell) => decodeValue(cell)) }),
        } as unknown as MaterializedNode;
        const composition = maskComposition(node.children);
        if (composition?.kind === "raster")
            Object.assign(materialized, { sourceMaskRaster: true });
        else if (
            composition?.kind === "rectangle" &&
            materialized.children !== undefined
        ) {
            const mask = materialized.children.find(
                (child) => child.id === composition.maskId,
            );
            if (mask) {
                const clipped = {
                    ...mask,
                    type: "FRAME",
                    isMask: false,
                    layoutMode: "NONE",
                    clipsContent: true,
                    fills: [],
                    strokes: [],
                    effects: [],
                    layoutSizingHorizontal: "FIXED",
                    layoutSizingVertical: "FIXED",
                    children: materialized.children
                        .filter((child) => child.id !== mask.id)
                        .map((child) => ({
                            ...child,
                            x: child.x - mask.x,
                            y: child.y - mask.y,
                        })),
                };
                Object.assign(materialized, {
                    layoutMode: "NONE",
                    children: [clipped],
                });
            }
        }
        return materialized;
    }
    const root = materialize(source.root);
    const svg: NormalizationContext["exportSvgNode"] = async (node) => {
        const item = nodes.get(node.id)?.exports?.svg;
        if (item?.error) throw new CompatibilityError(item.error);
        return item?.value ?? "";
    };
    const png: NormalizationContext["exportPngNode"] = async (node) => {
        const item = nodes.get(node.id)?.exports?.png;
        if (item?.error) throw new CompatibilityError(item.error);
        return assets.bytes(item?.value ?? []);
    };
    const result = await normalizeRoot(root, {
        assets,
        target,
        metrics: zeroCaptureMetrics(),
        mixedValue: SOURCE_MIXED,
        exportSvgNode: svg,
        imageResolver: async (hash) => {
            const item = source.images[hash];
            if (item?.error) throw new CompatibilityError(item.error);
            return item?.value
                ? {
                      ...item.value,
                      bytes: assets.bytes(item.value.bytes),
                  }
                : undefined;
        },
        exportPngNode: source.pngEnabled ? png : undefined,
        exportScale: source.exportScale,
    });
    const paths = new Map<string, string>();
    function index(node: SourceNode, parent: string) {
        const path = parent ? `${parent} / ${node.name}` : node.name;
        paths.set(node.id, path);
        for (const child of node.children ?? []) index(child, path);
    }
    index(source.root, "");
    function enrich(items: readonly Diagnostic[]): Diagnostic[] {
        const enriched = items.map((item) => {
            const node = item.nodeId ? nodes.get(item.nodeId) : undefined;
            const original =
                item.propertyPath === "type"
                    ? node?.type
                    : item.propertyPath
                      ? node?.properties[item.propertyPath.split("[")[0]]
                      : undefined;
            const category: Diagnostic["category"] =
                /NODE_OMITTED|NODE_PLACEHOLDER|MASK_COMPOSITION_OMITTED/.test(
                    item.code,
                )
                    ? "omission"
                    : /IMAGE_SUBSTITUTED|SVG_FLATTENED|TEXT_AS_IMAGE|ICON_TEXT/.test(
                            item.code,
                        )
                      ? "image"
                      : /GEOMETRY|GRID_LAYOUT|CONTAINER_TYPE/.test(item.code)
                        ? "geometry"
                        : "approximation";
            return {
                ...item,
                nodePath: item.nodeId ? paths.get(item.nodeId) : undefined,
                originalValue: JSON.stringify(
                    original ?? { $source: "unavailable" },
                ),
                fallbackAction: item.message,
                category,
            };
        });
        return uniqueDiagnostics(enriched);
    }
    const components =
        result.ok && !result.empty && source.components
            ? await normalizeComponents(source, (capture) =>
                  normalizeValidatedSource(capture, target, assets),
              )
            : undefined;
    if (result.ok)
        return result.empty
            ? result
            : {
                  ...result,
                  snapshot: components
                      ? {
                            ...result.snapshot,
                            components: components.library,
                            ...(source.variables
                                ? { variables: source.variables }
                                : {}),
                        }
                      : result.snapshot,
                  warnings: [
                      ...enrich([...warnings, ...result.warnings]),
                      ...(components?.warnings ?? []),
                  ],
              };
    return {
        ...result,
        diagnostics: enrich([...warnings, ...result.diagnostics]),
    };
}
