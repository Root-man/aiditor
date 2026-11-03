# Proposal

## Why

Prompts need to carry files, directories and images, not just text, and the session should show them as compact chips rather than raw paths (frames 6, 6b and 6c). Issue #369 builds the chip itself; `@` references, `agent-attach-file` and folded pastes, later issues, create chips through it.

## What Changes

- A chip is one char of buffer text, drawn as ` <icon> <name> ` in the boxed `attachment` face: `▤` file, `▣` image, `▦` directory. Motion, deletion, kill, yank and undo treat it as the one char it is.
- On send, the prompt goes as text blocks split around its chips: a file or directory chip as a `resource_link`, an image chip as image content read from disk when the prompt is sent.
- The echoed prompt shows its chips the same way, and so does a replayed user message: a `[@name](file://…)` link in its text (how Claude's adapter stores a sent `resource_link`) becomes a chip, and an image block an `▣ image` chip. A replayed user message's chunks with one `messageId` join one block.
- `RET` on a chip visits its file in another window, or opens `find-file` in its directory; the echo line shows the chip's full path while point is on it.
- A chip whose file is gone from disk is struck through, its icon in `status-failed`.
- Faces gain a `strike` attribute, drawn as a strikethrough.

Not in scope: commands that create chips (`@` references, `agent-attach-file`, folded pastes); the `≡` pasted-text kind comes with folded pastes.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: chips in the prompt and transcript, sending them, replaying them, `RET` and the echo on a chip, missing files.
- `display-model`: a display property that draws a char as other text in its own faces; the `strike` face attribute.

## Impact

- `crates/aiditor-core/src/agents.rs`, new `agents/chips.rs`: the chip table, painting, sending, replay, `RET`, echo, missing state.
- `crates/aiditor-core/src/buffer.rs`, `display.rs`, `settings.rs`: the display property and `strike`.
- `crates/aiditor-core/src/watching.rs`: file checks and watcher reports reach chips.
- `crates/aiditor-services/src/agents.rs`: reads image chips' files into the prompt.
- `crates/aiditor/src/element.rs`: draws `strike`.
- `docs/editor.md`: the ACP section records the behaviour.
