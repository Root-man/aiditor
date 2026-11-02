# Design

## Context

The core is `handle(now: Instant, msg)` and never reads the clock; the frontend holds an `HH:MM` string refreshed on minute boundaries. `Prop::AlignRight` (#362) right-aligns the rest of a line, and leaves one space when it doesn't fit. Session output goes in at `prompt_start - PROMPT.len()`; `Core::frame` already flushes streamed blocks for shown buffers only.

## Goals / Non-Goals

**Goals:** frame 6's times, testable on the fake clock.

**Non-Goals:** tool groups and their durations, icons and status marks (their own issues); times in replay; a time on a message before its block ends.

## Decisions

**Wall time: `Msg::Clock(String)`.** `start_clock` sends its text to the core, which keeps `clock: Option<String>`. No text means no stamp, so tests that don't set it keep their text. Over reading jiff in the core, which breaks the fake clock, or an `Instant` anchor, which drifts across sleep.

**Times live in the rope,** after an `AlignRight` space, as #362 intended. A time is searchable and killed with its line; that matches the transcript being text. Over a line decoration drawing text outside the rope, which would need new display and wrap handling.

**Messages are stamped at arrival and written when the block ends.** The first line may still be growing while it streams, so the suffix goes in from `end_stream`, after the final render, and `Message.len` grows by it. Properties don't extend on an insert at their end (their end markers `Stay`), so the suffix stays out of them. The thought's header goes in the same way, at the block's start. `agent_prompted` and `agent_exited` now end the stream, as a turn's end must settle both.

**The working line has no marker.** `Session::turn: Option<Turn { start, len }>`, which replaces the `turn` flag, holds the line's length with its blank line, and the transcript's end is `prompt_start - PROMPT.len() - len`, one helper replacing the seven sites computing it. A marker would have to `Advance` on inserts at it, and it is always the last block anyway.

**Ticks repaint in `Core::frame`, not in the timer.** `handle` keeps its last `now` in the core. `render_streams` refreshes shown sessions from it, so a hidden session costs nothing and is current on its first frame. One repeating 1 s timer is armed in `handle` while a shown session has a turn or an ask, cancelled otherwise; the frame's shown set moves into a helper both use. A shown buffer is one in a window of the current workspace.

**Tick edits touch only what changed.** A helper replaces the time's range from the first differing char and re-adds its face, so point on the line stays and an open call isn't re-rendered each second. A header's first or last time goes through `show_tool`.

**Tool durations leave out permission waits,** since `waiting` reported them and the working line counts them. `Tool` keeps `arrived`, `waited` and `ran`; `arrived: None` marks a replayed call. `ran` is set on the first terminal status. A call that never reaches one shows no duration.

**Replay** is `session.load.is_some() && session.session.is_none()`, true until `session/load` is answered, as the service sends replayed history before the answer.

**Worked gets the clock time,** as frame 6 draws it, though the issue doesn't list it.

## Risks / Trade-offs

- A long first line wraps, and its time lands where the wrap leaves it. → The behaviour #362 specified.
- A clock stale after sleep until the frontend's next wakeup. → Under a minute; each wakeup rereads it.
- An ask for a call not shown has no counter. → Both adapters show the call first; merging them is planned.
- A message shows no time while streaming. → It gets one when its block ends.
