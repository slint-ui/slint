; Copyright © SixtyFPS GmbH <info@slint.dev>
; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

(comment) @comment @spell

; Different types:
(string_value) @string @spell
(escape_sequence) @string.escape

(color_value) @constant
[(children_identifier) (easing_kind_identifier)] @constant.builtin
(bool_value) @boolean
[(int_value) (length_value) (physical_length_value) (duration_value) (angle_value) (relative_font_size_value)] @number
(simple_identifier) @property @spell
(escape_sequence) @string.escape
(purity) @type.qualifier
(visibility) @type.qualifier
(animate_option_identifier) @keyword

[(float_value) (percent_value)] @float

(builtin_type_identifier) @type.builtin
(reference_identifier) @variable.builtin
(type [(type_list) (user_type_identifier) (anon_struct_block)]) @type @spell
(user_type_identifier) @type @spell
(visibility_modifier) @type.qualifier
[(comparison_operator) (mult_prec_operator) (add_prec_operator) (unary_prec_operator) (assignment_prec_operator)] @operator

; Functions and callbacks
(argument) @parameter
(function_call) @function.call

; definitions
(callback name: ((_) @function @spell))
(component id: ((_) @variable @spell))
(enum_definition name: ((_) @type))
(function_definition name: ((_) @function @spell))
(property name: ((_) @property @spell))
(struct_definition name: ((_) @type))
(typed_identifier name: ((_) @variable @spell))
(typed_identifier type: ((_) @type))

(binary_expression op: ((_) @operator))
(component (":=" @operator))
(component_definition ([":="] @operator))
(global_definition (":=" @operator))
(struct_definition (":=" @operator))
(unary_expression op: ((_) @operator))

(if_statement (["if" ":" "else"] @conditional))
(ternary_expression (["?" ":"] @conditional.ternary))

; Keywords:
[ ";" "." "," ] @punctuation.delimiter
[ "(" ")" "[" "]" "{" "}" ] @punctuation.bracket

[(linear_gradient_identifier) (radial_gradient_identifier) (radial_gradient_kind)] @keyword
(export) @include @keyword

(animate_option (":" @keyword))
(animate_statement ("animate" @keyword))
(assignment_expr name: ((_) @property))
(callback ("callback" @keyword))
(component_definition (["component" "inherits"] @keyword))
(enum_definition ("enum" @keyword))
(for_loop (["for" "in" ":"] @repeat @keyword))
(function_definition ("function" @keyword))
(function_call name: ((_) @function.call))
(global_definition ("global" @keyword))
(image_call ("@image-url" @keyword))
(imperative_block ("return" @keyword.return))
(import_statement (["import" "from" "as"] @include @keyword))
(property (["property" "<" ">"] @keyword))
(states_definition (["states" "when"] @keyword))
(struct_definition ("struct" @keyword))
(tr ("@tr" @keyword))
(transitions_definition (["transitions" "in" "out"] @keyword))
