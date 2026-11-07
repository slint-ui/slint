// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Gallery;

using var app = App.Create();
GalleryLogic.Connect(app);
await app.RunAsync();
