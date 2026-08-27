#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

function init_virtual_keyboard(mainWindow: any) {
    const window = mainWindow.window as slint.Window;

    mainWindow.VirtualKeyboardHandler.key_pressed = (key: string) => {
        window.dispatchEvent({ type: "key-pressed", text: key });
        window.dispatchEvent({ type: "key-released", text: key });
    };
}

const ui = slint.loadFile(
    new URL("../ui/main_window.slint", import.meta.url),
) as any;
const mainWindow = new ui.MainWindow();

init_virtual_keyboard(mainWindow);

await mainWindow.run();
