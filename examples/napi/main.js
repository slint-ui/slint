// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { ComponentCompiler } from 'slint-ui';

let compiler = new ComponentCompiler();
let definition = compiler.buildFromPath("window.slint");
let instance = definition.create();
instance.run();