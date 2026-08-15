(function_declaration
  name: (identifier) @definition.function)

(variable_declaration
  ["const" "var"]
  (identifier) @definition.struct
  (struct_declaration))

(variable_declaration
  ["const" "var"]
  (identifier) @definition.enum
  (enum_declaration))

(variable_declaration
  ["const" "var"]
  (identifier) @definition.type
  [
    (union_declaration)
    (opaque_declaration)
  ])

(variable_declaration
  ["const" "var"]
  (identifier) @definition.variable)

(container_field
  name: (identifier) @definition.field)

(test_declaration) @definition.function

(builtin_function
  (builtin_identifier) @_import
  (arguments
    (string
      (string_content) @definition.import))
  (#eq? @_import "@import"))
