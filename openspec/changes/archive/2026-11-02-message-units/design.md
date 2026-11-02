# Design

## Context

`Core::output` puts a block above the prompt line; `Stream` tracks the block chunks join, by kind only. Blocks are ordinary text, tracked by a start `MarkerId` and length (tool calls, plans, asks). `ContentChunk.message_id` is `Option<MessageId>` in the schema crate, stable, and sent by both adapters. Point differs per window, and `display::highlight_line` already decorates a window's lines at frame time. Frame 6 draws the bar as a 3 px left border on the last message while point is in the prompt, past a tool group, and the face list calls it "the message C-c acts on".

## Goals / Non-Goals

**Goals:** messages as an ordered list on the session; a frame-time bar; message motion.

**Non-Goals:** joining a user message split over chunks (the existing `show_user` shortcut stands); commands acting on the message at point; the `session-user` background; using the ids for anything yet; `C-c C-n` from the last message going to the prompt.

## Decisions

**A `Vec<Message>` on `Session`**: `Message { id: Option<MessageId>, start: MarkerId, len: usize }`, as `Ask` and the plan store blocks. Output only ever goes at the transcript's end, so the list is in buffer order and starts ascend, which lets lookups bisect. Tool rewrites move the markers and no message text ever changes.

**`output` takes a block kind.** `chunk: Option<bool>` becomes `Block`: `Line`, `Message(id)`, `Thought(id)`, `User(id)`. `Stream` keeps the kind, and a chunk joins when kinds compare equal, so the id check is that one comparison. `output` pushes or grows a `Message` for `Message` and `User`. Alternative: a separate pass recording messages from `agent_update` — it would need the same positions `output` computes.

**The bar is computed per frame, not a text property.** Point is per window, and a property would need moving on every point change. `Session::message_lines(buffer, point)` bisects to the message and gives its line range; `display::mark_message` pushes the decoration on lines in that range, as `highlight_line` does.

**"At point" means last starting at or before point**, not containing it, as frame 6 shows and as `vterm-mark-output` picks a command.

**`Decoration::Bar(FaceName)`**, 3 px, not a wider `Rule`: tool call rules stay 2 px and frame 6 draws the bar at 3.

**Motion mirrors `vterm_next_prompt`**: same filter and count handling over the message starts, echo `No message found`.

## Risks / Trade-offs

- [One stop per block, so text / tool / text with one id is two stops] → Matches how it reads, and the id is still on both.
- [A new id now splits blocks where they joined before] → Agents already mean a new message by it; the protocol says so.
- [Sessions keep a marker per message] → The same as tool calls; marker lookup is a map get.
