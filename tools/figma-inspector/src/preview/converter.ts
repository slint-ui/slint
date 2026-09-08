// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { ComponentGenerationOptions } from "./component-behavior";
import {
    binding,
    openElement,
    closeElement,
    printLine,
    type SlintLine,
} from "./slint-ir";
import { generateComponents } from "./component-generator";
import { uniqueDiagnostics } from "../plugin/snapshot";
import type {
    Diagnostic,
    FigmaSnapshot,
    SnapshotAutoLayout,
    SnapshotFrameLikeNode,
    SnapshotNode,
} from "../plugin/snapshot";
import { validateSnapshot } from "../plugin/snapshot";
import {
    number,
    lengthProperties,
    type RenderContext,
    type NodePlacement,
} from "./converter-context";
import { textRunNeedsStyled, textSource } from "./converter-text";
import {
    cornerRadiusSource,
    appearanceSource,
    strokeLayersSource,
    fillLayersSource,
    shadowLayersSource,
    innerShadowSource,
    imageFillSource,
    visualSource,
} from "./converter-visual";

export type ConversionResult =
    | {
          readonly ok: true;
          readonly source: string;
          readonly warnings: readonly Diagnostic[];
          readonly width: number;
          readonly height: number;
      }
    | {
          readonly ok: false;
          readonly diagnostics: readonly Diagnostic[];
      };

const property = binding;

function geometry(
    node: SnapshotNode,
    depth: number,
    normalizePosition = false,
    omitPosition = false,
    omitSize = false,
): SlintLine[] {
    const lines: SlintLine[] = [];
    if (!omitPosition) {
        lines.push(
            property("x", `${number(normalizePosition ? 0 : node.x)}px`, depth),
            property("y", `${number(normalizePosition ? 0 : node.y)}px`, depth),
        );
    }
    if (!omitSize) {
        const omitTextWidth =
            node.kind === "text" && node.textAutoResize === "width-and-height";
        const omitTextHeight =
            node.kind === "text" &&
            (node.textAutoResize === "width-and-height" ||
                node.textAutoResize === "height");
        if (!omitTextWidth)
            lines.push(property("width", `${number(node.width)}px`, depth));
        if (!omitTextHeight)
            lines.push(property("height", `${number(node.height)}px`, depth));
    }
    // Figma bakes a raster leaf's opacity into its PNG alpha. Ancestor
    // containers still retain their opacity through this geometry path.
    if (
        node.opacity !== 1 &&
        !(node.kind === "svg" && node.raster !== undefined)
    )
        lines.push(property("opacity", node.opacity, depth));
    if (node.rotation !== 0)
        lines.push(
            property(
                "transform-rotation",
                `${number(node.rotation)}deg`,
                depth,
            ),
            property(
                "transform-origin",
                "{ x: self.width / 2, y: self.height / 2 }",
                depth,
            ),
        );
    return lines;
}

function sizingConstraints(
    node: SnapshotNode,
    layout: SnapshotAutoLayout,
    depth: number,
    intrinsic = false,
    preview = true,
): SlintLine[] {
    const lines: SlintLine[] = [];
    // Text and child layouts can impose intrinsic minima. An empty container
    // without a layout already has a zero minimum; do not emit an override.
    const hasIntrinsicMinimum =
        node.kind === "text" ||
        (node.kind !== "group" &&
            "children" in node &&
            (node.children.length > 0 ||
                ("autoLayout" in node && node.autoLayout !== null)));
    const emitAxis = (
        sizing: SnapshotNode["layoutSizingHorizontal"],
        axis: "horizontal" | "vertical",
    ): void => {
        if (sizing === null) return;
        const dimension = axis === "horizontal" ? "width" : "height";
        const stretch =
            axis === "horizontal" ? "horizontal-stretch" : "vertical-stretch";
        const resolved = `${number(axis === "horizontal" ? node.width : node.height)}px`;
        // The viewer displays captured geometry. Layout-assigned Image widths
        // recurse in Slint's conditional Flexbox measurement; use the captured
        // width for raster text and icons, including HUG/HUG images.
        if (preview && node.kind === "svg" && axis === "horizontal") {
            lines.push(property("width", resolved, depth));
            return;
        }
        if (sizing === "fixed") {
            lines.push(property(dimension, resolved, depth));
            return;
        }
        const mainAxis =
            (layout.direction === "horizontal" && axis === "horizontal") ||
            (layout.direction === "vertical" && axis === "vertical");
        const otherSizing =
            axis === "horizontal"
                ? node.layoutSizingVertical
                : node.layoutSizingHorizontal;
        // Work around Slint's image layout-info cycle inside conditional
        // Flexbox component branches. A fixed opposite axis gives this icon
        // captured bounds we can use without an intrinsic-size dependency.
        if (
            intrinsic &&
            node.kind === "svg" &&
            otherSizing === "fixed" &&
            (sizing === "hug" || (sizing === "fill" && !mainAxis))
        ) {
            lines.push(
                property(dimension, resolved, depth),
                property(stretch, 0, depth),
            );
            return;
        }
        if (sizing === "hug") {
            const intrinsicText = node.kind === "text";
            const intrinsicContainer =
                node.kind === "frame" ||
                node.kind === "component" ||
                node.kind === "instance" ||
                node.kind === "component-set" ||
                node.kind === "section" ||
                node.kind === "container";
            // A HUG container whose size is inferred from a FILL child creates
            // a Slint layout-info cycle: the child asks for the parent's size
            // while the parent asks the child for its intrinsic size. The
            // snapshot already contains Figma's resolved dimensions, so use
            // those dimensions for the static preview. Text and leaf visuals
            // retain their intrinsic sizing behavior.
            const fillCycle =
                "children" in node &&
                node.children.some(
                    (child) =>
                        (axis === "horizontal"
                            ? child.layoutSizingHorizontal
                            : child.layoutSizingVertical) === "fill",
                );
            if (
                !intrinsicText &&
                intrinsicContainer &&
                (!intrinsic ||
                    fillCycle ||
                    !("autoLayout" in node && node.autoLayout))
            ) {
                lines.push(property(dimension, resolved, depth));
                return;
            }
            if (
                (!intrinsicText && !intrinsicContainer) ||
                (intrinsicText && !preview && axis === "horizontal")
            )
                lines.push(property(`preferred-${dimension}`, resolved, depth));
            lines.push(property(stretch, 0, depth));
            return;
        }

        lines.push(property(`preferred-${dimension}`, resolved, depth));
        if (hasIntrinsicMinimum)
            lines.push(property(`min-${dimension}`, "0px", depth));
        if (mainAxis) {
            lines.push(property(stretch, 1, depth));
        } else {
            lines.push(
                property("cross-axis-self-alignment", "stretch", depth),
                property(stretch, 0, depth),
            );
        }
    };
    emitAxis(node.layoutSizingHorizontal, "horizontal");
    emitAxis(node.layoutSizingVertical, "vertical");
    return lines;
}

function isFullParent(node: SnapshotNode, parent: SnapshotNode): boolean {
    return (
        node.x === 0 &&
        node.y === 0 &&
        node.width === parent.width &&
        node.height === parent.height
    );
}

function flexLayoutSource(
    node: SnapshotFrameLikeNode,
    context: RenderContext,
    depth: number,
): SlintLine[] {
    const layout = node.autoLayout;
    if (layout === null) return [];
    // An empty Slint flex layout has a maximum size equal to its padding.
    // Figma's empty FILL frames still stretch (spacers and hidden half-bars).
    // Keep the frame's sizing constraints but do not impose that empty layout.
    if (
        !node.children.some(
            (child) => child.visible && child.layoutPositioning !== "absolute",
        )
    )
        return [];
    const lines: SlintLine[] = [openElement("FlexboxLayout", depth)];
    const mainFill = node.children.some(
        (child) =>
            child.visible &&
            (layout.direction === "horizontal"
                ? child.layoutSizingHorizontal === "fill"
                : child.layoutSizingVertical === "fill"),
    );
    if (layout.direction !== "horizontal")
        lines.push(property("flex-direction", "column", depth + 1));
    if (!layout.wrap) lines.push(property("flex-wrap", "no-wrap", depth + 1));
    const padding = [
        layout.paddingTop,
        layout.paddingRight,
        layout.paddingBottom,
        layout.paddingLeft,
    ];
    if (padding.every((value) => value === padding[0]) && padding[0] !== 0)
        lines.push(property("padding", `${number(padding[0])}px`, depth + 1));
    else
        lines.push(
            ...lengthProperties(
                [
                    ["padding-left", layout.paddingLeft],
                    ["padding-right", layout.paddingRight],
                    ["padding-top", layout.paddingTop],
                    ["padding-bottom", layout.paddingBottom],
                ],
                depth + 1,
            ),
        );
    if (layout.itemSpacing !== 0)
        lines.push(
            property("spacing", `${number(layout.itemSpacing)}px`, depth + 1),
        );
    if (
        layout.wrap &&
        layout.counterAxisSpacing !== 0 &&
        layout.counterAxisSpacing !== layout.itemSpacing
    )
        lines.push(
            property(
                layout.direction === "horizontal"
                    ? "spacing-vertical"
                    : "spacing-horizontal",
                `${number(layout.counterAxisSpacing)}px`,
                depth + 1,
            ),
        );
    if (mainFill || layout.primaryAlignment !== "start")
        lines.push(
            property(
                "alignment",
                mainFill ? "stretch" : layout.primaryAlignment,
                depth + 1,
            ),
        );
    lines.push(
        property("cross-axis-alignment", layout.counterAlignment, depth + 1),
    );
    if (layout.wrap && layout.counterAxisAlignContent !== "stretch")
        lines.push(
            property(
                "cross-axis-line-alignment",
                layout.counterAxisAlignContent,
                depth + 1,
            ),
        );
    const flowChildren = node.children.filter(
        (child) => child.layoutPositioning !== "absolute",
    );
    const paintChildren = layout.reversePaintOrder
        ? [...flowChildren].reverse()
        : flowChildren;
    for (const child of paintChildren) {
        for (const line of nodeSource(child, context, {
            depth: depth + 1,
            flexItem: true,
            parentLayout: layout,
            parent: node,
            layoutOrder: layout.reversePaintOrder
                ? flowChildren.indexOf(child)
                : undefined,
        }))
            lines.push(line);
    }
    lines.push(closeElement(depth));
    return lines;
}

function absoluteChildrenSource(
    node: SnapshotFrameLikeNode,
    context: RenderContext,
    depth: number,
): SlintLine[] {
    const visibleChildren = node.children.filter((child) => child.visible);
    const firstAbsolute = visibleChildren.findIndex(
        (child) => child.layoutPositioning === "absolute",
    );
    if (
        firstAbsolute >= 0 &&
        visibleChildren
            .slice(firstAbsolute + 1)
            .some((child) => child.layoutPositioning !== "absolute")
    )
        context.warnings.push({
            severity: "warning",
            code: "ABSOLUTE_Z_ORDER_APPROXIMATED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: "children",
            message:
                "Absolute children are rendered above flow children, so interleaved child z-order is approximated",
        });
    const lines: SlintLine[] = [];
    for (const child of node.children) {
        if (child.layoutPositioning !== "absolute") continue;
        for (const line of nodeSource(child, context, { depth, parent: node }))
            lines.push(line);
    }
    return lines;
}

function nodeSource(
    node: SnapshotNode,
    context: RenderContext,
    {
        depth = 1,
        normalizePosition = false,
        flexItem = false,
        parentLayout,
        parent,
        layoutChild = false,
        layoutOrder,
        definitionRoot = false,
    }: NodePlacement = {},
): SlintLine[] {
    if (!node.visible) return [];
    const use = context.componentUses?.get(node);
    if (use && !definitionRoot) {
        const full =
            normalizePosition ||
            (parent !== undefined && isFullParent(node, parent));
        return [
            openElement(use.name, depth, node),
            ...geometry(
                { ...node, opacity: 1, rotation: 0 },
                depth + 1,
                normalizePosition,
                flexItem || full || layoutChild,
                flexItem || full,
            ),
            ...(flexItem && parentLayout
                ? sizingConstraints(
                      node,
                      parentLayout,
                      depth + 1,
                      context.componentTemplates === true || use !== undefined,
                      context.preview,
                  )
                : []),
            ...(layoutOrder === undefined
                ? []
                : [property("layout-order", layoutOrder, depth + 1)]),
            ...(context.componentTemplates
                ? use.templateBindings
                : use.bindings
            ).map((binding) => ({ ...binding, depth: depth + 1 })),
            closeElement(depth),
        ];
    }
    const rootFrameLike =
        node.kind === "frame" ||
        node.kind === "component" ||
        node.kind === "instance" ||
        node.kind === "component-set" ||
        node.kind === "section" ||
        node.kind === "container";
    const collapseRoot =
        !definitionRoot &&
        normalizePosition &&
        ((node.kind === "group" && node.opacity === 1 && node.rotation === 0) ||
            (rootFrameLike &&
                node.autoLayout === null &&
                node.fills.length === 0 &&
                node.strokes.length === 0 &&
                node.shadows.length === 0 &&
                node.cornerRadii.every((radius) => radius === 0) &&
                !node.clipsContent &&
                node.opacity === 1 &&
                node.rotation === 0));
    if (collapseRoot) {
        const lines: SlintLine[] = [];
        for (const child of node.children)
            for (const line of nodeSource(child, context, {
                depth,
                parent: node,
            }))
                lines.push(line);
        return lines;
    }
    const styledText = node.kind === "text" && textRunNeedsStyled(node);
    const paintBounds =
        node.kind === "svg"
            ? (node.paintBounds ?? node.raster?.bounds)
            : undefined;
    const element =
        node.kind === "text"
            ? styledText
                ? "StyledText"
                : "Text"
            : node.kind === "svg"
              ? paintBounds
                  ? "Rectangle"
                  : "Image"
              : "Rectangle";
    const lines: SlintLine[] = [openElement(element, depth, node)];
    const emitsRectangle = element === "Rectangle";
    const fullParent =
        emitsRectangle &&
        (normalizePosition ||
            (parent !== undefined && isFullParent(node, parent)));
    lines.push(
        ...geometry(
            node,
            depth + 1,
            normalizePosition,
            flexItem || fullParent || layoutChild,
            flexItem || fullParent,
        ),
    );
    if (flexItem && parentLayout !== undefined)
        lines.push(
            ...sizingConstraints(
                node,
                parentLayout,
                depth + 1,
                context.componentTemplates === true || use !== undefined,
                context.preview,
            ),
        );
    if (layoutOrder !== undefined)
        lines.push(property("layout-order", layoutOrder, depth + 1));
    if (node.kind === "group") {
        for (const child of node.children)
            for (const line of nodeSource(child, context, {
                depth: depth + 1,
                parent: node,
            }))
                lines.push(line);
    } else if (
        node.kind === "frame" ||
        node.kind === "component" ||
        node.kind === "instance" ||
        node.kind === "component-set" ||
        node.kind === "section" ||
        node.kind === "container" ||
        node.kind === "rectangle"
    ) {
        const externalStroke =
            node.strokes[0]?.align === "center" ||
            node.strokes[0]?.align === "outside";
        const container =
            node.kind === "frame" ||
            node.kind === "component" ||
            node.kind === "instance" ||
            node.kind === "component-set" ||
            node.kind === "section" ||
            node.kind === "container";
        const clipExternalContent =
            externalStroke && node.clipsContent && container;
        const spreadShadows = node.shadows.filter(
            (shadow) => shadow.kind !== "inner" && shadow.spread !== 0,
        );
        if (spreadShadows.length > 0)
            context.warnings.push({
                severity: "warning",
                code: "FEMTOVG_SHADOW_SPREAD_UNSUPPORTED",
                nodeId: node.id,
                nodeName: node.name,
                propertyPath: "effects",
                originalValue: spreadShadows
                    .map((shadow) => `${number(shadow.spread)}px`)
                    .join(", "),
                fallbackAction:
                    "The generated Slint preserves the spread for renderers that support it",
                category: "approximation",
                message:
                    "Shadow spread is preserved in the generated Slint, but this FemtoVG preview currently ignores it; render with Skia for Figma-equivalent spread",
            });
        lines.push(...appearanceSource(node, depth + 1, externalStroke));
        // Shadow helpers need an opaque shape to cast from. Emit them first so
        // they cannot hide layered fills or asymmetric border strips.
        lines.push(...shadowLayersSource(node, context, depth + 1));
        lines.push(...fillLayersSource(node, depth + 1));
        lines.push(...imageFillSource(node, depth + 1));
        lines.push(...innerShadowSource(node, depth + 1));
        if (!externalStroke) lines.push(...strokeLayersSource(node, depth + 1));
        const contentDepth = depth + (clipExternalContent ? 2 : 1);
        if (clipExternalContent) {
            lines.push(
                openElement("Rectangle", depth + 1),
                property("clip", true, depth + 2),
                ...cornerRadiusSource(node.cornerRadii, depth + 2),
            );
        }
        if (container && node.autoLayout !== null) {
            // Descendant output can exceed the engine's call argument limit.
            for (const line of flexLayoutSource(node, context, contentDepth))
                lines.push(line);
            for (const line of absoluteChildrenSource(
                node,
                context,
                contentDepth,
            ))
                lines.push(line);
        } else if (container) {
            for (const child of node.children)
                for (const line of nodeSource(child, context, {
                    depth: contentDepth,
                    parent: node,
                }))
                    lines.push(line);
        }
        if (clipExternalContent) lines.push(closeElement(depth + 1));
        if (externalStroke) lines.push(...strokeLayersSource(node, depth + 1));
    } else if (node.kind === "svg") {
        const visual = visualSource(node, depth, { flexItem, parent });
        if (visual === undefined) return [];
        lines.push(...visual);
    } else if (node.kind === "text") {
        lines.push(...textSource(node, styledText, context, depth + 1));
    }
    lines.push(closeElement(depth));
    return lines;
}

export function convertSnapshot(
    value: FigmaSnapshot | unknown,
    options: ComponentGenerationOptions & {
        target?: "preview" | "export";
        scope?: "tree" | "root-only";
    } = {},
): ConversionResult {
    // Project before validation so out-of-scope descendants and component
    // libraries cannot invalidate an otherwise supported selected root.
    let input = value;
    if (
        options.scope === "root-only" &&
        value &&
        typeof value === "object" &&
        "root" in value
    ) {
        const root = value.root;
        if (root && typeof root === "object") {
            const {
                components: _components,
                variables: _variables,
                ...snapshot
            } = value as FigmaSnapshot;
            input = {
                ...snapshot,
                root: {
                    ...root,
                    ...("children" in root ? { children: [] } : {}),
                },
            };
        }
    }
    const validation = validateSnapshot(input);
    if (!validation.ok) {
        return { ok: false, diagnostics: validation.diagnostics };
    }
    const snapshot = validation.snapshot;
    const warnings: Diagnostic[] = [];
    if (options.scope === "root-only") {
        const source = nodeSource(
            snapshot.root,
            { warnings, preview: false, componentTemplates: false },
            { depth: 0 },
        )
            .map(printLine)
            .concat("")
            .join("\n");
        return {
            ok: true,
            source,
            warnings: uniqueDiagnostics(warnings),
            width: snapshot.root.width,
            height: snapshot.root.height,
        };
    }
    const preview = options.target !== "export";
    let components: ReturnType<typeof generateComponents>;
    try {
        components = generateComponents(
            snapshot.root,
            snapshot.components,
            (node, uses) =>
                nodeSource(
                    node,
                    {
                        warnings,
                        preview,
                        componentUses: uses,
                        componentTemplates: true,
                    },
                    { depth: 1, normalizePosition: true, definitionRoot: true },
                ),
            snapshot.variables,
            options,
        );
    } catch (error) {
        return {
            ok: false,
            diagnostics: [
                {
                    severity: "error",
                    code: "COMPONENT_GENERATION_ERROR",
                    message:
                        error instanceof Error ? error.message : String(error),
                },
            ],
        };
    }
    warnings.push(...components.warnings);
    const context: RenderContext = {
        warnings,
        preview,
        componentUses: components.uses,
        componentTemplates: false,
    };
    const rootText = snapshot.root.kind === "text" ? snapshot.root : undefined;
    const rootStroke =
        "strokes" in snapshot.root ? snapshot.root.strokes[0] : undefined;
    const rootExpansion =
        rootStroke?.align === "center"
            ? 0.5
            : rootStroke?.align === "outside"
              ? 1
              : 0;
    const rootOutsets = rootStroke
        ? {
              top: rootStroke.strokeTopWeight * rootExpansion,
              right: rootStroke.strokeRightWeight * rootExpansion,
              bottom: rootStroke.strokeBottomWeight * rootExpansion,
              left: rootStroke.strokeLeftWeight * rootExpansion,
          }
        : { top: 0, right: 0, bottom: 0, left: 0 };
    const rootPaint =
        snapshot.root.kind === "svg"
            ? (snapshot.root.paintBounds ?? snapshot.root.raster?.bounds)
            : undefined;
    if (rootPaint) {
        rootOutsets.left = Math.max(0, -rootPaint.x);
        rootOutsets.top = Math.max(0, -rootPaint.y);
        rootOutsets.right = Math.max(
            0,
            rootPaint.x + rootPaint.width - snapshot.root.width,
        );
        rootOutsets.bottom = Math.max(
            0,
            rootPaint.y + rootPaint.height - snapshot.root.height,
        );
    }
    const outputWidth =
        snapshot.root.width + rootOutsets.left + rootOutsets.right;
    const outputHeight =
        snapshot.root.height + rootOutsets.top + rootOutsets.bottom;
    const rootUsesIntrinsicWidth =
        rootText?.textAutoResize === "width-and-height";
    const rootUsesIntrinsicHeight =
        rootText?.textAutoResize === "width-and-height" ||
        rootText?.textAutoResize === "height";
    const sourceLines: (SlintLine | string)[] = [
        ...components.source,
        "export component Demo inherits Window {",
        ...(rootUsesIntrinsicWidth
            ? []
            : [property("width", `${number(outputWidth)}px`, 1)]),
        ...(rootUsesIntrinsicHeight
            ? []
            : [property("height", `${number(outputHeight)}px`, 1)]),
        "    background: transparent;",
    ];
    if (rootText === undefined) {
        if (rootExpansion > 0 || rootPaint) {
            sourceLines.push(
                "    Rectangle {",
                property("x", `${number(rootOutsets.left)}px`, 2),
                property("y", `${number(rootOutsets.top)}px`, 2),
                property("width", `${number(snapshot.root.width)}px`, 2),
                property("height", `${number(snapshot.root.height)}px`, 2),
            );
            for (const line of nodeSource(snapshot.root, context, {
                depth: 2,
                normalizePosition: true,
            }))
                sourceLines.push(line);
            sourceLines.push("    }");
        } else
            for (const line of nodeSource(snapshot.root, context, {
                depth: 1,
                normalizePosition: true,
            }))
                sourceLines.push(line);
    } else {
        sourceLines.push("    FlexboxLayout {");
        for (const line of nodeSource(snapshot.root, context, {
            depth: 2,
            layoutChild: true,
        }))
            sourceLines.push(line);
        sourceLines.push("    }");
    }
    sourceLines.push("}", "");
    const source = sourceLines.map(printLine).join("\n");
    const uniqueWarnings = uniqueDiagnostics(warnings, (warning) =>
        JSON.stringify([warning.code, warning.nodeId, warning.propertyPath]),
    );
    return {
        ok: true,
        source,
        warnings: uniqueWarnings,
        width: outputWidth,
        height: outputHeight,
    };
}

export function convertSnapshotJson(json: string): ConversionResult {
    try {
        return convertSnapshot(JSON.parse(json) as unknown);
    } catch (error) {
        return {
            ok: false,
            diagnostics: [
                {
                    severity: "error",
                    code: "INVALID_SNAPSHOT_JSON",
                    message: `Snapshot JSON could not be parsed: ${error instanceof Error ? error.message : String(error)}`,
                },
            ],
        };
    }
}
