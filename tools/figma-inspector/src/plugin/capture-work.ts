// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export class CaptureCancelled extends Error {
    public constructor() {
        super("Capture cancelled");
    }
}

/** Session-only, bounded cache. Rejections and results from invalidated runs are not retained. */
export class CaptureCache {
    private entries = new Map<string, { value: unknown; size: number }>();
    private generation = 0;
    private pending = new Map<string, Promise<unknown>>();
    private retained = 0;
    public hits = 0;
    public constructor(private readonly budget = 32 * 1024 * 1024) {}
    public token(): number {
        return this.generation;
    }
    public valid(token: number): boolean {
        return token === this.generation;
    }
    public get retainedBytes(): number {
        return this.retained;
    }
    public clear(): void {
        this.generation++;
        this.entries.clear();
        this.pending.clear();
        this.retained = 0;
    }
    public async get<T>(
        key: string,
        load: () => Promise<T>,
        sizeOf: (value: T) => number,
        onHit?: () => void,
    ): Promise<T> {
        const generation = this.generation;
        const old = this.peek<T>(key, generation);
        if (old !== undefined) {
            onHit?.();
            return old;
        }
        const pending = this.pending.get(key);
        if (pending) {
            try {
                const value = (await pending) as T;
                this.hits++;
                onHit?.();
                return value;
            } catch (error) {
                // The older selection may have been cancelled while waiting for
                // an export slot. The current selection still needs its own load.
                if (error instanceof CaptureCancelled)
                    return this.get(key, load, sizeOf, onHit);
                throw error;
            }
        }
        // Publish before awaiting so concurrent captures share the host export.
        const operation = load();
        this.pending.set(key, operation);
        try {
            const value = await operation;
            this.put(key, value, sizeOf(value), generation);
            return value;
        } finally {
            // An invalidation may have installed a newer request for this key.
            if (this.pending.get(key) === operation) this.pending.delete(key);
        }
    }
    public peek<T>(key: string, token = this.generation): T | undefined {
        if (token !== this.generation) return undefined;
        const old = this.entries.get(key);
        if (!old) return undefined;
        this.hits++;
        this.entries.delete(key);
        this.entries.set(key, old);
        return old.value as T;
    }
    public put<T>(
        key: string,
        value: T,
        valueBytes: number,
        token = this.generation,
    ): boolean {
        const size = key.length * 2 + valueBytes;
        if (
            value === undefined ||
            token !== this.generation ||
            !Number.isFinite(size) ||
            size < 0 ||
            size > this.budget
        )
            return false;
        const replaced = this.entries.get(key);
        if (replaced) {
            this.entries.delete(key);
            this.retained -= replaced.size;
        }
        while (this.retained + size > this.budget && this.entries.size) {
            const oldest = this.entries.entries().next().value;
            if (!oldest) break;
            this.entries.delete(oldest[0]);
            this.retained -= oldest[1].size;
        }
        this.entries.set(key, { value, size });
        this.retained += size;
        return true;
    }
}

/** Limits host API work without changing the order of captured children. */
export function captureScheduler(limit: number) {
    let active = 0;
    const pending: (() => void)[] = [];
    return async <T>(operation: () => Promise<T>): Promise<T> => {
        if (active >= limit)
            await new Promise<void>((resolve) => pending.push(resolve));
        else active++;
        try {
            return await operation();
        } finally {
            const next = pending.shift();
            if (next) next();
            else active--;
        }
    };
}

export async function mapCaptureChildren<T, R>(
    items: readonly T[],
    limit: number,
    visit: (item: T) => Promise<R>,
    cancelled?: () => boolean,
): Promise<R[]> {
    const results: R[] = [];
    let next = 0;
    await Promise.all(
        Array.from({ length: Math.min(limit, items.length) }, async () => {
            while (next < items.length && !cancelled?.()) {
                const index = next++;
                results[index] = await visit(items[index]);
            }
        }),
    );
    return results;
}

/** Wall time with at least one active export; overlapping calls count once. */
export function captureBusyTime(now: () => number = Date.now) {
    let active = 0;
    let started = 0;
    let elapsed = 0;
    return {
        start(): () => void {
            if (active++ === 0) started = now();
            return () => {
                if (--active === 0) elapsed += Math.max(0, now() - started);
            };
        },
        duration(): number {
            return elapsed + (active ? Math.max(0, now() - started) : 0);
        },
    };
}
