// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { WindowPreferences } from "./window-preferences";
import { CaptureCache, reusableCapture } from "./capture-work";
import { CaptureAssetSender } from "../asset-transport";
import {
    type TimingTrace,
    TimingTraceBuilder,
    finalizeSandboxTiming,
} from "../performance/timing";
import {
    type CaptureInstrumentation,
    captureSelectionSource,
    captureColorProfile,
    sourceVariablesUnchanged,
    collectSelectionNodeIds,
    observeComponentDependencies,
} from "./capture";
import {
    type PluginToUiMessage,
    type PreviewTrigger,
    type UiToPluginMessage,
    isUiToPluginMessage,
} from "../protocol";

export function startPreview(): void {
    const windowPreferences = new WindowPreferences(figma.clientStorage);
    void windowPreferences.load().then((size) => {
        figma.showUI(__html__, { ...size, themeColors: true });
    });
    figma.skipInvisibleInstanceChildren = false;

    const captureCache = new CaptureCache(128 * 1024 * 1024);
    const cachedDependencies = new Map<string, () => void>();
    function invalidateCaptureCache() {
        captureCache.clear();
        for (const stop of cachedDependencies.values()) stop();
        cachedDependencies.clear();
    }
    let revision = 0;
    let selectedNodeIds = new Set<string>();
    let stopObservingComponents = () => {};
    let uiReady = false;
    let initialCaptureStarted = false;
    let devicePixelRatio = 1;
    const sandboxTraces = new Map<number, TimingTrace>();
    let observedPage: PageNode | undefined;
    let observedPageListener: ((event: NodeChangeEvent) => void) | undefined;
    let pinnedRoot: SceneNode | undefined;

    function startInitialCapture(): void {
        if (!uiReady || initialCaptureStarted) return;
        initialCaptureStarted = true;
        publishPinState();
        scheduleCapture("initial");
    }

    const captureAssetSender = new CaptureAssetSender();
    const captureIds = new WeakMap<object, string>();
    let nextCaptureId = 1;
    let latestCapture:
        | {
              revision: number;
              captureId: string;
              result: Extract<
                  Awaited<ReturnType<typeof captureSelectionSource>>,
                  { source: unknown }
              >;
              trace: TimingTraceBuilder;
          }
        | undefined;
    function sendFullCapture(capture: NonNullable<typeof latestCapture>): void {
        const packed = capture.trace.measure("captureSerialization", () =>
            captureAssetSender.pack(capture.result.source),
        );
        post(
            {
                type: "preview-capture",
                revision: capture.revision,
                captureId: capture.captureId,
                ...packed,
                selection: {
                    nodeId: capture.result.source.root.id,
                    nodeName: capture.result.source.root.name,
                },
                trigger: capture.trace.trigger,
            },
            capture.trace,
        );
    }

    function post(
        message: Extract<
            PluginToUiMessage,
            {
                type:
                    | "preview-capture"
                    | "preview-reuse"
                    | "preview-diagnostics"
                    | "preview-clear";
            }
        >,
        trace: TimingTraceBuilder,
        outcome:
            | "capture-error"
            | "conversion-error"
            | "cleared"
            | undefined = undefined,
    ): void {
        const wireTrace = trace.toTrace(outcome);
        const outgoing = {
            ...message,
            trace: wireTrace,
        } as PluginToUiMessage;
        trace.measure("messageDispatch", () => figma.ui.postMessage(outgoing));
        const sandboxTrace: TimingTrace = {
            ...trace.toTrace(outcome),
            // The UI transport duration must start at the exact wire timestamp,
            // while the dispatch phase is measured around the real host call.
            pluginSentAtEpochMs: wireTrace.pluginSentAtEpochMs,
        };
        sandboxTraces.set(message.revision, sandboxTrace);
        while (sandboxTraces.size > 50) {
            const oldest = sandboxTraces.keys().next().value;
            if (oldest === undefined) break;
            sandboxTraces.delete(oldest);
        }
        console.info(
            `[slint-preview] sent ${message.type} revision ${message.revision}`,
        );
    }

    function selectionInfo(): { nodeId: string; nodeName: string } | undefined {
        if (pinnedRoot !== undefined && !pinnedRoot.removed) {
            return { nodeId: pinnedRoot.id, nodeName: pinnedRoot.name };
        }
        const selection = figma.currentPage.selection;
        return selection.length === 1
            ? { nodeId: selection[0].id, nodeName: selection[0].name }
            : undefined;
    }

    function canPin(node: SceneNode | undefined): node is SceneNode {
        return node !== undefined && !node.removed;
    }

    function publishPinState(): void {
        const selection = figma.currentPage.selection;
        const selected = selection.length === 1 ? selection[0] : undefined;
        if (pinnedRoot !== undefined && !pinnedRoot.removed) {
            figma.ui.postMessage({
                type: "pin-state",
                pinned: true,
                canPin: false,
                pinnedRoot: {
                    nodeId: pinnedRoot.id,
                    nodeName: pinnedRoot.name,
                },
            });
            return;
        }
        figma.ui.postMessage({
            type: "pin-state",
            pinned: false,
            canPin: canPin(selected),
        });
    }

    function clearPin(): void {
        pinnedRoot = undefined;
    }

    async function sendCapture(trace: TimingTraceBuilder): Promise<void> {
        if (!uiReady) return;
        revision = Math.max(revision, trace.revision);
        const currentRevision = trace.revision;
        const captureScale = devicePixelRatio;
        const selection =
            pinnedRoot !== undefined && !pinnedRoot.removed
                ? [pinnedRoot]
                : figma.currentPage.selection;
        if (selection.length)
            figma.ui.postMessage({
                type: "preview-busy",
                revision: currentRevision,
            });
        const key =
            selection.length === 1
                ? `capture:${figma.currentPage.id}:${selection[0].id}:${captureScale}:${captureColorProfile()}`
                : undefined;
        const previousCapture = key
            ? captureCache.peek<
                  Awaited<ReturnType<typeof captureSelectionSource>>
              >(key)
            : undefined;
        const token = captureCache.token();
        const nodeIds =
            previousCapture?.nodeIds ?? collectSelectionNodeIds(selection);
        selectedNodeIds = new Set(nodeIds);
        const instrumentation: CaptureInstrumentation = {
            measureFontToImageConversion: (operation) =>
                trace.measureFontToImageConversion(operation),
        };
        const load = () =>
            captureSelectionSource(
                selection,
                figma.mixed,
                undefined,
                undefined,
                instrumentation,
                undefined,
                captureScale,
                () => currentRevision !== revision,
                captureCache,
                nodeIds,
                "tree",
                undefined,
                (document) => {
                    if (currentRevision === revision)
                        figma.ui.postMessage({
                            type: "preview-bulk",
                            revision: currentRevision,
                            document,
                        });
                },
            );
        let reused = false;
        const result = await trace.measureAsync("figmaCapture", async () => {
            const old = previousCapture;
            if (old?.ok && !old.empty) {
                if (
                    (await sourceVariablesUnchanged(old.source)) &&
                    captureCache.valid(token)
                ) {
                    reused = true;
                    return old;
                }
                invalidateCaptureCache();
            }
            if (!key) return load();
            const result = await load();
            if (
                result.ok &&
                !result.empty &&
                captureCache.valid(token) &&
                currentRevision === revision &&
                reusableCapture(result.source)
            ) {
                let bytes = 0;
                const metadata = JSON.stringify(
                    result.source,
                    (_key, value) => {
                        if (value instanceof Uint8Array) {
                            bytes += value.byteLength;
                            return null;
                        }
                        return value;
                    },
                );
                captureCache.put(
                    key,
                    result,
                    metadata.length * 2 + bytes,
                    token,
                );
            }
            return result;
        });
        const work = result.captureMetrics.work && {
            ...result.captureMetrics.work,
            ...(reused
                ? {
                      pngExports: 0,
                      svgExports: 0,
                      textCacheHits: 0,
                      pngByType: {},
                      svgByType: {},
                  }
                : {}),
        };
        trace.setCaptureMetrics({
            ...result.captureMetrics,
            ...(reused
                ? { durationMs: 0, requests: 0, exports: 0, cacheHits: 0 }
                : {}),
            work,
            sourceCacheHit: reused,
            timings: reused ? undefined : result.captureMetrics.timings,
        });
        if (currentRevision !== revision) {
            console.info(
                `[slint-preview] superseded capture revision ${currentRevision}`,
            );
            return;
        }
        if (result.ok && result.empty === true) {
            stopObservingComponents();
            selectedNodeIds = new Set();
            post(
                {
                    type: "preview-clear",
                    revision: currentRevision,
                    trigger: trace.trigger,
                },
                trace,
                "cleared",
            );
            return;
        }
        if (!result.ok) {
            // Keep watching every synchronously collected descendant. This lets a
            // nodechange that fixes an exported or unsupported child recover
            // without requiring a second selectionchange.
            selectedNodeIds = new Set(result.nodeIds);
            post(
                {
                    type: "preview-diagnostics",
                    revision: currentRevision,
                    diagnostics: result.diagnostics,
                    selection: selectionInfo(),
                    trigger: trace.trigger,
                },
                trace,
                "capture-error",
            );
            return;
        }
        if (currentRevision !== revision) {
            console.info(
                `[slint-preview] superseded capture revision ${currentRevision}`,
            );
            return;
        }
        selectedNodeIds = new Set(result.nodeIds);
        for (const [cachedKey, stop] of cachedDependencies) {
            if (captureCache.peek(cachedKey) === undefined) {
                stop();
                cachedDependencies.delete(cachedKey);
            }
        }
        if (
            key &&
            captureCache.peek(key) !== undefined &&
            !cachedDependencies.has(key)
        )
            cachedDependencies.set(
                key,
                observeComponentDependencies(
                    result.source.components?.definitions.map(
                        (definition) => definition.id,
                    ) ?? [],
                    selectedNodeIds,
                    invalidateCaptureCache,
                ),
            );
        stopObservingComponents();
        stopObservingComponents = observeComponentDependencies(
            result.source.components?.definitions.map(
                (definition) => definition.id,
            ) ?? [],
            selectedNodeIds,
            () => {
                invalidateCaptureCache();
                scheduleCapture("node-change");
            },
        );
        try {
            let captureId = captureIds.get(result.source);
            if (!captureId) {
                captureId = String(nextCaptureId++);
                captureIds.set(result.source, captureId);
            }
            latestCapture = {
                revision: currentRevision,
                captureId,
                result,
                trace,
            };
            if (reused) {
                trace.set("captureSerialization", 0);
                post(
                    {
                        type: "preview-reuse",
                        revision: currentRevision,
                        captureId,
                        selection: {
                            nodeId: result.source.root.id,
                            nodeName: result.source.root.name,
                        },
                        trigger: trace.trigger,
                    },
                    trace,
                );
            } else sendFullCapture(latestCapture);
        } catch (error) {
            post(
                {
                    type: "preview-diagnostics",
                    revision: currentRevision,
                    diagnostics: [
                        {
                            severity: "error",
                            code: "CAPTURE_SERIALIZATION_ERROR",
                            message:
                                error instanceof Error
                                    ? error.message
                                    : String(error),
                        },
                    ],
                    selection: {
                        nodeId: result.source.root.id,
                        nodeName: result.source.root.name,
                    },
                    trigger: trace.trigger,
                },
                trace,
                "conversion-error",
            );
        }
    }

    function scheduleCapture(
        trigger: PreviewTrigger,
        trace?: TimingTraceBuilder,
    ): void {
        if (!uiReady || !initialCaptureStarted) return;
        const nextRevision = revision + 1;
        revision = nextRevision;
        void sendCapture(
            trace ?? new TimingTraceBuilder(nextRevision, trigger),
        );
    }

    function isDescendantOfSelection(node: SceneNode): boolean {
        let current: (BaseNode & ChildrenMixin) | null = node.parent;
        while (current !== null) {
            if (selectedNodeIds.has(current.id)) return true;
            current = current.parent;
        }
        return false;
    }

    function observePage(): void {
        stopObservingComponents();
        invalidateCaptureCache();
        if (observedPage !== undefined && observedPageListener !== undefined) {
            observedPage.off("nodechange", observedPageListener);
        }
        observedPage = figma.currentPage;
        observedPageListener = (event) => {
            invalidateCaptureCache();
            if (pinnedRoot?.removed) {
                clearPin();
                publishPinState();
                scheduleCapture("selection-change");
                return;
            }
            if (
                event.nodeChanges.some((change) => {
                    if (
                        pinnedRoot !== undefined &&
                        change.id === pinnedRoot.id &&
                        change.type === "DELETE"
                    ) {
                        clearPin();
                        publishPinState();
                        scheduleCapture("selection-change");
                        return false;
                    }
                    if (selectedNodeIds.has(change.id)) return true;
                    if (change.type === "CREATE" && !change.node.removed) {
                        return isDescendantOfSelection(change.node);
                    }
                    return false;
                })
            ) {
                scheduleCapture("node-change");
            }
        };
        observedPage.on("nodechange", observedPageListener);
    }

    figma.on("selectionchange", () => {
        const trace = new TimingTraceBuilder(revision + 1, "selection-change");
        if (pinnedRoot === undefined) {
            publishPinState();
            scheduleCapture("selection-change", trace);
        }
    });
    figma.on("stylechange", () => captureCache.clear());
    figma.on("currentpagechange", () => {
        observePage();
        if (pinnedRoot !== undefined) {
            clearPin();
            publishPinState();
        }
        scheduleCapture("selection-change");
    });
    observePage();

    figma.ui.onmessage = (message: UiToPluginMessage) => {
        if (!isUiToPluginMessage(message)) {
            console.warn("[slint-preview] ignored unknown UI message");
            return;
        }
        if (message.type === "capture-reuse-miss") {
            if (
                latestCapture?.revision === revision &&
                message.revision === revision &&
                latestCapture.captureId === message.captureId
            )
                sendFullCapture(latestCapture);
        } else if (message.type === "reset-capture-assets") {
            captureAssetSender.reset();
        } else if (message.type === "ui-ready") {
            if (uiReady || initialCaptureStarted) {
                console.info("[slint-preview] UI readiness repeated");
                return;
            }
            devicePixelRatio = message.devicePixelRatio;
            uiReady = true;
            console.info(
                `[slint-preview] UI is ready at device pixel ratio ${devicePixelRatio}`,
            );
            startInitialCapture();
        } else if (message.type === "pixel-density-changed") {
            if (!uiReady || message.devicePixelRatio === devicePixelRatio)
                return;
            devicePixelRatio = message.devicePixelRatio;
            scheduleCapture("density-change");
        } else if (message.type === "pin-selection") {
            const selection = figma.currentPage.selection;
            const selected = selection.length === 1 ? selection[0] : undefined;
            if (canPin(selected)) {
                pinnedRoot = selected;
                selectedNodeIds = new Set();
                publishPinState();
                scheduleCapture("pin-change");
            } else {
                publishPinState();
            }
        } else if (message.type === "unpin") {
            clearPin();
            publishPinState();
            scheduleCapture("selection-change");
        } else if (message.type === "resizeWindow") {
            figma.ui.resize(message.width, message.height);
            windowPreferences.save(message);
        } else if (message.type === "clipboard-result") {
            figma.notify(message.success ? "Copied!" : "Failed to copy");
        } else if (message.type === "preview-complete") {
            const sandboxTrace = sandboxTraces.get(message.revision);
            if (sandboxTrace === undefined) {
                console.warn(
                    `[slint-preview] ignored completion for unknown revision ${message.revision}`,
                );
                return;
            }
            if (
                message.revision !== sandboxTrace.revision ||
                message.trace.revision !== sandboxTrace.revision ||
                message.trace.traceId !== String(sandboxTrace.revision)
            ) {
                console.warn(
                    `[slint-preview] ignored mismatched completion for revision ${message.revision}`,
                );
                return;
            }
            const finalizedTrace = finalizeSandboxTiming(
                message.trace,
                sandboxTrace,
            );
            sandboxTraces.delete(message.revision);
            console.info("[slint-preview] finalized trace", finalizedTrace);
            figma.ui.postMessage({
                type: "preview-finalized",
                revision: message.revision,
                trace: finalizedTrace,
            });
        }
    };
}
