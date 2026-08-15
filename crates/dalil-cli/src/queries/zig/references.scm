(call_expression
  function: (identifier) @reference.function)

(field_expression
  member: (identifier) @reference.field)

(parameter
  type: (identifier) @reference.type)

(variable_declaration
  type: (identifier) @reference.type)

(container_field
  type: (identifier) @reference.type)

(function_declaration
  type: (identifier) @reference.type)

(error_union_type
  (identifier) @reference.type)

(identifier) @reference.identifier
