; Keep literals and comments stable while the surrounding layout evolves.
[
  (rust_attr)
  (line_comment)
  (block_comment)
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

[
  (line_comment)
  (block_comment)
] @multi_line_indent_all
[
  (line_comment)
  (block_comment)
] @prepend_input_softline @append_input_softline
[
  (line_comment)
  (block_comment)
] @allow_blank_line_before
; Add a special case for single-line comments that end with the ignore directive,
; so that they can be used to disable formatting for the following item without forcing a blank line.
;
; Note: We must use a topiary-compatible capture name for the comment, otherwise topariy rejects the query.
((line_comment) @prepend_input_softline
  . (_) @leaf
  (#eq? @prepend_input_softline "// slint-fmt:ignore"))
 

; Round one focuses on the highest-signal spacing choices first.
(exported_definition
  "export" @append_space
)

(export_statement
  "export" @append_space
)

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

[ "(" "[" ] @append_antispace
[ ")" "]" ";" "," ] @prepend_antispace

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

(export_statement
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

(keys
  "+" @append_space @prepend_space)
(keys
  "?" @prepend_antispace)

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

(unary_expression
  op: _ @append_antispace)

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
  [
    (animate_statement) @prepend_spaced_softline @allow_blank_line_before
    (binding_alias) @prepend_spaced_softline @allow_blank_line_before
    (callback) @prepend_spaced_softline @allow_blank_line_before
    (callback_alias) @prepend_spaced_softline @allow_blank_line_before
    (callback_event) @prepend_spaced_softline @allow_blank_line_before
    (changed_event) @prepend_spaced_softline @allow_blank_line_before
    (children_identifier) @prepend_spaced_softline @allow_blank_line_before
    (component) @prepend_spaced_softline @allow_blank_line_before
    (for_loop) @prepend_spaced_softline @allow_blank_line_before
    (function_definition) @prepend_spaced_softline @allow_blank_line_before
    (if_statement) @prepend_spaced_softline @allow_blank_line_before
    (property) @prepend_spaced_softline @allow_blank_line_before
    (property_assignment) @prepend_spaced_softline @allow_blank_line_before
    (states_definition) @prepend_spaced_softline @allow_blank_line_before
    (transitions_definition) @prepend_spaced_softline @allow_blank_line_before
    ; comments may "hang" from the end of, so should not have a new line prepended
    (line_comment) @allow_blank_line_before
    (block_comment) @allow_blank_line_before
  ]*
  "}" @prepend_indent_end @prepend_empty_softline
)

; only prepend a space to the closing } if there is content inside
(block
  (_)+
  (#single_line_only!)
  "}" @prepend_space)

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

(value_list
  "[" @append_indent_start @append_empty_softline
  (_)+ @allow_blank_line_before @prepend_spaced_softline
  "]" @prepend_indent_end @prepend_empty_softline)


(in_out_transition
  [ "in" "out" "in-out" ] @append_space
  "{" @prepend_space @append_indent_start @append_spaced_softline
  (animate_statement)* @allow_blank_line_before
  "}" @allow_blank_line_before @prepend_spaced_softline @prepend_indent_end
)

(in_out_transition
  (animate_statement) @append_spaced_softline
  .
  (animate_statement) @allow_blank_line_before
)

(states_definition
  "states" @append_space
  "[" @append_indent_start @append_spaced_softline
  (state_definition)* @prepend_spaced_softline @allow_blank_line_before
  "]" @prepend_indent_end @prepend_spaced_softline 
)

(state_definition
  "{" @append_indent_start @append_spaced_softline
  [
   (in_out_transition) @prepend_spaced_softline @allow_blank_line_before
   (assignment_block) @prepend_spaced_softline @allow_blank_line_before
   (assignment_expr) @prepend_spaced_softline @allow_blank_line_before
   _
  ]*
  "}" @prepend_indent_end @prepend_spaced_softline
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

; Top-level items should always break onto their own line, while preserving
; existing blank lines between user-defined sections.
(sourcefile
  [
    (import_statement)
    (export_statement)
    (rust_attr)
    (exported_definition)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ] @append_antispace @append_hardline
  .
  [
    (import_statement) @allow_blank_line_before @prepend_antispace
    (export_statement) @allow_blank_line_before @prepend_antispace
    (rust_attr) @allow_blank_line_before @prepend_antispace
    (exported_definition) @allow_blank_line_before @prepend_antispace
    (component_definition) @allow_blank_line_before @prepend_antispace
    (struct_definition) @allow_blank_line_before @prepend_antispace
    (enum_definition) @allow_blank_line_before @prepend_antispace
    (global_definition) @allow_blank_line_before @prepend_antispace
    (interface_definition) @allow_blank_line_before @prepend_antispace
  ]
)

(sourcefile
  [
    (import_statement)
    (export_statement)
    (rust_attr)
    (exported_definition)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
    (interface_definition)
  ] @append_antispace @append_hardline
  .
  [
    (line_comment)
    (block_comment)
  ] @allow_blank_line_before
)

(sourcefile
  [
    (line_comment)
    (block_comment)
  ] @append_antispace @append_hardline
  .
  [
    (import_statement) @allow_blank_line_before @prepend_antispace
    (rust_attr) @allow_blank_line_before @prepend_antispace
    (export_statement) @allow_blank_line_before @prepend_antispace
    (exported_definition) @allow_blank_line_before @prepend_antispace
    (component_definition) @allow_blank_line_before @prepend_antispace
    (struct_definition) @allow_blank_line_before @prepend_antispace
    (enum_definition) @allow_blank_line_before @prepend_antispace
    (global_definition) @allow_blank_line_before @prepend_antispace
    (interface_definition) @allow_blank_line_before @prepend_antispace
  ]
)
