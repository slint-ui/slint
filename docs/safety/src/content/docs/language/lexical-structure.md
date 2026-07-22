---
title: Lexical Structure
description: Identifiers, contextual keywords, and element type names in the Slint language.
---

## Tokens

After whitespace and comments are removed (see [Source Files](../source-files/)), the remaining input is a sequence of tokens. {#sls.lex.tokens}

## Identifiers

An identifier consists of one or more characters drawn from the following classes: {#sls.lex.identifier.classes}

- The first character shall be a Unicode alphanumeric character or `U+005F` (LOW LINE, `_`).
- Each subsequent character shall be a Unicode alphanumeric character, `U+005F` (LOW LINE, `_`), or `U+002D` (HYPHEN-MINUS, `-`).

A `U+002D` (`-`) shall not appear as the first character of an identifier. {#sls.lex.identifier.no-leading-hyphen}

## Identifier Normalization

Two identifiers are considered the same if and only if their *normalized forms* are equal.
The normalized form of an identifier is obtained by applying the following replacements in order over the characters of the source identifier: {#sls.lex.identifier.normalization}

- The first character, if it is `U+005F` (`_`) or `U+002D` (`-`), is replaced with `U+005F` (`_`).
- Each subsequent `U+005F` (`_`) is replaced with `U+002D` (`-`).
- All other characters are left unchanged.

For example, `foo_bar` and `foo-bar` are the same identifier; `_foo_bar` and `-foo-bar` are the same identifier. {#sls.lex.identifier.normalization-example}

The kebab-case form (with `U+002D`) is the canonical written form. {#sls.lex.identifier.canonical-form}

## Contextual Keywords

Slint has no globally reserved words.
Each language construct is introduced by a *contextual keyword*: an identifier that has a special meaning only when it appears in a position where the grammar expects that construct. {#sls.lex.contextual-keywords}
