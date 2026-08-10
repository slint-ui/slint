;; Copyright © Luke. D Jones <luke@ljones.dev>
;; SPDX-License-Identifier: MIT

[(line_comment) (block_comment)] @comment @spell

; Different types:
(string_value) @string @spell

(escape_sequence) @string.escape

(color_value) @constant

[
  (children_identifier)
  (easing_kind_identifier)
] @constant.builtin

(bool_value) @boolean

[
  (int_value)
  (physical_length_value)
] @number

[
  (angle_value)
  (duration_value)
  (float_value)
  (length_value)
  (percent_value)
  (relative_font_size_value)
] @number.float

(purity) @type.qualifier

(function_visibility) @type.qualifier

(property_visibility) @type.qualifier

(builtin_type_identifier) @type.builtin

(reference_identifier) @variable.builtin

(type
  [
    (type_list)
    (user_type_identifier)
    (anon_struct_block)
  ]) @type

(user_type_identifier) @type

; Functions and callbacks
(argument) @variable.parameter

(function_call
  name: (_) @function.call)

; definitions
(callback
  name: (_) @function)

(callback_alias
  name: (_) @function)

(callback_event
  name: (simple_identifier) @function.call)

(component
  id: (_) @variable)

(enum_definition
  name: (_) @type)

(function_definition
  name: (_) @function)

(function_declaration
  name: (_) @function)

(struct_definition
  name: (_) @type)

(typed_identifier
  type: (_) @type)

; Operators
(binary_expression
  op: (_) @operator)

(unary_expression
  op: (_) @operator)

[
  (comparison_operator)
  (mult_prec_operator)
  (add_prec_operator)
  (unary_prec_operator)
  (assignment_prec_operator)
] @operator

[
  ":="
  "=>"
  "->"
  "<=>"
] @operator

; Punctuation
[
  ";"
  "."
  ","
  ":"
] @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

(property
  [
    "<"
    ">"
  ] @punctuation.bracket)

; Properties, Variables and Constants:
(component
  id: (simple_identifier) @constant)

(property
  name: (simple_identifier) @property)

(property_assignment
  property: (simple_identifier) @property)

(binding_alias
  name: (simple_identifier) @property)

(struct_field_definition
  name: (simple_identifier) @variable.member)

(anon_struct_assignment
  member: (simple_identifier) @variable.member)

(property_assignment
  property: (simple_identifier) @property)

(callback
  name: (simple_identifier) @variable)

(typed_identifier
  name: (_) @variable)

(simple_indexed_identifier
  name: (simple_identifier) @variable
  index_var: (simple_identifier) @variable)

(expression
  (simple_identifier) @variable)

(member_access
  member:
    (expression
      (simple_identifier) @property))

(state_definition
  name: (simple_identifier) @constant
  "when" @keyword)

; Attributes:
[
  (linear_gradient_identifier)
  (radial_gradient_identifier)
  (radial_gradient_kind)
  (conic_gradient_identifier)
] @attribute

(image_call
  "@image-url" @attribute)

(tr
  "@tr" @attribute)

(rust_attr
  "@rust-attr" @attribute)

(keys
  "@keys" @attribute)

(markdown
  "@markdown" @attribute)

(keys
  (simple_identifier) @constant)

(keys
  "+" @operator)

; Keywords:
(animate_option_identifier) @keyword

(export_statement
  "export" @keyword)

(exported_definition
  "export" @keyword)

(export_type
  "as" @keyword)

(if_statement
  "if" @keyword.conditional)

(if_expr
  [
    "if"
    "else"
  ] @keyword.conditional)

(ternary_expression
  [
    "?"
    ":"
  ] @keyword.conditional.ternary)

(animate_statement
  "animate" @keyword)

(gradient_call
  [
    "at"
    "from"
  ] @keyword)

(callback
  "callback" @keyword)

(changed_event
  "changed" @keyword)

(component_definition
  "component" @keyword)

(component_modifier
  "inherits" @keyword)

(uses_clause
  "uses" @keyword)

(used_interface
  "from" @keyword
  source: (_) @variable)

(implements_clause
  "implements" @keyword)

(enum_definition
  "enum" @keyword)

(for_loop
  [
    "for"
    "in"
  ] @keyword.repeat)

(function_definition
  "function" @keyword.function)

(function_declaration
  "function" @keyword.function)

(global_definition
  "global" @keyword)

(let_statement
  "let" @keyword
  name: (_) @variable
  "=" @operator)

(return_statement
  "return" @keyword.return)

(import_statement
  [
    "import"
    "from"
  ] @keyword.import)

(import_type
  "as" @keyword.import)

(property
  "property" @keyword)

(states_definition
  "states" @keyword)

(struct_definition
  "struct" @keyword)

(transitions_definition
  [
    "transitions"
    "in"
    "out"
  ] @keyword)
