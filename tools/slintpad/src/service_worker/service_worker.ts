// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

/// <reference lib="WebWorker" />
//

const sw = self as ServiceWorkerGlobalScope & typeof globalThis;

const known_clients: [string, MessagePort][] = [];

function find_prefix(url: string): [string, MessagePort | null, number] {
    for (const [index, val] of known_clients.entries()) {
        if (url.startsWith(val[0])) {
            return [val[0], val[1], index];
        }
    }
    return ["", null, -1];
}

function resolve(port: MessagePort, url: string): Promise<string> {
    return new Promise((resolve) => {
        const channel = new MessageChannel();
        channel.port1.onmessage = (m: MessageEvent) => {
            channel.port1.close();
            resolve(m.data);
        };
        port.postMessage({ type: "MapUrl", url: url }, [channel.port2]);
    });
}

sw.addEventListener("install", (_event) => {
    sw.skipWaiting();
});

sw.addEventListener("activate", (event) => {
    event.waitUntil(sw.clients.claim());
});

sw.addEventListener("message", (event: ExtendableMessageEvent) => {
    if (event.data.type === "EditorOpened") {
        const prefix = event.data.url_prefix;
        if (prefix != null) {
            known_clients.push([prefix, event.ports[0]]);
        }
    } else {
        console.error("Service Worker: Got an unexpected message", event.data);
    }
});

sw.addEventListener("fetch", (event: FetchEvent) => {
    const url = event.request.url;

    const [_, port, index] = find_prefix(url);

    if (port != null) {
        const promises = resolve(port, url)
            .then((new_url) => fetch(new_url))
            .catch((e) => {
                delete known_clients[index];
                throw e;
            });
        event.respondWith(promises);
    }
});
