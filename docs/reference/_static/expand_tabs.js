/*
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Based on code from stsewd her https://github.com/readthedocs/readthedocs.org/commit/738b6b2836a7e0cadad48e7f407fdeaf7ba7a1d7
*/

$(document).ready(function () {
    const tabName = location.hash.substring(1);
    if (tabName !== null) {
        const tab = $('[data-sync-id~="' + tabName + '"]');
        if (tab.length > 0) {
            tab.click();
        }
    }
});
