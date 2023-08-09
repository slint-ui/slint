// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// Test that the Slint event loop processes libuv's events.

import test from 'ava'
import http from 'http';
import fetch from "node-fetch";

import { runEventLoop, ComponentCompiler, runEventLoopWithCallback, quitEventLoop } from '../index'
/*
test('merged event loops with timer', (t) => {

    setTimeout(() => {
        quitEventLoop();
    }, 0);

    runEventLoop();
})
*/
test('merged event loops with network', (t) => {
    const port = 8000;
    const host = "localhost";

    let received_response = "";

    runEventLoopWithCallback(() => {
        console.log("start");
        const listener = (request, result) => {
            console.log("in http server handler");
            result.writeHead(200);
            result.end("Hello World");
        };

        const server = http.createServer(listener);
        server.listen(port, host, () => {
            console.log(`server ready at ${host}:${port}`);

            console.log("initiating fetch");
            fetch(`http://${host}:${port}/`).then(async (response) => {
                return response.text();
            }).then((text) => {
                console.log(`retrieved ${text}`);
                received_response = text;
                quitEventLoop();
                server.close();
            });
        });
        console.log("listen called");
    });

    t.is(received_response, "Hello World");
})
