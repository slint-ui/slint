// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

module.exports = grammar({
  name: "slint",

  extras: ($) => [
    /\s|\\\r?\n/,
    $.comment,
  ],

  rules: {
    source_file: ($) => repeat($._definition),

    _definition: ($) =>
      choice(
        $.struct_definition,
        $.component_definition,
        $.import_statement,
        $.export_statement,
        $.global_definition,
      ),

    global_definition: ($) =>
      seq(
        optional($.visibility_modifier),
        "global",
        $.var_identifier,
        ":=",
        $.field_declaration_list,
      ),

    import_statement: ($) =>
      seq(
        "import",
        optional(seq(
          "{",
          commaSep($._type_identifier),
          "}",
          "from",
        )),
        $.string,
        ";",
      ),

    export_statement: ($) =>
      seq(
        "export",
        "{",
        commaSep($._type_identifier),
        "}",
      ),

    struct_definition: ($) =>
      seq(
        optional($.visibility_modifier),
        "struct",
        field("name", $._type_identifier),
        ":=",
        optional(field("super_type", $._type_identifier)),
        $.struct_field_declaration_list,
      ),

    struct_field_declaration_list: ($) =>
      seq(
        $.struct_field_declaration_list_body,
        "}",
      ),

    struct_field_declaration_list_body: ($) =>
      seq(
        "{",
        repeat(seq(
          field("name", $.var_identifier),
          ":",
          $._expression,
          optional(","),
        )),
      ),

    component_definition: ($) =>
      seq(
        optional($.visibility_modifier),
        optional(
          seq(
            field("name", $._type_identifier),
            ":=",
          ),
        ),
        field("super_type", $._type_identifier),
        $.field_declaration_list,
      ),

    field_declaration_list: ($) =>
      seq(
        $.field_declaration_list_body,
        "}",
      ),

    field_declaration_list_body: ($) =>
      seq(
        "{",
        repeat(choice(
          $.component_definition,
          $.property_definition,
          $.callback_definition,
          $.variable_definition,
          $.variable_set_equal,
          $.for_loop_definition,
          $.if_statement_definition,
          $.animate_statement,
          $.callback_event,
          $.callback_call,
          $.var_identifier,
          $.states_definition,
          $.transitions_definition,
        )),
      ),

    transitions_definition: ($) =>
      seq(
        "transitions",
        $.transitions_list_definition,
      ),
    transitions_list_definition: ($) =>
      seq(
        "[",
        repeat(
          seq(
            choice(
              "in",
              "out",
            ),
            $.var_identifier,
            ":",
            $.field_declaration_list,
          ),
        ),
        "]",
      ),

    states_definition: ($) =>
      seq(
        "states",
        $.states_list_definition,
      ),

    states_list_definition: ($) =>
      seq(
        "[",
        repeat(
          seq(
            alias($.var_identifier, $.state_identifier),
            "when",
            $._expression,
            ":",
            $.field_declaration_list,
          ),
        ),
        "]",
      ),

    animate_statement: ($) =>
      seq(
        "animate",
        $.var_identifier,
        $.animate_declaration_list,
      ),
    animate_declaration_list: ($) =>
      seq(
        "{",
        repeat(
          seq(
            $.builtin_type_identifier,
            ":",
            $._expression,
            ";",
          ),
        ),
        "}",
      ),

    callback_event: ($) =>
      seq(
        $.function_identifier,
        "=>",
        $.field_declaration_list,
      ),
    callback_call: ($) =>
      seq(
        $.var_identifier,
        ";",
      ),

    if_statement_definition: ($) =>
      seq(
        choice(
          "if",
          "else if",
          "else",
        ),
        optional($._expression),
        choice(
          seq(
            $.field_declaration_list,
          ),
          seq(
            ":",
            $.component_definition,
          ),
        ),
      ),

    for_loop_definition: ($) =>
      seq(
        "for",
        $.var_identifier,
        "in",
        $.var_identifier,
        ":",
        $.component_definition,
      ),

    property_definition: ($) =>
      seq(
        "property",
        "<",
        $._propterty_kind,
        ">",
        field("name", $.var_identifier),
        optional($.property_expr),
        ";",
      ),
    property_expr: ($) =>
      seq(
        choice(
          "=",
          ":",
        ),
        choice(
          $._expression,
          $.list_definition,
        ),
      ),

    _propterty_kind: ($) =>
      choice(
        field("type", $._type_identifier),
        $.property_type_list,
      ),

    property_type_list: ($) =>
      seq(
        "[",
        field("type", $._type_identifier),
        "]",
      ),

    list_definition: ($) =>
      seq(
        $.list_definition_body,
        "]",
      ),

    list_definition_body: ($) =>
      seq(
        "[",
        repeat(
          seq(
            $.struct_field_declaration_list,
            optional(","),
          ),
        ),
      ),

    variable_definition: ($) =>
      seq(
        field("name", $.var_identifier),
        ":",
        $._expression,
        ";",
      ),

    variable_set_equal: ($) =>
      seq(
        field("prev_name", $.var_identifier),
        $.assignment_prec_operator,
        $._expression,
        ";",
      ),

    _expression: ($) =>
      choice(
        $._expression_body,
        $.expression_body_paren,
      ),

    expression_body_paren: ($) =>
      seq(
        "(",
        $._expression_body,
        ")",
      ),

    _expression_body: ($) =>
      seq(
        choice(
          $.value,
          $.string,
          $.function_call,
          $.var_identifier,
          $.builtin_type_identifier,
          $.unary_expression,
          $._binary_expression,
          $.ternary_expression,
        ),
      ),

    unary_expression: ($) =>
      prec.left(
        2,
        seq($.unary_prec_operator, $._expression),
      ),

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
        seq(
          $._expression,
          $.comparison_operator,
          $._expression,
        ),
      ),

    mult_binary_expression: ($) =>
      prec.left(
        2,
        seq($._expression, $.mult_prec_operator, $._expression),
      ),
    ternary_expression: ($) =>
      prec.left(
        3,
        seq(
          $._expression,
          "?",
          $._expression,
          ":",
          $._expression,
        ),
      ),

    add_binary_expression: ($) =>
      prec.left(
        1,
        seq($._expression, $.add_prec_operator, $._expression),
      ),

    callback_definition: ($) =>
      seq(
        "callback",
        $.function_identifier,
        optional($.call_signature),
        optional(
          seq(
            "->",
            $._type_identifier,
          ),
        ),
        ";",
      ),

    call_signature: ($) => field("parameters", $.formal_parameters),

    formal_parameters: ($) =>
      seq(
        "(",
        optional(seq(
          commaSep1($.formal_parameter),
          optional(","),
        )),
        ")",
      ),
    formal_parameter: ($) => $._expression,

    operators: ($) =>
      choice(
        $.comparison_operator,
        $.mult_prec_operator,
        $.add_prec_operator,
        $.unary_prec_operator,
        $.assignment_prec_operator,
      ),

    unary_prec_operator: ($) =>
      choice(
        "!",
        "-",
      ),

    add_prec_operator: ($) =>
      choice(
        "+",
        "-",
      ),
    mult_prec_operator: ($) =>
      choice(
        "*",
        "/",
        "&&",
        "||",
      ),
    comparison_operator: ($) =>
      choice(
        ">",
        "<",
        ">=",
        "<=",
      ),
    assignment_prec_operator: ($) =>
      prec.left(
        1,
        choice(
          "=",
          ":",
          "+=",
          "-=",
          "*=",
          "/=",
        ),
      ),

    // This is taken from tree-sitter-javascript
    // https://github.com/tree-sitter/tree-sitter-javascript/blob/fdeb68ac8d2bd5a78b943528bb68ceda3aade2eb/grammar.js#L866
    /////////////////////////////////////////////////////////////////////
    string: ($) =>
      choice(
        seq(
          '"',
          repeat(choice(
            alias($.unescaped_double_string_fragment, $.string_fragment),
            $.escape_sequence,
          )),
          '"',
        ),
        seq(
          "'",
          repeat(choice(
            alias($.unescaped_single_string_fragment, $.string_fragment),
            $.escape_sequence,
          )),
          "'",
        ),
      ),

    unescaped_double_string_fragment: ($) =>
      token.immediate(prec(1, /[^"\\]+/)),

    // same here
    unescaped_single_string_fragment: ($) =>
      token.immediate(prec(1, /[^'\\]+/)),

    escape_sequence: ($) =>
      token.immediate(seq(
        "\\",
        choice(
          /[^xu0-7]/,
          /[0-7]{1,3}/,
          /x[0-9a-fA-F]{2}/,
          /u[0-9a-fA-F]{4}/,
          /u{[0-9a-fA-F]+}/,
        ),
      )),

    escape_sequence: ($) =>
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

    visibility_modifier: ($) => "export",

    _identifier: ($) => /([a-zA-Z_]+-?)+/,
    prefix_identifier: ($) => $._identifier,
    post_identifier: ($) =>
      choice(
        $._identifier,
        $.function_call,
      ),
    user_type_identifier: ($) => prec(1, $._identifier),
    _type_identifier: ($) =>
      choice(
        $.user_type_identifier,
        $.builtin_type_identifier,
      ),
    var_identifier: ($) =>
      seq(
        choice(
          $._identifier,
          $.reference_identifier,
          $.children_identifier,
          field("match_all", "*"),
          seq($._identifier, repeat(seq(".", $.post_identifier))),
          seq($.reference_identifier, repeat(seq(".", $.post_identifier))),
        ),
        optional($.index_operator),
      ),

    children_identifier: ($) => seq("@", "children"),

    index_operator: ($) =>
      seq(
        "[",
        $._expression,
        "]",
      ),

    function_identifier: ($) =>
      seq(
        optional("@"),
        $._identifier,
      ),

    function_call: ($) => seq($.function_identifier, $.call_signature),

    reference_identifier: ($) =>
      choice(
        "parent",
        "root",
        "this",
      ),

    value: ($) =>
      choice(
        $.value_with_units,
        $.number,
        $.language_constant,
        $.color,
      ),

    color: ($) =>
      seq(
        "#",
        choice(
          /[\da-zA-Z]{3}/,
          /[\da-zA-Z]{6}/,
        ),
      ),

    value_with_units: ($) =>
      seq(
        $.number,
        $.unit_type,
      ),
    number: ($) =>
      choice(
        $.int_number,
        $.float_number,
      ),
    int_number: ($) => /\d+/,
    float_number: ($) => /\d+\.\d+/,

    unit_type: ($) =>
      choice(
        "px",
        "%",
        "ms",
        "rem",
      ),

    language_constant: ($) =>
      choice(
        "black",
        "blue",
        "ease",
        "ease-in",
        "ease_in",
        "ease_in_out",
        "ease-in-out",
        "ease_out",
        "ease-out",
        "end",
        "green",
        "red",
        "red",
        "start",
        "yellow",
        "true",
        "false",
        "transparent",
      ),

    builtin_type_identifier: ($) =>
      prec(
        2,
        choice(
          "angle",
          "bool",
          "brush",
          // "color", // Having color as a builtin type causes problems because slint also uses color as a variable name
          "duration",
          "easing",
          "float",
          "image",
          "int",
          "length",
          "percent",
          "physical-length",
          "physical_length",
          "relative-font-size",
          "string",
        ),
      ),

    // https://github.com/tree-sitter/tree-sitter-c/blob/e348e8ec5efd3aac020020e4af53d2ff18f393a9/grammar.js#L1009
    comment: ($) =>
      token(choice(
        seq("//", /(\\(.|\r?\n)|[^\\\n])*/),
        seq(
          "/*",
          /[^*]*\*+([^/*][^*]*\*+)*/,
          "/",
        ),
      )),
  },
});

function commaSep1(rule) {
  return seq(rule, repeat(seq(",", rule)));
}

function commaSep(rule) {
  return optional(commaSep1(rule));
}
