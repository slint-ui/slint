// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// Test that the Slint event loop processes libuv's events.

import test from 'ava'
import * as http from 'http';
import fetch from "node-fetch";

import { run_event_loop, quit_event_loop, private_api } from '../index'


test.serial('merged event loops with timer', async (t) => {

    let invoked = false;

    await run_event_loop(() => {

        setTimeout(() => {
            invoked = true;
            quit_event_loop();
        }, 2);
    });
    t.true(invoked)
})


test.serial('merged event loops with networking', async (t) => {
    const listener = (request, result) => {
        result.writeHead(200);
        result.end("Hello World");
    };

    let received_response = "";

    await run_event_loop(() => {

        const server = http.createServer(listener);
        server.listen(async () => {
            let host = "localhost";
            let port = (server.address() as any).port;
            console.log(`server ready at ${host}:${port}`);

            fetch(`http://${host}:${port}/`).then(async (response) => {
                return response.text();
            }).then((text) => {
                received_response = text;
                //console.log("received ", text);
                quit_event_loop();
                server.close();
            });

        });

    })

    t.is(received_response, "Hello World");
})

test.serial('quit event loop on last window closed', async (t) => {
    let compiler = new private_api.ComponentCompiler;
    let definition = compiler.buildFromSource(`

    export component App inherits Window {
        width: 300px;
        height: 300px;
    }`, "");
    t.not(definition, null);

    let instance = definition!.create() as any;
    t.not(instance, null);

    instance.window().show();
    await run_event_loop(() => {
        setTimeout(() => {
            instance.window().hide();
        }, 2);
    });

})
