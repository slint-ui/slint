; Copyright © SixtyFPS GmbH <info@slint-ui.com>
; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

(comment) @comment @spell

; Different types:
(string_value) @string @spell
(bool_value) @boolean
(int_value) @number
[(float_value) (percent_value)] @float

(builtin_type_identifier) @type.builtin
(reference_identifier) @variable.builtin
(user_type_identifier) @spell
(type [(type_list) (user_type_identifier) (type_anon_struct)]) @type
[(comparison_operator) (mult_prec_operator) (add_prec_operator) (unary_prec_operator) (assignment_prec_operator)] @operator

; Functions and callbacks
(function_call) @function.call
(parameter) @parameter
(function ("function" @keyword))
(callback ("callback" @keyword))
(block ("return" @keyword.return))

; definitions
(callback name: ((_) @spell))
(function name: ((_) @spell))
(property name: ((_) @property @spell))
(component id: ((_) @variable @spell))
(struct_definition (type_anon_struct name: ((_) @field)))
(global_definition (["global" ":="] @include @keyword))
(struct_definition (["struct" ":="] @include @keyword))
(property (["property" "<" ">"] @keyword))
(visibility_modifier) @type.qualifier

; Keywords:
[ ";" "." "," ] @punctuation.delimiter
[ "(" ")" "[" "]" "{" "}" ] @punctuation.bracket

(ternary_expression (["?" ":"] @conditional.ternary))
(if_statement (["if" ":" "else"] @conditional))
(for_loop (["for" "in" ":"] @repeat @keyword))

(animate_statement (["animate"] @keyword))
(component_definition (["component" "inherits" ":="] @keyword))
(export_statement (["export" "as"] @include @keyword))
(import_statement (["import" "from" "as"] @include @keyword))
(states_definition (["states" "when"] @keyword))
(transitions_definition (["transitions" "in" "out"] @keyword))
