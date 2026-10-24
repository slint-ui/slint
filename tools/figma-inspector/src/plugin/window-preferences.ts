// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export interface WindowSize {
    width: number;
    height: number;
}

const storageKey = "preview-window-size-v1";
const defaultSize: WindowSize = { width: 640, height: 640 };

export function isWindowSize(value: unknown): value is WindowSize {
    if (typeof value !== "object" || value === null) return false;
    const size = value as Partial<WindowSize>;
    return (
        typeof size.width === "number" &&
        Number.isSafeInteger(size.width) &&
        typeof size.height === "number" &&
        Number.isSafeInteger(size.height) &&
        size.width >= 500 &&
        size.width <= 4096 &&
        size.height >= 480 &&
        size.height <= 4096
    );
}

export class WindowPreferences {
    private pending: Promise<void> = Promise.resolve();
    constructor(
        private readonly storage: Pick<
            ClientStorageAPI,
            "getAsync" | "setAsync"
        >,
    ) {}

    async load(): Promise<WindowSize> {
        try {
            const size: unknown = await this.storage.getAsync(storageKey);
            if (isWindowSize(size))
                return { width: size.width, height: size.height };
        } catch {
            /* Storage failure must not prevent the preview from opening. */
        }
        return { ...defaultSize };
    }

    save(size: WindowSize): void {
        if (!isWindowSize(size)) return;
        const stored = { width: size.width, height: size.height };
        this.pending = this.pending
            .then(() => this.storage.setAsync(storageKey, stored))
            .catch(() => {});
    }
}
