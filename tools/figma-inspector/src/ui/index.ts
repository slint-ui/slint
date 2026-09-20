// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

declare const DEVELOPMENT: boolean;
import type { ExportPackage } from "../protocol";
import { downloadExport } from "./export-download";
import { exportValidationSource } from "../export/generate";
import { CaptureAssetReceiver } from "../asset-transport";
import {
    TIMING_PHASES,
    type TimingTrace,
    TimingTraceBuilder,
    cloneTimingBreakdowns,
    defaultClock,
    freezeTimingBreakdowns,
    transportDuration,
} from "../performance/timing";
import {
    type PluginToUiMessage,
    type PreviewTrigger,
    isPluginToUiMessage,
} from "../protocol";
import {
    materializePreviewAssets,
    packPreviewAssets,
} from "../asset-transport";
import { PreviewController } from "../preview/controller";
import { mountWorkspace, mountDialogFrame } from "./workspace";
import { SourcePanelController } from "./source-panel";
import { ConversionClient } from "./conversion-client";

import HighlightWorker from "./highlight.worker?worker&inline";
import ConversionWorker from "./conversion.worker?worker&inline";

const app = document.querySelector<HTMLElement>("#app");
if (app === null) throw new Error("Preview UI is missing its dialog frame");
mountDialogFrame(app);
mountWorkspace();

const status = document.querySelector<HTMLElement>("#status");
const canvas = document.querySelector<HTMLCanvasElement>("#preview-canvas");
const diagnostics = document.querySelector<HTMLElement>("#diagnostics");
const selection = document.querySelector<HTMLElement>("#selection");
const sourceView = document.querySelector<HTMLElement>("#source-view");
const timingTotal = document.querySelector<HTMLElement>("#timing-total");
const timingLabel = document.querySelector<HTMLElement>("#timing-label");
const timingPhases = document.querySelector<HTMLElement>("#timing-phases");
const copyButton = document.querySelector<HTMLButtonElement>("#copy-button");
const copySnapshot =
    document.querySelector<HTMLButtonElement>("#copy-snapshot");
const copySource = document.querySelector<HTMLButtonElement>("#copy-source");
const exportButton =
    document.querySelector<HTMLButtonElement>("#export-button");
const copyTrace = document.querySelector<HTMLButtonElement>("#copy-trace");
const pinButton = document.querySelector<HTMLButtonElement>("#pin-button");

if (
    status === null ||
    canvas === null ||
    diagnostics === null ||
    selection === null ||
    sourceView === null ||
    exportButton === null ||
    copyButton === null ||
    pinButton === null
) {
    throw new Error("Preview UI is missing required elements");
}

const selectionElement = selection;
const sourceViewElement = sourceView;
const timingTotalElement = timingTotal;
const timingLabelElement = timingLabel;
const timingPhasesElement = timingPhases;
const pinButtonElement = pinButton;
const exportButtonElement = exportButton;
let exporting = false;
const copyButtonElement = copyButton;
const copySourceElement = copySource;
const isFigmaUi = window.parent !== window;
let reportedDevicePixelRatio: number | undefined;
let densityMediaQuery: MediaQueryList | undefined;

function readDevicePixelRatio(): number | undefined {
    const ratio = window.devicePixelRatio;
    return Number.isFinite(ratio) && ratio > 0 ? ratio : undefined;
}

function armDensityMediaQuery(): void {
    if (!isFigmaUi || window.matchMedia === undefined) return;
    const ratio = readDevicePixelRatio();
    if (ratio === undefined) return;
    const nextQuery = window.matchMedia(`(resolution: ${ratio}dppx)`);
    nextQuery.addEventListener("change", reportDensityChange);
    densityMediaQuery?.removeEventListener("change", reportDensityChange);
    densityMediaQuery = nextQuery;
}

function reportDensityChange(): void {
    const ratio = readDevicePixelRatio();
    if (ratio === undefined || ratio === reportedDevicePixelRatio) return;
    reportedDevicePixelRatio = ratio;
    window.parent.postMessage(
        {
            pluginMessage: {
                type: "pixel-density-changed",
                devicePixelRatio: ratio,
            },
        },
        "*",
    );
    armDensityMediaQuery();
}

const keepaliveCanvas = document.createElement("canvas");
keepaliveCanvas.id = "preview-keepalive";
keepaliveCanvas.width = 1;
keepaliveCanvas.height = 1;
keepaliveCanvas.setAttribute("aria-hidden", "true");
keepaliveCanvas.style.cssText =
    "position: fixed; width: 1px; height: 1px; opacity: 0; pointer-events: none;";
document.body.append(keepaliveCanvas);
let successfulOutput:
    | {
          source: string;
          json: string;
          revision: number;
          name: string;
          exportPackage: ExportPackage;
      }
    | undefined;
const pendingOutputs = new Map<
    number,
    {
        source: string;
        json: string;
        revision: number;
        name: string;
        exportPackage: ExportPackage;
    }
>();
const previewAssetDisposers = new Map<number, () => void>();
function disposePreviewAssets(revision: number): void {
    previewAssetDisposers.get(revision)?.();
    previewAssetDisposers.delete(revision);
}
let latestInputRevision = 0;
const previewBusy = document.querySelector<HTMLElement>("#preview-busy");
function setPreviewBusy(busy: boolean): void {
    if (previewBusy) previewBusy.hidden = !busy;
    document
        .querySelector(".preview-shell")
        ?.setAttribute("aria-busy", String(busy));
}

function updateOutputProvenance(trace: TimingTrace): void {
    disposePreviewAssets(trace.revision);
    const completedOutput = pendingOutputs.get(trace.revision);
    // Retire payloads even when a newer revision owns the visible preview.
    for (const revision of pendingOutputs.keys())
        if (revision <= trace.revision) pendingOutputs.delete(revision);
    if (
        trace.revision <
        Math.max(controller.currentRevision, latestInputRevision)
    )
        return;
    if (trace.outcome === "rendered" || trace.outcome === "unchanged") {
        renderingSource = false;
        setPreviewBusy(false);
        successfulOutput = completedOutput ?? successfulOutput;
        conversionClient.present(trace.revision);
        if (
            successfulOutput &&
            latestSourceRevision !== successfulOutput.revision
        ) {
            latestSource = successfulOutput.source;
            if (!latestSource) sourcePanel.clear();
            latestSnapshotJson = successfulOutput.json;
            latestSourceRevision = successfulOutput.revision;
            sourceViewElement.dataset.snapshotBytes = String(
                latestSnapshotJson.length,
            );
            renderCopyState();
            sourcePanel.setSource(latestSource);
        }
    } else if (trace.outcome?.endsWith("error")) {
        conversionClient.fail(trace.revision);
        setPreviewBusy(false);
        renderingSource = false;
        clearOutput();
        document.getElementById("diagnostics-tab")?.click();
    }

    if (DEVELOPMENT && copySnapshot && successfulOutput)
        copySnapshot.title = `Copy JSON: ${successfulOutput.name}, revision ${successfulOutput.revision}`;
}
let latestSnapshotJson = "";
let latestSource = "";
let latestSourceRevision = 0;
let generatedSourceMetrics:
    | { readonly revision: number; readonly lineCount: number }
    | undefined;
let renderingSource = false;
let activeRenderTrace: TimingTrace | undefined;
let latestTrace: TimingTrace | undefined;
let displayedTraceRevision = 0;
const traceHistory: TimingTrace[] = [];
let pinState: {
    pinned: boolean;
    canPin: boolean;
    pinnedRoot?: { nodeId: string; nodeName: string };
} = { pinned: false, canPin: false };

function renderCopyState(): void {
    exportButtonElement.disabled = exporting || !successfulOutput;
    copyButtonElement.disabled = !successfulOutput;
    if (copySourceElement) copySourceElement.disabled = !successfulOutput;
    if (copySnapshot) copySnapshot.disabled = !successfulOutput;
}

type SourceTheme = "light-slint" | "dark-slint";

let sourceTheme: SourceTheme = "light-slint";
const darkModeQuery = window.matchMedia?.("(prefers-color-scheme: dark)");
if (darkModeQuery?.matches === true) sourceTheme = "dark-slint";
const sourcePanel = new SourcePanelController(sourceViewElement, {
    createWorker: () => new HighlightWorker(),
    canHighlight: () => !renderingSource && latestSource !== "",
    getRevision: () => latestSourceRevision,
    reportClipboardResult: (success) => {
        if (isFigmaUi)
            window.parent.postMessage(
                { pluginMessage: { type: "clipboard-result", success } },
                "*",
            );
    },
    onHighlightAccepted: (revision) => {
        sourceViewElement.dataset.revision = String(revision);
    },
});
sourcePanel.setTheme(sourceTheme);

darkModeQuery?.addEventListener("change", ({ matches }) => {
    sourceTheme = matches ? "dark-slint" : "light-slint";
    sourcePanel.setTheme(sourceTheme);
});
window.addEventListener("pagehide", () => {
    sourcePanel.dispose();
    for (const dispose of previewAssetDisposers.values()) dispose();
    previewAssetDisposers.clear();
});

function renderPinState(): void {
    const pinned = pinState.pinned && pinState.pinnedRoot !== undefined;
    pinButtonElement.disabled = !isFigmaUi || (!pinned && !pinState.canPin);
    pinButtonElement.setAttribute("aria-pressed", String(pinned));
    pinButtonElement.setAttribute(
        "aria-label",
        pinned
            ? `Unpin ${pinState.pinnedRoot?.nodeName ?? "preview"}`
            : "Pin selected preview root",
    );
    pinButtonElement.setAttribute(
        "title",
        pinned
            ? `Unpin ${pinState.pinnedRoot?.nodeName ?? "preview"}`
            : "Pin selected preview root",
    );
    if (pinned) {
        selectionElement.textContent = pinState.pinnedRoot?.nodeName ?? "";
        selectionElement.dataset.nodeId = pinState.pinnedRoot?.nodeId ?? "";
    }
}

function showTrace(trace: TimingTrace, acknowledge = true): void {
    if (activeRenderTrace?.revision === trace.revision)
        activeRenderTrace = undefined;
    updateOutputProvenance(trace);
    const readonlyTrace = freezeTrace(trace);
    if (acknowledge && isFigmaUi) {
        window.parent.postMessage(
            {
                pluginMessage: {
                    type: "preview-complete",
                    revision: readonlyTrace.revision,
                    trace: readonlyTrace,
                },
            },
            "*",
        );
        // In Figma mode this is only the provisional UI completion. The
        // sandbox-finalized trace is the sole complete history entry.
        return;
    }
    if (
        !DEVELOPMENT ||
        !timingTotalElement ||
        !timingLabelElement ||
        !timingPhasesElement
    )
        return;
    console.info("[slint-preview] completed trace", readonlyTrace);
    const existingTrace = traceHistory.findIndex(
        (item) => item.revision === readonlyTrace.revision,
    );
    if (existingTrace >= 0) traceHistory.splice(existingTrace, 1);
    traceHistory.unshift(readonlyTrace);
    traceHistory.splice(50);
    // A superseded operation still belongs in history, but can never replace
    // the state or trace shown for a newer accepted revision.
    if (readonlyTrace.revision < displayedTraceRevision) return;
    displayedTraceRevision = readonlyTrace.revision;
    latestTrace = readonlyTrace;
    const total = readonlyTrace.totalMs;
    timingTotalElement.textContent =
        total === undefined ? "No visible render" : `${total.toFixed(1)} ms`;
    timingTotalElement.dataset.outcome = readonlyTrace.outcome ?? "unknown";
    timingTotalElement.dataset.revision = String(readonlyTrace.revision);
    const renderKind =
        readonlyTrace.outcome === "rendered"
            ? readonlyTrace.coldStart
                ? "cold WASM"
                : "warm WASM"
            : "no render";
    const triggerLabel =
        readonlyTrace.trigger === "pin-change"
            ? "Pin change"
            : readonlyTrace.trigger === "node-change"
              ? "Node change"
              : readonlyTrace.trigger === "selection-change"
                ? "Selection change"
                : readonlyTrace.trigger === "density-change"
                  ? "Display density change"
                  : "Initial load";
    timingLabelElement.textContent = `${triggerLabel} received → ${readonlyTrace.outcome === "rendered" ? "component shown + browser presentation" : "completed"} · ${readonlyTrace.outcome ?? "pending"} · ${renderKind}`;
    timingLabelElement.title =
        "Starts when the plugin receives the event, not at the physical click. Rendering waits for Slint image loads to settle and a completed renderer frame, then two browser frames. OS display presentation is not observable here.";
    const timingRows = TIMING_PHASES.flatMap((phase) => {
        const rows = (() => {
            const term = document.createElement("dt");
            term.textContent = phase;
            const value = document.createElement("dd");
            const duration = readonlyTrace.phases[phase];
            value.textContent =
                duration === null ? "—" : `${duration.toFixed(1)} ms`;
            value.dataset.phase = phase;
            return [term, value];
        })();
        if (phase === "slintConversion") {
            const sourceTerm = document.createElement("dt");
            sourceTerm.className = "timing-breakdown";
            sourceTerm.textContent = "Generated source";
            const sourceValue = document.createElement("dd");
            sourceValue.className = "timing-breakdown";
            sourceValue.dataset.generatedSourceLines = "";
            const lineCount =
                generatedSourceMetrics?.revision === readonlyTrace.revision
                    ? generatedSourceMetrics.lineCount
                    : undefined;
            sourceValue.textContent =
                lineCount === undefined ? "—" : countLabel(lineCount, "line");
            return [...rows, sourceTerm, sourceValue];
        }
        if (phase !== "figmaCapture") return rows;
        const breakdownTerm = document.createElement("dt");
        breakdownTerm.className = "timing-breakdown";
        breakdownTerm.textContent = "Font → image conversion";
        const breakdownValue = document.createElement("dd");
        breakdownValue.className = "timing-breakdown";
        breakdownValue.dataset.breakdown = "fontToImageConversion";
        const conversion = readonlyTrace.breakdowns.fontToImageConversion;
        breakdownValue.textContent =
            conversion === null
                ? "—"
                : `${conversion.totalMs.toFixed(1)} ms · ${conversion.count} export${conversion.count === 1 ? "" : "s"}`;
        const captureTerm = document.createElement("dt");
        captureTerm.className = "timing-capture-metrics";
        captureTerm.dataset.captureMetric = "font-image";
        captureTerm.textContent = "↳ font → image";
        const captureValue = document.createElement("dd");
        captureValue.className = "timing-capture-metrics";
        captureValue.dataset.captureMetric = "font-image";
        const metrics = readonlyTrace.captureMetrics;
        captureValue.textContent =
            metrics === undefined
                ? "—"
                : `${metrics.durationMs.toFixed(1)} ms · ${countLabel(metrics.exports, "export")} · ${metrics.cacheHits} reused`;
        captureValue.dataset.captureDurationMs =
            metrics === undefined ? "—" : String(metrics.durationMs);
        captureValue.dataset.captureRequests =
            metrics === undefined ? "—" : String(metrics.requests);
        captureValue.dataset.captureExports =
            metrics === undefined ? "—" : String(metrics.exports);
        captureValue.dataset.captureCacheHits =
            metrics === undefined ? "—" : String(metrics.cacheHits);
        return [
            ...rows,
            breakdownTerm,
            breakdownValue,
            captureTerm,
            captureValue,
        ];
    });
    timingPhasesElement.replaceChildren(...timingRows);
    document.documentElement.dataset.traceRevision = String(
        readonlyTrace.revision,
    );
    document.documentElement.dataset.traceOutcome =
        readonlyTrace.outcome ?? "unknown";
}

function countLabel(value: number, singular: string): string {
    return `${value} ${value === 1 ? singular : `${singular}s`}`;
}

function sourceLineCount(source: string): number {
    if (source === "") return 0;
    const lineBreaks = source.match(/\r\n|\r|\n/gu)?.length ?? 0;
    return lineBreaks + (/(?:\r\n|\r|\n)$/u.test(source) ? 0 : 1);
}

function freezeTrace(trace: TimingTrace): TimingTrace {
    return Object.freeze({
        ...trace,
        phases: Object.freeze({ ...trace.phases }),
        breakdowns: freezeTimingBreakdowns(trace.breakdowns),
        captureMetrics:
            trace.captureMetrics === undefined
                ? undefined
                : Object.freeze({ ...trace.captureMetrics }),
    });
}

if (DEVELOPMENT)
    Object.defineProperty(window, "__slintPreviewDebug", {
        configurable: false,
        enumerable: false,
        writable: false,
        value: Object.freeze({
            getPendingOutputCount: (): number => pendingOutputs.size,
            getLatestTrace: (): TimingTrace | undefined => latestTrace,
            getTraces: (): readonly TimingTrace[] =>
                Object.freeze(traceHistory.slice()),
        }),
    });

const controller = new PreviewController(
    canvas,
    status,
    diagnostics,
    showTrace,
    keepaliveCanvas,
);
const isPreviewTrigger = (value: string): value is PreviewTrigger =>
    value === "initial" ||
    value === "selection-change" ||
    value === "node-change" ||
    value === "pin-change" ||
    value === "density-change";
let interpreterInitialization: Promise<void> | undefined;
const MAX_LIVE_EXPORT_VALIDATION_LENGTH = 1024 * 1024;

function receivedTrace(
    trace: TimingTrace | undefined,
    trigger: PreviewTrigger = "initial",
): TimingTrace {
    const receivedAtEpochMs =
        trace?.uiReceivedAtEpochMs ?? defaultClock.epochNow();
    if (trace !== undefined) {
        return {
            ...trace,
            uiReceivedAtEpochMs: receivedAtEpochMs,
            phases: {
                ...trace.phases,
                messageTransport: transportDuration(trace, receivedAtEpochMs),
            },
            breakdowns: cloneTimingBreakdowns(trace.breakdowns),
            captureMetrics:
                trace.captureMetrics === undefined
                    ? undefined
                    : { ...trace.captureMetrics },
        };
    }
    return {
        ...new TimingTraceBuilder(0, trigger).toTrace(),
        uiReceivedAtEpochMs: receivedAtEpochMs,
    };
}

function acceptSource(
    source: string,
    revision: number,
    exportPackage: ExportPackage,
    trace?: TimingTrace,
    warnings: readonly import("../plugin/snapshot").Diagnostic[] = [],
): void {
    if (
        !Number.isSafeInteger(revision) ||
        revision <= controller.currentRevision ||
        revision < latestInputRevision
    ) {
        return;
    }
    beginPreview(revision);
    pendingOutputs.set(revision, {
        source: exportPackage.source,
        exportPackage,
        json: "",
        revision,
        name: selectionElement.textContent ?? "Preview",
    });
    generatedSourceMetrics = {
        revision,
        lineCount: sourceLineCount(exportPackage.source),
    };
    renderingSource = true;
    setPreviewBusy(true);
    const renderTrace = trace ?? receivedTrace(undefined);
    const withRevision = {
        ...renderTrace,
        revision,
        traceId: String(revision),
    };
    activeRenderTrace = withRevision;
    const expandedValidation = exportValidationSource(exportPackage);
    const validation =
        expandedValidation.length <= MAX_LIVE_EXPORT_VALIDATION_LENGTH
            ? materializePreviewAssets(packPreviewAssets(expandedValidation))
            : undefined;
    if (
        revision <= controller.currentRevision ||
        revision < latestInputRevision
    ) {
        validation?.dispose();
        disposePreviewAssets(revision);
        return;
    }
    const disposeSource = previewAssetDisposers.get(revision);
    previewAssetDisposers.set(revision, () => {
        disposeSource?.();
        validation?.dispose();
    });
    const renderWarnings =
        validation === undefined
            ? [
                  ...warnings,
                  {
                      severity: "warning" as const,
                      code: "EXPORT_VALIDATION_SKIPPED",
                      category: "omission" as const,
                      message:
                          "Live export validation was skipped because the expanded export exceeds the browser compiler safety limit",
                  },
              ]
            : warnings;
    if (interpreterInitialization === undefined) {
        interpreterInitialization = controller.initialize(
            source,
            revision,
            withRevision,
            renderWarnings,
            validation?.source,
        );
        void interpreterInitialization.catch(() => {
            interpreterInitialization = undefined;
        });
    } else {
        controller.requestRender(
            source,
            revision,
            withRevision,
            renderWarnings,
            validation?.source,
        );
    }
}

window.addEventListener("error", (event) => {
    if (
        !event.filename.startsWith("wasm:") &&
        !(event.error instanceof WebAssembly.RuntimeError)
    )
        return;
    event.preventDefault();
    const stage = controller.currentCompileStage;
    controller.failCurrentRender(
        `Preview runtime failed${stage ? ` during ${stage}` : ""}: ${event.message || String(event.error)}`,
        latestInputRevision,
        activeRenderTrace,
    );
});

function acceptSelection(
    info: { nodeId: string; nodeName: string } | undefined,
): void {
    if (pinState.pinned && pinState.pinnedRoot !== undefined) {
        renderPinState();
        return;
    }
    selectionElement.textContent = info?.nodeName ?? "";
    selectionElement.dataset.nodeId = info?.nodeId ?? "";
}

function acceptDiagnostics(
    diagnostics: readonly {
        message: string;
        code?: string;
        propertyPath?: string;
    }[],
    revision: number,
    trace?: TimingTrace,
): void {
    if (revision < latestInputRevision) return;
    latestInputRevision = revision;
    conversionClient.fail(revision);
    const diagnosticTrace = trace ?? receivedTrace(undefined);
    const withRevision = {
        ...diagnosticTrace,
        revision,
        traceId: String(revision),
    };
    controller.showDiagnostic(
        diagnostics
            .map(
                (item) =>
                    `${item.code ?? "ERROR"}: ${item.message}${item.propertyPath === undefined ? "" : ` (${item.propertyPath})`}`,
            )
            .join("\n"),
        revision,
        withRevision,
        withRevision.outcome === "conversion-error"
            ? "conversion-error"
            : undefined,
    );
}

function clearOutput(): void {
    successfulOutput = undefined;
    pendingOutputs.clear();
    latestSource = "";
    latestSnapshotJson = "";
    generatedSourceMetrics = undefined;
    sourcePanel.clear();
    delete sourceViewElement.dataset.snapshotBytes;
    renderCopyState();
}

function beginPreview(revision: number): void {
    if (revision <= latestInputRevision) return;
    latestInputRevision = revision;
    controller.reserveRevision(revision);
    clearOutput();
    conversionClient.clear();
    renderingSource = true;
    setPreviewBusy(true);
}

function acceptClear(
    revision: number,
    trace?: TimingTrace,
    trigger: PreviewTrigger = "selection-change",
): void {
    if (
        !Number.isSafeInteger(revision) ||
        revision <= controller.currentRevision ||
        revision < latestInputRevision
    ) {
        return;
    }
    latestInputRevision = revision;
    renderingSource = false;
    setPreviewBusy(false);
    clearOutput();
    conversionClient.clear();
    latestSourceRevision = revision;
    acceptSelection(undefined);
    const clearTrace = receivedTrace(trace, trigger);
    void controller.clearPreview(revision, {
        ...clearTrace,
        revision,
        traceId: String(revision),
    });
}

const captureAssetCache = new CaptureAssetReceiver();
const conversionClient = new ConversionClient(() => new ConversionWorker());
function convertInWorker(
    message: Extract<PluginToUiMessage, { type: "preview-capture" }>,
): void {
    if (message.revision < latestInputRevision) return;
    beginPreview(message.revision);
    controller.reserveRevision(message.revision);
    renderingSource = true;
    setPreviewBusy(true);
    acceptSelection(message.selection);
    const trace = {
        ...receivedTrace(message.trace, message.trigger),
        revision: message.revision,
        traceId: String(message.revision),
    };
    const started = defaultClock.monotonicNow();
    const fail = (error: Error) => {
        if (message.revision !== latestInputRevision) return;
        acceptDiagnostics(
            [{ code: "CONVERSION_WORKER_ERROR", message: error.message }],
            message.revision,
            { ...trace, outcome: "conversion-error" },
        );
    };
    try {
        const captureAssets =
            message.captureAssetVersion === undefined
                ? undefined
                : captureAssetCache.resolve(message.captureAssets ?? []);
        conversionClient.capture(
            { ...message, ...(captureAssets ? { captureAssets } : {}), trace },
            (result, workerMs) => {
                if (message.revision !== latestInputRevision) return;
                const resultTrace = result.trace ?? trace;
                const finished = {
                    ...result,
                    trace: {
                        ...resultTrace,
                        phases: {
                            ...resultTrace.phases,
                            workerQueueAndTransport: Math.max(
                                0,
                                defaultClock.monotonicNow() -
                                    started -
                                    workerMs,
                            ),
                        },
                    },
                };
                window.dispatchEvent(
                    new MessageEvent("message", {
                        data: { pluginMessage: finished },
                    }),
                );
            },
            fail,
        );
    } catch (error) {
        window.parent.postMessage(
            { pluginMessage: { type: "reset-capture-assets" } },
            "*",
        );
        fail(error instanceof Error ? error : new Error(String(error)));
    }
}
window.addEventListener("pagehide", () => conversionClient.clear());

window.addEventListener("message", (event: MessageEvent<unknown>) => {
    const message =
        typeof event.data === "object" &&
        event.data !== null &&
        "pluginMessage" in event.data
            ? event.data.pluginMessage
            : undefined;
    if (isPluginToUiMessage(message)) {
        if (message.type === "preview-busy") {
            if (message.revision <= latestInputRevision) return;
            beginPreview(message.revision);
            return;
        }
        if (message.type === "pin-state") {
            pinState = {
                pinned: message.pinned,
                canPin: message.canPin,
                pinnedRoot: message.pinnedRoot,
            };
            renderPinState();
            return;
        }
        console.info(`[slint-preview] received revision ${message.revision}`);
        if (message.type === "preview-finalized") {
            showTrace(message.trace, false);
            return;
        }
        if (
            !Number.isSafeInteger(message.revision) ||
            message.revision <= controller.currentRevision ||
            message.revision < latestInputRevision
        ) {
            return;
        }
        if (message.type === "preview-capture") {
            if (message.revision < latestInputRevision) return;
            convertInWorker(message);
        } else if (message.type === "preview-source") {
            const trace = receivedTrace(
                message.trace,
                isPreviewTrigger(message.trigger ?? "")
                    ? message.trigger
                    : "initial",
            );
            const started = defaultClock.monotonicNow();
            try {
                const decoded =
                    typeof message.source === "string"
                        ? { source: message.source, dispose: undefined }
                        : materializePreviewAssets(message.source);
                if (decoded.dispose)
                    previewAssetDisposers.set(
                        message.revision,
                        decoded.dispose,
                    );
                if (typeof message.source !== "string")
                    trace.phases.assetUnpacking =
                        defaultClock.monotonicNow() - started;
                acceptSelection(message.selection);
                acceptSource(
                    decoded.source,
                    message.revision,
                    message.exportPackage,
                    trace,
                    message.warnings ?? [],
                );
            } catch (error) {
                disposePreviewAssets(message.revision);
                acceptSelection(message.selection);
                acceptDiagnostics(
                    [
                        {
                            code: "PREVIEW_ASSET_PREPARATION_FAILED",
                            message:
                                error instanceof Error
                                    ? error.message
                                    : String(error),
                        },
                    ],
                    message.revision,
                    trace,
                );
            }
        } else if (message.type === "preview-diagnostics") {
            acceptSelection(message.selection);
            acceptDiagnostics(
                message.diagnostics,
                message.revision,
                receivedTrace(
                    message.trace,
                    isPreviewTrigger(message.trigger ?? "")
                        ? message.trigger
                        : "initial",
                ),
            );
        } else {
            acceptClear(
                message.revision,
                message.trace,
                isPreviewTrigger(message.trigger ?? "")
                    ? message.trigger
                    : "selection-change",
            );
        }
    }
});

pinButtonElement.addEventListener("click", () => {
    if (!isFigmaUi || pinButtonElement.disabled) return;
    window.parent.postMessage(
        {
            pluginMessage: {
                type: pinState.pinned ? "unpin" : "pin-selection",
            },
        },
        "*",
    );
});

document.documentElement.dataset.previewScriptReady = "true";
if (isFigmaUi) {
    document.documentElement.dataset.figmaUi = "true";
    const ratio = readDevicePixelRatio();
    if (ratio !== undefined) {
        reportedDevicePixelRatio = ratio;
        window.parent.postMessage(
            {
                pluginMessage: { type: "ui-ready", devicePixelRatio: ratio },
            },
            "*",
        );
        armDensityMediaQuery();
        window.addEventListener("resize", reportDensityChange);
    }
}

const copySlint = () => {
    if (successfulOutput) sourcePanel.copy(successfulOutput.source);
};
copyButtonElement.addEventListener("click", copySlint);
if (DEVELOPMENT) {
    copySource?.addEventListener("click", copySlint);
    copyTrace?.addEventListener("click", () =>
        sourcePanel.copy(
            latestTrace ? JSON.stringify(latestTrace, null, 2) : "",
        ),
    );
    copySnapshot?.addEventListener("click", () => {
        const output = successfulOutput;
        if (!output) return;
        if (output.json) {
            sourcePanel.copy(output.json);
            return;
        }
        copySnapshot.disabled = true;
        void conversionClient
            .snapshot(output.revision)
            .then((json) => {
                if (successfulOutput !== output) return;
                output.json = json;
                latestSnapshotJson = json;
                sourceViewElement.dataset.snapshotBytes = String(json.length);
                sourcePanel.copy(json);
            })
            .catch((error) => {
                if (successfulOutput !== output) return;
                copySnapshot.title = `Copy failed: ${error instanceof Error ? error.message : String(error)}`;
            })
            .finally(() => {
                if (successfulOutput === output) copySnapshot.disabled = false;
            });
    });
}
exportButtonElement.addEventListener("click", async () => {
    const output = successfulOutput;
    if (!output || exporting) return;
    exporting = true;
    exportButtonElement.textContent = "Preparing ZIP…";
    renderCopyState();
    try {
        await downloadExport(
            output.exportPackage,
            output.name,
            () => successfulOutput === output,
        );
        exportButtonElement.title = "Export ZIP";
    } catch (error) {
        console.error("Export ZIP failed", error);
        exportButtonElement.title = "Export failed. Click to retry.";
    } finally {
        exporting = false;
        exportButtonElement.textContent = "Export ZIP";
        renderCopyState();
    }
});
