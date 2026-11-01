((function_item) @local.scope
 (#set! local.scope-inherits false))
[(block) (closure_expression)] @local.scope

(parameter pattern: (identifier) @local.definition)
(closure_parameters (identifier) @local.definition)
(let_declaration
  pattern: (identifier) @local.definition
  value: (_) @local.definition-value)
(let_declaration
  pattern: (identifier) @local.definition
  !value)

(identifier) @local.reference
