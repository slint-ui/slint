// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Todo;

using var window = MainWindow.Create();
TodoLogic.Connect(window);
window.ShowHeader = true;
// The browser's thread can't block, so Run would throw.
await window.RunAsync();
