// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Test that the Slint event loop processes libuv's events.

import test from "ava";
import * as http from "node:http";

import { runEventLoop, quitEventLoop, private_api } from "../dist/index.js";

test.serial("merged event loops with timer", async (t) => {
    let invoked = false;

    await runEventLoop(() => {
        setTimeout(() => {
            invoked = true;
            quitEventLoop();
        }, 2);
    });
    t.true(invoked);
});

test.serial("merged event loops with networking", async (t) => {
    const listener = (request, result) => {
        result.writeHead(200);
        result.end("Hello World");
    };

    let received_response = "";

    await runEventLoop(() => {
        const server = http.createServer(listener);
        server.listen(async () => {
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

    t.is(received_response, "Hello World");
});

test.serial(
    "quit event loop on last window closed with callback",
    async (t) => {
        const compiler = new private_api.ComponentCompiler();
        const definition = compiler.buildFromSource(
            `

    export component App inherits Window {
        width: 300px;
        height: 300px;
    }`,
            "",
        );
        t.not(definition.App, null);

        const instance = definition.App!.create() as any;
        t.not(instance, null);

        instance.window().show();
        await runEventLoop(() => {
            setTimeout(() => {
                instance.window().hide();
            }, 2);
        });
    },
);
