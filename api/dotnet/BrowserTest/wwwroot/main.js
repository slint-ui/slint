// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { dotnet } from "./_framework/dotnet.js";

const params = new URLSearchParams(location.search);
let runtime = dotnet;
// ?renderer=software picks Slint's software renderer instead of FemtoVG on WebGL.
// Slint reads Emscripten's environment, which only this sets; Environment.SetEnvironmentVariable in C# doesn't.
if (params.has("renderer")) {
    runtime = runtime.withEnvironmentVariable("SLINT_BACKEND", `emscripten-${params.get("renderer")}`);
}
await runtime.run();
