---
title: Exports
description: Making components in a file available to other files.
---

## Placement

Export statements may appear at the top level of a `.slint` source file, interleaved with [component definitions](../file-structure/#sls.file.component.definition-forms). \{#sls.export.placement}

## Privacy

Names defined in a `.slint` source file are private to that file by default.
A name is visible to importers only if it is explicitly exported. \{#sls.export.default-private}

## Form

An export statement has one of the following forms: \{#sls.export.forms}

```slint no-test
export component Name { /* ... */ }
export { Name, ... }
```

## Export at the Declaration Site

The form `export component Name { ... }` defines a component and exports it in a single statement.
The exported name is the component's own name. \{#sls.export.declaration-site}

## Export Lists

The form `export { Name, ... }` exports one or more names already defined in the current file.
A trailing comma is permitted. \{#sls.export.list}

An export list item is either a bare identifier `Name` or a renaming of the form `Name as Other`.
A renaming exports the locally-defined `Name` under the external name `Other`.
The local name `Name` remains defined in the current file. \{#sls.export.rename}

Each identifier on the left of `as`, and each bare identifier, shall refer to a name defined in the current file. \{#sls.export.left-must-exist}

## Duplicate Exports

A source file shall not export the same external name more than once.
This includes the combination of any declaration-site export and any export-list entry. \{#sls.export.no-duplicates}
