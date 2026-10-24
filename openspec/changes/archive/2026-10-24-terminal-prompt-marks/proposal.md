# Proposal

## Why

M2 calls for shell integration marks: later commands (jump to the previous prompt, copy a command's output, start `find-file` in the shell's directory) need to know where each prompt, command and output sits in a terminal and what the shell's directory is. alacritty_terminal drops OSC 133 and OSC 7, so the core must pick them out itself (#243).

## What Changes

- A terminal keeps OSC 133 marks: `A` (prompt), `B` (input), `C` (output) and `D` (end, with its exit code when given), each at the absolute line (scrollback plus screen) of the cursor where it arrived.
- A terminal keeps the shell's cwd from OSC 7 (`file://host/path` or `kitty-shell-cwd://host/path`), percent-decoded, only when the host is empty, `localhost` or this machine and the path is absolute.
- Marks are ignored on the alternate screen; clearing the scrollback (ED 3, or a full reset) drops them; once the scrollback is full, marks are dropped and no longer kept.
- Parsing a batch with no OSC stays about as fast as before (core benchmark).
- Nothing reads the marks or the cwd yet; this change only records them.

## Capabilities

### New Capabilities

### Modified Capabilities
- `terminal-buffers`: adds prompt marks and the shell's cwd read from terminal output.

## Impact

- `crates/aiditor-core/src/terminal.rs`: `Terminal::feed` runs vte's low-level parser alongside alacritty's; new `marks` and `cwd` accessors.
- `crates/aiditor-core/Cargo.toml`: `libc` as a direct dependency for `gethostname` (already in the tree through alacritty_terminal).
- `crates/aiditor-core/benches/core.rs`: the existing 64 KB batch benchmarks cover the no-OSC case.
- `docs/editor.md`: the Terminal paragraph records what was built.
