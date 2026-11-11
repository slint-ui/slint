# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

param([switch]$AllControls)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../../../..')).Path
$caseDirectory = Join-Path $repoRoot 'tests/cases/component_slots'
$casePrefix = 'reusable_editor_tabs_' + [guid]::NewGuid().ToString('N')
$createdCases = [System.Collections.Generic.List[string]]::new()
$previousFilter = $env:SLINT_TEST_FILTER

try {
    foreach ($fixture in Get-ChildItem -LiteralPath $PSScriptRoot -Filter '*.slint') {
        $casePath = Join-Path $caseDirectory ($casePrefix + '_' + $fixture.BaseName + '.slint')
        if (Test-Path -LiteralPath $casePath) {
            throw "Test adapter already exists: $casePath"
        }

        $source = [System.IO.File]::ReadAllText($fixture.FullName)
        $snippets = [regex]::Matches($source, '(?s)```rust\s*\r?\n.*?```')
        $importPath = '../../../tools/editor/controls/tests/' + $fixture.Name
        $adapter = 'export { TestCase } from "' + $importPath + '";' + "`n`n/*`n"
        $adapter += ($snippets | ForEach-Object { $_.Value }) -join "`n`n"
        $adapter += "`n*/`n"
        $createdCases.Add($casePath)
        [System.IO.File]::WriteAllText($casePath, $adapter, [System.Text.UTF8Encoding]::new($false))
    }

    $env:SLINT_TEST_FILTER = if ($AllControls) { 'reusable_' } else { $casePrefix }
    & cargo test --manifest-path (Join-Path $repoRoot 'tests/Cargo.toml') -p test-driver-rust --no-default-features --test component_slots
    if ($LASTEXITCODE -ne 0) {
        throw "Tab regression tests failed with exit code $LASTEXITCODE"
    }
} finally {
    $env:SLINT_TEST_FILTER = $previousFilter
    foreach ($casePath in $createdCases) {
        if (Test-Path -LiteralPath $casePath) {
            Remove-Item -LiteralPath $casePath
        }
    }
}
