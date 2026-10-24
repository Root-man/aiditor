## ADDED Requirements

### Requirement: Key sequences name navigation and function keys
A key sequence SHALL be able to name Home, End, PageUp, PageDown, Delete and F1 to F12 as `<home>`, `<end>`, `<prior>`, `<next>`, `<delete>` and `<f1>` to `<f12>`, with modifiers as any key.

#### Scenario: Names parse and print
- **WHEN** `<home> <end> <prior> <next> <delete> <f1> C-<f12>` is parsed and written back
- **THEN** it reads the same
