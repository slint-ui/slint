// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { VariableLibrary, VariableValue } from "./variable-library";
import type { ComponentLibrary } from "./components";
import { planReachableVariants } from "./components";
import type { CaptureCache } from "./capture-work";
import {
    captureBusyTime,
    captureScheduler,
    mapCaptureChildren,
} from "./capture-work";
import { maskComposition } from "./mask-composition";
import {
    type CaptureInstrumentation,
    type ImageResolver,
    normalizeSource,
    type PngExporter,
    requiresVisualExport,
    type SvgExporter,
} from "./normalize";
import { isPngByteArray, pngDimensions } from "../images";
import type { CaptureResult, CaptureWork } from "./snapshot";
import type { VisualBounds } from "./source";
import {
    decodeValue,
    encodeValue,
    type SourceBytes,
    type SourceCapture,
    type SourceNode,
    type SourceResult,
    sourceImageHashes,
} from "./source";

export type {
    CaptureInstrumentation,
    ImageResolver,
    PngExporter,
    SvgExporter,
} from "./normalize";

// Running host exports cannot be aborted. Keep their slots occupied across
// interactive revisions so superseded captures cannot multiply host work.
const selectionExportScheduler = captureScheduler(4);

/** Watch off-page component definitions without loading or observing every page. */
export function observeComponentDependencies(
    definitionIds: readonly string[],
    nodeIds: ReadonlySet<string>,
    changed: () => void,
): () => void {
    let disposed = false;
    const pages = new Map<PageNode, (event: NodeChangeEvent) => void>();
    for (const id of definitionIds) {
        void figma
            .getNodeByIdAsync(id)
            .then((node) => {
                if (disposed) return;
                let ancestor: BaseNode | null = node;
                while (ancestor && ancestor.type !== "PAGE")
                    ancestor = ancestor.parent;
                if (
                    !ancestor ||
                    ancestor.type !== "PAGE" ||
                    ancestor === figma.currentPage ||
                    pages.has(ancestor)
                )
                    return;
                const page = ancestor;
                const listener = (event: NodeChangeEvent) => {
                    if (
                        event.nodeChanges.some((change) => {
                            if (nodeIds.has(change.id)) return true;
                            if (change.type !== "CREATE" || change.node.removed)
                                return false;
                            let parent: BaseNode | null = change.node.parent;
                            while (parent) {
                                if (nodeIds.has(parent.id)) return true;
                                parent = parent.parent;
                            }
                            return false;
                        })
                    )
                        changed();
                };
                pages.set(page, listener);
                page.on("nodechange", listener);
            })
            .catch((error) => {
                if (!disposed)
                    console.warn(
                        "[slint-preview] component dependency observation failed",
                        String(error),
                    );
            });
    }
    return () => {
        disposed = true;
        for (const [page, listener] of pages)
            if (!page.removed) page.off("nodechange", listener);
        pages.clear();
    };
}

const svgExportSettings = {
    format: "SVG_STRING",
    contentsOnly: true,
    useAbsoluteBounds: false,
    svgOutlineText: true,
    svgIdAttribute: false,
    svgSimplifyStroke: false,
    colorProfile: "SRGB",
} as const;

const exportSvg: SvgExporter = (node) => node.exportAsync(svgExportSettings);

const pngExportSettings = {
    format: "PNG",
    // Export the complete node box so transparent padding remains part of the
    // raster. The converter maps these pixels back to the captured logical
    // bounds explicitly.
    contentsOnly: true,
    useAbsoluteBounds: true,
    constraint: { type: "SCALE", value: 1 },
    colorProfile: "SRGB",
} as const;

// Geometric bounds clip centered/outside strokes. For axis-aligned nodes,
// preserve the full painted rectangle and its offset from the layout box.
function overflowingPaintBounds(node: SceneNode): VisualBounds | undefined {
    if (
        !("absoluteTransform" in node) ||
        !("absoluteBoundingBox" in node) ||
        !("absoluteRenderBounds" in node)
    )
        return undefined;
    const transform = node.absoluteTransform;
    if (
        !transform ||
        Math.abs(transform[0][0] - 1) > 0.00001 ||
        Math.abs(transform[1][1] - 1) > 0.00001 ||
        Math.abs(transform[0][1]) > 0.00001 ||
        Math.abs(transform[1][0]) > 0.00001
    )
        return undefined;
    const box = node.absoluteBoundingBox;
    const paint = node.absoluteRenderBounds;
    if (!box || !paint || paint.width <= 0 || paint.height <= 0)
        return undefined;
    const x = paint.x - box.x;
    const y = paint.y - box.y;
    if (
        x >= -0.001 &&
        y >= -0.001 &&
        x + paint.width <= box.width + 0.001 &&
        y + paint.height <= box.height + 0.001
    )
        return undefined;
    return { x, y, width: paint.width, height: paint.height };
}

const exportPng: PngExporter = (node, exportScale) =>
    node.exportAsync({
        ...pngExportSettings,
        useAbsoluteBounds: overflowingPaintBounds(node) === undefined,
        constraint: { type: "SCALE", value: exportScale },
    });

const resolveImage: ImageResolver = async (hash) => {
    const image = figma.getImageByHash(hash);
    if (image === null) return undefined;
    const [bytes, size] = await Promise.allSettled([
        image.getBytesAsync(),
        image.getSizeAsync(),
    ]);
    if (bytes.status === "rejected") throw bytes.reason;
    return size.status === "fulfilled"
        ? {
              bytes: bytes.value,
              width: size.value.width,
              height: size.value.height,
          }
        : {
              bytes: bytes.value,
              sizeError: String(
                  size.reason instanceof Error
                      ? size.reason.message
                      : size.reason,
              ),
          };
};

const styledTextSegmentFields = [
    "fontName",
    "fontSize",
    "fontWeight",
    "fontStyle",
    "textDecoration",
    "textDecorationStyle",
    "textDecorationOffset",
    "textDecorationThickness",
    "textDecorationColor",
    "textDecorationSkipInk",
    "textCase",
    "lineHeight",
    "letterSpacing",
    "fills",
    "textStyleId",
    "fillStyleId",
    "listOptions",
    "listSpacing",
    "indentation",
    "paragraphIndent",
    "paragraphSpacing",
    "hyperlink",
    "boundVariables",
    "textStyleOverrides",
    "openTypeFeatures",
] as const;

const sourceProperties = [
    "blendMode",
    "bottomLeftRadius",
    "bottomRightRadius",
    "characters",
    "boundVariables",
    "resolvedVariableModes",
    "componentPropertyReferences",
    "componentProperties",
    "clipsContent",
    "columnIndex",
    "cornerRadius",
    "counterAxisAlignContent",
    "counterAxisAlignItems",
    "counterAxisSpacing",
    "dashPattern",
    "effects",
    "fills",
    "fontName",
    "fontSize",
    "fontWeight",
    "height",
    "isMask",
    "maskType",
    "itemReverseZIndex",
    "itemSpacing",
    "layoutMode",
    "layoutPositioning",
    "layoutSizingHorizontal",
    "layoutSizingVertical",
    "layoutWrap",
    "letterSpacing",
    "lineHeight",
    "maxLines",
    "numColumns",
    "numRows",
    "opacity",
    "paddingBottom",
    "paddingLeft",
    "paddingRight",
    "paddingTop",
    "primaryAxisAlignItems",
    "rotation",
    "rowIndex",
    "scaleFactor",
    "strokes",
    "strokesIncludedInLayout",
    "text",
    "textAlignHorizontal",
    "textAlignVertical",
    "textAutoResize",
    "textCase",
    "textTruncation",
    "topLeftRadius",
    "topRightRadius",
    "visible",
    "width",
    "x",
    "y",
    "strokeWeight",
    "strokeTopWeight",
    "strokeBottomWeight",
    "strokeLeftWeight",
    "strokeRightWeight",
    "strokeAlign",
    "layoutAlign",
    "relativeTransform",
    "absoluteTransform",
    "minWidth",
    "maxWidth",
    "minHeight",
    "maxHeight",
] as const;

export function collectSelectionNodeIds(
    selection: readonly SceneNode[],
): string[] {
    const ids: string[] = [];
    function visit(node: SceneNode) {
        ids.push(node.id);
        if ("children" in node) for (const child of node.children) visit(child);
    }
    if (selection.length === 1) visit(selection[0]);
    return ids;
}

// A rasterized mask consumes its whole sibling composition. Retain enough
// captured metadata to identify that boundary without reading/exporting its contents.
function captureMaskSummary<Bytes extends SourceBytes>(
    children: readonly SceneNode[],
    mixed: unknown,
): SourceNode<Bytes>[] | undefined {
    try {
        if (!children.some((child) => "isMask" in child && child.isMask))
            return undefined;
        const summary = children.map((child) => {
            const properties: SourceNode["properties"] = {
                visible: child.visible,
                isMask: "isMask" in child && child.isMask,
            };
            {
                const raw = child as unknown as Record<string, unknown>;
                for (const key of sourceProperties)
                    if (key in raw)
                        properties[key] = encodeValue(raw[key], mixed);
            }
            return {
                id: child.id,
                name: child.name,
                type: child.type,
                properties,
                errors: [],
            };
        });
        return maskComposition(summary)?.kind === "raster"
            ? summary
            : undefined;
    } catch {
        return undefined;
    }
}

type CaptureBytes<Binary extends boolean> = Binary extends true
    ? Uint8Array
    : number[];

export async function captureSource<Binary extends boolean = false>(
    root: SceneNode,
    mixed: unknown,
    exportSvgNode: SvgExporter = exportSvg,
    imageResolver: ImageResolver = resolveImage,
    exportPngNode?: PngExporter,
    exportScale = 1,
    includeHidden = false,
    instrumentation?: CaptureInstrumentation,
    cancelled?: () => boolean,
    concurrency = 4,
    binary = false as Binary,
    cache?: CaptureCache,
    schedule = captureScheduler(concurrency),
    planMaskExports = true,
    scope: "tree" | "root-only" = "tree",
): Promise<{
    source: SourceCapture<CaptureBytes<Binary>>;
    durationMs: number;
    fontMetrics: { requests: number; exports: number; cacheHits: number };
    work: CaptureWork;
}> {
    type Bytes = CaptureBytes<Binary>;
    const retainBytes = (bytes: Uint8Array): Bytes =>
        (binary ? bytes : Array.from(bytes)) as Bytes;
    const fontMetrics = { requests: 0, exports: 0, cacheHits: 0 };
    const work: { -readonly [Key in keyof CaptureWork]: CaptureWork[Key] } = {
        capturedNodes: 0,
        componentFamilies: 0,
        componentVariants: 0,
        pngExports: 0,
        svgExports: 0,
        textCacheHits: 0,
    };
    const fontTiming = captureBusyTime();
    const rawPng =
        exportPngNode ?? (exportSvgNode === exportSvg ? exportPng : undefined);
    const png: PngExporter | undefined =
        rawPng &&
        ((node, scale) =>
            schedule(async () => {
                if (cancelled?.()) throw Error("Capture cancelled");
                const stop =
                    node.type === "TEXT" ? fontTiming.start() : undefined;
                try {
                    work.pngExports++;
                    return await rawPng(node, scale);
                } finally {
                    stop?.();
                }
            }));
    const scheduledSvg: SvgExporter = (node) =>
        schedule(async () => {
            if (cancelled?.()) throw Error("Capture cancelled");
            const stop = node.type === "TEXT" ? fontTiming.start() : undefined;
            try {
                work.svgExports++;
                return await exportSvgNode(node);
            } finally {
                stop?.();
            }
        });
    const source: SourceCapture<Bytes> = {
        sourceVersion: 1,
        root: undefined as unknown as SourceNode<Bytes>,
        images: {},
        exportScale,
        pngEnabled: png !== undefined,
    };
    const images = new Map<string, Promise<void>>();
    async function readImages(value: unknown): Promise<void> {
        if (cancelled?.()) return;
        for (const hash of sourceImageHashes(value)) {
            if (cancelled?.()) break;
            if (!images.has(hash))
                images.set(
                    hash,
                    (async () => {
                        try {
                            const image = cache
                                ? await cache.get(
                                      `image:${hash}`,
                                      () => imageResolver(hash),
                                      (value) => value?.bytes.byteLength ?? 0,
                                  )
                                : await imageResolver(hash);
                            source.images[hash] = image
                                ? {
                                      value: {
                                          ...image,
                                          bytes: retainBytes(image.bytes),
                                      },
                                  }
                                : {};
                        } catch (error) {
                            source.images[hash] = {
                                error: String(
                                    error instanceof Error
                                        ? error.message
                                        : error,
                                ),
                            };
                        }
                    })(),
                );
            await images.get(hash);
        }
    }
    const componentOwners = new Map<string, ComponentNode | ComponentSetNode>();
    const capturedNodes = new Map<string, SourceNode<Bytes>>();
    const references: ComponentLibrary<SourceNode<Bytes>>["references"] = {};
    const requiredVariants = new Map<string, Set<string>>();
    const completeOwners = new Set<string>();
    const requireVariant = (
        owner: ComponentNode | ComponentSetNode,
        variantId: string,
    ) => {
        componentOwners.set(owner.id, owner);
        let required = requiredVariants.get(owner.id);
        if (!required) {
            required = new Set();
            requiredVariants.set(owner.id, required);
        }
        required.add(variantId);
    };
    const requireCompleteOwner = (owner: ComponentNode | ComponentSetNode) => {
        componentOwners.set(owner.id, owner);
        completeOwners.add(owner.id);
        const required = new Set<string>();
        for (const child of owner.type === "COMPONENT_SET"
            ? owner.children
            : [owner])
            if (child.type === "COMPONENT") required.add(child.id);
        requiredVariants.set(owner.id, required);
    };
    const mainComponents = new Map<string, Promise<ComponentNode | null>>();
    async function readComponent(node: SceneNode): Promise<void> {
        let main: ComponentNode | null = null;
        if (
            node.type === "COMPONENT_SET" &&
            "componentPropertyDefinitions" in node
        ) {
            if (node === root) requireCompleteOwner(node);
            else componentOwners.set(node.id, node);
            return;
        }
        if (node.type === "COMPONENT" && "componentPropertyDefinitions" in node)
            main = node;
        else if (
            node.type === "INSTANCE" &&
            "getMainComponentAsync" in node &&
            typeof node.getMainComponentAsync === "function"
        ) {
            let pending = mainComponents.get(node.id);
            if (!pending) {
                pending = node.getMainComponentAsync();
                mainComponents.set(node.id, pending);
            }
            main = await pending;
        }
        if (!main) return;
        const owner =
            main.parent?.type === "COMPONENT_SET" ? main.parent : main;
        const hasInstanceSwapContract = Object.values(
            owner.componentPropertyDefinitions ?? {},
        ).some((property) => property.type === "INSTANCE_SWAP");
        if (node === root && node.type === "COMPONENT")
            requireCompleteOwner(owner);
        else if (hasInstanceSwapContract) requireCompleteOwner(owner);
        else requireVariant(owner, main.id);
        references[node.id] = {
            definitionId: owner.id,
            variantId: main.id,
            ...(node.type === "INSTANCE" && "componentProperties" in node
                ? {
                      properties: Object.fromEntries(
                          Object.entries(node.componentProperties).map(
                              ([key, property]) => [key, property.value],
                          ),
                      ),
                  }
                : {}),
        };
    }
    async function visit(
        node: SceneNode,
        hidden: boolean,
        suppressed = false,
        ancestors = "",
    ): Promise<SourceNode<Bytes>> {
        const raw = node as unknown as Record<string, unknown>;
        const result: SourceNode<Bytes> = {
            id: node.id,
            name: node.name,
            type: node.type,
            properties: {},
            errors: [],
        };
        capturedNodes.set(node.id, result);
        if (
            scope === "tree" &&
            ["COMPONENT", "COMPONENT_SET", "INSTANCE"].includes(node.type)
        )
            await readComponent(node);
        const invisible =
            hidden || ("visible" in node && node.visible === false);
        const captureMaskRaster = async () => {
            let raster: SourceResult<Bytes>;
            try {
                if (!png) throw Error("PNG export is disabled");
                raster = {
                    value: retainBytes(
                        await png(
                            node as Parameters<PngExporter>[0],
                            exportScale,
                        ),
                    ),
                };
            } catch (error) {
                raster = {
                    error: String(
                        error instanceof Error ? error.message : error,
                    ),
                };
            }
            result.exports = { svg: {}, png: raster };
            if (raster.value) {
                const bounds = overflowingPaintBounds(node);
                if (bounds) result.exports.rasterBounds = bounds;
            }
        };
        let maskExport: Promise<void> | undefined;
        if (cancelled?.()) return result;
        for (const key of sourceProperties) {
            try {
                if (key in raw)
                    result.properties[key] = encodeValue(raw[key], mixed);
            } catch (error) {
                result.properties[key] = { $source: "unavailable" };
                result.errors.push({
                    property: key,
                    message: String(
                        error instanceof Error ? error.message : error,
                    ),
                });
            }
        }
        if (node.type === "TEXT" && "getStyledTextSegments" in node) {
            try {
                result.segments = {
                    value: encodeValue(
                        node.getStyledTextSegments(
                            styledTextSegmentFields as never,
                        ),
                        mixed,
                    ),
                };
            } catch (error) {
                result.segments = {
                    error: String(
                        error instanceof Error ? error.message : error,
                    ),
                };
            }
        }
        if (scope === "tree" && node.type === "TABLE" && "cellAt" in node) {
            result.cells = [];
            for (let row = 0; row < node.numRows; row++)
                for (let column = 0; column < node.numColumns; column++) {
                    try {
                        const cell = node.cellAt(row, column);
                        const data: Record<string, unknown> = {};
                        for (const key of sourceProperties)
                            if (key in cell)
                                data[key] = (
                                    cell as unknown as Record<string, unknown>
                                )[key];
                        result.cells.push(encodeValue(data, mixed));
                    } catch (error) {
                        result.cells.push(null);
                        result.errors.push({
                            property: `cellAt(${row},${column})`,
                            message: String(error),
                        });
                    }
                }
        }
        if (
            (!invisible || (node === root && !includeHidden)) &&
            !suppressed &&
            !cancelled?.()
        ) {
            await readImages(result.properties);
            if (result.segments?.value) await readImages(result.segments.value);
            if (cancelled?.()) return result;
            // Root snippets use native text. Container exports would bake in
            // descendants; boolean operands instead define the selected shape.
            if (
                (scope === "tree" ||
                    (node.type !== "TEXT" &&
                        (!("children" in node) ||
                            node.type === "BOOLEAN_OPERATION"))) &&
                requiresVisualExport(
                    { ...decodeValue(result.properties), type: result.type },
                    result.segments?.error
                        ? { error: result.segments.error }
                        : result.segments
                          ? { segments: decodeValue(result.segments.value) }
                          : undefined,
                )
            ) {
                async function attempt<T>(
                    operation: () => Promise<T>,
                ): Promise<SourceResult<T>> {
                    try {
                        if (cancelled?.()) throw new Error("Capture cancelled");
                        return { value: await operation() };
                    } catch (error) {
                        return {
                            error: String(
                                error instanceof Error ? error.message : error,
                            ),
                        };
                    }
                }
                const textKey =
                    cache && node.type === "TEXT"
                        ? (() => {
                              try {
                                  return JSON.stringify([
                                      ancestors,
                                      result.id,
                                      result.properties,
                                      result.segments,
                                      exportScale,
                                      overflowingPaintBounds(node),
                                      "resolvedVariableModes" in node
                                          ? node.resolvedVariableModes
                                          : null,
                                  ]);
                              } catch {
                                  return undefined;
                              }
                          })()
                        : undefined;
                if (node.type === "TEXT") {
                    fontMetrics.requests++;
                    fontMetrics.exports++;
                }
                const operation = async () => {
                    const readSvg = () =>
                        attempt(() =>
                            scheduledSvg(node as Parameters<SvgExporter>[0]),
                        );
                    const readPng = () =>
                        png
                            ? attempt(async () => {
                                  const load = async () => {
                                      const value = await png(
                                          node as Parameters<PngExporter>[0],
                                          exportScale,
                                      );
                                      if (
                                          !isPngByteArray(value) ||
                                          pngDimensions(value) === undefined
                                      )
                                          throw Error(
                                              "PNG export did not return valid PNG bytes",
                                          );
                                      return value;
                                  };
                                  const bytes =
                                      cache && textKey
                                          ? await cache.get(
                                                `text:${textKey}`,
                                                load,
                                                (value) => value.byteLength,
                                                () => {
                                                    fontMetrics.cacheHits++;
                                                    work.textCacheHits++;
                                                    fontMetrics.exports--;
                                                },
                                            )
                                          : await load();
                                  if (
                                      !isPngByteArray(bytes) ||
                                      pngDimensions(bytes) === undefined
                                  )
                                      throw new Error(
                                          "PNG export did not return valid PNG bytes",
                                      );
                                  return retainBytes(bytes);
                              })
                            : undefined;
                    if (png === undefined) {
                        const [svg, raster] = await Promise.all([
                            readSvg(),
                            readPng(),
                        ]);
                        result.exports = {
                            svg,
                            ...(raster ? { png: raster } : {}),
                        };
                        return;
                    }
                    const raster = await readPng();
                    result.exports = {
                        svg: {},
                        svgOmitted: "png",
                        png: raster ?? { error: "PNG exporter is unavailable" },
                    };
                };
                if (
                    instrumentation &&
                    (node.type === "CONNECTOR" ||
                        node.type === "SHAPE_WITH_TEXT")
                )
                    await instrumentation.measureFontToImageConversion(
                        operation,
                    );
                else await operation();
                if (result.exports?.png?.value) {
                    const bounds = overflowingPaintBounds(node);
                    if (bounds) result.exports.rasterBounds = bounds;
                }
            }
        }
        if (cancelled?.()) return result;
        if ("children" in node) result.children = [];
        if (scope === "tree" && "children" in node) {
            let children: readonly SceneNode[] = [];
            try {
                children = node.children;
            } catch (error) {
                result.errors.push({
                    property: "children",
                    message: String(error),
                });
            }
            const visibleChildren = children.filter((child) => {
                if (
                    includeHidden ||
                    !("visible" in child) ||
                    child.visible !== false
                )
                    return true;
                // A hidden child can still be part of the public boolean
                // contract. Keep it so native export can evaluate the
                // binding even though the preview omits its pixels.
                const refs =
                    "componentPropertyReferences" in child
                        ? child.componentPropertyReferences
                        : undefined;
                return (
                    !!refs &&
                    typeof refs === "object" &&
                    !Array.isArray(refs) &&
                    typeof (refs as { visible?: unknown }).visible === "string"
                );
            });
            const masks =
                planMaskExports && !includeHidden && !invisible && !suppressed
                    ? captureMaskSummary<Bytes>(visibleChildren, mixed)
                    : undefined;
            // The parent raster consumes this composition. Start it before
            // traversing descendants and avoid exporting images it replaces.
            // Keep full descendant metadata for offline diagnostics.
            if (planMaskExports && masks && !result.exports && !cancelled?.())
                maskExport = captureMaskRaster();
            const childAncestors = cache
                ? ancestors + JSON.stringify(result.properties)
                : "";
            {
                result.children = await mapCaptureChildren(
                    visibleChildren,
                    concurrency,
                    (child) =>
                        visit(
                            child,
                            invisible,
                            suppressed ||
                                (!includeHidden &&
                                    (result.exports !== undefined ||
                                        maskExport !== undefined)),
                            childAncestors,
                        ),
                    cancelled,
                );
            }
        }
        if (maskExport) await maskExport;
        // A mask affects its following siblings. Export their existing common
        // parent, rather than exporting the mask shape alone. Inventory and
        // source descendants remain available for offline diagnosis.
        if (
            maskComposition(result.children)?.kind === "raster" &&
            !invisible &&
            !suppressed &&
            !cancelled?.() &&
            !result.exports
        ) {
            await captureMaskRaster();
        }
        return result;
    }
    source.root = await visit(root, false);
    const definitions: ComponentLibrary<SourceNode<Bytes>>["definitions"] = [];
    const processedVariants = new Map<string, Set<string>>();
    // Visit only variants required by the selected tree. Each visit can
    // discover another instance family, so this is a fixed-point queue rather
    // than an eager expansion of every known component set.
    while (true) {
        if (cancelled?.()) throw Error("Capture cancelled");
        const owner = [...componentOwners.values()]
            .filter((n) => {
                const required = requiredVariants.get(n.id);
                const processed = processedVariants.get(n.id);
                return (
                    required !== undefined &&
                    [...required].some((id) => !processed?.has(id))
                );
            })
            .sort((a, b) => (a.id < b.id ? -1 : 1))[0];
        if (!owner) break;
        const processed =
            processedVariants.get(owner.id) ??
            (() => {
                const value = new Set<string>();
                processedVariants.set(owner.id, value);
                return value;
            })();
        const required = requiredVariants.get(owner.id) ?? new Set<string>();
        const ownerVariants =
            owner.type === "COMPONENT_SET"
                ? owner.children.filter(
                      (n): n is ComponentNode => n.type === "COMPONENT",
                  )
                : [owner];
        const byId = new Map(
            ownerVariants.map((variant) => [variant.id, variant]),
        );
        for (const variantId of [...required].sort()) {
            if (processed.has(variantId)) continue;
            const variant = byId.get(variantId);
            if (!variant) {
                // A stale registry entry must never become a phantom source
                // node or silently disappear from the native export graph.
                throw Error(
                    `Component reachability failed: missing variant ${variantId} in family ${owner.id}`,
                );
            }
            if (!capturedNodes.has(variant.id)) await visit(variant, false);
            processed.add(variantId);
        }
    }
    for (const owner of [...componentOwners.values()].sort((a, b) =>
        a.id < b.id ? -1 : 1,
    )) {
        const processed = processedVariants.get(owner.id);
        if (!processed?.size) continue;
        const axes: ComponentLibrary<
            SourceNode<Bytes>
        >["definitions"][number]["axes"] = {};
        for (const [key, property] of Object.entries(
            owner.componentPropertyDefinitions ?? {},
        )) {
            if (
                property.type === "VARIANT" &&
                typeof property.defaultValue === "string"
            )
                axes[key] = {
                    defaultValue: property.defaultValue,
                    options: [...(property.variantOptions ?? [])],
                };
        }
        const contract: NonNullable<
            ComponentLibrary<
                SourceNode<Bytes>
            >["definitions"][number]["contract"]
        > = { version: 1, properties: {}, bindings: {} };
        for (const [key, property] of Object.entries(
            owner.componentPropertyDefinitions ?? {},
        )) {
            if (
                property.type === "TEXT" ||
                property.type === "BOOLEAN" ||
                property.type === "INSTANCE_SWAP"
            )
                contract.properties[key] = {
                    type: property.type,
                    defaultValue: property.defaultValue,
                };
        }
        const collectBindings = (node: SourceNode<Bytes>) => {
            const refs = node.properties.componentPropertyReferences;
            if (refs && typeof refs === "object" && !Array.isArray(refs)) {
                const bindings = Object.fromEntries(
                    Object.entries(refs).filter(
                        ([, key]) =>
                            typeof key === "string" &&
                            key in contract.properties,
                    ),
                );
                if (Object.keys(bindings).length)
                    contract.bindings[node.id] = bindings;
            }
            for (const child of node.children ?? []) collectBindings(child);
        };
        const variants = [];
        for (const variant of (owner.type === "COMPONENT_SET"
            ? owner.children.filter(
                  (n): n is ComponentNode => n.type === "COMPONENT",
              )
            : [owner]
        )
            .filter((candidate) => processed.has(candidate.id))
            .sort((a, b) => (a.id < b.id ? -1 : 1))) {
            const captured = capturedNodes.get(variant.id);
            if (!captured) continue;
            collectBindings(captured);
            variants.push({
                id: variant.id,
                values: variant.variantProperties ?? {},
                root: captured,
            });
        }
        if (variants.length)
            definitions.push({
                id: owner.id,
                name: owner.name,
                scope: completeOwners.has(owner.id) ? "complete" : "private",
                axes,
                contract,
                variants,
            });
    }
    if (definitions.length) {
        const selectedOwner =
            root.type === "COMPONENT_SET"
                ? [...componentOwners.values()].find(
                      (owner) => owner.id === root.id,
                  )
                : root.type === "COMPONENT"
                  ? [...componentOwners.values()].find(
                        (owner) =>
                            owner.id === references[root.id]?.definitionId,
                    )
                  : undefined;
        const plan = planReachableVariants(
            { version: 1, definitions, references },
            source.root,
            {
                kind:
                    root.type === "COMPONENT_SET"
                        ? "component-set"
                        : root.type === "COMPONENT"
                          ? "component"
                          : root.type === "INSTANCE"
                            ? "instance"
                            : "frame",
                definitionId: selectedOwner?.id,
            },
        );
        if (plan.diagnostics.length)
            throw Error(
                `Component reachability failed: ${plan.diagnostics.join("; ")}`,
            );
        const plannedDefinitions = definitions
            .map((definition) => {
                const retained = plan.variants.get(definition.id);
                if (!retained) return undefined;
                const axes = plan.completeFamilies.has(definition.id)
                    ? definition.axes
                    : Object.fromEntries(plan.axes.get(definition.id) ?? []);
                return {
                    ...definition,
                    axes,
                    variants: definition.variants.filter((variant) =>
                        retained.has(variant.id),
                    ),
                };
            })
            .filter(
                (
                    definition,
                ): definition is ComponentLibrary<
                    SourceNode<Bytes>
                >["definitions"][number] =>
                    definition !== undefined && definition.variants.length > 0,
            );
        const plannedDefinitionIds = new Set(
            plannedDefinitions.map((definition) => definition.id),
        );
        const retainedNodeIds = new Set<string>();
        const collectNodeIds = (node: SourceNode<Bytes>) => {
            retainedNodeIds.add(node.id);
            for (const child of node.children ?? []) collectNodeIds(child);
        };
        collectNodeIds(source.root);
        for (const definition of plannedDefinitions)
            for (const variant of definition.variants)
                collectNodeIds(variant.root);
        const plannedReferences = Object.fromEntries(
            Object.entries(references).filter(
                ([nodeId, reference]) =>
                    retainedNodeIds.has(nodeId) &&
                    plannedDefinitionIds.has(reference.definitionId) &&
                    !!plan.variants
                        .get(reference.definitionId)
                        ?.has(reference.variantId),
            ),
        );
        source.components = {
            version: 1,
            definitions: plannedDefinitions,
            references: plannedReferences,
        };
    }

    // Variable identities and mode values are captured once; conversion remains offline.
    if (definitions.length && typeof figma !== "undefined" && figma.variables) {
        const variables: VariableLibrary = {
            version: 1,
            variables: {},
            collections: {},
            bindings: {},
        };
        const needed = new Set<string>();
        for (const node of capturedNodes.values()) {
            const fields: Record<string, string> = {};
            const raw = node.properties.boundVariables;
            const map: Record<string, string> = {
                width: "width",
                height: "height",
                opacity: "opacity",
                cornerRadius: "border-radius",
                itemSpacing: "spacing",
                paddingLeft: "padding-left",
                paddingRight: "padding-right",
                paddingTop: "padding-top",
                paddingBottom: "padding-bottom",
                characters: "text",
                fontSize: "font-size",
            };
            if (raw && typeof raw === "object" && !Array.isArray(raw))
                for (const [field, alias] of Object.entries(raw)) {
                    if (
                        map[field] &&
                        alias &&
                        typeof alias === "object" &&
                        !Array.isArray(alias) &&
                        alias.type === "VARIABLE_ALIAS" &&
                        typeof alias.id === "string"
                    )
                        fields[map[field]] = alias.id;
                }
            const fills = node.properties.fills;
            if (Array.isArray(fills) && fills.length === 1) {
                const paint = fills[0];
                if (
                    paint &&
                    typeof paint === "object" &&
                    !Array.isArray(paint) &&
                    paint.type === "SOLID" &&
                    (paint.opacity === undefined || paint.opacity === 1)
                ) {
                    const bound = paint.boundVariables;
                    if (
                        bound &&
                        typeof bound === "object" &&
                        !Array.isArray(bound)
                    ) {
                        const alias = bound.color;
                        if (
                            alias &&
                            typeof alias === "object" &&
                            !Array.isArray(alias) &&
                            alias.type === "VARIABLE_ALIAS" &&
                            typeof alias.id === "string"
                        )
                            fields[
                                node.type === "TEXT" ? "color" : "background"
                            ] = alias.id;
                    }
                }
            }
            if (Object.keys(fields).length) {
                Object.values(fields).forEach((id) => {
                    needed.add(id);
                });
                const modes = node.properties.resolvedVariableModes;
                variables.bindings[node.id] = {
                    fields,
                    modes:
                        modes &&
                        typeof modes === "object" &&
                        !Array.isArray(modes)
                            ? Object.fromEntries(
                                  Object.entries(modes).filter(
                                      (entry): entry is [string, string] =>
                                          typeof entry[1] === "string",
                                  ),
                              )
                            : {},
                };
            }
        }
        for (const id of needed) {
            if (cancelled?.()) throw Error("Capture cancelled");
            const variable = await figma.variables.getVariableByIdAsync(id);
            if (!variable) throw Error(`Bound variable ${id} is unavailable`);
            if (
                variable.resolvedType !== "COLOR" &&
                variable.resolvedType !== "FLOAT" &&
                variable.resolvedType !== "STRING" &&
                variable.resolvedType !== "BOOLEAN"
            )
                throw Error(
                    `Unsupported bound variable type: ${variable.resolvedType}`,
                );
            variables.variables[id] = {
                name: variable.name,
                type: variable.resolvedType,
                collectionId: variable.variableCollectionId,
                values: variable.valuesByMode as Record<string, VariableValue>,
            };
            for (const value of Object.values(variable.valuesByMode))
                if (
                    typeof value === "object" &&
                    "type" in value &&
                    value.type === "VARIABLE_ALIAS"
                )
                    needed.add(value.id);
            if (!variables.collections[variable.variableCollectionId]) {
                const collection =
                    await figma.variables.getVariableCollectionByIdAsync(
                        variable.variableCollectionId,
                    );
                if (!collection)
                    throw Error(
                        `Variable collection ${variable.variableCollectionId} is unavailable`,
                    );
                variables.collections[collection.id] = {
                    name: collection.name,
                    defaultModeId: collection.defaultModeId,
                    modes: collection.modes.map((m) => ({
                        id: m.modeId,
                        name: m.name,
                    })),
                };
            }
        }
        // Resolved mode maps include unrelated collections; keep the captured graph closed.
        for (const b of Object.values(variables.bindings))
            b.modes = Object.fromEntries(
                Object.entries(b.modes).filter(
                    ([id]) => variables.collections[id],
                ),
            );
        if (needed.size) source.variables = variables;
    }

    work.capturedNodes = capturedNodes.size;
    work.componentFamilies = definitions.length;
    work.componentVariants = definitions.reduce(
        (count, definition) => count + definition.variants.length,
        0,
    );
    return { source, durationMs: fontTiming.duration(), fontMetrics, work };
}

export type SourceCaptureResult =
    | Extract<CaptureResult, { ok: false } | { empty: true }>
    | {
          readonly ok: true;
          readonly empty?: false;
          readonly source: SourceCapture<Uint8Array>;
          readonly nodeIds: readonly string[];
          readonly captureMetrics: CaptureResult["captureMetrics"];
      };

export async function captureSelection(
    selection: readonly SceneNode[],
    mixedValue: unknown,
    exportSvgNode: SvgExporter = exportSvg,
    imageResolver: ImageResolver = resolveImage,
    instrumentation?: CaptureInstrumentation,
    exportPngNode?: PngExporter,
    exportScale = 1,
): Promise<CaptureResult> {
    const captured = await captureSelectionSource(
        selection,
        mixedValue,
        exportSvgNode,
        imageResolver,
        instrumentation,
        exportPngNode,
        exportScale,
    );
    if (!captured.ok || captured.empty) return captured;
    const result = await normalizeSource(captured.source);
    if (result.ok && result.empty) return result;
    return {
        ...result,
        nodeIds: captured.nodeIds,
        captureMetrics: {
            ...result.captureMetrics,
            durationMs: captured.captureMetrics.durationMs,
            work: captured.captureMetrics.work,
        },
    };
}

export async function captureSelectionSource(
    selection: readonly SceneNode[],
    mixedValue: unknown,
    exportSvgNode: SvgExporter = exportSvg,
    imageResolver: ImageResolver = resolveImage,
    instrumentation?: CaptureInstrumentation,
    exportPngNode?: PngExporter,
    exportScale = 1,
    cancelled?: () => boolean,
    cache?: CaptureCache,
    selectionNodeIds?: readonly string[],
): Promise<SourceCaptureResult> {
    const captureMetrics = {
        durationMs: 0,
        requests: 0,
        exports: 0,
        cacheHits: 0,
    };
    if (selection.length === 0)
        return { ok: true, empty: true, nodeIds: [], captureMetrics };
    if (selection.length !== 1)
        return {
            ok: false,
            nodeIds: [],
            captureMetrics,
            diagnostics: [
                {
                    code: "MULTIPLE_SELECTION",
                    severity: "error",
                    message: "Select exactly one node to preview it",
                },
            ],
        };
    // The interactive caller inventories synchronously for event tracking.
    // Standalone captures still collect their own complete descendant list.
    const nodeIds = selectionNodeIds ?? collectSelectionNodeIds(selection);
    try {
        const captured = await captureSource(
            selection[0],
            mixedValue,
            exportSvgNode,
            imageResolver,
            exportPngNode,
            exportScale,
            false,
            instrumentation,
            cancelled,
            4,
            true,
            cache,
            selectionExportScheduler,
        );
        return {
            ok: true,
            source: captured.source,
            nodeIds: [
                ...new Set([
                    ...nodeIds,
                    ...Object.keys(
                        captured.source.components?.references ?? {},
                    ),
                    ...(captured.source.components?.definitions.flatMap((d) => [
                        d.id,
                        ...d.variants.flatMap((v) => {
                            const ids: string[] = [];
                            const walk = (
                                n: SourceNode<CaptureBytes<true>>,
                            ) => {
                                ids.push(n.id);
                                for (const child of n.children ?? [])
                                    walk(child);
                            };
                            walk(v.root);
                            return ids;
                        }),
                    ]) ?? []),
                ]),
            ],
            captureMetrics: {
                ...captured.fontMetrics,
                durationMs: captured.durationMs,
                work: captured.work,
            },
        };
    } catch (error) {
        return {
            ok: false,
            nodeIds,
            captureMetrics,
            diagnostics: [
                {
                    code: "CAPTURE_ERROR",
                    severity: "error",
                    message:
                        error instanceof Error ? error.message : String(error),
                },
            ],
        };
    }
}
