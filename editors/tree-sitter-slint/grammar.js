// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore mult prec

module.exports = grammar({
  name: "slint",

  extras: ($) => [/[\s\r\n]+/, $.comment],
  conflicts: ($) => [[$._assignment_value_block], [$.assignment_block]],

  rules: {
    sourcefile: ($) => repeat($._definition),

    _definition: ($) =>
      choice($.import_statement, $._exported_type, $._local_type),

    export: (_) => "export",

    _local_type: ($) =>
      choice(
        $.struct_definition,
        $.enum_definition,
        $.global_definition,
        $.component_definition,
      ),

    _exported_type: ($) =>
      prec.left(
        seq(
          $.export,
          choice(
            optional(seq("{", commaSep($.export_type), "}")),
            $._local_type,
          ),
        ),
      ),

    import_statement: ($) =>
      seq(
        "import",
        optional(seq("{", commaSep($.import_type), "}", "from")),
        $.string_value,
        ";",
      ),

    export_type: ($) =>
      seq(
        field("local_name", $._type_identifier),
        optional(seq("as", field("export_name", $._type_identifier))),
      ),

    import_type: ($) =>
      seq(
        field("import_name", $.user_type_identifier),
        optional(seq("as", field("local_name", $.user_type_identifier))),
      ),

    component: ($) =>
      seq(
        optional(seq(field("id", $.simple_identifier), ":=")),
        field("type", $.user_type_identifier),
        $.block,
      ),

    component_definition: ($) =>
      seq(
        choice(
          seq(
            // new syntax
            "component",
            field("name", $.user_type_identifier),
            optional(
              seq("inherits", field("base_type", $.user_type_identifier)),
            ),
          ),
          seq(
            // old syntax
            field("name", $.user_type_identifier),
            ":=",
            field("base_type", $.user_type_identifier),
          ),
        ),
        $.block,
      ),

    _property_type: ($) => seq("<", field("type", $.type), ">"),

    imperative_block: ($) =>
      seq("{", repeat($._imperative_block_statement), "}"),

    _imperative_block_statement: ($) =>
      prec(
        1,
        choice(
          seq($.assignment_block, optional(";")),
          seq($.assignment_expr, optional(";")),
          seq($.if_expr, optional(";")),
          $.callback_event,
          $.binding,
          seq($.expression, optional(";")),
          seq("return", optional($.expression), ";"),
        ),
      ),

    _binding: ($) =>
      field(
        "binding",
        choice(seq($.imperative_block, optional(";")), seq($.expression, ";")),
      ),

    property: ($) =>
      seq(
        field("visibility", optional($.property_visibility)),
        "property",
        seq(
          $._property_type,
          field("name", $.simple_identifier),
          choice(
            seq(field("binding_op", ":"), $._binding),
            seq(field("binding_op", "<=>"), $.expression, ";"),
            ";",
          ),
        ),
      ),

    binding_alias: ($) =>
      seq(
        field("visibility", optional($.property_visibility)),
        optional("property"),
        field("name", $.simple_identifier),
        "<=>",
        field("alias", $.expression),
        ";",
      ),

    binding: ($) => seq(field("name", $.simple_identifier), ":", $._binding),

    global_block: ($) =>
      seq(
        "{",
        repeat(
          choice(
            $.property,
            $.binding_alias,
            $.callback,
            $.callback_event,
            $.function_definition,
          ),
        ),
        "}",
      ),

    global_definition: ($) =>
      seq(
        "global",
        field("name", $.user_type_identifier),
        optional(":="), // old syntax!
        $.global_block,
      ),

    struct_block: ($) =>
      seq(
        "{",
        commaSep(
          seq(field("name", $.simple_identifier), ":", field("type", $.type)),
        ),
        optional(","),
        "}",
      ),

    struct_definition: ($) =>
      seq(
        "struct",
        field("name", $.user_type_identifier),
        optional(":="), // old syntax!
        $.struct_block,
      ),

    enum_block: ($) =>
      seq(
        "{",
        commaSep(field("name", $.user_type_identifier)),
        optional(","),
        "}",
      ),

    enum_definition: ($) =>
      seq("enum", field("name", $.user_type_identifier), $.enum_block),

    anon_struct_block: ($) =>
      prec(
        100,
        seq(
          "{",
          commaSep(seq($.simple_identifier, ":", $.expression)),
          optional(","),
          "}",
        ),
      ),

    block: ($) => seq("{", repeat($._block_statement), "}"),

    _block_statement: ($) =>
      choice(
        $.animate_statement,
        $.binding_alias,
        $.callback,
        $.callback_alias,
        $.callback_event,
        $.children_identifier, // No `;` after this one!
        $.changed_callback,
        $.component,
        $.for_loop,
        $.function_definition,
        $.if_statement,
        $.property,
        $.property_assignment,
        $.states_definition,
        $.transitions_definition,
      ),

    property_assignment: ($) =>
      seq(
        field("property", $.simple_identifier),
        ":",
        field(
          "value",
          choice(
            seq($.imperative_block, optional(";")),
            seq($.expression, ";"),
          ),
        ),
      ),

    in_out_transition: ($) =>
      seq(
        choice("in", "out", "in-out"),
        optional(seq($.expression, ":")),
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
            field("name", $.simple_identifier),
            "when",
            $.expression,
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

    animate_statement: ($) => seq("animate", $.expression, $.animate_body),

    animate_option_identifier: (_) =>
      choice("delay", "duration", "iteration-count", "direction", "easing"),

    animate_option: ($) =>
      seq(
        field("option", $.animate_option_identifier),
        ":",
        field("expression", $.expression),
        ";",
      ),

    animate_body: ($) => seq("{", repeat($.animate_option), "}"),

    if_expr: ($) =>
      seq(
        "if",
        field("condition", $.expression),
        $.imperative_block,
        optional(seq("else", choice($.if_expr, $.imperative_block))),
      ),

    if_statement: ($) =>
      seq("if", field("condition", $.expression), ":", $.component),

    for_loop: ($) =>
      seq(
        "for",
        field("identifier", $.simple_indexed_identifier),
        "in",
        field("range", $.for_range),
        ":",
        $.component,
      ),

    for_range: ($) => choice($.value_list, $.expression),

    type_list: ($) => seq("[", commaSep($.type), optional(","), "]"),

    type: ($) => choice($._type_identifier, $.type_list, $.struct_block),

    _assignment_setup: ($) =>
      seq(field("name", $.expression), field("op", $.assignment_prec_operator)),

    _assignment_value_block: ($) => field("value", seq($.block, optional(";"))),

    assignment_block: ($) =>
      seq($._assignment_setup, $._assignment_value_block, optional(";")),

    assignment_expr: ($) =>
      prec.right(2, seq($._assignment_setup, field("value", $.expression))),

    expression: ($) =>
      prec.right(
        choice(
          $.parens_op,
          $.index_op,
          $.tr,
          $.value,
          $.gradient_call,
          $.image_call,
          $.reference_identifier,
          $.simple_identifier,
          $.function_call,
          $.member_access,
          $.unary_expression,
          $.binary_expression,
          $.ternary_expression,
        ),
      ),

    parens_op: ($) => seq("(", field("left", $.expression), ")"),

    index_op: ($) =>
      prec(
        18,
        seq(
          field("left", $.expression),
          "[",
          field("index", $.expression),
          "]",
        ),
      ),

    // @tr(...)
    tr: ($) =>
      seq(
        "@tr",
        "(",
        optional(field("context", seq($.string_value, "=>"))),
        field("message", $.string_value),
        optional(
          seq(
            "|",
            field("pipe", $.string_value),
            "%",
            field("percent", $.expression),
          ),
        ),
        field(
          "arguments",
          optional(seq(",", commaSep1($.expression), optional(","))),
        ),
        ")",
      ),

    member_access: ($) =>
      prec.left(
        17,
        seq(field("base", $.expression), ".", field("member", $.expression)),
      ),

    unary_expression: ($) =>
      prec.left(
        14,
        seq(field("op", $.unary_prec_operator), field("left", $.expression)),
      ),

    binary_expression: ($) =>
      prec.left(
        1,
        choice(
          $._add_binary_expression,
          $._comparison_binary_expression,
          $._logic_binary_expression,
          $._mult_binary_expression,
        ),
      ),

    _add_binary_expression: ($) =>
      prec.left(
        11,
        seq(
          field("left", $.expression),
          field("op", $.add_prec_operator),
          field("right", $.expression),
        ),
      ),

    _comparison_binary_expression: ($) =>
      prec.left(
        9,
        seq(
          field("left", $.expression),
          field("op", $.comparison_operator),
          field("right", $.expression),
        ),
      ),

    logical_and: (_) => "&&",
    logical_or: (_) => "||",

    _logic_binary_expression: ($) =>
      choice(
        prec.left(
          4,
          seq(
            field("left", $.expression),
            field("op", $.logical_and),
            field("right", $.expression),
          ),
        ),
        prec.left(
          3,
          seq(
            field("left", $.expression),
            field("op", $.logical_or),
            field("right", $.expression),
          ),
        ),
      ),

    _mult_binary_expression: ($) =>
      prec.left(
        12,
        seq(
          field("left", $.expression),
          field("op", $.mult_prec_operator),
          field("right", $.expression),
        ),
      ),

    ternary_expression: ($) =>
      prec.left(
        3,
        seq(
          field("condition", $.expression),
          "?",
          field("left", $.expression),
          ":",
          field("right", $.expression),
        ),
      ),

    callback: ($) =>
      seq(
        optional($.purity),
        "callback",
        field("name", $.simple_identifier),
        optional($._callback_signature),
        optional(seq("->", field("return_type", $._type_identifier))),
        ";",
      ),

    purity: (_) => field("value", "pure"),

    function_visibility: (_) => field("value", choice("public", "private")),

    function_definition: ($) =>
      seq(
        repeat(choice($.purity, $.function_visibility)),
        "function",
        field("name", $.simple_identifier),
        optional($._function_signature),
        optional(seq("->", field("return_type", $.type))),
        $.imperative_block,
      ),

    callback_alias: ($) =>
      seq(
        optional($.purity),
        "callback",
        field("name", $.simple_identifier),
        "<=>",
        field("alias", $.expression),
        ";",
      ),

    callback_event: ($) =>
      seq(
        field("name", choice($.function_call, $.simple_identifier)),
        "=>",
        field("action", $.imperative_block),
      ),

    changed_callback: ($) =>
      seq(
        "changed",
        field("name", $.simple_identifier),
        "=>",
        field("action", $.imperative_block),
      ),

    function_call: ($) =>
      prec(
        17,
        seq(
          field("name", $.simple_identifier),
          field("arguments", $.arguments),
        ),
      ),

    gradient_call: ($) =>
      choice(
        seq(
          field("name", $.linear_gradient_identifier),
          "(",
          field(
            "arguments",
            seq(
              field("angle", choice($.angle_value, $.int_value, $.float_value)),
              ",",
              field("colors", commaSep2($.gradient_color)),
              optional(","),
            ),
          ),
          ")",
        ),
        seq(
          field("name", $.radial_gradient_identifier),
          "(",
          field(
            "arguments",
            seq(
              field("type", $.radial_gradient_kind),
              ",",
              field("colors", commaSep2($.gradient_color)),
              optional(","),
            ),
          ),
          ")",
        ),
      ),

    gradient_color: ($) => seq($.argument, optional($.percent_value)),

    image_call: ($) =>
      seq(
        field("name", "@image-url"),
        "(",
        field("image", $.string_value),
        optional(seq(",", "nine-slice", "(", repeat($._int_number), ")")),
        ")",
      ),

    typed_identifier: ($) =>
      seq(field("name", $.simple_identifier), ":", field("type", $.type)),

    _function_signature: ($) =>
      seq(
        "(",
        field(
          "arguments",
          optional(seq(commaSep1($.typed_identifier), optional(","))),
        ),
        ")",
      ),

    _callback_signature: ($) =>
      prec.left(
        seq(
          "(",
          field("arguments", optional(seq(commaSep1($.type), optional(",")))),
          ")",
        ),
      ),

    argument: ($) => $.expression,

    arguments: ($) =>
      prec(
        17,
        seq("(", optional(seq(commaSep1($.argument), optional(","))), ")"),
      ),

    unary_prec_operator: (_) => choice("!", "-", "+"),

    add_prec_operator: (_) => choice("+", "-"),
    mult_prec_operator: (_) => choice("*", "/"),
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
          repeat(choice($._unescaped_string_fragment, $.escape_sequence)),
          '"',
        ),
      ),

    _unescaped_string_fragment: (_) => token.immediate(prec(1, /[^"\\]+/)),

    escape_sequence: ($) =>
      seq(
        "\\",
        choice(
          /u\{[0-9a-fA-F]+\}/,
          "n",
          "\\",
          '"',
          seq("{", $.expression, "}"),
        ),
      ),
    /////////////////////////////////////////////////////////////////////

    property_visibility: (_) => choice("private", "in", "out", "in-out"),

    _identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_-]*/,
    simple_identifier: ($) => $._identifier,
    simple_indexed_identifier: ($) =>
      seq(
        field("name", $.simple_identifier),
        optional(seq("[", field("index_var", $.simple_identifier), "]")),
      ),

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

    easing_kind_identifier: ($) =>
      choice(
        "linear",
        "ease-in-quad",
        "ease-out-quad",
        "ease-in-out-quad",
        "ease",
        "ease-in",
        "ease-out",
        "ease-in-out",
        "ease-in-quart",
        "ease-out-quart",
        "ease-in-out-quart",
        "ease-in-quint",
        "ease-out-quint",
        "ease-in-out-quint",
        "ease-in-expo",
        "ease-out-expo",
        "ease-in-out-expo",
        "ease-in-sine",
        "ease-out-sine",
        "ease-in-out-sine",
        "ease-in-back",
        "ease-out-back",
        "ease-in-out-back",
        "ease-in-circ",
        "ease-out-circ",
        "ease-in-out-circ",
        "ease-in-elastic",
        "ease-out-elastic",
        "ease-in-out-elastic",
        "ease-in-bounce",
        "ease-out-bounce",
        "ease-in-out-bounce",
        seq("cubic-bezier", $.arguments),
      ),

    user_type_identifier: ($) => prec(1, $._identifier),
    _type_identifier: ($) =>
      choice($.builtin_type_identifier, $.user_type_identifier),

    value_list: ($) => seq("[", commaSep($.expression), optional(","), "]"),

    value: ($) =>
      prec(10, choice($.anon_struct_block, $.value_list, $._basic_value)),

    children_identifier: (_) => "@children",

    linear_gradient_identifier: (_) =>
      choice("@linear-gradient", "@linear_gradient"),
    radial_gradient_identifier: (_) =>
      choice("@radial-gradient", "@radial_gradient"),
    radial_gradient_kind: (_) => choice("circle"),

    reference_identifier: (_) => choice("parent", "root", "self"),

    _number: ($) => choice($._int_number, $._float_number),

    _int_number: (_) => /\d+/,
    _float_number: (_) => /(\d*\.\d+|\d+\.\d*)/,

    int_value: ($) => $._int_number,
    float_value: ($) => $._float_number,
    bool_value: (_) => choice("true", "false"),
    string_value: ($) => $._string,
    color_value: (_) => /#[\da-fA-F]+/,
    // brush_value: ($) => ???,
    physical_length_value: ($) => seq($._number, token.immediate("phx")),
    length_value: ($) =>
      seq($._number, token.immediate(choice("px", "cm", "mm", "in", "pt"))),
    duration_value: ($) => seq($._number, token.immediate(choice("ms", "s"))),
    angle_value: ($) =>
      seq($._number, token.immediate(choice("deg", "grad", "turn", "rad"))),
    // easing_value: ($) => ???,
    percent_value: ($) => seq($._number, token.immediate("%")),
    // image_value: ($) => ???.
    relative_font_size_value: ($) => seq($._number, token.immediate("rem")),

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
        $.easing_kind_identifier,
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

function commaSep(rule) {
  return optional(commaSep1(rule));
}

function commaSep1(rule) {
  return seq(rule, repeat(seq(",", rule)));
}

function commaSep2(rule) {
  return seq(rule, ",", rule, repeat(seq(",", rule)));
}
