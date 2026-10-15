// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { codegenAliases, type CodegenVariable } from "./codegen-variables";
import type { VariableLibrary, VariableValue } from "./variable-library";
import type { ComponentLibrary } from "./components";
import { planReachableVariants } from "./components";
import type { CaptureCache, CaptureScheduler } from "./capture-work";
import {
    CaptureCancelled,
    captureBusyTime,
    captureScheduler,
    mapCaptureChildren,
} from "./capture-work";
import { maskComposition } from "./mask-composition";
import {
    CaptureTimingCollector,
    type CaptureTimings,
} from "../performance/capture-timing";
import { solidVectorSvg, unpaintedVectorSvg } from "./vector-svg";
import { restProperty, REST_UNAVAILABLE } from "./rest-properties";
import {
    type CaptureInstrumentation,
    type ImageResolver,
    type PngExporter,
    requiresVisualExport,
    type SvgExporter,
} from "./normalize";
import { isPngByteArray, pngDimensions, validSvgDocument } from "../images";
import type { CaptureResult, CaptureWork } from "./snapshot";
import type { VisualBounds } from "./source";
import {
    decodeValue,
    encodeValue,
    type SourceBytes,
    type SourceCapture,
    type SourceNode,
    type SourceResult,
    type SourceValue,
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
export type CaptureScope = "tree" | "root-only";

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

export const svgExportSettings = {
    format: "SVG_STRING",
    contentsOnly: true,
    useAbsoluteBounds: false,
    svgOutlineText: true,
    svgIdAttribute: false,
    svgSimplifyStroke: false,
    colorProfile: "SRGB",
} as const;

const exportSvg: SvgExporter = (node) => node.exportAsync(svgExportSettings);

function hasUntransformedBounds(node: SceneNode): boolean {
    if (!("absoluteTransform" in node)) return true;
    const transform = node.absoluteTransform;
    return (
        transform !== undefined &&
        Math.abs(transform[0][0] - 1) <= 0.00001 &&
        Math.abs(transform[1][1] - 1) <= 0.00001 &&
        Math.abs(transform[0][1]) <= 0.00001 &&
        Math.abs(transform[1][0]) <= 0.00001
    );
}

function hasNoEffects(effects: unknown): boolean {
    return Array.isArray(effects) && effects.length === 0;
}

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
    if (!hasUntransformedBounds(node)) return undefined;
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

export async function captureSource(
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
    cache?: CaptureCache,
    schedule = captureScheduler(concurrency),
    scope: CaptureScope = "tree",
    onBulkDocument?: (document: Record<string, unknown>) => void,
): Promise<{
    source: SourceCapture<Uint8Array>;
    durationMs: number;
    fontMetrics: { requests: number; exports: number; cacheHits: number };
    work: CaptureWork;
    timings: CaptureTimings;
}> {
    type Bytes = Uint8Array;
    const timings = new CaptureTimingCollector();
    const encode = (value: unknown) =>
        timings.measure("encoding", () => encodeValue(value, mixed));
    const identities = new WeakMap<object, string>();
    const idOf = (node: { id: string }): string => {
        const old = identities.get(node);
        if (old !== undefined) return old;
        const id = node.id;
        identities.set(node, id);
        return id;
    };
    const colorProfile =
        typeof figma === "undefined" ? "LEGACY" : captureColorProfile();
    const fontMetrics = { requests: 0, exports: 0, cacheHits: 0 };
    const pngByType: Record<string, number> = {};
    const svgByType: Record<string, number> = {};
    const work: { -readonly [Key in keyof CaptureWork]: CaptureWork[Key] } = {
        capturedNodes: 0,
        componentFamilies: 0,
        componentVariants: 0,
        pngExports: 0,
        svgExports: 0,
        textCacheHits: 0,
        pngByType,
        svgByType,
    };
    const propertyKeysByType = new Map<
        string,
        readonly (typeof sourceProperties)[number][]
    >();
    const restNodes = new Map<string, Record<string, unknown>>();
    const restLoads = new Map<string, Promise<void>>();
    const restDocuments: Record<string, unknown>[] = [];
    const loadRest = (node: SceneNode): Promise<void> => {
        if (
            exportSvgNode !== exportSvg ||
            scope !== "tree" ||
            !("exportAsync" in node)
        )
            return Promise.resolve();
        const id = idOf(node);
        const old = restLoads.get(id);
        if (old) return old;
        const pending = timings.measureAsync("bulkJson", async () => {
            try {
                const response = await node.exportAsync({
                    format: "JSON_REST_V1",
                });
                const data = response as {
                    document?: Record<string, unknown>;
                    nodes?: Record<
                        string,
                        { document?: Record<string, unknown> }
                    >;
                };
                const document = data.document ?? data.nodes?.[id]?.document;
                if (!document) return;
                if (onBulkDocument) {
                    restDocuments.push(document);
                    timings.measure("bulkDispatch", () =>
                        onBulkDocument(document),
                    );
                }
                const collect = (data: Record<string, unknown>) => {
                    if (
                        typeof data.id !== "string" ||
                        typeof data.type !== "string"
                    )
                        return;
                    restNodes.set(data.id, data);
                    if (Array.isArray(data.children))
                        for (const child of data.children) collect(child);
                };
                timings.measure("bulkIndex", () => collect(document));
            } catch {
                // JSON export is unavailable for some Figma node types and editor versions.
            }
        });
        restLoads.set(id, pending);
        return pending;
    };
    await loadRest(root);
    const fontTiming = captureBusyTime();
    const queued: CaptureScheduler = async (operation) => {
        const stop = timings.start("exportQueue");
        let started = false;
        try {
            return await schedule(() => {
                started = true;
                stop();
                return operation();
            });
        } finally {
            if (!started) stop();
        }
    };
    const rawPng =
        exportPngNode ?? (exportSvgNode === exportSvg ? exportPng : undefined);
    const png: PngExporter | undefined =
        rawPng &&
        ((node, scale) =>
            queued(async () => {
                if (cancelled?.()) throw new CaptureCancelled();
                const stop =
                    node.type === "TEXT" ? fontTiming.start() : undefined;
                try {
                    work.pngExports++;
                    pngByType[node.type] = (pngByType[node.type] ?? 0) + 1;
                    return await timings.measureAsync("pngExports", () =>
                        rawPng(node, scale),
                    );
                } finally {
                    stop?.();
                }
            }));
    const scheduledSvg = (
        node: Parameters<SvgExporter>[0],
        useAbsoluteBounds?: boolean,
    ): Promise<string> =>
        queued(async () => {
            if (cancelled?.()) throw new CaptureCancelled();
            const stop = node.type === "TEXT" ? fontTiming.start() : undefined;
            try {
                work.svgExports++;
                svgByType[node.type] = (svgByType[node.type] ?? 0) + 1;
                return await timings.measureAsync("svgExports", () =>
                    exportSvgNode === exportSvg &&
                    useAbsoluteBounds !== undefined
                        ? node.exportAsync({
                              ...svgExportSettings,
                              useAbsoluteBounds,
                          })
                        : exportSvgNode(node),
                );
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
                                      () =>
                                          timings.measureAsync(
                                              "imageReads",
                                              () => imageResolver(hash),
                                          ),
                                      (value) => value?.bytes.byteLength ?? 0,
                                  )
                                : await timings.measureAsync("imageReads", () =>
                                      imageResolver(hash),
                                  );
                            source.images[hash] = image
                                ? {
                                      value: {
                                          ...image,
                                          bytes: image.bytes,
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
    type ComponentPropertyDefinitions =
        ComponentNode["componentPropertyDefinitions"];
    const ownerChildren = new Map<string, readonly ComponentNode[]>();
    const ownerVariants = (
        owner: ComponentNode | ComponentSetNode,
    ): readonly ComponentNode[] => {
        const id = idOf(owner);
        const cached = ownerChildren.get(id);
        if (cached) return cached;
        const children =
            owner.type === "COMPONENT_SET"
                ? owner.children.filter(
                      (child): child is ComponentNode =>
                          child.type === "COMPONENT",
                  )
                : [owner];
        ownerChildren.set(id, children);
        return children;
    };
    const componentDefinitions = new Map<
        string,
        ComponentPropertyDefinitions | null
    >();
    const errorMessage = (error: unknown) =>
        String(error instanceof Error ? error.message : error);
    const recordReadFailure = (
        node: SourceNode<Bytes>,
        property: string,
        message: string,
    ) => {
        if (
            !node.errors.some(
                (error) =>
                    error.property === property && error.message === message,
            )
        )
            node.errors.push({ property, message });
    };
    const readComponentDefinitions = (
        owner: ComponentNode | ComponentSetNode,
        node: SourceNode<Bytes>,
    ): ComponentPropertyDefinitions | undefined => {
        const cached = componentDefinitions.get(idOf(owner));
        if (cached !== undefined) return cached ?? undefined;
        try {
            const definitions = owner.componentPropertyDefinitions ?? {};
            componentDefinitions.set(idOf(owner), definitions);
            return definitions;
        } catch (error) {
            componentDefinitions.set(idOf(owner), null);
            recordReadFailure(
                node,
                "componentPropertyDefinitions",
                `${errorMessage(error)}; reusable component metadata is omitted but the captured appearance is retained`,
            );
            return undefined;
        }
    };
    const requiredVariants = new Map<string, Set<string>>();
    const completeOwners = new Set<string>();
    const requireVariant = (
        owner: ComponentNode | ComponentSetNode,
        variantId: string,
    ) => {
        componentOwners.set(idOf(owner), owner);
        let required = requiredVariants.get(idOf(owner));
        if (!required) {
            required = new Set();
            requiredVariants.set(idOf(owner), required);
        }
        required.add(variantId);
    };
    const requireCompleteOwner = (owner: ComponentNode | ComponentSetNode) => {
        if (completeOwners.has(idOf(owner))) return;
        componentOwners.set(idOf(owner), owner);
        completeOwners.add(idOf(owner));
        const required = new Set<string>();
        for (const child of ownerVariants(owner))
            if (child.type === "COMPONENT") required.add(idOf(child));
        requiredVariants.set(idOf(owner), required);
    };
    const mainComponents = new Map<string, Promise<ComponentNode | null>>();
    const mainOwners = new Map<string, ComponentNode | ComponentSetNode>();
    async function readComponent(
        node: SceneNode,
        captured: SourceNode<Bytes>,
    ): Promise<void> {
        let main: ComponentNode | null = null;
        if (node.type === "COMPONENT_SET") {
            if (!readComponentDefinitions(node, captured)) return;
            if (node === root) requireCompleteOwner(node);
            else componentOwners.set(idOf(node), node);
            return;
        }
        if (node.type === "COMPONENT") main = node;
        else if (
            node.type === "INSTANCE" &&
            "getMainComponentAsync" in node &&
            typeof node.getMainComponentAsync === "function"
        ) {
            const componentId = restNodes.get(captured.id)?.componentId;
            const key =
                typeof componentId === "string"
                    ? `component:${componentId}`
                    : captured.id;
            let pending = mainComponents.get(key);
            if (!pending) {
                pending = timings.measureAsync("componentLookup", () =>
                    node.getMainComponentAsync(),
                );
                mainComponents.set(key, pending);
            }
            main = await pending;
        }
        if (!main) return;
        let owner = mainOwners.get(idOf(main));
        if (!owner) {
            const parent = main.parent;
            owner = parent?.type === "COMPONENT_SET" ? parent : main;
            mainOwners.set(idOf(main), owner);
        }
        if (!restNodes.has(idOf(owner))) await loadRest(owner);
        const definitions = readComponentDefinitions(owner, captured);
        if (!definitions) return;
        const hasInstanceSwapContract = Object.values(definitions).some(
            (property) => property.type === "INSTANCE_SWAP",
        );
        if (node === root && node.type === "COMPONENT")
            requireCompleteOwner(owner);
        else if (hasInstanceSwapContract) requireCompleteOwner(owner);
        else requireVariant(owner, idOf(main));
        let properties: Record<string, string | boolean> | undefined;
        if (node.type === "INSTANCE")
            try {
                properties = Object.fromEntries(
                    Object.entries(node.componentProperties).map(
                        ([key, property]) => [key, property.value],
                    ),
                );
            } catch (error) {
                recordReadFailure(
                    captured,
                    "componentProperties",
                    `${errorMessage(error)}; instance overrides are unavailable but the captured appearance is retained`,
                );
            }
        references[idOf(node)] = {
            definitionId: idOf(owner),
            variantId: idOf(main),
            ...(properties ? { properties } : {}),
        };
    }
    const hiddenCaptures = new Set<string>();
    const metadataById = new Map<
        string,
        {
            properties: SourceNode<Bytes>["properties"];
            errors: SourceNode<Bytes>["errors"];
            segments?: SourceNode<Bytes>["segments"];
            cells?: SourceNode<Bytes>["cells"];
            contextProperties: Record<string, unknown>;
            requiredByProperty: boolean;
            bulkProperties?: readonly string[];
        }
    >();
    const componentReads = new Map<
        string,
        Promise<SourceNode<Bytes>["errors"]>
    >();
    const childrenById = new Map<string, readonly SceneNode[]>();
    async function visit(
        node: SceneNode,
        {
            hidden = false,
            suppressed = false,
            ancestors = [] as readonly Record<string, unknown>[],
            cacheableContext = true,
            requiredByAncestor = false,
            svgContextSafe = true,
            definitionRoot = false,
        }: {
            hidden?: boolean;
            suppressed?: boolean;
            ancestors?: readonly Record<string, unknown>[];
            cacheableContext?: boolean;
            requiredByAncestor?: boolean;
            svgContextSafe?: boolean;
            definitionRoot?: boolean;
        } = {},
    ): Promise<SourceNode<Bytes>> {
        const { nodeType, raw, ownRequiredByProperty, result } =
            timings.measure("nodeReads", () => {
                const nodeType = node.type;
                const raw = node as unknown as Record<string, unknown>;
                const ownRequiredByProperty =
                    "componentPropertyReferences" in raw &&
                    typeof (
                        raw.componentPropertyReferences as
                            | { visible?: unknown }
                            | undefined
                    )?.visible === "string";
                const result: SourceNode<Bytes> = {
                    id: idOf(node),
                    name: node.name,
                    type: nodeType,
                    properties: {},
                    errors: [],
                };
                return { nodeType, raw, ownRequiredByProperty, result };
            });
        const oldMetadata = metadataById.get(result.id);
        const requiredByProperty =
            requiredByAncestor ||
            (oldMetadata?.requiredByProperty ?? ownRequiredByProperty);
        capturedNodes.set(result.id, result);
        work.capturedNodes = capturedNodes.size;
        let discoveryError: unknown;
        let discoveryFailed = false;
        let discovery: Promise<void> | undefined;
        if (
            scope === "tree" &&
            ["COMPONENT", "COMPONENT_SET", "INSTANCE"].includes(nodeType)
        ) {
            let pending = componentReads.get(result.id);
            if (!pending) {
                const component = { ...result, errors: [] };
                pending = timings.measureAsync(
                    "componentDiscovery",
                    async () => {
                        await readComponent(node, component);
                        return component.errors;
                    },
                );
                componentReads.set(result.id, pending);
            }
            discovery = pending
                .then((errors) => {
                    for (const error of errors)
                        recordReadFailure(
                            result,
                            error.property,
                            error.message,
                        );
                })
                .catch((error) => {
                    discoveryFailed = true;
                    discoveryError = error;
                });
        }
        const complete = async () => {
            await discovery;
            if (discoveryFailed) throw discoveryError;
            return result;
        };
        const invisible = timings.measure(
            "nodeReads",
            () =>
                hidden ||
                (!definitionRoot &&
                    "visible" in node &&
                    node.visible === false),
        );
        if (invisible) hiddenCaptures.add(idOf(node));
        else hiddenCaptures.delete(idOf(node));
        const captureMaskRaster = async () => {
            let raster: SourceResult<Bytes>;
            try {
                if (!png) throw Error("PNG export is disabled");
                raster = {
                    value: await png(
                        node as Parameters<PngExporter>[0],
                        exportScale,
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
        if (cancelled?.()) return complete();
        if (oldMetadata) {
            result.properties = oldMetadata.properties;
            result.errors.push(...oldMetadata.errors);
            result.segments = oldMetadata.segments;
            result.cells = oldMetadata.cells;
            result.bulkProperties = oldMetadata.bulkProperties;
        } else {
            timings.measure("metadata", () => {
                let propertyKeys = propertyKeysByType.get(nodeType);
                if (!propertyKeys) {
                    propertyKeys = timings.measure("nodeReads", () =>
                        sourceProperties.filter((key) => key in raw),
                    );
                    if (exportSvgNode === exportSvg)
                        propertyKeysByType.set(nodeType, propertyKeys);
                }
                const rest = restNodes.get(result.id);
                const bulkProperties: string[] = [];
                const values: Record<string, unknown> = {};
                const live: string[] = [];
                for (const key of propertyKeys) {
                    const cached =
                        rest &&
                        (rest.type === nodeType ||
                            (nodeType === "INSTANCE" && rest.type === "FRAME"))
                            ? restProperty(rest, key)
                            : REST_UNAVAILABLE;
                    if (cached === REST_UNAVAILABLE) live.push(key);
                    else if (onBulkDocument) {
                        result.properties[key] = cached as SourceValue;
                        bulkProperties.push(key);
                    } else values[key] = cached;
                }
                timings.measure("nodeReads", () => {
                    const reads: string[] = [];
                    const read = (key: string) => {
                        reads.push(key);
                        return raw[key];
                    };
                    for (const key of [
                        "relativeTransform",
                        "absoluteTransform",
                    ])
                        if (live.includes(key)) {
                            try {
                                values[key] = read(key);
                            } catch (error) {
                                result.properties[key] = {
                                    $source: "unavailable",
                                };
                                result.errors.push({
                                    property: key,
                                    message: errorMessage(error),
                                });
                            }
                        }
                    const relative = values.relativeTransform as
                        | number[][]
                        | undefined;
                    const absolute = values.absoluteTransform as
                        | number[][]
                        | undefined;
                    const position =
                        Array.isArray(relative) &&
                        relative.length === 2 &&
                        Array.isArray(relative[0]) &&
                        relative[0].length === 3 &&
                        Array.isArray(relative[1]) &&
                        relative[1].length === 3 &&
                        Number.isFinite(relative[0][2]) &&
                        Number.isFinite(relative[1][2])
                            ? [relative[0][2], relative[1][2]]
                            : undefined;
                    const box = rest?.absoluteBoundingBox as
                        | {
                              x?: number;
                              y?: number;
                              width?: number;
                              height?: number;
                          }
                        | undefined;
                    const dimensions =
                        [
                            "FRAME",
                            "INSTANCE",
                            "COMPONENT",
                            "COMPONENT_SET",
                            "RECTANGLE",
                            "SECTION",
                        ].includes(nodeType) &&
                        Array.isArray(absolute) &&
                        absolute.length === 2 &&
                        Array.isArray(absolute[0]) &&
                        absolute[0].length === 3 &&
                        Array.isArray(absolute[1]) &&
                        absolute[1].length === 3 &&
                        absolute[0][0] === 1 &&
                        absolute[0][1] === 0 &&
                        absolute[1][0] === 0 &&
                        absolute[1][1] === 1 &&
                        box &&
                        Number.isFinite(box.x) &&
                        Number.isFinite(box.y) &&
                        box.x === absolute[0][2] &&
                        box.y === absolute[1][2] &&
                        typeof box.width === "number" &&
                        Number.isFinite(box.width) &&
                        box.width > 0 &&
                        typeof box.height === "number" &&
                        Number.isFinite(box.height) &&
                        box.height > 0;
                    for (const key of live) {
                        if (
                            key === "relativeTransform" ||
                            key === "absoluteTransform" ||
                            key === "resolvedVariableModes"
                        )
                            continue;
                        try {
                            // Figma defines x and y as the relative matrix's translation.
                            // https://developers.figma.com/docs/plugins/api/properties/nodes-x/
                            if ((key === "x" || key === "y") && position)
                                values[key] = position[key === "x" ? 0 : 1];
                            else if (
                                (key === "width" || key === "height") &&
                                dimensions
                            )
                                values[key] = box![key];
                            else values[key] = read(key);
                        } catch (error) {
                            result.properties[key] = { $source: "unavailable" };
                            result.errors.push({
                                property: key,
                                message: errorMessage(error),
                            });
                        }
                    }
                    if (live.includes("resolvedVariableModes")) {
                        const bindings =
                            values.boundVariables ??
                            result.properties.boundVariables;
                        const fills = values.fills ?? result.properties.fills;
                        const empty = (value: unknown) =>
                            value === undefined ||
                            (value !== null &&
                                typeof value === "object" &&
                                !Array.isArray(value) &&
                                Object.keys(value).length === 0);
                        const unbound =
                            empty(bindings) &&
                            (fills === undefined ||
                                (Array.isArray(fills) &&
                                    fills.every(
                                        (paint) =>
                                            paint &&
                                            typeof paint === "object" &&
                                            empty(paint.boundVariables),
                                    )));
                        if (!unbound) {
                            try {
                                values.resolvedVariableModes = read(
                                    "resolvedVariableModes",
                                );
                            } catch (error) {
                                result.properties.resolvedVariableModes = {
                                    $source: "unavailable",
                                };
                                result.errors.push({
                                    property: "resolvedVariableModes",
                                    message: errorMessage(error),
                                });
                            }
                        }
                    }
                    timings.recordPropertyReads(nodeType, reads);
                    if (nodeType === "TEXT") {
                        const bounds = overflowingPaintBounds(node);
                        if (bounds) values.textPaintBounds = bounds;
                    }
                });
                timings.measure("encoding", () => {
                    for (const [key, value] of Object.entries(values))
                        result.properties[key] = encodeValue(value, mixed);
                });
                if (bulkProperties.length)
                    result.bulkProperties = bulkProperties;
            });
        }
        // Backdrop/mask composition needs pixels outside this subtree. Do not
        // treat a metadata fingerprint as proof that those pixels are unchanged.
        const contextProperties =
            oldMetadata?.contextProperties ??
            timings.measure("encoding", () => {
                const decoded: Record<string, unknown> = {};
                const bulk = new Set(result.bulkProperties);
                const keys =
                    nodeType === "TEXT"
                        ? Object.keys(result.properties)
                        : [
                              "width",
                              "height",
                              "opacity",
                              "isMask",
                              "blendMode",
                              "effects",
                              "fills",
                              "strokes",
                              "cornerRadius",
                              "topLeftRadius",
                              "topRightRadius",
                              "bottomLeftRadius",
                              "bottomRightRadius",
                              "clipsContent",
                              "absoluteTransform",
                              "relativeTransform",
                          ];
                for (const key of keys) {
                    const value = result.properties[key];
                    decoded[key] = bulk.has(key) ? value : decodeValue(value);
                }
                return decoded;
            });
        const rasterContextSafe =
            cacheableContext &&
            result.errors.length === 0 &&
            contextProperties.isMask !== true &&
            (contextProperties.blendMode === undefined ||
                contextProperties.blendMode === "NORMAL" ||
                contextProperties.blendMode === "PASS_THROUGH") &&
            !(
                Array.isArray(contextProperties.effects) &&
                contextProperties.effects.some(
                    (effect: { type?: string }) =>
                        effect.type === "BACKGROUND_BLUR",
                )
            );
        if (!oldMetadata) {
            timings.measure("metadata", () => {
                if (nodeType === "TEXT" && "getStyledTextSegments" in node) {
                    try {
                        timings.recordPropertyReads(nodeType, [
                            "getStyledTextSegments",
                        ]);
                        result.segments = {
                            value: encode(
                                timings.measure("nodeReads", () =>
                                    node.getStyledTextSegments(
                                        styledTextSegmentFields as never,
                                    ),
                                ),
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
                if (
                    scope === "tree" &&
                    nodeType === "TABLE" &&
                    "cellAt" in node
                ) {
                    result.cells = [];
                    for (let row = 0; row < node.numRows; row++)
                        for (
                            let column = 0;
                            column < node.numColumns;
                            column++
                        ) {
                            try {
                                const cell = node.cellAt(row, column);
                                const data: Record<string, unknown> = {};
                                for (const key of sourceProperties)
                                    if (key in cell)
                                        data[key] = (
                                            cell as unknown as Record<
                                                string,
                                                unknown
                                            >
                                        )[key];
                                result.cells.push(encode(data));
                            } catch (error) {
                                result.cells.push(null);
                                result.errors.push({
                                    property: `cellAt(${row},${column})`,
                                    message: String(error),
                                });
                            }
                        }
                }
            });
            if (!result.errors.length && !result.segments?.error)
                metadataById.set(result.id, {
                    properties: result.properties,
                    errors: [...result.errors],
                    segments: result.segments,
                    cells: result.cells,
                    contextProperties,
                    requiredByProperty: ownRequiredByProperty,
                    bulkProperties: result.bulkProperties,
                });
        }
        if (
            (!invisible ||
                requiredByProperty ||
                (node === root && !includeHidden)) &&
            (!suppressed || requiredByProperty) &&
            !cancelled?.()
        ) {
            await readImages(result.properties);
            if (result.segments?.value) await readImages(result.segments.value);
            if (cancelled?.()) return complete();
            // Root snippets use native text. Container exports would bake in
            // descendants; boolean operands instead define the selected shape.
            if (
                (scope === "tree" ||
                    (nodeType !== "TEXT" &&
                        (!("children" in node) ||
                            nodeType === "BOOLEAN_OPERATION"))) &&
                requiresVisualExport(
                    {
                        ...contextProperties,
                        type: result.type,
                    },
                    result.segments?.error
                        ? { error: result.segments.error }
                        : result.segments
                          ? {
                                segments: decodeValue(result.segments.value),
                            }
                          : undefined,
                )
            ) {
                async function attempt<T>(
                    operation: () => Promise<T>,
                ): Promise<SourceResult<T>> {
                    try {
                        if (cancelled?.()) throw new CaptureCancelled();
                        return { value: await operation() };
                    } catch (error) {
                        return {
                            error: String(
                                error instanceof Error ? error.message : error,
                            ),
                        };
                    }
                }
                const vectorSvg =
                    nodeType === "VECTOR" &&
                    !("children" in node) &&
                    rasterContextSafe &&
                    svgContextSafe &&
                    hasUntransformedBounds(node) &&
                    hasNoEffects(contextProperties.effects) &&
                    [contextProperties.fills, contextProperties.strokes].every(
                        (paints) =>
                            Array.isArray(paints) &&
                            paints.every(
                                (paint: {
                                    type?: string;
                                    blendMode?: string;
                                }) =>
                                    [
                                        "SOLID",
                                        "GRADIENT_LINEAR",
                                        "GRADIENT_RADIAL",
                                    ].includes(paint.type ?? "") &&
                                    (paint.blendMode === undefined ||
                                        paint.blendMode === "NORMAL"),
                            ),
                    );
                if (
                    vectorSvg &&
                    exportSvgNode === exportSvg &&
                    colorProfile !== "DISPLAY_P3"
                ) {
                    try {
                        const inline =
                            solidVectorSvg(
                                contextProperties,
                                raw.vectorPaths,
                            ) ??
                            unpaintedVectorSvg(
                                {
                                    ...contextProperties,
                                    fills: raw.fills,
                                    strokes: raw.strokes,
                                    fillStyleId: raw.fillStyleId,
                                },
                                raw.vectorNetwork,
                            );

                        if (inline !== undefined) {
                            result.exports = {
                                svg: { value: inline },
                                pngOmitted: "svg",
                            };
                            return complete();
                        }
                    } catch {
                        // Figma can reject optional path reads while still allowing exportAsync.
                    }
                }
                if (
                    exportSvgNode === exportSvg &&
                    colorProfile !== "DISPLAY_P3" &&
                    rasterContextSafe &&
                    svgContextSafe &&
                    [
                        "BOOLEAN_OPERATION",
                        "ELLIPSE",
                        "POLYGON",
                        "STAR",
                    ].includes(nodeType) &&
                    hasUntransformedBounds(node) &&
                    overflowingPaintBounds(node) === undefined
                ) {
                    try {
                        const inline = solidVectorSvg(
                            contextProperties,
                            raw.fillGeometry,
                        );
                        if (inline !== undefined)
                            result.exports = {
                                svg: { value: inline },
                                pngOmitted: "svg",
                            };
                    } catch {
                        /* Figma can reject optional geometry reads while still allowing image export. */
                    }
                }
                if (!result.exports) {
                    const paintBounds = overflowingPaintBounds(node);
                    const visualKey =
                        cache &&
                        rasterContextSafe &&
                        (nodeType === "TEXT" ||
                            (["VECTOR", "ELLIPSE", "LINE"].includes(nodeType) &&
                                !("children" in node)))
                            ? (() => {
                                  try {
                                      const geometry =
                                          nodeType === "VECTOR"
                                              ? [
                                                    raw.vectorPaths,
                                                    vectorSvg
                                                        ? undefined
                                                        : raw.vectorNetwork,
                                                    raw.strokeCap,
                                                    raw.strokeJoin,
                                                    raw.strokeMiterLimit,
                                                ]
                                              : nodeType === "ELLIPSE" ||
                                                  nodeType === "LINE"
                                                ? [
                                                      nodeType,
                                                      raw.arcData,
                                                      raw.strokeCap,
                                                      raw.strokeJoin,
                                                      raw.strokeMiterLimit,
                                                  ]
                                                : [];
                                      if (
                                          nodeType === "VECTOR" &&
                                          (geometry[0] === undefined ||
                                              (!vectorSvg &&
                                                  geometry[1] === undefined))
                                      )
                                          return undefined;
                                      if (
                                          result.segments?.error ||
                                          (nodeType === "TEXT" &&
                                              "hasMissingFont" in node &&
                                              node.hasMissingFont)
                                      )
                                          return undefined;
                                      return JSON.stringify([
                                          colorProfile,
                                          geometry,
                                          ancestors.map((ancestor) => {
                                              const { origin, ...context } =
                                                  ancestor;
                                              const transform =
                                                  contextProperties.absoluteTransform as
                                                      | number[][]
                                                      | undefined;
                                              const clipOrigin = origin as
                                                  | number[]
                                                  | undefined;
                                              return {
                                                  ...context,
                                                  ...(clipOrigin && transform
                                                      ? {
                                                            origin: [
                                                                clipOrigin[0] -
                                                                    transform[0][2],
                                                                clipOrigin[1] -
                                                                    transform[1][2],
                                                            ],
                                                        }
                                                      : {}),
                                              };
                                          }),
                                          hasUntransformedBounds(node)
                                              ? undefined
                                              : result.id,
                                          Object.fromEntries(
                                              Object.entries(
                                                  result.properties,
                                              ).filter(
                                                  ([key]) =>
                                                      ![
                                                          "x",
                                                          "y",
                                                          "absoluteTransform",
                                                          "relativeTransform",
                                                          "componentPropertyReferences",
                                                      ].includes(key),
                                              ),
                                          ),
                                          hasUntransformedBounds(node)
                                              ? (
                                                    contextProperties.absoluteTransform as
                                                        | number[][]
                                                        | undefined
                                                )?.map(
                                                    (row) =>
                                                        (((row[2] *
                                                            exportScale) %
                                                            1) +
                                                            1) %
                                                        1,
                                                )
                                              : [
                                                    contextProperties.absoluteTransform,
                                                    contextProperties.relativeTransform,
                                                ],
                                          result.segments,
                                          paintBounds,
                                          "resolvedVariableModes" in node
                                              ? node.resolvedVariableModes
                                              : null,
                                      ]);
                                  } catch {
                                      return undefined;
                                  }
                              })()
                            : undefined;
                    const rasterKey = visualKey
                        ? `png:${exportScale}:${visualKey}`
                        : undefined;
                    if (nodeType === "TEXT") {
                        fontMetrics.requests++;
                        fontMetrics.exports++;
                    }
                    const operation = async () => {
                        const readSvg = () =>
                            attempt(() =>
                                scheduledSvg(
                                    node as Parameters<SvgExporter>[0],
                                ),
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
                                          cache && rasterKey
                                              ? await cache.get(
                                                    `raster:${rasterKey}`,
                                                    load,
                                                    (value) => value.byteLength,
                                                    () => {
                                                        if (
                                                            nodeType === "TEXT"
                                                        ) {
                                                            fontMetrics.cacheHits++;
                                                            work.textCacheHits++;
                                                            fontMetrics.exports--;
                                                        }
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
                                      return bytes;
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
                        if (vectorSvg) {
                            const svg = await attempt(async () => {
                                const load = async () => {
                                    const value = (
                                        await scheduledSvg(
                                            node as Parameters<SvgExporter>[0],
                                            paintBounds === undefined,
                                        )
                                    ).trim();
                                    if (!validSvgDocument(value))
                                        throw Error(
                                            "SVG export did not return a valid SVG document",
                                        );
                                    return value;
                                };
                                return cache && visualKey
                                    ? cache.get(
                                          `svg:${visualKey}`,
                                          load,
                                          (value) => value.length * 2,
                                      )
                                    : load();
                            });
                            if (svg.value !== undefined) {
                                result.exports = {
                                    svg,
                                    pngOmitted: "svg",
                                };
                                if (paintBounds)
                                    result.exports.svgBounds = paintBounds;
                                return;
                            }
                        }
                        const raster = await readPng();
                        result.exports = {
                            svg: {},
                            svgOmitted: "png",
                            png: raster ?? {
                                error: "PNG exporter is unavailable",
                            },
                        };
                    };
                    if (
                        instrumentation &&
                        (nodeType === "CONNECTOR" ||
                            nodeType === "SHAPE_WITH_TEXT")
                    )
                        await instrumentation.measureFontToImageConversion(
                            operation,
                        );
                    else await operation();
                }
                if (result.exports?.png?.value) {
                    const bounds = overflowingPaintBounds(node);
                    if (bounds) result.exports.rasterBounds = bounds;
                }
            }
        }
        if (cancelled?.()) return complete();
        if ("children" in node) result.children = [];
        if (scope !== "root-only" && "children" in node) {
            let children: readonly SceneNode[] = [];
            try {
                children =
                    childrenById.get(result.id) ??
                    timings.measure("nodeReads", () => node.children);
                childrenById.set(result.id, children);
            } catch (error) {
                result.errors.push({
                    property: "children",
                    message: String(error),
                });
            }
            const visibleChildren = timings.measure("nodeReads", () =>
                children.filter((child) => {
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
                        typeof (refs as { visible?: unknown }).visible ===
                            "string"
                    );
                }),
            );
            const planMaskExports = scope === "tree";
            const masks =
                planMaskExports && !includeHidden && !invisible && !suppressed
                    ? captureMaskSummary<Bytes>(visibleChildren, mixed)
                    : undefined;
            // The parent raster consumes this composition. Start it before
            // traversing descendants and avoid exporting images it replaces.
            // Keep full descendant metadata for offline diagnostics.
            if (planMaskExports && masks && !result.exports && !cancelled?.())
                maskExport = captureMaskRaster();
            const childContext: Record<string, unknown> = {};
            if (
                contextProperties.opacity !== undefined &&
                contextProperties.opacity !== 1
            )
                childContext.opacity = contextProperties.opacity;
            if (contextProperties.clipsContent === true) {
                for (const key of [
                    "width",
                    "height",
                    "cornerRadius",
                    "topLeftRadius",
                    "topRightRadius",
                    "bottomLeftRadius",
                    "bottomRightRadius",
                ])
                    childContext[key] = contextProperties[key];
                const transform = contextProperties.absoluteTransform as
                    | number[][]
                    | undefined;
                if (transform)
                    childContext.origin = [transform[0][2], transform[1][2]];
                else childContext.id = result.id;
            }
            if (!svgContextSafe)
                childContext.effects = contextProperties.effects;
            if (!hasUntransformedBounds(node))
                childContext.transform = (
                    contextProperties.absoluteTransform as
                        | number[][]
                        | undefined
                )?.map((row) => row.slice(0, 2));
            const childAncestors =
                cache && Object.keys(childContext).length
                    ? [...ancestors, childContext]
                    : ancestors;
            result.children = await mapCaptureChildren(
                visibleChildren,
                concurrency,
                (child) =>
                    visit(child, {
                        hidden: invisible,
                        suppressed:
                            suppressed ||
                            (!includeHidden &&
                                (result.exports !== undefined ||
                                    maskExport !== undefined)),
                        ancestors: childAncestors,
                        cacheableContext: rasterContextSafe,
                        requiredByAncestor: requiredByProperty,
                        svgContextSafe:
                            svgContextSafe &&
                            (contextProperties.effects === undefined ||
                                (Array.isArray(contextProperties.effects) &&
                                    contextProperties.effects.every((effect) =>
                                        [
                                            "DROP_SHADOW",
                                            "INNER_SHADOW",
                                        ].includes(effect.type),
                                    ))),
                    }),
                cancelled,
            );
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
        return complete();
    }
    source.root = await visit(root);
    const definitions: ComponentLibrary<SourceNode<Bytes>>["definitions"] = [];
    const processedVariants = new Map<string, Set<string>>();
    // Visit only variants required by the selected tree. Each visit can
    // discover another instance family, so this is a fixed-point queue rather
    // than an eager expansion of every known component set.
    while (!cancelled?.()) {
        const pending: ComponentNode[] = [];
        for (const owner of [...componentOwners.values()].sort((a, b) =>
            idOf(a) < idOf(b) ? -1 : 1,
        )) {
            const processed =
                processedVariants.get(idOf(owner)) ?? new Set<string>();
            processedVariants.set(idOf(owner), processed);
            const variants = ownerVariants(owner);
            const byId = new Map(
                variants.map((variant) => [idOf(variant), variant]),
            );
            for (const id of [
                ...(requiredVariants.get(idOf(owner)) ?? []),
            ].sort()) {
                if (processed.has(id)) continue;
                const variant = byId.get(id);
                if (!variant)
                    throw Error(
                        `Component reachability failed: missing variant ${id} in family ${idOf(owner)}`,
                    );
                processed.add(id);
                if (!capturedNodes.has(id) || hiddenCaptures.has(id))
                    pending.push(variant);
            }
        }
        if (!pending.length) break;
        await mapCaptureChildren(
            pending,
            concurrency,
            (variant) => visit(variant, { definitionRoot: true }),
            cancelled,
        );
    }
    if (cancelled?.()) throw new CaptureCancelled();
    timings.measure("componentAssembly", () => {
        for (const owner of [...componentOwners.values()].sort((a, b) =>
            idOf(a) < idOf(b) ? -1 : 1,
        )) {
            const processed = processedVariants.get(idOf(owner));
            if (!processed?.size) continue;
            const metadata = componentDefinitions.get(idOf(owner));
            if (!metadata) continue;
            const retainedVariants = ownerVariants(owner)
                .filter((candidate) => processed.has(idOf(candidate)))
                .sort((a, b) => (idOf(a) < idOf(b) ? -1 : 1));
            const retainedValues = new Map<
                ComponentNode,
                Record<string, string>
            >();
            let invalidValues: string | undefined;
            for (const variant of retainedVariants)
                try {
                    retainedValues.set(
                        variant,
                        timings.measure(
                            "nodeReads",
                            () => variant.variantProperties ?? {},
                        ),
                    );
                } catch (error) {
                    invalidValues = errorMessage(error);
                    break;
                }
            if (invalidValues) {
                recordReadFailure(
                    source.root,
                    "variantProperties",
                    `${invalidValues}; ${owner.name} is retained as static visual content`,
                );
                for (const [id, reference] of Object.entries(references))
                    if (reference.definitionId === idOf(owner))
                        delete references[id];
                continue;
            }
            const axes: ComponentLibrary<
                SourceNode<Bytes>
            >["definitions"][number]["axes"] = {};
            for (const [key, property] of Object.entries(metadata)) {
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
            for (const [key, property] of Object.entries(metadata)) {
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
                        contract.bindings[idOf(node)] = bindings;
                }
                for (const child of node.children ?? []) collectBindings(child);
            };
            const variants = [];
            for (const variant of retainedVariants) {
                const captured = capturedNodes.get(idOf(variant));
                if (!captured) continue;
                collectBindings(captured);
                variants.push({
                    id: idOf(variant),
                    values: retainedValues.get(variant) ?? {},
                    root: captured,
                });
            }
            if (variants.length)
                definitions.push({
                    id: idOf(owner),
                    name: owner.name,
                    scope: completeOwners.has(idOf(owner))
                        ? "complete"
                        : "private",
                    axes,
                    contract,
                    variants,
                });
        }
        if (definitions.length) {
            const selectedOwner =
                root.type === "COMPONENT_SET"
                    ? [...componentOwners.values()].find(
                          (owner) => idOf(owner) === idOf(root),
                      )
                    : root.type === "COMPONENT"
                      ? [...componentOwners.values()].find(
                            (owner) =>
                                idOf(owner) ===
                                references[idOf(root)]?.definitionId,
                        )
                      : undefined;
            const plan = timings.measure("componentPlanning", () =>
                planReachableVariants(
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
                        definitionId: selectedOwner && idOf(selectedOwner),
                    },
                ),
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
                        : Object.fromEntries(
                              plan.axes.get(definition.id) ?? [],
                          );
                    return {
                        ...definition,
                        axes,
                        variants: definition.variants.filter((variant) =>
                            retained.has(idOf(variant)),
                        ),
                    };
                })
                .filter(
                    (
                        definition,
                    ): definition is ComponentLibrary<
                        SourceNode<Bytes>
                    >["definitions"][number] =>
                        definition !== undefined &&
                        definition.variants.length > 0,
                );
            const plannedDefinitionIds = new Set(
                plannedDefinitions.map((definition) => definition.id),
            );
            const retainedNodeIds = new Set<string>();
            const collectNodeIds = (node: SourceNode<Bytes>) => {
                retainedNodeIds.add(idOf(node));
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
    });

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
                variables.bindings[idOf(node)] = {
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
            if (cancelled?.()) throw new CaptureCancelled();
            const variable = await timings.measureAsync("variableReads", () =>
                figma.variables.getVariableByIdAsync(id),
            );
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
                const collection = await timings.measureAsync(
                    "variableReads",
                    () =>
                        figma.variables.getVariableCollectionByIdAsync(
                            variable.variableCollectionId,
                        ),
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
    if (restDocuments.length) {
        const seen = new Set<SourceNode<Bytes>>();
        const records = new Map<string, Record<string, unknown>>();
        const sets = new Map<string, number>();
        source.bulkPropertySets = [];
        const compact = (node: SourceNode<Bytes>) => {
            if (seen.has(node)) return;
            seen.add(node);
            if (node.bulkProperties?.length) {
                const rest = restNodes.get(idOf(node))!;
                const fields = [
                    ...node.bulkProperties,
                    "rectangleCornerRadii",
                    "strokeDashes",
                ];
                records.set(idOf(node), {
                    id: idOf(node),
                    type: rest.type,
                    ...Object.fromEntries(
                        fields
                            .filter((key) => key in rest)
                            .map((key) => [key, rest[key]]),
                    ),
                });
                const signature = node.bulkProperties.join(",");
                let index = sets.get(signature);
                if (index === undefined) {
                    index = source.bulkPropertySets!.length;
                    sets.set(signature, index);
                    source.bulkPropertySets!.push([...node.bulkProperties]);
                }
                node.bulkPropertySet = index;
                node.properties = { ...node.properties };
                for (const key of node.bulkProperties)
                    delete node.properties[key];
                delete node.bulkProperties;
            }
            for (const child of node.children ?? []) compact(child);
        };
        compact(source.root);
        for (const definition of source.components?.definitions ?? [])
            for (const variant of definition.variants) compact(variant.root);
        source.bulkDocuments = [
            {
                id: `capture-bulk:${source.root.id}`,
                type: "FRAME",
                children: [...records.values()],
            },
        ];
    }
    return {
        source,
        durationMs: fontTiming.duration(),
        fontMetrics,
        work,
        timings: timings.snapshot(),
    };
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
    scope: CaptureScope = "tree",
    schedule: CaptureScheduler = selectionExportScheduler,
    onBulkDocument?: (document: Record<string, unknown>) => void,
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
            cache,
            schedule,
            scope,
            onBulkDocument,
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
                            const walk = (n: SourceNode<Uint8Array>) => {
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
                timings: captured.timings,
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

/** Read only variables bound to the captured root, without expanding its subtree. */
export async function captureCodegenVariables(
    root: SourceNode<SourceBytes>,
): Promise<CodegenVariable[]> {
    const variables = new Map<
        string,
        Promise<Omit<CodegenVariable, "field">>
    >();
    return Promise.all(
        codegenAliases(root).map(async ({ field, id }) => {
            if (!variables.has(id))
                variables.set(
                    id,
                    (async () => {
                        try {
                            const variable =
                                await figma.variables.getVariableByIdAsync(id);
                            if (!variable) return {};
                            const collection =
                                await figma.variables.getVariableCollectionByIdAsync(
                                    variable.variableCollectionId,
                                );
                            if (!collection) return {};
                            return {
                                name: variable.name,
                                collection: collection.name,
                                modes: collection.modes.length,
                                type: variable.resolvedType,
                            };
                        } catch {
                            return {};
                        }
                    })(),
                );
            return { field, ...(await variables.get(id)) };
        }),
    );
}

/** Recheck variable definitions because variable edits need not change node bindings. */
export async function sourceVariablesUnchanged(
    source: SourceCapture<SourceBytes>,
): Promise<boolean> {
    if (!source.variables)
        return !JSON.stringify(source, (_key, value) =>
            value instanceof Uint8Array ? null : value,
        ).includes('"VARIABLE_ALIAS"');
    try {
        const variables = await Promise.all(
            Object.entries(source.variables.variables).map(
                async ([id, captured]) => {
                    const current =
                        await figma.variables.getVariableByIdAsync(id);
                    return (
                        current !== null &&
                        current.name === captured.name &&
                        current.resolvedType === captured.type &&
                        current.variableCollectionId ===
                            captured.collectionId &&
                        JSON.stringify(current.valuesByMode) ===
                            JSON.stringify(captured.values)
                    );
                },
            ),
        );
        const collections = await Promise.all(
            Object.entries(source.variables.collections).map(
                async ([id, captured]) => {
                    const current =
                        await figma.variables.getVariableCollectionByIdAsync(
                            id,
                        );
                    return (
                        current !== null &&
                        current.name === captured.name &&
                        current.defaultModeId === captured.defaultModeId &&
                        JSON.stringify(
                            current.modes.map((mode) => ({
                                id: mode.modeId,
                                name: mode.name,
                            })),
                        ) === JSON.stringify(captured.modes)
                    );
                },
            ),
        );
        return variables.every(Boolean) && collections.every(Boolean);
    } catch {
        return false;
    }
}

export function captureColorProfile(): string {
    return figma.root?.documentColorProfile ?? "LEGACY";
}
