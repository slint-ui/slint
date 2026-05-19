; Keep literals and comments stable while the surrounding layout evolves.
[
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
; Allow top-level comments to be seperated from the following item by a blank line (e.g. for license headers)
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
  name: (user_type_identifier) @prepend_space @append_space
  base_type: (user_type_identifier) @prepend_space
)

[ "import" "from" "as" "global" "struct" "enum" ] @append_space

[ "property" "callback" "function" "animate" "if" "for" "when" "else" "return" ] @append_space

[ "private" "public" "pure" "changed" ] @append_space

[":=" "<=>" "=>"] @prepend_space @append_space
":" @prepend_antispace @append_space

[ "." "(" "[" "<" ] @append_antispace
[ "." ")" "]" ";" "," ">" ] @prepend_antispace

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

(property
  "<" @append_antispace
  ">" @prepend_antispace @append_space
)

(arguments) @prepend_antispace

; Blocks should read like ordinary Slint by default while still allowing
; simple one-liners to stay inline.
(block
  "{" @prepend_space @append_indent_start
  (_)+ @prepend_spaced_softline
  "}" @prepend_spaced_softline @prepend_indent_end
)

(imperative_block
  "{" @prepend_space @append_indent_start
  (_)+ @prepend_spaced_softline
  "}" @prepend_spaced_softline @prepend_indent_end
)

(global_block
  "{" @prepend_space @append_indent_start
  (_)+ @prepend_spaced_softline
  "}" @prepend_spaced_softline @prepend_indent_end
)

(struct_block
  "{" @prepend_space @append_indent_start
  (_)+ @prepend_spaced_softline
  "}" @prepend_spaced_softline @prepend_indent_end
)

(enum_block
  "{" @prepend_space @append_indent_start
  (_)+ @prepend_spaced_softline
  "}" @prepend_spaced_softline @prepend_indent_end
)

(animate_body
  "{" @prepend_space @append_indent_start
  (_)+ @prepend_spaced_softline
  "}" @prepend_spaced_softline @prepend_indent_end
)

; Anonymous object literals follow the same inline-vs-multiline decision, but
; they should not force a space before the opening brace in expression position.
(anon_struct_block
  "{" @append_indent_start
  (_)+ @prepend_spaced_softline
  "}" @prepend_spaced_softline @prepend_indent_end
)

; Imports usually stay grouped. Definitions are easier to scan with a blank line.
(sourcefile
  (import_statement) @append_antispace @append_hardline
  .
  (import_statement)
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
  ]

  (#delimiter! "\n\n")
)

(sourcefile
  [
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
  ] @append_antispace @append_delimiter
  .
  [
    (export)
    (component_definition)
    (struct_definition)
    (enum_definition)
    (global_definition)
  ]

  (#delimiter! "\n\n")
)
