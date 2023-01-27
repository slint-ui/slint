// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

module.exports = grammar({
    name: "slint",

    extras: ($) => [/[\s\r\n]+/, $.comment],
    conflicts: ($) => [
        [$._expression, $.if_statement],
        [$._assignment_value_block],
        [$.block, $.block_statement],
        [$.block_statement, $._expression_body],
        [$.function_identifier, $.post_identifier],
        [$.function_identifier, $.var_identifier],
        [$.assignment_block],
    ],

    rules: {
        source_file: ($) => repeat($._definition),

        _definition: ($) =>
            choice(
                $.import_statement,
                $.struct_definition,
                $.global_definition,
                $.component_definition,
            ),

        import_statement: ($) =>
            seq(
                "import",
                optional(seq("{", commaSep($.type_identifier), "}", "from")),
                $.string_value,
                ";",
            ),

        export_modifier: (_) => "export",

        component: ($) =>
            seq(
                optional(seq(field("id", $.var_identifier), ":=")),
                field("type", $.type_identifier),
                $.block,
            ),

        component_definition: ($) =>
            choice(
                seq(
                    // new syntax
                    field("export", optional($.export_modifier)),
                    "component",
                    field("name", $.type_identifier),
                    optional(
                        seq("inherits", field("base_type", $.type_identifier)),
                    ),
                    $.block,
                ),
                seq(
                    // old syntax
                    field("export", optional($.export_modifier)),
                    field("name", $.type_identifier),
                    ":=",
                    field("base_type", $.type_identifier),
                    $.block,
                ),
            ),

        property_alias: ($) =>
            seq(
                field("visibility", optional($.visibility_modifier)),
                optional("property"),
                optional(
                    seq(
                        "<",
                        field("type", choice($.type_identifier, $.type_list)),
                        ">",
                    ),
                ),
                field("name", $.var_identifier),
                field("binding_op", "<=>"),
                field("binding", $.var_identifier),
                ";",
            ),

        property: ($) =>
            seq(
                field("visibility", optional($.visibility_modifier)),
                "property",
                "<",
                field("type", choice($.type_identifier, $.type_list)),
                ">",
                field("name", $.var_identifier),
                choice(
                    optional(
                        seq(
                            field("binding_op", ":"),
                            field(
                                "binding",
                                choice(
                                    seq($.value_list, ";"),
                                    seq($.block, optional(";")),
                                    seq($._expression, ";"),
                                ),
                            ),
                        ),
                    ),
                    ";",
                ),
            ),

        binding: ($) =>
            seq(
                field("name", $.var_identifier),
                ":",
                field("expression", $._expression),
                ";",
            ),

        global_definition: ($) =>
            seq(
                field("export", optional($.export_modifier)),
                "global",
                field("name", $.type_identifier),
                optional(":="), // old syntax!
                "{",
                commaSep(choice($.property, $.callback)),
                optional(","),
                "}",
            ),

        struct_definition: ($) =>
            seq(
                field("export", optional($.export_modifier)),
                "struct",
                field("name", $.type_identifier),
                optional(":="), // old syntax!
                $.type_anon_struct,
            ),

        anon_struct: ($) =>
            seq(
                "{",
                commaSep(seq($.var_identifier, ":", $._expression)),
                optional(","),
                "}",
            ),

        block: ($) =>
            choice(
                seq(
                    "{",
                    repeat($.block_statement),
                    optional(
                        seq(
                            choice(
                                $._expression,
                                $.assignment_block,
                                $.assignment_expr,
                            ),
                            optional(";"),
                        ),
                    ), // "return value"
                    "}",
                ),
            ),

        block_statement: ($) =>
            choice(
                $.for_loop,
                $.if_statement,
                $.animate_statement,
                seq($.assignment_block, optional(";")),
                seq($.assignment_expr, ";"),
                seq("return", $._expression, ";"),
                $.component,
                $.property_alias,
                $.property,
                $.binding,
                $.callback,
                $.callback_event,
                $.callback_alias,
                $.function_call,
                seq($.var_identifier, ";"),
                // $.states_definition,
                // $.transitions_definition,
            ),

        // transitions_definition: ($) =>
        //   seq(
        //     "transitions",
        //     $.transitions_list_definition,
        //   ),
        // transitions_list_definition: ($) =>
        //   seq(
        //     "[",
        //     repeat(
        //       seq(
        //         choice(
        //           "in",
        //           "out",
        //         ),
        //         $.var_identifier,
        //         ":",
        //         $.block,
        //       ),
        //     ),
        //     "]",
        //   ),
        //
        // states_definition: ($) =>
        //   seq(
        //     "states",
        //     $.states_list_definition,
        //   ),
        //
        // states_list_definition: ($) =>
        //   seq(
        //     "[",
        //     repeat(
        //       seq(
        //         alias($.var_identifier, $.state_identifier),
        //         "when",
        //         $._expression,
        //         ":",
        //         $.block,
        //       ),
        //     ),
        //     "]",
        //   ),

        animate_statement: ($) =>
            seq("animate", $.var_identifier, $.animate_body),

        animate_body: ($) =>
            seq(
                "{",
                repeat(
                    seq($._builtin_type_identifier, ":", $._expression, ";"),
                ),
                "}",
            ),

        if_statement: ($) =>
            seq(
                "if",
                field(
                    "condition",
                    choice($._expression_body, seq("(", $._expression, ")")),
                ),
                optional(":"),
                $.component,
                optional(seq("else", choice($.if_statement, $.component))),
            ),

        for_loop: ($) =>
            seq(
                "for",
                field("identifier", $.var_identifier),
                "in",
                field("range", $.for_range),
                ":",
                $.component,
            ),

        value_list: ($) =>
            seq(
                "[",
                commaSep(choice($.var_identifier, $.value, $.anon_struct)),
                optional(","),
                "]",
            ),

        type_anon_struct: ($) =>
            seq(
                "{",
                repeat(
                    seq(
                        field("name", $.var_identifier),
                        ":",
                        field("type", $.type),
                    ),
                ),
                "}",
            ),

        type: ($) => choice($.type_identifier, $.type_list, $.type_anon_struct),

        type_list: ($) => seq("[", commaSep($.type), optional(","), "]"),

        for_range: ($) => choice($._int_number, $.value_list, $.var_identifier),

        // list_definition: ($) =>
        //   seq(
        //     $.list_definition_body,
        //     "]",
        //   ),
        //
        // list_definition_body: ($) =>
        //   seq(
        //     "[",
        //     repeat(
        //       seq(
        //         $.block,
        //         optional(","),
        //       ),
        //     ),
        //   ),

        _assignment_setup: ($) =>
            seq(
                field("name", $.var_identifier),
                field("op", $.assignment_prec_operator),
            ),

        _assignment_value_block: ($) =>
            field("value", seq($.block, optional(";"))),

        _assignment_value_expr: ($) =>
            field("value", choice($._expression, $.value_list)),

        assignment_block: ($) =>
            seq($._assignment_setup, $._assignment_value_block, optional(";")),

        assignment_expr: ($) =>
            seq($._assignment_setup, $._assignment_value_expr),

        _expression: ($) =>
            choice($._expression_body, seq("(", $._expression, ")")),

        expression_body_paren: ($) =>
            seq("(", optional($._expression_body), ")"),

        _expression_body: ($) =>
            choice(
                $.value,
                $.function_call,
                $.var_identifier,
                $.type_identifier,
                $.unary_expression,
                $._binary_expression,
                $.ternary_expression,
            ),

        unary_expression: ($) =>
            prec.left(2, seq($.unary_prec_operator, $._expression)),

        _binary_expression: ($) =>
            prec.left(
                1,
                choice(
                    $.mult_binary_expression,
                    $.add_binary_expression,
                    $.comparison_binary_expression,
                ),
            ),

        comparison_binary_expression: ($) =>
            prec.left(
                0,
                seq($._expression, $.comparison_operator, $._expression),
            ),

        mult_binary_expression: ($) =>
            prec.left(
                2,
                seq($._expression, $.mult_prec_operator, $._expression),
            ),
        ternary_expression: ($) =>
            prec.left(
                3,
                seq($._expression, "?", $._expression, ":", $._expression),
            ),

        add_binary_expression: ($) =>
            prec.left(
                1,
                seq($._expression, $.add_prec_operator, $._expression),
            ),

        callback: ($) =>
            seq(
                optional(field("purity", "pure")),
                "callback",
                field("name", $.function_identifier),
                optional($.call_signature),
                optional(seq("->", field("return_type", $.type_identifier))),
                ";",
            ),

        function: ($) =>
            seq(
                choice(
                    seq(
                        optional(field("purity", "pure")),
                        optional(
                            field(
                                "visibility",
                                optional(choice("private", "public")),
                            ),
                        ),
                    ),
                    seq(
                        optional(
                            field(
                                "visibility",
                                optional(choice("private", "public")),
                            ),
                        ),
                        optional(field("purity", "pure")),
                    ),
                ),
                field("name", $.function_identifier),
                optional($.call_signature),
                optional(seq("->", field("return_type", $.type_identifier))),
                $.block,
            ),

        callback_alias: ($) =>
            seq(
                optional(field("purity", "pure")),
                "callback",
                field("name", $.function_identifier),
                "<=>",
                field("alias", $.var_identifier),
                ";",
            ),

        callback_event: ($) =>
            seq(
                field("name", $.function_identifier),
                optional($.call_signature),
                "=>",
                field("action", $.block),
            ),

        function_call: ($) =>
            seq(
                field("name", $.var_identifier),
                optional($.call_signature),
                ";",
            ),

        call_signature: ($) =>
            seq(
                "(",
                field(
                    "parameters",
                    optional(
                        seq(commaSep1($._formal_parameter), optional(",")),
                    ),
                ),
                ")",
            ),

        _formal_parameter: ($) => $._expression,

        operators: ($) =>
            choice(
                $.comparison_operator,
                $.mult_prec_operator,
                $.add_prec_operator,
                $.unary_prec_operator,
                $.assignment_prec_operator,
            ),

        unary_prec_operator: (_) => choice("!", "-"),

        add_prec_operator: (_) => choice("+", "-"),
        mult_prec_operator: (_) => choice("*", "/", "&&", "||"),
        comparison_operator: (_) => choice(">", "<", ">=", "<=", "==", "!="),
        assignment_prec_operator: (_) =>
            prec.left(1, choice("=", ":", "+=", "-=", "*=", "/=")),

        // This is taken from tree-sitter-javascript
        // https://github.com/tree-sitter/tree-sitter-javascript/blob/fdeb68ac8d2bd5a78b943528bb68ceda3aade2eb/grammar.js#L866
        /////////////////////////////////////////////////////////////////////
        _string: ($) =>
            choice(
                seq(
                    '"',
                    repeat(
                        choice(
                            alias(
                                $._unescaped_double_string_fragment,
                                $._string_fragment,
                            ),
                            $._escape_sequence,
                        ),
                    ),
                    '"',
                ),
            ),

        _unescaped_double_string_fragment: (_) =>
            token.immediate(prec(1, /[^"\\]+/)),

        // same here
        _unescaped_single_string_fragment: (_) =>
            token.immediate(prec(1, /[^'\\]+/)),

        _escape_sequence: (_) =>
            token.immediate(
                seq(
                    "\\",
                    choice(
                        /[^xu0-7]/,
                        /[0-7]{1,3}/,
                        /x[0-9a-fA-F]{2}/,
                        /u[0-9a-fA-F]{4}/,
                        /u{[0-9a-fA-F]+}/,
                    ),
                ),
            ),

        _escape_sequence: (_) =>
            token.immediate(
                seq(
                    "\\",
                    choice(
                        /[^xu]/,
                        /u[0-9a-fA-F]{4}/,
                        /u{[0-9a-fA-F]+}/,
                        /x[0-9a-fA-F]{2}/,
                    ),
                ),
            ),
        /////////////////////////////////////////////////////////////////////

        visibility_modifier: (_) => choice("private", "in", "out", "in-out"),

        _identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_-]*/,
        // prefix_identifier: ($) => $._identifier,
        post_identifier: ($) => choice($._identifier, $.function_call),

        // Do not use strings here: Otherwise you can not assign to variables
        // with one of these names
        _builtin_type_identifier: (_) =>
            prec(
                2,
                choice(
                    /int/,
                    /float/,
                    /bool/,
                    /string/,
                    /color/,
                    /brush/,
                    /physical-length/,
                    /length/,
                    /duration/,
                    /angle/,
                    /easing/,
                    /percent/,
                    /image/,
                    /relative-font-size/,
                ),
            ),

        _user_type_identifier: ($) => prec(1, $._identifier),
        type_identifier: ($) =>
            choice($._user_type_identifier, $._builtin_type_identifier),

        var_identifier: ($) =>
            seq(
                choice(
                    $._identifier,
                    $.reference_identifier,
                    $.children_identifier,
                    field("match_all", "*"),
                    seq($._identifier, repeat(seq(".", $.post_identifier))),
                    seq(
                        $.reference_identifier,
                        repeat(seq(".", $.post_identifier)),
                    ),
                ),
                optional($.index_operator),
            ),

        children_identifier: (_) => "@children",

        index_operator: ($) => seq("[", $._expression, "]"),

        function_identifier: ($) => seq(optional("@"), $._identifier),

        function_call: ($) => seq($.function_identifier, $.call_signature),

        reference_identifier: (_) => choice("parent", "root", "self"),

        _number: ($) => choice($._int_number, $._float_number),

        _int_number: (_) => /\d+/,
        _float_number: (_) => /\d+\.\d+/,

        int_value: ($) => field("value", $._int_number),
        float_value: ($) => field("value", $._float_number),
        bool_value: (_) => field("value", choice("true", "false")),
        string_value: ($) => field("value", $._string),
        color_value: (_) => field("value", /#[\da-fA-F]+/),
        // brush_value: ($) => ???,
        physical_length_value: ($) =>
            seq(field("value", $._number), field("unit", choice("phx"))),
        length_value: ($) =>
            seq(
                field("value", $._number),
                field("unit", choice("px", "cm", "mm", "in", "pt")),
            ),
        duration_value: ($) =>
            seq(field("value", $._number), field("unit", choice("ms", "s"))),
        angle_value: ($) =>
            seq(
                field("value", $._number),
                field("unit", choice("deg", "grad", "turn", "rad")),
            ),
        // easing_value: ($) => ???,
        percent_value: ($) =>
            seq(field("value", $._number), field("unit", "%")),
        // image_value: ($) => ???.
        relative_font_size_value: ($) =>
            seq(field("value", $._number), field("unit", "rem")),

        value: ($) =>
            choice(
                $.int_value,
                $.float_value,
                $.bool_value,
                $.string_value,
                $.color_value,
                $.physical_length_value,
                $.length_value,
                $.duration_value,
                $.angle_value,
                $.percent_value,
                $.relative_font_size_value,
            ),

        comment: (_) =>
            token(
                choice(
                    seq("//", /[^\n\r]*/),
                    seq("/*", /[^*]*\*+([^/*][^*]*\*+)*/, "/"),
                ),
            ),
    },
});

function commaSep1(rule) {
    return seq(rule, repeat(seq(",", rule)));
}

function commaSep(rule) {
    return optional(commaSep1(rule));
}
