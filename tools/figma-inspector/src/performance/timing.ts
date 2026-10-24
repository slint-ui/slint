// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { PreviewTrigger } from "../protocol";
import type { CaptureMetrics } from "../plugin/snapshot";

export const TIMING_PHASES = [
    "figmaCapture",
    "captureSerialization",
    "captureParse",
    "normalization",
    "workerQueueAndTransport",
    "jsonSerialization",
    "jsonParseValidation",
    "slintConversion",
    "assetPacking",
    "messageDispatch",
    "messageTransport",
    "assetUnpacking",
    "previewQueue",
    "wasmInitialization",
    "slintCompilation",
    "componentReplacement",
    "show",
    "presentation",
    "unattributedOverhead",
] as const;

export type TimingPhase = (typeof TIMING_PHASES)[number];
export type TraceOutcome =
    | "rendered"
    | "unchanged"
    | "superseded"
    | "capture-error"
    | "conversion-error"
    | "compilation-error"
    | "cleared";

type FontToImageConversionBreakdown = {
    readonly totalMs: number;
    readonly count: number;
};

export type TimingBreakdowns = {
    readonly fontToImageConversion: FontToImageConversionBreakdown | null;
};

export type TimingTrace = {
    readonly traceId: string;
    readonly revision: number;
    readonly trigger: PreviewTrigger;
    readonly startedAtEpochMs: number;
    readonly pluginSentAtEpochMs: number;
    readonly uiReceivedAtEpochMs?: number;
    readonly completedAtEpochMs?: number;
    readonly coldStart?: boolean;
    readonly outcome?: TraceOutcome;
    readonly captureMetrics?: CaptureMetrics;
    readonly phases: Record<TimingPhase, number | null>;
    readonly breakdowns: TimingBreakdowns;
    readonly totalPluginMs?: number;
    readonly totalUiMs?: number;
    readonly totalMs?: number;
};

export type Clock = {
    readonly monotonicNow: () => number;
    readonly epochNow: () => number;
};

export const defaultClock: Clock = {
    monotonicNow: () =>
        typeof performance !== "undefined" &&
        typeof performance.now === "function"
            ? performance.now()
            : Date.now(),
    epochNow: () => Date.now(),
};

function blankPhases(): Record<TimingPhase, number | null> {
    return Object.fromEntries(
        TIMING_PHASES.map((phase) => [phase, null]),
    ) as Record<TimingPhase, number | null>;
}

function nonNegativeDuration(start: number, end: number): number {
    const duration = end - start;
    return Number.isFinite(duration) && duration >= 0 ? duration : 0;
}

export function cloneTimingBreakdowns(
    breakdowns: TimingBreakdowns,
): TimingBreakdowns {
    const conversion = breakdowns.fontToImageConversion;
    return {
        fontToImageConversion: conversion === null ? null : { ...conversion },
    };
}

export function freezeTimingBreakdowns(
    breakdowns: TimingBreakdowns,
): TimingBreakdowns {
    const cloned = cloneTimingBreakdowns(breakdowns);
    return Object.freeze({
        fontToImageConversion:
            cloned.fontToImageConversion === null
                ? null
                : Object.freeze(cloned.fontToImageConversion),
    });
}

export class TimingTraceBuilder {
    public readonly startedAtEpochMs: number;
    private readonly startedAtMonotonicMs: number;
    private readonly values: Record<TimingPhase, number | null> = blankPhases();
    private readonly fontToImageConversion = {
        totalMs: 0,
        count: 0,
    };
    private captureMetrics: CaptureMetrics | undefined;

    public constructor(
        public readonly revision: number,
        public readonly trigger: PreviewTrigger,
        public readonly clock: Clock = defaultClock,
    ) {
        this.startedAtEpochMs = clock.epochNow();
        this.startedAtMonotonicMs = clock.monotonicNow();
    }

    public measure<T>(phase: TimingPhase, operation: () => T): T {
        const start = this.clock.monotonicNow();
        try {
            return operation();
        } finally {
            this.values[phase] = nonNegativeDuration(
                start,
                this.clock.monotonicNow(),
            );
        }
    }

    public async measureAsync<T>(
        phase: TimingPhase,
        operation: () => Promise<T>,
    ): Promise<T> {
        const start = this.clock.monotonicNow();
        try {
            return await operation();
        } finally {
            this.values[phase] = nonNegativeDuration(
                start,
                this.clock.monotonicNow(),
            );
        }
    }

    public async measureFontToImageConversion<T>(
        operation: () => Promise<T>,
    ): Promise<T> {
        const start = this.clock.monotonicNow();
        try {
            return await operation();
        } finally {
            this.fontToImageConversion.totalMs += nonNegativeDuration(
                start,
                this.clock.monotonicNow(),
            );
            this.fontToImageConversion.count += 1;
        }
    }

    public set(phase: TimingPhase, durationMs: number): void {
        this.values[phase] = nonNegativeDuration(0, durationMs);
    }

    public setCaptureMetrics(metrics: CaptureMetrics): void {
        if (!isCaptureMetrics(metrics))
            throw new Error("Invalid capture metrics");
        this.captureMetrics = { ...metrics };
    }

    public toTrace(
        outcome?: TraceOutcome,
        completedAtEpochMs?: number,
    ): TimingTrace {
        const completed = completedAtEpochMs ?? this.clock.epochNow();
        return {
            traceId: String(this.revision),
            revision: this.revision,
            trigger: this.trigger,
            startedAtEpochMs: this.startedAtEpochMs,
            pluginSentAtEpochMs: this.clock.epochNow(),
            completedAtEpochMs: completed,
            outcome,
            captureMetrics:
                this.captureMetrics === undefined
                    ? undefined
                    : { ...this.captureMetrics },
            phases: { ...this.values },
            breakdowns: freezeTimingBreakdowns({
                fontToImageConversion:
                    this.fontToImageConversion.count === 0
                        ? null
                        : { ...this.fontToImageConversion },
            }),
            totalPluginMs: nonNegativeDuration(
                this.startedAtMonotonicMs,
                this.clock.monotonicNow(),
            ),
        };
    }
}

/** Account for wall-clock time not covered by the named phases. */
export function deriveUnattributedOverhead(
    phases: Record<TimingPhase, number | null>,
    totalMs: number,
): number {
    const attributed = TIMING_PHASES.reduce(
        (sum, phase) =>
            phase === "unattributedOverhead" ? sum : sum + (phases[phase] ?? 0),
        0,
    );
    return Math.max(0, totalMs - attributed);
}

/** Time from the pre-dispatch wire timestamp to UI receipt, including dispatch. */
export function transportDuration(
    trace: TimingTrace,
    receivedAtEpochMs: number,
): number {
    return Math.max(0, receivedAtEpochMs - trace.pluginSentAtEpochMs);
}

/** Split the UI's inclusive bridge interval once the dispatch duration is known. */
export function finalizeSandboxTiming(
    uiTrace: TimingTrace,
    sandboxTrace: TimingTrace,
): TimingTrace {
    const dispatch = sandboxTrace.phases.messageDispatch;
    const inclusiveTransport = uiTrace.phases.messageTransport;
    const phases = {
        ...uiTrace.phases,
        messageDispatch: dispatch,
        // Dispatch starts at the wire timestamp, so it is already included
        // in the provisional transport phase. Final phases must not overlap.
        messageTransport:
            inclusiveTransport === null
                ? null
                : Math.max(0, inclusiveTransport - (dispatch ?? 0)),
    };
    if (uiTrace.totalMs !== undefined)
        phases.unattributedOverhead = deriveUnattributedOverhead(
            phases,
            uiTrace.totalMs,
        );
    return {
        ...uiTrace,
        pluginSentAtEpochMs: sandboxTrace.pluginSentAtEpochMs,
        totalPluginMs: sandboxTrace.totalPluginMs,
        phases,
        breakdowns: freezeTimingBreakdowns(sandboxTrace.breakdowns),
    };
}

function isFontToImageConversionBreakdown(
    value: unknown,
): value is FontToImageConversionBreakdown | null {
    if (value === null) return true;
    if (typeof value !== "object") return false;
    const candidate = value as Record<string, unknown>;
    return (
        typeof candidate.totalMs === "number" &&
        Number.isFinite(candidate.totalMs) &&
        candidate.totalMs >= 0 &&
        typeof candidate.count === "number" &&
        Number.isSafeInteger(candidate.count) &&
        candidate.count > 0
    );
}

export function isTimingTrace(value: unknown): value is TimingTrace {
    if (typeof value !== "object" || value === null) return false;
    const candidate = value as Record<string, unknown>;
    if (
        typeof candidate.traceId !== "string" ||
        typeof candidate.revision !== "number" ||
        !Number.isSafeInteger(candidate.revision) ||
        typeof candidate.trigger !== "string" ||
        typeof candidate.startedAtEpochMs !== "number" ||
        !Number.isFinite(candidate.startedAtEpochMs) ||
        typeof candidate.pluginSentAtEpochMs !== "number" ||
        !Number.isFinite(candidate.pluginSentAtEpochMs) ||
        typeof candidate.phases !== "object" ||
        candidate.phases === null ||
        typeof candidate.breakdowns !== "object" ||
        candidate.breakdowns === null ||
        !("fontToImageConversion" in candidate.breakdowns)
    ) {
        return false;
    }
    const breakdowns = candidate.breakdowns as Record<string, unknown>;
    if (!isFontToImageConversionBreakdown(breakdowns.fontToImageConversion))
        return false;
    if (
        candidate.trigger !== "initial" &&
        candidate.trigger !== "selection-change" &&
        candidate.trigger !== "node-change" &&
        candidate.trigger !== "pin-change" &&
        candidate.trigger !== "density-change"
    ) {
        return false;
    }
    if (
        candidate.outcome !== undefined &&
        candidate.outcome !== "rendered" &&
        candidate.outcome !== "unchanged" &&
        candidate.outcome !== "superseded" &&
        candidate.outcome !== "capture-error" &&
        candidate.outcome !== "conversion-error" &&
        candidate.outcome !== "compilation-error" &&
        candidate.outcome !== "cleared"
    ) {
        return false;
    }
    for (const key of [
        "uiReceivedAtEpochMs",
        "completedAtEpochMs",
        "totalPluginMs",
        "totalUiMs",
        "totalMs",
    ] as const) {
        if (
            candidate[key] !== undefined &&
            (typeof candidate[key] !== "number" ||
                !Number.isFinite(candidate[key]) ||
                candidate[key] < 0)
        ) {
            return false;
        }
    }
    const phases = candidate.phases as Record<string, unknown>;
    return (
        (candidate.captureMetrics === undefined ||
            isCaptureMetrics(candidate.captureMetrics)) &&
        TIMING_PHASES.every((phase) => {
            const duration = phases[phase];
            return (
                duration === null ||
                (typeof duration === "number" &&
                    Number.isFinite(duration) &&
                    duration >= 0)
            );
        })
    );
}

export function isCaptureMetrics(value: unknown): value is CaptureMetrics {
    if (typeof value !== "object" || value === null) return false;
    const candidate = value as Record<string, unknown>;
    const counts = ["requests", "exports", "cacheHits"] as const;
    if (
        typeof candidate.durationMs !== "number" ||
        !Number.isFinite(candidate.durationMs) ||
        candidate.durationMs < 0 ||
        counts.some(
            (key) =>
                typeof candidate[key] !== "number" ||
                !Number.isSafeInteger(candidate[key]) ||
                candidate[key] < 0,
        )
    )
        return false;
    if (candidate.work !== undefined) {
        if (typeof candidate.work !== "object" || candidate.work === null)
            return false;
        const work = candidate.work as Record<string, unknown>;
        for (const key of [
            "capturedNodes",
            "componentFamilies",
            "componentVariants",
            "pngExports",
            "svgExports",
            "textCacheHits",
        ]) {
            if (
                typeof work[key] !== "number" ||
                !Number.isSafeInteger(work[key]) ||
                work[key] < 0
            )
                return false;
        }
    }
    const requests = candidate.requests as number;
    const exports = candidate.exports as number;
    const cacheHits = candidate.cacheHits as number;
    return requests === exports + cacheHits;
}
