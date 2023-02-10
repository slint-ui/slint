; Copyright © SixtyFPS GmbH <info@slint-ui.com>
; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

(user_type_identifier) @type

(var_identifier) @variable

(var_identifier
  (post_identifier) @variable)

(function_identifier) @function

(reference_identifier) @keyword
(visibility_modifier) @include

(comment) @comment

(value) @number

[
"in"
"for"
] @repeat

"@" @keyword

[
"import"
"from"
] @include

[
"if"
"else"
] @conditional

[
"root"
"parent"
"self"
] @variable.builtin

[
"true"
"false"
] @boolean


[
"struct"
"property"
"callback"
"in"
"animate"
"states"
"when"
"out"
"transitions"
"global"
] @keyword

; Punctuation
[
","
"."
] @punctuation.delimiter

; Brackets
[
"("
")"
"["
"]"
"{"
"}"
] @punctuation.bracket
