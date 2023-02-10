// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore mult prec

module.exports = grammar({
    name: "slint",

    extras: ($) => [/[\s\r\n]+/, $.comment],
    conflicts: ($) => [
        [$._assignment_value_block],
        [$._binding],
        [$._expression_body, $.value],
        [$._var_identifier_start, $.var_identifier],
        [$.anon_struct, $.assignment_prec_operator, $.binding],
        [$.assignment_block],
        [$.binding_block, $._binding_block_statement],
        [$._binding_block_statement, $.block],
        [$._export_modifier, $.export_statement],
        [$.function_identifier, $.post_identifier],
        [$.var_identifier],
    ],

    rules: {
        sourcefile: ($) => repeat($._definition),

        _definition: ($) =>
            choice(
                $.export_statement,
                $.import_statement,
                $._exportable_definition,
            ),

        _exportable_definition: ($) =>
            seq(
                field("export", optional($._export_modifier)),
                choice(
                    $.struct_definition,
                    $.global_definition,
                    $.component_definition,
                ),
            ),

        export_statement: ($) =>
            seq(
                "export",
                optional(
                    seq(
                        "{",
                        commaSep(
                            seq(
                                $._type_identifier,
                                optional(seq("as", $._type_identifier)),
                            ),
                        ),
                        "}",
                    ),
                ),
            ),

        import_statement: ($) =>
            seq(
                "import",
                optional(
                    seq(
                        "{",
                        commaSep(
                            seq(
                                $._type_identifier,
                                optional(seq("as", $._type_identifier)),
                            ),
                        ),
                        "}",
                        "from",
                    ),
                ),
                $.string_value,
                ";",
            ),

        _export_modifier: (_) => "export",

        component: ($) =>
            seq(
                optional(seq(field("id", $.var_identifier), ":=")),
                field("type", $.user_type_identifier),
                $.block,
            ),

        component_definition: ($) =>
            choice(
                seq(
                    // new syntax
                    "component",
                    field("name", $._type_identifier),
                    optional(
                        seq("inherits", field("base_type", $._type_identifier)),
                    ),
                    $.block,
                ),
                seq(
                    // old syntax
                    field("name", $._type_identifier),
                    ":=",
                    field("base_type", $._type_identifier),
                    $.block,
                ),
            ),

        _property_type: ($) => seq("<", field("type", $.type), ">"),
        binding_block: ($) =>
            seq(
                "{",
                repeat($._binding_block_statement),
                optional(
                    seq(
                        choice(
                            $._expression,
                            $.assignment_block,
                            $.assignment_expr,
                        ),
                        optional(";"),
                    ),
                ),
                "}",
            ),

        _binding_block_statement: ($) =>
            choice(
                seq($.assignment_block, optional(";")),
                seq($.assignment_expr, ";"),
                $.if_expr,
                $.binding,
                $.binding_alias,
                $.callback_event,
                $.callback_alias,
                seq(optional("return"), $._expression, ";"),
            ),

        _binding: ($) =>
            field(
                "binding",
                choice(
                    seq($.binding_block, optional(";")),
                    seq($._expression, ";"),
                ),
            ),

        property: ($) =>
            seq(
                field("visibility", optional($.visibility_modifier)),
                "property",
                choice(
                    seq(
                        $._property_type,
                        field("name", $.var_identifier),
                        choice(
                            optional(seq(field("binding_op", ":"), $._binding)),
                            ";",
                        ),
                    ),
                    seq(
                        optional($._property_type),
                        field("name", $.var_identifier),
                        field("binding_op", "<=>"),
                        field("binding", $.var_identifier),
                        ";",
                    ),
                ),
            ),

        binding_alias: ($) =>
            seq(
                field("name", $.var_identifier),
                "<=>",
                field("alias", $.var_identifier),
                ";",
            ),

        binding: ($) =>
            seq(field("name", $.var_identifier), ":", $._binding, ";"),

        global_definition: ($) =>
            seq(
                "global",
                field("name", $._type_identifier),
                optional(":="), // old syntax!
                "{",
                repeat(
                    choice(
                        $.property,
                        $.callback,
                        $.callback_event,
                        $.function,
                    ),
                ),
                "}",
            ),

        struct_definition: ($) =>
            seq(
                "struct",
                field("name", $._type_identifier),
                optional(":="), // old syntax!
                $.type_anon_struct,
            ),

        anon_struct: ($) =>
            prec(
                100,
                seq(
                    "{",
                    commaSep(seq($.var_identifier, ":", $._expression)),
                    optional(","),
                    "}",
                ),
            ),

        block: ($) =>
            seq(
                "{",
                repeat($._block_statement),
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

        _block_statement: ($) =>
            choice(
                $._binding_block_statement,
                $.for_loop,
                $.if_statement,
                $.animate_statement,
                $.children_identifier, // No `;` after this one!
                $.component,
                $.property,
                $.callback,
                $.function,
                $.states_definition,
                $.transitions_definition,
            ),

        in_out_transition: ($) =>
            seq(
                choice("in", "out"),
                optional(seq($.var_identifier, ":")),
                "{",
                repeat($.animate_statement),
                "}",
            ),

        transitions_definition: ($) =>
            seq("transitions", "[", repeat($.in_out_transition), "]"),

        states_definition: ($) =>
            seq(
                "states",
                "[",
                repeat(
                    seq(
                        field("name", $.var_identifier),
                        "when",
                        $._expression,
                        ":",
                        "{",
                        repeat(
                            choice(
                                $.in_out_transition,
                                $.assignment_block,
                                seq($.assignment_expr, ";"),
                            ),
                        ),
                        optional($.assignment_expr),
                        "}",
                    ),
                ),
                "]",
            ),

        animate_statement: ($) =>
            seq("animate", $.var_identifier, $.animate_body),

        animate_body: ($) =>
            seq("{", repeat(seq($._identifier, ":", $._expression, ";")), "}"),

        if_expr: ($) =>
            seq(
                "if",
                field(
                    "condition",
                    choice($._expression_body, seq("(", $._expression, ")")),
                ),
                $.binding_block,
                optional(seq("else", choice($.if_expr, $.binding_block))),
            ),

        if_statement: ($) =>
            seq(
                "if",
                field(
                    "condition",
                    choice($._expression_body, seq("(", $._expression, ")")),
                ),
                ":",
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

        type_anon_struct: ($) =>
            seq(
                "{",
                commaSep(
                    seq(
                        field("name", $.var_identifier),
                        ":",
                        field("type", $.type),
                    ),
                ),
                optional(","),
                "}",
            ),
        type_list: ($) => seq("[", commaSep($.type), optional(","), "]"),

        type: ($) =>
            choice($._type_identifier, $.type_list, $.type_anon_struct),

        for_range: ($) => choice($._int_number, $.value_list, $.var_identifier),

        _assignment_setup: ($) =>
            seq(
                field("name", $.var_identifier),
                field("op", $.assignment_prec_operator),
            ),

        _assignment_value_block: ($) =>
            field("value", seq($.block, optional(";"))),

        _assignment_value_expr: ($) => field("value", $._expression),

        assignment_block: ($) =>
            seq($._assignment_setup, $._assignment_value_block, optional(";")),

        assignment_expr: ($) =>
            seq($._assignment_setup, $._assignment_value_expr),

        _expression: ($) =>
            choice($._expression_body, seq("(", $._expression, ")")),

        _expression_body: ($) =>
            choice(
                seq($.value, optional($._accessor_postfix)),
                seq($.function_call, optional($._accessor_postfix)),
                $.var_identifier,
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
                seq(
                    field("left", $._expression),
                    field("op", $.mult_prec_operator),
                    field("right", $._expression),
                ),
            ),
        ternary_expression: ($) =>
            prec.left(
                3,
                seq(
                    field("condition", $._expression),
                    "?",
                    field("left", $._expression),
                    ":",
                    field("right", $._expression),
                ),
            ),

        add_binary_expression: ($) =>
            prec.left(
                1,
                seq(
                    field("left", $._expression),
                    field("op", $.add_prec_operator),
                    field("right", $._expression),
                ),
            ),

        callback: ($) =>
            seq(
                optional(field("purity", "pure")),
                "callback",
                field("name", $.function_identifier),
                optional($.call_signature),
                optional(seq("->", field("return_type", $._type_identifier))),
                ";",
            ),

        function: ($) =>
            seq(
                optional(
                    choice(
                        field("purity", "pure"),
                        field("visibility", choice("private", "public")),
                        seq(
                            field("purity", "pure"),
                            field("visibility", choice("private", "public")),
                        ),
                        seq(
                            field("visibility", choice("private", "public")),
                            field("purity", "pure"),
                        ),
                    ),
                ),
                "function",
                field("name", $.function_identifier),
                optional($.function_signature),
                optional(seq("->", field("return_type", $.type))),
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

        typed_identifier: ($) =>
            seq(field("name", $.simple_identifier), ":", field("type", $.type)),

        function_signature: ($) =>
            seq(
                "(",
                field(
                    "parameters",
                    optional(seq(commaSep1($.typed_identifier), optional(","))),
                ),
                ")",
            ),

        parameter: ($) => $._expression,

        call_signature: ($) =>
            seq(
                "(",
                field(
                    "parameters",
                    optional(seq(commaSep1($.parameter), optional(","))),
                ),
                ")",
            ),

        operators: ($) =>
            choice(
                $.comparison_operator,
                $.mult_prec_operator,
                $.add_prec_operator,
                $.unary_prec_operator,
                $.assignment_prec_operator,
            ),

        unary_prec_operator: (_) => choice("!", "-", "+"),

        add_prec_operator: (_) => choice("+", "-"),
        mult_prec_operator: (_) => choice("*", "/", "&&", "||"),
        comparison_operator: (_) => choice(">", "<", ">=", "<=", "==", "!="),
        assignment_prec_operator: (_) =>
            prec.left(1, choice("=", ":", "+=", "-=", "*=", "/=")),

        // This is inspired from tree-sitter-javascript
        // https://github.com/tree-sitter/tree-sitter-javascript/blob/fdeb68ac8d2bd5a78b943528bb68ceda3aade2eb/grammar.js#L866
        /////////////////////////////////////////////////////////////////////
        _string: ($) =>
            choice(
                seq(
                    '"',
                    repeat(
                        choice(
                            $._unescaped_string_fragment,
                            $._escape_sequence,
                        ),
                    ),
                    '"',
                ),
            ),

        _unescaped_string_fragment: (_) => token.immediate(prec(1, /[^"\\]+/)),

        _escape_sequence: ($) =>
            seq(
                "\\",
                choice(
                    /u\{[0-9a-fA-F]+\}/,
                    "n",
                    "\\",
                    '"',
                    seq("{", $._expression, "}"),
                ),
            ),
        /////////////////////////////////////////////////////////////////////

        visibility_modifier: (_) => choice("private", "in", "out", "in-out"),

        _identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_-]*/,
        simple_identifier: ($) => $._identifier,
        // prefix_identifier: ($) => $._identifier,
        post_identifier: ($) => choice($._identifier, $.function_call),

        builtin_type_identifier: (_) =>
            choice(
                "int",
                "float",
                "bool",
                "string",
                "color",
                "brush",
                "physical-length",
                "length",
                "duration",
                "angle",
                "easing",
                "percent",
                "image",
                "relative-font-size",
            ),

        user_type_identifier: ($) => prec(1, $._identifier),
        _type_identifier: ($) =>
            choice($.builtin_type_identifier, $.user_type_identifier),

        value_list: ($) =>
            seq("[", commaSep($._expression), optional(","), "]"),

        value: ($) =>
            choice(
                seq("(", $.value, ")"),
                seq($.color_value, $.percent_value), // gradient values
                $.anon_struct,
                $.value_list,
                $._basic_value,
            ),

        _var_identifier_start: ($) =>
            choice($._identifier, $.reference_identifier),
        _accessor_postfix: ($) =>
            repeat1(choice(seq(".", $.post_identifier), $.index_operator)),

        var_identifier: ($) =>
            choice(
                field("match_all", "*"),
                seq($._var_identifier_start, optional($._accessor_postfix)),
                seq($._identifier, repeat(seq(".", $.post_identifier))),
            ),

        children_identifier: (_) => "@children",

        index_operator: ($) => seq("[", $._expression, "]"),

        function_identifier: ($) => seq(optional("@"), $._identifier),

        function_call: ($) => seq($.function_identifier, $.call_signature),

        reference_identifier: (_) => choice("parent", "root", "self"),

        _number: ($) => choice($._int_number, $._float_number),

        _int_number: (_) => /\d+/,
        _float_number: (_) => /(\d*\.\d+|\d+\.\d*)/,

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

        _basic_value: ($) =>
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
