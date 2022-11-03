; Copyright © SixtyFPS GmbH <info@slint-ui.com>
; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

(user_type_identifier) @type

(var_identifier) @variable

(state_identifier) @field

(var_identifier
  (post_identifier) @variable)

(function_identifier) @function

(reference_identifier) @keyword
(visibility_modifier) @include

(comment) @comment

(value) @number
(string) @string

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
"this"
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

[
 "black"
 "transparent"
 "blue"
 "ease"
 "ease_in"
 "ease-in"
 "ease_in_out"
 "ease-in-out"
 "ease_out"
 "ease-out"
 "end"
 "green"
 "red"
 "red"
 "start"
 "yellow"
 ] @constant.builtin


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

[
"angle"
"bool"
"brush"
; "color" // This causes problems
"duration"
"easing"
"float"
"image"
"int"
"length"
"percent"
"physical-length"
"physical_length"
"relative-font-size"
"string"
] @type.builtin

