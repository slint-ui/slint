// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

slint!(
    import { SubType } from "./tests/typeloader/dependency_local.slint";
    import { AnotherType } from "dependency_from_incpath.slint";
    import { LibraryType } from "@library";

    export component Main {
        SubType {}
        AnotherType {}
        LibraryType {}
    }
);
