# Proposal

## Why

Copy mode's prompt jumps and output selection ([#245](https://github.com/Root-man/aiditor/issues/245), [#246](https://github.com/Root-man/aiditor/issues/246)) and the shell-following directory ([#244](https://github.com/Root-man/aiditor/issues/244)) need OSC 133 and OSC 7. zsh and starship send neither, and Ghostty's integration only loads in shells Ghostty starts, so a plain zsh terminal gets none of these features.

## What Changes

- When a terminal runs the default shell and that shell is zsh, the PTY service starts it with `ZDOTDIR` pointing at a folder aiditor writes, as Ghostty does.
- That folder's `.zshenv` restores the user's `ZDOTDIR` (or unsets it), sources the user's `.zshenv`, and in interactive shells adds hooks: `133;D;<status>` (after a command), `133;A` and `7` before each prompt, `133;B` at the end of the prompt, and `133;C` before a command runs. The hooks run after other precmd hooks, such as starship's, and add `B` to the prompt once.
- Bash, fish and explicit commands start as before (a `shortcut:` comment).
- The decision is recorded in `docs/editor.md`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `terminal-processes`: adds a requirement for zsh shell integration on the default shell.

## Impact

- `crates/aiditor-services/src/terminal.rs`: `Terminals` gets the integration folder; spawn writes the `.zshenv` and sets `ZDOTDIR`.
- A new `crates/aiditor-services/src/zshenv` script, compiled into the binary.
- `crates/aiditor/src/main.rs` and `editor.rs`: pass `<data dir>/shell/zsh` to `Terminals`.
- `crates/aiditor-services/tests/terminal.rs`: tests driving a real zsh.
- `docs/editor.md`: the Terminal paragraph.
