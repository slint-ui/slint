/*
 * Expands a specific tab of sphinx-tabs.
 * Usage:
 * - docs.readthedocs.io/?tab=Name
 * - docs.readthedocs.io/?tab=Name#section
 * Where 'Name' is the title of the tab (case sensitive).
 */

// Thanks to https://github.com/readthedocs/readthedocs.org/commit/738b6b2836a7e0cadad48e7f407fdeaf7ba7a1d7

$(document).ready(function () {
    const tabName = location.hash.substring(1);
    if (tabName !== null) {
        const tab = $('[data-sync-id~="' + tabName + '"]');
        if (tab.length > 0) {
            tab.click();
        }
    }
});
