---
title: Source Files
description: Slint source file extension, encoding, line terminators, whitespace, and comments.
---

## File Extension

Slint source files use the file extension `.slint`. \{#sls.source.file-extension}

## Encoding

A `.slint` source file shall be encoded as UTF-8. \{#sls.source.encoding.utf8}

A UTF-8 byte order mark (`U+FEFF`) may appear at the start of the file and is ignored. \{#sls.source.encoding.bom}

## Line Terminators

The compiler recognizes the character `U+000A` (LINE FEED) and the two-character sequence `U+000D U+000A` (CARRIAGE RETURN, LINE FEED) as equivalent line terminators. \{#sls.source.line-terminators}

A `U+000D` (CARRIAGE RETURN) that is not immediately followed by `U+000A` is not a line terminator.
Source files should not contain bare carriage returns. \{#sls.source.bare-cr}

## Whitespace

The whitespace characters are `U+0020` (SPACE), `U+0009` (CHARACTER TABULATION), and the line terminators defined in [Line Terminators](#line-terminators). \{#sls.source.whitespace.chars}

Whitespace separates tokens but is otherwise insignificant; any sequence of one or more whitespace characters is equivalent to a single space. \{#sls.source.whitespace.collapse}

## Comments

A line comment begins with two consecutive solidus characters `//` and extends to the next line terminator. \{#sls.source.comment.line}

A block comment is delimited by `/*` and `*/`.
Block comments nest: an inner `/*` inside a block comment increments the nesting depth, and the comment terminates only when a matching `*/` returns the nesting depth to zero. \{#sls.source.comment.block}

Comments are equivalent to whitespace; they have no other semantic effect. \{#sls.source.comment.whitespace-equivalent}

The token sequences `///` and `/**` are recognized as ordinary line and block comments, respectively, and carry no special meaning. \{#sls.source.comment.no-doc-comments}

```slint
// A line comment.

/* A block comment
   that spans multiple lines. */

/* Block comments /* may be nested */ like this. */
```
