## Why

The 2026-10-27 refactor scan (`docs/refactor-scan-2026-10-27.md`) listed six problems, and each one was confirmed against the code at `155539d`. The worst is that the core benchmark binary panics on its long-line assertion, which was written before soft wrap (#290). Because it panics, the scenarios after it never run and nothing is written to `BENCH_OUTPUT`. The other five are a quadratic step in rendering a session's markdown, layout work for wrapped rows that are never shown, a session left busy for good when the agent's connection closes, transcript colours that ignore a theme reload, and a late directory listing that can fill a newer prompt.

## What Changes

- **Benchmarks run again.** The 1 MB single-line scenario turns on truncation, so its name and assertion keep their meaning. A new wrapped-line scenario measures the default, and a new scenario measures rendering a long styled agent message.
- **Markdown properties are composed in one ordered sweep.** The faces that apply to each piece come from the ranges open at that cut. Today every piece scans every range. Inner faces still win over outer ones, and the rendered output is unchanged.
- **Only visible wrapped rows are laid out.** A window whose first row sits deep inside a long line starts laying out at that row's first char, not at the start of the line.
- **A closed connection ends the agent.** When an agent closes its stdout after its session opened, the service terminates its process group the same way a kill does. The core then gets the usual exit message and ends the turn.
- **Session transcripts follow the theme.** When reloaded settings change the theme, every colour that session buffers stored from the old theme's `dim`, `code_block`, `text.title`, `text.literal` and `text.uri` is replaced with the new theme's colour.
- **Only the current directory listing fills a prompt.** Each listing request carries an id, and a listing whose id is not the one the open prompt awaits is ignored.

## Capabilities

### New Capabilities

### Modified Capabilities
- `display-model`: a wrapped line is laid out only from the window's first row through the rows it can show, not from the line's start.
- `agent-processes`: an agent whose connection closes after its session opened is terminated, so its exit reaches the core.
- `fake-agent`: a `closeStdout` step, so tests can reproduce an agent that stops speaking while it keeps running.
- `session-buffers`: transcript colours follow a reloaded theme.
- `file-visiting`: the file prompt ignores a listing it no longer awaits.

## Impact

- `aiditor-core`:
  - `benches/scenarios/mod.rs`
  - `markdown.rs`
  - `display.rs` and `frame.rs` (wrapped layout)
  - `agents.rs` and `lib.rs` (theme remap on `SettingsLoaded`)
  - `buffer.rs` (mapping stored properties)
  - `minibuffer.rs`, `lib.rs` (`FileOp::ListDir`, `Msg::Listed`) and `harness.rs` (listing ids)
- `aiditor-services`:
  - `agents.rs` (terminate on close)
  - `bin/aiditor-fake-agent.rs` (the new step)
  - `files.rs` (echo the listing id)
- `aiditor`: the app's dispatch of `FileOp::ListDir` and `Msg::Listed`, if it names their fields.
- The markdown sweep and the bench fix change no behaviour.
