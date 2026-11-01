# Proposal

## Why

Rust parameters currently lose their parameter face at uses. Issue #334 asks for query-driven local highlighting so references retain the face of their definition.

## What Changes

- Run bundled JavaScript and TypeScript locals queries, and add a Rust locals query because its grammar crate ships none.
- Resolve references within lexical scopes, preserving the definition face through shadowing and references outside the visible window.
- Apply the same behavior inside existing Markdown language fences and refresh bindings after edits.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `syntax-highlighting`: propagate local definition faces to references while retaining visible-range painting.

## Impact

The core syntax module, a Rust query, syntax tests and the language-support documentation. No new dependencies, settings, commands or frontend changes.
