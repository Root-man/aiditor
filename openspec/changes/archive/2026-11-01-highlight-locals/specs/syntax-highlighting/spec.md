## ADDED Requirements

### Requirement: Local references retain definition faces
Languages with a locals query SHALL draw a resolved local reference with its definition's syntax face, including dotted-name fallback and theme overrides. Rust SHALL support function parameters and simple local bindings; JavaScript, TypeScript and TSX SHALL use their bundled locals queries. Scope nesting, shadowing, definition-value ranges and scope inheritance settings SHALL control resolution. Highlight query predicates about `local` SHALL be respected. A definition outside the visible range SHALL still determine a visible reference's face. Existing Markdown language fences SHALL have the same local highlighting as standalone files, with separate bindings per fence. Edits, undo and redo SHALL refresh local highlighting on the next draw.

#### Scenario: Rust parameter uses
- **GIVEN** a Rust function `fn f(value: i32) { consume(value); consume(value); }`
- **WHEN** the buffer is drawn
- **THEN** the parameter and both uses have the `variable.parameter` face

#### Scenario: Shadowing and scope exit
- **GIVEN** a parameter used before, inside and after a block that defines a local of the same name
- **WHEN** the buffer is drawn
- **THEN** uses before the local's initializer ends retain the parameter face, subsequent uses inside the block have the local definition face, and uses after the block have the parameter face

#### Scenario: Local shadows a builtin
- **GIVEN** JavaScript declares a local called `console` or `require`
- **WHEN** its references are drawn
- **THEN** their faces follow the local definition and query patterns restricted to nonlocals apply to unresolved builtins

#### Scenario: Offscreen definition and theme change
- **GIVEN** a Rust parameter is above the visible range
- **WHEN** its uses are drawn and the theme's parameter face changes
- **THEN** only requested ranges are returned and visible uses take the current parameter face

#### Scenario: Separate Markdown fences
- **GIVEN** one Rust fence defines and uses a parameter and another fence uses the same name without a definition
- **WHEN** the file is drawn
- **THEN** the first fence uses the parameter face and the second uses its ordinary syntax face

#### Scenario: Editing bindings
- **GIVEN** a function parameter and a reference of the same name
- **WHEN** the parameter is renamed, then the edit is undone and redone
- **THEN** each draw uses the bindings in the current text
