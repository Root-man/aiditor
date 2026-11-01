# Tasks

## 1. Injected syntax

- [x] 1.1 Parse inline regions and the six labeled fence languages with included ranges; verify inline/fence face tests and plain fallback.
- [x] 1.2 Keep injections current with host edits and compose only visible highlight ranges; verify Unicode, continuation, boundary/language edit and incremental-versus-fresh tests.

## 2. Integration

- [x] 2.1 Add emphasis palette entries and document Markdown support; verify display harness coverage for inline faces, fenced faces, edit/undo/redo and revert.
- [x] 2.2 Run formatting, workspace clippy/tests and CI structural/Python checks; prepare a reproducible app fixture and record any remaining manual checks.

## Manual checks

The user confirmed on 2026-11-01 that all checks below pass. The session cannot control native apps, so these checks were performed by the user with the prepared scratch file:

```sh
cargo run -p aiditor -- --config-dir /tmp/aiditor-332-review/config /tmp/aiditor-332-review/sample.md
```

1. Confirm inline italic/bold, code-span and link colors with source markup visible; the six known fences have language colors and the unknown fence stays literal.
2. `C-s fn main RET C-a`, type `// `, then `C-x u`: Rust becomes a comment and undo restores its keyword/function colors.
3. Change the first fence's `rust` label to `unknown` and back: its content switches between literal and Rust colors. Scroll through the quoted emphasis and table to confirm styles remain aligned.

Native appearance and input checks passed. Separate OpenSpec verification and the shipping review follow.
