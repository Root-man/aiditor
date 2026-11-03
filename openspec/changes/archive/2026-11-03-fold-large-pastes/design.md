# Design

## Context

See proposal.md for motivation. Session prompts are editable rope tails; properties and markers follow edits, while sending reads the original rope. TAB already handles tool folds. File chips use replacement strings in the attachment face.

## Goals / Non-Goals

**Goals:** Keep pasted input byte-for-byte intact, reuse display properties, preserve existing yank prefixes and yank-pop behavior.

**Non-Goals:** Persist paste boundaries through ACP history replay, turn ordinary typing into pastes, add a full-output buffer or a new key binding.

## Decisions

- Track pasted ranges with session-owned markers. Paint them after prompt chips and after echoed markdown. Avoid synthetic text headers, which would contaminate copy, undo and ACP input.
- Render the folded range as one replacement chip. For the open state, replace its first character with the same header, a display newline and that original character; show up to four original lines and hide the tail behind a remaining-line count. This small extension to replacement strings avoids a second copy of the paste in the buffer.
- Let TAB at any position in a paste toggle it, returning point to its start on collapse. Sending folds every paste in the sent range; later TAB can preview it in the transcript.
- Count lines using Rust's `str::lines`, so a terminal newline does not create an extra pasted line. Accept a nonnegative threshold, including zero. Settings reload affects subsequent yanks.
- Preserve the raw text under normal editing. Marker ranges follow edits; empty or invalid ranges lose their fold. Undo restores text through existing buffer history, without promising restoration of presentation metadata.

## Risks / Trade-offs

- Overlapping markdown/chip properties could override a paste → repaint the paste last and clear properties inside each paste's range.
- Streamed output or timestamps move prompt ranges → use markers, not saved offsets, and repaint after transcript rendering.
- Synthetic display lines can affect wrapping and cursor placement → cover the display projection and GPUI painting with tests.
- ACP replay has no paste-boundary metadata → replayed plain text remains plain; only the current session's sent copies retain folds.
