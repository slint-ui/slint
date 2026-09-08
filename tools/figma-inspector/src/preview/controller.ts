// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import wasmDataUrl from "@interpreter/slint_wasm_interpreter_bg.wasm?url&inline";
import initialize, {
    compile_from_string,
    run_event_loop,
    type WrappedInstance,
} from "@interpreter/slint_wasm_interpreter.js";
import {
    cloneTimingBreakdowns,
    defaultClock,
    deriveUnattributedOverhead,
} from "../performance/timing";
import type { TimingTrace } from "../performance/timing";
import type { Diagnostic } from "../plugin/snapshot";
import { warningSummaries } from "../plugin/snapshot";
import { FIRST_BUTTON_SOURCE } from "./sources";

type PreviewState = "initializing" | "compiling" | "ready" | "error";
type RenderRequest = {
    readonly source: string;
    readonly revision: number;
    readonly trace?: TimingTrace;
    readonly warnings: readonly Diagnostic[];
    readonly acceptedAtMonotonicMs: number;
};
type WorkingTrace = Omit<
    TimingTrace,
    | "phases"
    | "outcome"
    | "completedAtEpochMs"
    | "totalUiMs"
    | "totalMs"
    | "coldStart"
> & {
    phases: TimingTrace["phases"];
    outcome?: TimingTrace["outcome"];
    completedAtEpochMs?: number;
    coldStart?: boolean;
    totalUiMs?: number;
    totalMs?: number;
};
type DiagnosticOutcome = "capture-error" | "conversion-error";
type RenderOutcome = DiagnosticOutcome | "compilation-error";

export class PreviewController {
    private instance: WrappedInstance | undefined;
    private initialized = false;
    private renderedSource: string | undefined;
    /** The newest accepted source or diagnostic action, regardless of state. */
    private nextRevision = 0;
    private minimumRevision = 0;
    private pendingRender: RenderRequest | undefined;
    private processing = false;
    private initializationPromise: Promise<void> | undefined;
    private wasmInitializationDurationMs = 0;
    private readonly freedInstances = new WeakSet<object>();
    /** True after Slint has acquired the preview canvas's WebGL context. */
    private canvasHasSlintContext = false;

    public constructor(
        private readonly canvas: HTMLCanvasElement,
        private readonly status: HTMLElement,
        private readonly diagnostics: HTMLElement,
        private readonly onTrace?: (trace: TimingTrace) => void,
        private readonly keepaliveCanvas?: HTMLCanvasElement,
    ) {}

    private keepaliveInstance: WrappedInstance | undefined;

    public get currentInstance(): WrappedInstance | undefined {
        return this.instance;
    }

    public get currentRevision(): number {
        return this.nextRevision;
    }

    /** Prevent an older in-flight render from presenting while conversion runs. */
    public reserveRevision(revision: number): void {
        if (
            !Number.isSafeInteger(revision) ||
            revision <= this.minimumRevision ||
            revision <= this.nextRevision
        )
            return;
        this.canvas.style.visibility = "hidden";
        this.diagnostics.hidden = true;
        this.diagnostics.textContent = "";
        this.minimumRevision = revision;
        this.supersedePendingRender();
    }

    public initialize(
        source: string = FIRST_BUTTON_SOURCE,
        revision = this.nextRevision + 1,
        trace?: TimingTrace,
        warnings: readonly Diagnostic[] = [],
    ): Promise<void> {
        if (this.initializationPromise !== undefined) {
            return this.initializationPromise;
        }
        const acceptedAtMonotonicMs = defaultClock.monotonicNow();
        this.initializationPromise = this.initializeInterpreter(
            source,
            revision,
            trace,
            warnings,
            acceptedAtMonotonicMs,
        );
        void this.initializationPromise.catch(() => {
            this.initializationPromise = undefined;
        });
        return this.initializationPromise;
    }

    public requestRender(
        source: string,
        revision: number,
        trace?: TimingTrace,
        warnings: readonly Diagnostic[] = [],
    ): void {
        if (
            !Number.isSafeInteger(revision) ||
            revision <= this.nextRevision ||
            revision < this.minimumRevision
        ) {
            return;
        }
        this.canvas.style.visibility = "hidden";
        this.supersedePendingRender();
        this.nextRevision = revision;
        this.pendingRender = {
            source,
            revision,
            trace,
            warnings,
            acceptedAtMonotonicMs: defaultClock.monotonicNow(),
        };
        if (this.initialized && !this.processing) {
            void this.drainRenderQueue();
        }
    }

    public showDiagnostic(
        message: string,
        revision: number,
        trace?: TimingTrace,
        outcome?: DiagnosticOutcome,
    ): void {
        if (
            !Number.isSafeInteger(revision) ||
            revision <= this.nextRevision ||
            revision < this.minimumRevision
        ) {
            return;
        }
        this.canvas.style.visibility = "hidden";
        this.supersedePendingRender();
        this.nextRevision = revision;
        const acceptedAtMonotonicMs = defaultClock.monotonicNow();
        const exactOutcome =
            outcome ??
            (trace?.outcome === "conversion-error"
                ? "conversion-error"
                : "capture-error");
        this.showError(
            message,
            revision,
            trace,
            exactOutcome,
            acceptedAtMonotonicMs,
        );
    }

    public clearPreview(revision: number, trace?: TimingTrace): void {
        if (
            !Number.isSafeInteger(revision) ||
            revision <= this.nextRevision ||
            revision < this.minimumRevision
        ) {
            return;
        }
        this.canvas.style.visibility = "hidden";
        this.supersedePendingRender();
        this.nextRevision = revision;
        const acceptedAtMonotonicMs = defaultClock.monotonicNow();
        const workingTrace =
            trace === undefined ? undefined : cloneTrace(trace);
        const previousInstance = this.instance;
        this.instance = undefined;
        this.renderedSource = undefined;
        this.diagnostics.hidden = true;
        this.diagnostics.textContent = "";
        delete this.diagnostics.dataset.severity;
        this.clearCanvas();
        this.setState("ready", revision);
        void this.finishClear(
            previousInstance,
            revision,
            workingTrace,
            acceptedAtMonotonicMs,
        );
    }

    private async finishClear(
        previousInstance: WrappedInstance | undefined,
        revision: number,
        trace: WorkingTrace | undefined,
        acceptedAtMonotonicMs: number,
    ): Promise<void> {
        if (previousInstance !== undefined) {
            try {
                await previousInstance.hide();
            } catch {
                // A concurrently superseded render may already have hidden it.
            } finally {
                this.freeInstance(previousInstance);
            }
        }
        if (!this.isCurrentRevision(revision)) {
            finishTrace(
                trace,
                "superseded",
                acceptedAtMonotonicMs,
                this.onTrace,
            );
            return;
        }
        const finished = finishTrace(
            trace,
            "cleared",
            acceptedAtMonotonicMs,
            this.onTrace,
        );
        this.dispatch("preview-cleared", revision, finished);
    }

    private async initializeInterpreter(
        source: string,
        revision: number,
        trace: TimingTrace | undefined,
        warnings: readonly Diagnostic[],
        acceptedAtMonotonicMs: number,
    ): Promise<void> {
        this.canvas.style.visibility = "hidden";
        this.setState("initializing", revision);
        this.nextRevision = Math.max(this.nextRevision, revision);
        if (
            this.pendingRender !== undefined &&
            this.pendingRender.revision <= revision
        ) {
            this.supersedePendingRender();
        }
        this.processing = true;
        const workingTrace =
            trace === undefined ? undefined : cloneTrace(trace);
        try {
            const initializationStart = defaultClock.monotonicNow();
            const wasmBytes = Uint8Array.from(
                atob(wasmDataUrl.split(",")[1]),
                (byte) => byte.charCodeAt(0),
            );
            await initialize({ module_or_path: wasmBytes });
            try {
                run_event_loop();
            } catch {
                // The browser event loop intentionally exits through an exception.
            }
            await this.initializeKeepalive();
            this.wasmInitializationDurationMs = Math.max(
                0,
                defaultClock.monotonicNow() - initializationStart,
            );
            if (workingTrace !== undefined) {
                workingTrace.phases.wasmInitialization =
                    this.wasmInitializationDurationMs;
                workingTrace.coldStart = true;
            }
            this.initialized = true;
            try {
                await this.renderSource(
                    source,
                    revision,
                    workingTrace,
                    warnings,
                    acceptedAtMonotonicMs,
                );
            } catch (error) {
                this.showError(
                    this.errorMessage(error),
                    revision,
                    workingTrace,
                    "compilation-error",
                    acceptedAtMonotonicMs,
                );
            }
        } catch (error) {
            // Initialization belongs to the runtime, not just the selection
            // which happened to start it. Fail the newest queued selection so
            // a rejected shared promise cannot strand its progress indicator.
            const pending = this.pendingRender;
            this.pendingRender = undefined;
            this.showError(
                `${this.errorMessage(error)}. Restart the plugin if retrying the selection does not recover.`,
                pending?.revision ?? revision,
                pending?.trace ?? workingTrace,
                "compilation-error",
                pending?.acceptedAtMonotonicMs ?? acceptedAtMonotonicMs,
            );
            throw error;
        } finally {
            this.processing = false;
        }
        if (this.initialized && this.pendingRender !== undefined) {
            await this.drainRenderQueue();
        }
    }

    private async drainRenderQueue(): Promise<void> {
        if (!this.initialized || this.processing) return;
        this.processing = true;
        try {
            while (this.pendingRender !== undefined) {
                const request = this.pendingRender;
                this.pendingRender = undefined;
                try {
                    await this.renderSource(
                        request.source,
                        request.revision,
                        request.trace,
                        request.warnings,
                        request.acceptedAtMonotonicMs,
                    );
                } catch (error) {
                    this.showError(
                        this.errorMessage(error),
                        request.revision,
                        request.trace,
                        "compilation-error",
                        request.acceptedAtMonotonicMs,
                    );
                }
            }
        } finally {
            this.processing = false;
        }
    }

    private async initializeKeepalive(): Promise<void> {
        if (
            this.keepaliveCanvas === undefined ||
            this.keepaliveInstance !== undefined
        )
            return;
        // Keep the event loop alive across clear/replacement without retaining
        // a second copy of a potentially large selected design.
        const result = await compile_from_string(
            `export component PreviewKeepalive inherits Window {
                width: 1px;
                height: 1px;
                background: transparent;
            }`,
            "",
            undefined,
        );
        let instance: WrappedInstance | undefined;
        try {
            const component = result.component;
            if (component === undefined)
                throw new Error("Could not compile preview keepalive");
            try {
                instance = await component.create(this.keepaliveCanvas.id);
                await instance.show();
                this.keepaliveInstance = instance;
                instance = undefined;
            } finally {
                if (instance !== undefined) this.freeInstance(instance);
                component.free();
            }
        } finally {
            result.free();
        }
    }

    private async renderSource(
        source: string,
        revision: number,
        trace: TimingTrace | undefined,
        warnings: readonly Diagnostic[],
        acceptedAtMonotonicMs: number,
    ): Promise<void> {
        const workingTrace =
            trace === undefined ? undefined : cloneTrace(trace);
        const renderStart = defaultClock.monotonicNow();
        if (!this.isCurrentRevision(revision)) {
            finishTrace(
                workingTrace,
                "superseded",
                acceptedAtMonotonicMs,
                this.onTrace,
            );
            return;
        }
        if (this.instance !== undefined && this.renderedSource === source) {
            this.showWarnings(warnings);
            this.setState("ready", revision);
            const finished = finishTrace(
                workingTrace,
                "unchanged",
                acceptedAtMonotonicMs,
                this.onTrace,
            );
            this.dispatch("preview-rendered", revision, finished);
            return;
        }
        if (workingTrace !== undefined) {
            const initializationMs = workingTrace.coldStart
                ? this.wasmInitializationDurationMs
                : 0;
            workingTrace.phases.previewQueue = Math.max(
                0,
                renderStart - acceptedAtMonotonicMs - initializationMs,
            );
        }
        this.setState("compiling", revision);
        const compilationStart = defaultClock.monotonicNow();
        const result = await compile_from_string(source, "", undefined);
        if (workingTrace !== undefined) {
            workingTrace.phases.slintCompilation = Math.max(
                0,
                defaultClock.monotonicNow() - compilationStart,
            );
        }
        try {
            if (!this.isCurrentRevision(revision)) {
                finishTrace(
                    workingTrace,
                    "superseded",
                    acceptedAtMonotonicMs,
                    this.onTrace,
                );
                return;
            }
            const error = result.error_string.trim();
            if (error !== "") {
                this.showError(
                    error,
                    revision,
                    workingTrace,
                    "compilation-error",
                    acceptedAtMonotonicMs,
                );
                return;
            }
            const component = result.component;
            if (component === undefined) {
                this.showError(
                    "Slint compiler did not return a component",
                    revision,
                    workingTrace,
                    "compilation-error",
                    acceptedAtMonotonicMs,
                );
                return;
            }
            let instance: WrappedInstance | undefined;
            try {
                if (!this.isCurrentRevision(revision)) {
                    finishTrace(
                        workingTrace,
                        "superseded",
                        acceptedAtMonotonicMs,
                        this.onTrace,
                    );
                    return;
                }
                const replacementStart = defaultClock.monotonicNow();
                const previousInstance = this.instance;
                // create_with_existing_window() consumes the JS wrapper that
                // it receives. Remove it from controller state before the
                // transfer so a superseding render can never reuse a moved
                // wrapper if this transition is discarded.
                this.instance = undefined;
                instance =
                    previousInstance === undefined
                        ? await component.create(this.canvas.id)
                        : await component.create_with_existing_window(
                              previousInstance,
                          );
                if (workingTrace !== undefined) {
                    workingTrace.phases.componentReplacement = Math.max(
                        0,
                        defaultClock.monotonicNow() - replacementStart,
                    );
                }
                const showStart = defaultClock.monotonicNow();
                await instance.show();
                this.canvasHasSlintContext = true;
                if (workingTrace !== undefined) {
                    workingTrace.phases.show = Math.max(
                        0,
                        defaultClock.monotonicNow() - showStart,
                    );
                }
                // Keep the completed transition as the valid handoff point
                // for the newest pending render, even if this revision is
                // already stale. The local variable is cleared because the
                // controller now owns the instance.
                this.instance = instance;
                this.renderedSource = source;
                instance = undefined;
                if (!this.isCurrentRevision(revision)) {
                    this.clearCanvas();
                    finishTrace(
                        workingTrace,
                        "superseded",
                        acceptedAtMonotonicMs,
                        this.onTrace,
                    );
                    return;
                }
                const presentationStart = defaultClock.monotonicNow();
                await waitForPresentation();
                if (workingTrace !== undefined) {
                    workingTrace.phases.presentation = Math.max(
                        0,
                        defaultClock.monotonicNow() - presentationStart,
                    );
                }
                if (!this.isCurrentRevision(revision)) {
                    this.clearCanvas();
                    finishTrace(
                        workingTrace,
                        "superseded",
                        acceptedAtMonotonicMs,
                        this.onTrace,
                    );
                    return;
                }
                this.showWarnings(warnings);
                this.setState("ready", revision);
                const finished = finishTrace(
                    workingTrace,
                    "rendered",
                    acceptedAtMonotonicMs,
                    this.onTrace,
                );
                this.dispatch("preview-rendered", revision, finished);
            } catch (error) {
                if (instance !== undefined) this.freeInstance(instance);
                throw error;
            } finally {
                component.free();
            }
        } finally {
            result.free();
        }
    }

    private setState(state: PreviewState, revision = this.nextRevision): void {
        if (revision < this.nextRevision) return;
        if (revision < this.minimumRevision) return;
        if (state === "ready" && this.instance !== undefined)
            this.canvas.style.visibility = "visible";
        this.status.dataset.state = state;
        this.status.dataset.revision = String(revision);
        this.status.textContent = state[0].toUpperCase() + state.slice(1);
    }

    private showError(
        message: string,
        revision = this.nextRevision,
        trace?: TimingTrace,
        outcome: RenderOutcome = "compilation-error",
        acceptedAtMonotonicMs = defaultClock.monotonicNow(),
    ): void {
        if (!this.isCurrentRevision(revision)) {
            finishTrace(
                trace === undefined ? undefined : cloneTrace(trace),
                "superseded",
                acceptedAtMonotonicMs,
                this.onTrace,
            );
            return;
        }
        // Failed output is never visible or reusable as a successful preview.
        this.canvas.style.visibility = "hidden";
        this.renderedSource = undefined;
        this.setState("error", revision);
        delete this.diagnostics.dataset.severity;
        this.diagnostics.hidden = false;
        this.diagnostics.textContent = message;
        const finished = finishTrace(
            trace === undefined ? undefined : cloneTrace(trace),
            outcome,
            acceptedAtMonotonicMs,
            this.onTrace,
        );
        this.dispatch("preview-error", revision, finished);
    }

    private showWarnings(warnings: readonly Diagnostic[]): void {
        if (warnings.length === 0) {
            this.diagnostics.hidden = true;
            this.diagnostics.textContent = "";
            delete this.diagnostics.dataset.severity;
            return;
        }
        this.diagnostics.hidden = false;
        this.diagnostics.dataset.severity = "warning";
        const details = document.createElement("details");
        const summary = document.createElement("summary");
        const counts = new Map<string, number>();
        const summaries = warningSummaries(warnings);
        for (const warning of summaries)
            counts.set(
                warning.category ?? "approximation",
                (counts.get(warning.category ?? "approximation") ?? 0) + 1,
            );
        summary.textContent = [...counts]
            .map(([kind, count]) => `${count} ${kind}${count === 1 ? "" : "s"}`)
            .join(" · ");
        details.append(summary);
        for (const item of summaries) {
            const entry = document.createElement("p");
            entry.textContent = `${item.code}: ${item.message}`;
            details.append(entry);
        }
        this.diagnostics.replaceChildren(details);
    }

    private supersedePendingRender(): void {
        const pending = this.pendingRender;
        this.pendingRender = undefined;
        if (pending !== undefined) {
            finishTrace(
                pending.trace === undefined
                    ? undefined
                    : cloneTrace(pending.trace),
                "superseded",
                pending.acceptedAtMonotonicMs,
                this.onTrace,
            );
        }
    }

    private clearCanvas(): void {
        // Calling getContext on a never-rendered canvas claims that context
        // type. Slint must be allowed to acquire WebGL when the first source
        // arrives after an initial empty-selection clear.
        if (!this.canvasHasSlintContext) return;
        // Slint normally owns a WebGL drawing buffer. Clear it in place so the
        // interpreter can bind the same canvas again after a new selection.
        const webgl =
            this.canvas.getContext("webgl2") ?? this.canvas.getContext("webgl");
        if (webgl !== null) {
            webgl.clearColor(0, 0, 0, 0);
            webgl.clear(
                webgl.COLOR_BUFFER_BIT |
                    webgl.DEPTH_BUFFER_BIT |
                    webgl.STENCIL_BUFFER_BIT,
            );
        }
    }

    private freeInstance(instance: WrappedInstance): void {
        if (this.freedInstances.has(instance)) return;
        this.freedInstances.add(instance);
        instance.free();
    }

    private isCurrentRevision(revision: number): boolean {
        return (
            revision === this.nextRevision && revision >= this.minimumRevision
        );
    }

    private errorMessage(error: unknown): string {
        return error instanceof Error ? error.message : String(error);
    }

    private dispatch(
        type: string,
        revision: number,
        trace?: TimingTrace,
    ): void {
        this.canvas.dispatchEvent(
            new CustomEvent(type, {
                detail: { revision, trace },
                bubbles: true,
            }),
        );
    }
}

function cloneTrace(trace: TimingTrace): WorkingTrace {
    return {
        ...trace,
        phases: { ...trace.phases },
        breakdowns: cloneTimingBreakdowns(trace.breakdowns),
        captureMetrics:
            trace.captureMetrics === undefined
                ? undefined
                : { ...trace.captureMetrics },
    };
}

function finishTrace(
    trace: WorkingTrace | undefined,
    outcome: TimingTrace["outcome"],
    acceptedAtMonotonicMs: number,
    onTrace: ((trace: TimingTrace) => void) | undefined,
): TimingTrace | undefined {
    if (trace === undefined) return undefined;
    const completedAtEpochMs = defaultClock.epochNow();
    trace.totalUiMs = Math.max(
        0,
        trace.uiReceivedAtEpochMs === undefined
            ? defaultClock.monotonicNow() - acceptedAtMonotonicMs
            : completedAtEpochMs - trace.uiReceivedAtEpochMs,
    );
    trace.outcome = outcome;
    trace.completedAtEpochMs = completedAtEpochMs;
    trace.totalMs = Math.max(0, completedAtEpochMs - trace.startedAtEpochMs);
    trace.phases.unattributedOverhead = deriveUnattributedOverhead(
        trace.phases,
        trace.totalMs,
    );
    const completed = trace as TimingTrace;
    onTrace?.(completed);
    return completed;
}

async function waitForPresentation(): Promise<void> {
    const nextFrame = (): Promise<void> =>
        typeof requestAnimationFrame === "function"
            ? new Promise((resolve) => requestAnimationFrame(() => resolve()))
            : new Promise((resolve) => setTimeout(resolve, 0));
    await nextFrame();
    await nextFrame();
}
