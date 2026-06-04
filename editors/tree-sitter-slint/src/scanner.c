// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "tree_sitter/parser.h"

enum TokenType {
    BLOCK_COMMENT,
};

void *tree_sitter_slint_external_scanner_create(void)
{
    return NULL;
}
void tree_sitter_slint_external_scanner_destroy(UNUSED void *payload) { }
unsigned tree_sitter_slint_external_scanner_serialize(UNUSED void *payload, UNUSED char *buffer)
{
    return 0;
}
void tree_sitter_slint_external_scanner_deserialize(UNUSED void *payload, UNUSED const char *buffer,
                                                    UNUSED unsigned length)
{
}

// We need to provide a custom scanner for the TreeSitter Slint grammar because the block comments
// in Slint support nesting. However comments in TreeSitter are represented as extras. Which are
// created at the scanner/lexer phase and not at the parsing phase. So they can only simple
// regex-based tokens which cannot express the nesting in Slint's block comments. This can be solved
// by a custom scanner, which is also what languages like OCaml and Rust do for their tree-sitter
// grammars.
bool tree_sitter_slint_external_scanner_scan(UNUSED void *payload, TSLexer *lexer,
                                             const bool *valid_symbols)
{
    if (!valid_symbols[BLOCK_COMMENT]) {
        return false;
    }

    // Skip whitespace before attempting to match a block comment.
    // tree-sitter may call the external scanner before or alongside
    // whitespace skipping, so we handle it here for robustness.
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t' || lexer->lookahead == '\n'
           || lexer->lookahead == '\r') {
        lexer->advance(lexer, true); // true = skip, don't include in token
    }

    // Must start with /*
    if (lexer->lookahead != '/')
        return false;
    lexer->advance(lexer, false);
    if (lexer->lookahead != '*')
        return false;
    lexer->advance(lexer, false);

    // Track nesting depth; we've already consumed the opening /*
    unsigned depth = 1;

    while (!lexer->eof(lexer)) {
        if (lexer->lookahead == '/') {
            lexer->advance(lexer, false);
            if (lexer->lookahead == '*') {
                lexer->advance(lexer, false);
                depth++;
            }
        } else if (lexer->lookahead == '*') {
            lexer->advance(lexer, false);
            if (lexer->lookahead == '/') {
                lexer->advance(lexer, false);
                depth--;
                if (depth == 0) {
                    lexer->result_symbol = BLOCK_COMMENT;
                    lexer->mark_end(lexer);
                    return true;
                }
            }
        } else {
            lexer->advance(lexer, false);
        }
    }

    // Unterminated comment
    return false;
}
