; Keep literals and comments stable while the surrounding layout evolves.
[
  (attribute)
  (comment)
  (bool_value)
  (int_value)
  (float_value)
  (string_value)
  (color_value)
  (physical_length_value)
  (length_value)
  (duration_value)
  (angle_value)
  (percent_value)
  (relative_font_size_value)
] @leaf

(comment) @multi_line_indent_all
(comment) @prepend_input_softline @append_input_softline
(comment) @allow_blank_line_before
; Preserve the conventional blank line between a leading header comment block and the first item.
(sourcefile
  . (comment)+
  . (_) @allow_blank_line_before
 )

; Add a special case for single-line comments that end with the ignore directive,
; so that they can be used to disable formatting for the following item without forcing a blank line.
;
; Note: We must use a topiary-compatible capture name for the comment, otherwise topariy rejects the query.
((comment) @prepend_input_softline
  . (_) @leaf
  (#eq? @prepend_input_softline "// slint-fmt:ignore"))
 

; Round one focuses on the highest-signal spacing choices first.
(export) @append_space

(component_definition
  "component" @append_space
)

(component_definition
  name: (user_type_identifier) @append_space
  base_type: (user_type_identifier) @prepend_space
)

[ "import" "from" "as" "global" "struct" "enum" "interface" "uses" "implements" "inherits" ] @append_space

[ "property" "callback" "function" "animate" "if" "for" "when" "else" "return" "let" ] @append_space

[ "private" "public" "protected" "pure" "changed" ] @append_space

[":=" "<=>" "=>"] @prepend_space @append_space
":" @prepend_antispace @append_space

[ "(" "[" "<" ] @append_antispace
[ ")" "]" ";" "," ">" ] @prepend_antispace

; Most adjacent named nodes in Slint want a separating space unless punctuation
; explicitly cancels it.
(_
  (_) @append_space
  .
  (_)
)

; Comma-separated forms stay compact inline and break cleanly when already multiline.
(_
  "," @append_spaced_softline
)

(import_statement
  "{" @append_space
  "}" @prepend_space @append_space
  "from" @append_space
)

(reexport_statement
  "{" @append_space
  "}" @prepend_space @append_space
  "from" @append_space
)

(uses_clause
  "{" @append_space
  "}" @prepend_space @append_space
)

(used_interface
  "from" @append_space
)

(property
  "<" @append_antispace
  ">" @prepend_antispace @append_space
)

(arguments) @prepend_antispace

(callback
  name: (simple_identifier)
  "(" @prepend_antispace
)

(function_definition
  name: (simple_identifier)
  "(" @prepend_antispace
)

(function_declaration
  name: (simple_identifier)
  "(" @prepend_antispace
)

(simple_indexed_identifier
  "[" @prepend_antispace @append_antispace
  "]" @prepend_antispace
)

(index_op
  "[" @prepend_antispace @append_antispace
  "]" @prepend_antispace
)

; Integer literals keep a separating space so `1 .foo` doesn't collapse into `1.foo`.
(member_access
  base: (expression
          (value
            (int_value)))
  "." @prepend_space @append_antispace
)

(member_access
  base: (expression
          [
            (keys)
            (parens_op)
            (index_op)
            (tr)
            (markdown)
            (gradient_call)
            (image_call)
            (reference_identifier)
            (simple_identifier)
            (function_call)
            (member_access)
            (unary_expression)
            (binary_expression)
            (ternary_expression)
            (value
              [
                (anon_struct_block)
                (value_list)
                (bool_value)
                (float_value)
                (string_value)
                (color_value)
                (physical_length_value)
                (length_value)
                (duration_value)
                (angle_value)
                (percent_value)
                (relative_font_size_value)
              ])
          ])
  "." @prepend_antispace @append_antispace
)

(if_statement
  ":" @prepend_space @append_space
)

(ternary_expression
  "?" @prepend_space @append_space
)

(ternary_expression
  left: (_) @append_antispace @append_delimiter
  .
  ":" @delete
  .
  right: (_)

  (#delimiter! " : ")
  (#query_name! "ternary colon delimiter")
)

(for_loop
  "for" @append_space
  "in" @prepend_space @append_space
  ":" @prepend_space @append_space
)

; Blocks should read like ordinary Slint by default while still allowing
; simple one-liners to stay inline.
(block
  "{" @prepend_space @append_indent_start @append_spaced_softline
  . (_) @allow_blank_line_before
)

(block
  (_) @append_spaced_softline
  .
  (_) @allow_blank_line_before
)

(block
  (_) @append_spaced_softline
  .
  "}" @allow_blank_line_before @prepend_indent_end
)

(imperative_block
  "{" @prepend_space @append_indent_start @append_spaced_softline
  . (_) @allow_blank_line_before
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(imperative_block
  (_) @append_spaced_softline
  .
  (_) @allow_blank_line_before
)

(global_block
  "{" @prepend_space @append_indent_start
  (_)+ @allow_blank_line_before @prepend_spaced_softline
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(global_block
  (_)+ @prepend_antispace
  (#multi_line_only!)
)

(global_block
  "}" @prepend_antispace
  (#multi_line_only!)
)

(interface_block
  "{" @prepend_space @append_indent_start
  (_)+ @allow_blank_line_before @prepend_spaced_softline
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(interface_block
  (_)+ @prepend_antispace
  (#multi_line_only!)
)

(interface_block
  "}" @prepend_antispace
  (#multi_line_only!)
)

(struct_block
  "{" @prepend_space @append_indent_start
  (_)+ @allow_blank_line_before @prepend_spaced_softline
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(struct_block
  (_)+ @prepend_antispace
  (#multi_line_only!)
)

(struct_block
  "}" @prepend_antispace
  (#multi_line_only!)
)

(enum_block
  "{" @prepend_space @append_indent_start
  (_)+ @allow_blank_line_before @prepend_spaced_softline
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(enum_block
  (_)+ @prepend_antispace
  (#multi_line_only!)
)

(enum_block
  "}" @prepend_antispace
  (#multi_line_only!)
)

(animate_body
  "{" @prepend_space @append_indent_start
  (_)+ @allow_blank_line_before @prepend_spaced_softline
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(animate_body
  (_)+ @prepend_antispace
  (#multi_line_only!)
)

(animate_body
  "}" @prepend_antispace
  (#multi_line_only!)
)

(animate_statement
  "animate" @append_space @append_begin_scope
  (animate_body) @prepend_space @prepend_end_scope

  (#scope_id! "animate-targets")
)

(animate_statement
  (expression) @append_antispace @append_delimiter
  .
  "," @delete
  .
  (_)

  (#delimiter! ", ")
  (#query_name! "animate target comma delimiter")
)

; Anonymous object literals follow the same inline-vs-multiline decision, but
; they should not force a space before the opening brace in expression position.
(anon_struct_block
  "{" @append_indent_start
  (_)+ @allow_blank_line_before @prepend_spaced_softline
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(anon_struct_block
  (_)+ @prepend_antispace
  (#multi_line_only!)
)

(anon_struct_block
  "}" @prepend_antispace
  (#multi_line_only!)
)

(in_out_transition
  [ "in" "out" "in-out" ] @append_space
  ":" @prepend_space @append_space
  "{" @prepend_space @append_indent_start @append_spaced_softline
  . (animate_statement) @allow_blank_line_before
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(in_out_transition
  (animate_statement) @append_spaced_softline
  .
  (animate_statement) @allow_blank_line_before
)

(states_definition
  "states" @append_space
  "[" @append_indent_start
  name: (simple_identifier) @allow_blank_line_before @prepend_input_softline
  ":" @prepend_space @append_space
  "{" @prepend_space @append_indent_start
  (_)+ @allow_blank_line_before @prepend_spaced_softline
  "}" @prepend_spaced_softline @prepend_indent_end
  "]" @allow_blank_line_before @prepend_input_softline @prepend_indent_end
)

(states_definition
  name: (simple_identifier) @prepend_antispace
  (#multi_line_only!)
)

(states_definition
  [
    (in_out_transition)
    (assignment_block)
    (assignment_expr)
  ] @prepend_antispace
  (#multi_line_only!)
)

(states_definition
  "]" @prepend_antispace
  (#multi_line_only!)
)

(states_definition
  [
    (in_out_transition)
    (assignment_block)
    (assignment_expr)
  ] @allow_blank_line_before @prepend_spaced_softline
)

(transitions_definition
  "transitions" @append_space
  "[" @append_indent_start
  (in_out_transition)+ @allow_blank_line_before @prepend_input_softline
  "]" @allow_blank_line_before @prepend_input_softline @prepend_indent_end
)

(transitions_definition
  (in_out_transition)+ @prepend_antispace
  (#multi_line_only!)
)

(transitions_definition
  "]" @prepend_antispace
  (#multi_line_only!)
)

; Imports usually stay grouped. Definitions are easier to scan with a blank line.
(sourcefile
  (import_statement) @append_antispace @append_hardline
  .
  (import_statement)
)

(sourcefile
  (reexport_statement) @append_antispace @append_hardline
  .
  (reexport_statement)
)

(sourcefile
  (import_statement) @append_antispace @append_delimiter
  .
  (reexport_statement)

  (#delimiter! "\n\n")
)

(sourcefile
  (import_statement) @append_antispace @append_delimiter
  .
  [
    (export)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ]

  (#delimiter! "\n\n")
)

(sourcefile
  (reexport_statement) @append_antispace @append_delimiter
  .
  [
    (export)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ]

  (#delimiter! "\n\n")
)

(sourcefile
  [
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ] @append_antispace @append_delimiter
  .
  [
    (export)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ]

  (#delimiter! "\n\n")
)

(sourcefile
  [
    (import_statement)
    (reexport_statement)
    (attribute)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ] @append_antispace @append_hardline
  .
  (comment) @allow_blank_line_before
)

(sourcefile
  (comment) @append_antispace @append_hardline
  .
  [
    (import_statement) @prepend_antispace
    (attribute) @prepend_antispace
    (reexport_statement) @prepend_antispace
    (export) @prepend_antispace
    (component_definition) @prepend_antispace
    (struct_definition) @prepend_antispace
    (enum_definition) @prepend_antispace
    (global_definition) @prepend_antispace
    (interface_definition) @prepend_antispace
  ]
)

(sourcefile
  [
    (export)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ] @append_antispace @append_hardline
  .
  [
    (import_statement) @allow_blank_line_before @prepend_antispace
    (reexport_statement) @allow_blank_line_before @prepend_antispace
  ]
)

(sourcefile
  [
    (import_statement)
    (reexport_statement)
    (attribute)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ]
  .
  (comment)+
  .
  [
    (import_statement) @allow_blank_line_before
    (attribute) @allow_blank_line_before
    (reexport_statement) @allow_blank_line_before
    (export) @allow_blank_line_before
    (component_definition) @allow_blank_line_before
    (struct_definition) @allow_blank_line_before
    (enum_definition) @allow_blank_line_before
    (global_definition) @allow_blank_line_before
    (interface_definition) @allow_blank_line_before
  ]
)

(sourcefile
  (attribute) @append_antispace @append_hardline
  .
  [
    (export) @prepend_antispace
    (component_definition) @prepend_antispace
    (struct_definition) @prepend_antispace
    (enum_definition) @prepend_antispace
    (global_definition) @prepend_antispace
    (interface_definition) @prepend_antispace
  ]
)
