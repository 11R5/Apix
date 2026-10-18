; GraphQL highlighting for the body editor. gpui-component ships the GraphQL
; grammar without a highlight query, so Apix brings its own.

(comment) @comment
(description) @comment

(string_value) @string
(int_value) @number
(float_value) @number
(boolean_value) @boolean
(null_value) @constant
(enum_value) @constant

(variable) @variable.special
(named_type (name) @type)
(operation_definition (name) @function)
(fragment_name) @function
(field (name) @property)
(alias (name) @property)
(object_field (name) @property)
(argument (name) @variable)
(directive "@" @attribute (name) @attribute)

(operation_type) @keyword
[
  "fragment"
  "on"
  "extend"
  "schema"
  "type"
  "interface"
  "union"
  "enum"
  "input"
  "scalar"
  "implements"
  "directive"
  "repeatable"
] @keyword

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[":" "=" "|" "&" "!" "..."] @punctuation.delimiter
