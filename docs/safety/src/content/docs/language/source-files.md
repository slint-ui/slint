---
title: Source Files
description: Slint source file extension, encoding, line terminators, whitespace, and comments.
---

## File Extension

Slint source files use the file extension `.slint`. {#sls_file_extension}

## Encoding

A `.slint` source file shall be encoded as UTF-8. {#sls_encoding_utf8}

A UTF-8 byte order mark (`U+FEFF`) may appear at the start of the file and is ignored. {#sls_encoding_bom}

## Line Terminators

The compiler recognizes the character `U+000A` (LINE FEED) and the two-character sequence `U+000D U+000A` (CARRIAGE RETURN, LINE FEED) as equivalent line terminators. {#sls_line_terminators}

A `U+000D` (CARRIAGE RETURN) that is not immediately followed by `U+000A` is not a line terminator.
Source files should not contain bare carriage returns. {#sls_bare_cr}

## Whitespace

The whitespace characters are `U+0020` (SPACE), `U+0009` (CHARACTER TABULATION), and the line terminators defined in [Line Terminators](#line-terminators). {#sls_whitespace_chars}

Whitespace separates tokens but is otherwise insignificant; any sequence of one or more whitespace characters is equivalent to a single space. {#sls_whitespace_collapse}

## Comments

A line comment begins with two consecutive solidus characters `//` and extends to the next line terminator. {#sls_comment_line}

A block comment is delimited by `/*` and `*/`.
Block comments nest: an inner `/*` inside a block comment increments the nesting depth, and the comment terminates only when a matching `*/` returns the nesting depth to zero. {#sls_comment_block}

Comments are equivalent to whitespace; they have no other semantic effect. {#sls_comment_whitespace_equivalent}

The token sequences `///` and `/**` are recognized as ordinary line and block comments, respectively, and carry no special meaning. {#sls_comment_doc_unspecial}

```slint
// A line comment.

/* A block comment
   that spans multiple lines. */

/* Block comments /* may be nested */ like this. */
```
