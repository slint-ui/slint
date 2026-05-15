// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Test that the Slint event loop processes libuv's events.

import { test, expect, afterEach } from "vitest";
import * as http from "node:http";

import {
    loadSource,
    runEventLoop,
    quitEventLoop,
    private_api,
} from "../dist/index.js";
import { hasIntegratedEventLoop } from "../rust-module.cjs";

afterEach(() => {
    quitEventLoop();
});

test.sequential("merged event loops with timer", async () => {
    let invoked = false;

    await runEventLoop(() => {
        setTimeout(() => {
            invoked = true;
            quitEventLoop();
        }, 2);
    });
    expect(invoked).toBe(true);
});

test.sequential("merged event loops with networking", async () => {
    const listener = (request, result) => {
        result.writeHead(200);
        result.end("Hello World");
    };

    let received_response = "";

    await runEventLoop(() => {
        const server = http.createServer(listener);
        server.listen(() => {
            const host = "localhost";
            const port = (server.address() as any).port;
            console.log(`server ready at ${host}:${port}`);

            (fetch as any)(`http://${host}:${port}/`)
                .then((response: Response) => {
                    return response.text();
                })
                .then((text: string) => {
                    received_response = text;
                    //console.log("received ", text);
                    quitEventLoop();
                    server.close();
                });
        });
    });

    expect(received_response).toBe("Hello World");
});

test.sequential("event loop restart", async () => {
    let first_run = false;
    let second_run = false;

    await runEventLoop(() => {
        setTimeout(() => {
            first_run = true;
            quitEventLoop();
        }, 2);
    });
    expect(first_run).toBe(true);

    await runEventLoop(() => {
        setTimeout(() => {
            second_run = true;
            quitEventLoop();
        }, 2);
    });
    expect(second_run).toBe(true);
});

test.sequential("set property from JS timer mid-run", async () => {
    const ui = loadSource(
        `export component App inherits Window {
            in-out property <string> label: "initial";
        }`,
        "test.slint",
    ) as any;
    const app = new ui.App();
    app.show();

    await runEventLoop(() => {
        setTimeout(() => {
            app.label = "updated";
            quitEventLoop();
        }, 2);
    });
    expect(app.label).toBe("updated");
});

test.sequential("slint timer fires through integrated event loop", async () => {
    const ui = loadSource(
        `export component App inherits Window {
            in-out property <int> counter: 0;
            timer := Timer {
                interval: 50ms;
                triggered => { counter += 1; }
            }
        }`,
        "test.slint",
    ) as any;
    const app = new ui.App();
    app.show();

    await runEventLoop(() => {
        setTimeout(() => {
            expect(app.counter).toBeGreaterThanOrEqual(1);
            quitEventLoop();
        }, 200);
    });
});

test.sequential("js and slint timers fire in order", async () => {
    const events: string[] = [];

    const ui = loadSource(
        `export component App inherits Window {
            callback timer_fired();
            timer := Timer {
                interval: 80ms;
                triggered => { timer_fired(); }
            }
        }`,
        "test.slint",
    ) as any;
    const app = new ui.App();
    app.timer_fired = () => events.push("slint");
    app.show();

    // JS timers are placed midway between 80ms Slint firings
    // (80, 160, 240) so there's ≥40ms margin on each side.
    await runEventLoop(() => {
        setTimeout(() => events.push("js-120"), 120);
        setTimeout(() => events.push("js-200"), 200);
        setTimeout(() => {
            events.push("js-280");
            quitEventLoop();
        }, 280);
    });

    if (hasIntegratedEventLoop()) {
        // With the integrated event loop, Slint and JS timers
        // interleave precisely.
        expect(events).toEqual([
            "slint",
            "js-120",
            "slint",
            "js-200",
            "slint",
            "js-280",
        ]);
    } else {
        // Polling fallback (Windows / Deno): ordering between Slint
        // and JS timers isn't guaranteed, just check both fired.
        expect(
            events.filter((e) => e === "slint").length,
        ).toBeGreaterThanOrEqual(1);
        expect(events).toContain("js-120");
        expect(events).toContain("js-200");
        expect(events).toContain("js-280");
        expect(events.indexOf("js-120")).toBeLessThan(events.indexOf("js-200"));
        expect(events.indexOf("js-200")).toBeLessThan(events.indexOf("js-280"));
    }
});
