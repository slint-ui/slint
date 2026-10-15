// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export const CAPTURE_TIMING_CATEGORIES = [
    "nodeReads",
    "encoding",
    "metadata",
    "componentLookup",
    "componentAssembly",
    "componentPlanning",
    "exportQueue",
    "bulkIndex",
    "bulkDispatch",
    "componentDiscovery",
    "bulkJson",
    "pngExports",
    "svgExports",
    "imageReads",
    "variableReads",
] as const;
export type CaptureTimingCategory = (typeof CAPTURE_TIMING_CATEGORIES)[number];
export type OperationTiming = {
    readonly count: number;
    readonly totalMs: number;
    readonly busyMs: number;
    readonly synchronousOverlapMs: number;
};
export type CaptureTimings = {
    readonly durationMs: number;
    readonly trackedMs: number;
    readonly bulkWorkerIndexMs?: number;
    readonly propertyReads?: Record<string, Record<string, number>>;
    readonly operations: Record<CaptureTimingCategory, OperationTiming>;
};

export class CaptureTimingCollector {
    private readonly started: number;
    private updated: number;
    private trackedMs = 0;
    private synchronous = 0;
    private readonly propertyReads = new Map<string, Record<string, number>>();

    public recordPropertyReads(type: string, keys: readonly string[]): void {
        let counts = this.propertyReads.get(type);
        if (!counts) {
            counts = Object.create(null) as Record<string, number>;
            this.propertyReads.set(type, counts);
        }
        for (const key of keys) counts[key] = (counts[key] ?? 0) + 1;
    }
    private readonly activeCategories = new Set<CaptureTimingCategory>();
    private readonly operations = Object.fromEntries(
        CAPTURE_TIMING_CATEGORIES.map((key) => [
            key,
            {
                count: 0,
                totalMs: 0,
                busyMs: 0,
                synchronousOverlapMs: 0,
                active: 0,
            },
        ]),
    ) as Record<
        CaptureTimingCategory,
        {
            count: number;
            totalMs: number;
            busyMs: number;
            synchronousOverlapMs: number;
            active: number;
        }
    >;

    public constructor(
        private readonly now: () => number = () =>
            typeof performance === "undefined" ? Date.now() : performance.now(),
    ) {
        this.started = this.updated = now();
    }

    private advance(): number {
        const now = this.now();
        const elapsed = Math.max(0, now - this.updated);
        let active = false;
        for (const category of this.activeCategories) {
            const operation = this.operations[category];
            operation.busyMs += elapsed;
            if (this.synchronous > 0) operation.synchronousOverlapMs += elapsed;
            active = true;
        }
        if (active) this.trackedMs += elapsed;
        this.updated = now;
        return now;
    }

    public start(category: CaptureTimingCategory): () => void {
        const started = this.advance();
        const operation = this.operations[category];
        operation.count++;
        operation.active++;
        this.activeCategories.add(category);
        return () => {
            const completed = this.advance();
            operation.totalMs += Math.max(0, completed - started);
            operation.active--;
            if (operation.active === 0) this.activeCategories.delete(category);
        };
    }

    public measure<T>(category: CaptureTimingCategory, read: () => T): T {
        const stop = this.start(category);
        this.synchronous++;
        try {
            return read();
        } finally {
            stop();
            this.synchronous--;
        }
    }

    public async measureAsync<T>(
        category: CaptureTimingCategory,
        operation: () => Promise<T>,
    ): Promise<T> {
        const stop = this.start(category);
        try {
            return await operation();
        } finally {
            stop();
        }
    }

    public snapshot(): CaptureTimings {
        this.advance();
        return {
            durationMs: Math.max(0, this.updated - this.started),
            trackedMs: this.trackedMs,
            ...(this.propertyReads.size
                ? {
                      propertyReads: Object.fromEntries(
                          [...this.propertyReads].map(([type, counts]) => [
                              type,
                              { ...counts },
                          ]),
                      ),
                  }
                : {}),
            operations: Object.fromEntries(
                CAPTURE_TIMING_CATEGORIES.map((key) => {
                    const { count, totalMs, busyMs, synchronousOverlapMs } =
                        this.operations[key];
                    return [
                        key,
                        { count, totalMs, busyMs, synchronousOverlapMs },
                    ];
                }),
            ) as CaptureTimings["operations"],
        };
    }
}

export function isCaptureTimings(value: unknown): value is CaptureTimings {
    if (!value || typeof value !== "object") return false;
    const candidate = value as CaptureTimings;
    const duration = (value: unknown): value is number =>
        typeof value === "number" && Number.isFinite(value) && value >= 0;
    if (
        !duration(candidate.durationMs) ||
        !duration(candidate.trackedMs) ||
        candidate.trackedMs > candidate.durationMs ||
        (candidate.bulkWorkerIndexMs !== undefined &&
            !duration(candidate.bulkWorkerIndexMs)) ||
        !candidate.operations
    )
        return false;
    if (
        candidate.propertyReads !== undefined &&
        (!candidate.propertyReads ||
            typeof candidate.propertyReads !== "object" ||
            Array.isArray(candidate.propertyReads) ||
            Object.values(candidate.propertyReads).some(
                (counts) =>
                    !counts ||
                    typeof counts !== "object" ||
                    Array.isArray(counts) ||
                    Object.values(counts).some(
                        (count) => !Number.isSafeInteger(count) || count <= 0,
                    ),
            ))
    )
        return false;
    return CAPTURE_TIMING_CATEGORIES.every((key) => {
        const operation = candidate.operations[key];
        return (
            !!operation &&
            Number.isSafeInteger(operation.count) &&
            operation.count >= 0 &&
            duration(operation.totalMs) &&
            duration(operation.busyMs) &&
            duration(operation.synchronousOverlapMs) &&
            operation.synchronousOverlapMs <= operation.busyMs &&
            operation.busyMs <= candidate.trackedMs
        );
    });
}
