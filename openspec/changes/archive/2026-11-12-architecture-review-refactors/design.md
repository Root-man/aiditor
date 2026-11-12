# Design

## Context

See proposal.md for the motivation. What shapes the approach:

- The core is a state machine, `handle(&mut Core, now, Msg) -> Effects`, with no async runtime and no clock (`core-loop` spec). Anything slow leaves as an `Effect` and comes back as a `Msg`.
- `Core::handle` (`lib.rs:1216`) ends with 13 passes that run after every message. Three of them (`sync_terminals`, `sync_agents`, `sync_session_lists`) exist because a buffer can be killed at five sites (`visiting.rs:561`, `workspace.rs:128`, `minibuffer.rs:1043`, `:1156`, `:1170`) and none can tell the side maps `Core.terminals`, `Core.agents` and `Core.session_lists`.
- `Syntax` (`syntax.rs`) already supports what off-thread parsing needs: `edit` shifts the tree, its injections and its locals and sets `stale`; `reparse` reuses the old tree; `snapshot` makes an independent copy; a `Rope` clone is O(1). Parsing runs today from `window_display` and `session_display` (`frame.rs:337`, `:458`), from `syntax_snapshot` (`minibuffer.rs:322`) and from `Syntax::for_file` on open (`buffer.rs:186`).
- `Session` (`agents.rs:75`) keeps `messages`, `tools`, `plan`, `asks`, `answers`, `run` and `groups`, each with start and end markers. `Session::block_at` (`agents/groups.rs:525`) probes them one kind after another. The 2026-11-09 scan already gave every block an end marker and added the stable point for streamed messages and the append path for open tool output; this design keeps both fast paths.
- About 230 tests in `agents.rs` assert the session buffer's exact text and properties. They are the safety net for #1 and #3 and must pass unchanged.
- The effect runner is written in `editor.rs:476`, `bin/aiditor-headless.rs:67`, `tests/file_sync.rs:92` and `tests/scenario.rs:77`. The headless one is synchronous on purpose: a save finishes before the next key.

## Goals / Non-Goals

**Goals:**

- Land the five items as separate steps in this order: #4, #3, #2, #1, #5. #4 first so the parse effect of #2 is added to one runner. #3 before #1 so a session's link to its buffer is settled before its blocks move.
- Every step leaves `cargo test` green on its own. No step needs a later one.
- #1, #3, #4 and #5 change no observable text, face, effect or message.

**Non-Goals:**

- Changing how markers or properties are stored. Properties hold two markers each and make up most of a long transcript's markers; the block list does not reduce them. The new prompt-typing benchmark records the cost.
- A second session renderer (frame 7) or re-wrapping on a width change. #1 only makes the place where one would go.
- A parse time budget in the core. The core reads no clock, so the cut is by size.
- Making `aiditor-headless` asynchronous.

## Decisions

### D4. One runner that returns what the caller must schedule (#4)

`aiditor-services` gets `runner.rs`:

```rust
pub struct Runner { pub tx, pub grep, pub terminals, pub agents, pub watcher, pub recovery, pub config_dir }
pub enum Run {
    Done,
    /// A new terminal's or agent's channel, for the caller to drain into the core.
    Drain(Receiver<Msg>),
    /// Blocking work whose result is one message; the caller picks the thread.
    Blocking(Box<dyn FnOnce() -> Msg + Send>),
    /// A message for the core right away, such as a recovery failure.
    Reply(Msg),
    /// Clipboard, open and quit, which only the caller can do.
    Platform(Effect),
}
impl Runner { pub fn run(&mut self, effect: Effect) -> Run; pub fn shutdown(&mut self); }
```

The match in `run` is exhaustive with no `_` arm, so a new `Effect` fails to compile until it is placed. `File`, `LoadConfiguration` and the new parse effect return `Blocking`; the app runs it on `spawn_blocking`, the tests call it inline, which keeps `file_sync.rs` deterministic. `CaptureEnvironment` spawns on the Tokio runtime as it does today. `shutdown` is today's quit sequence: join the recovery thread, kill terminals and agents.

The fields are public and the caller builds the parts, so `editor.rs` keeps its test hook on the watcher and `file_sync.rs` keeps its own watcher. A test may take an effect before the runner sees it (`file_sync.rs` holds file operations to reorder them; `scenario.rs` panics on effects it does not expect).

*Alternative:* a runner that does everything asynchronously itself. Rejected: `file_sync.rs` would have to wait for file replies, and `docs/flaky-tests.md` already lists timing failures. *Alternative:* move headless onto the runner too. Rejected: it would need the watcher, a runtime and an inline grep mode to save two match arms.

### D3. `Buffer.kind`, one file state, one kill function (#3)

```rust
pub(crate) enum Kind {
    Text,
    Terminal { id: TerminalId, terminal: Box<Terminal>, copy: Option<Rope>, commands: Vec<ShellCommand> },
    Session { agent: AgentId, prompt: MarkerId },
    WorkspaceList { snapshot: workspace_list::Snapshot, rows: Vec<(usize, workspace_list::Row)> },
    SessionList(sessions::SessionList),
}
```

- `Content` goes away: `Buffer` holds `text: Rope` and `kind`. `Kind::Terminal` carries what `Content::Terminal` carries today, moved as it is.
- `Session` stays in `Core.agents`, keyed by `AgentId`, because `Core::session_buffer` borrows the session and its buffer apart. The buffer holds only the id, which turns the 16 `agents.values().find(|s| s.buffer == …)` scans into `agents.get`.
- `Core.terminals` (`TerminalId → BufferId`) stays: output arrives by terminal id. `Core.session_lists` moves into its buffer's kind.
- The file-only fields (`file`, `crlf`, `bom`, `disk`, `deleted`, `conflict`, `conflict_asked`, `head`, `head_asked`, `language_version`, `version_asked`, `version_request`) move into `file: Option<FileState>`. This is about 150 mechanical call-site edits with no behavior behind them, so it is the last task of the group and lands alone.
- `Core::kill_buffer` (`visiting.rs:555`) becomes the only non-test caller of `Buffers::kill`. It matches the returned buffer's kind and pushes what the three sync passes push today onto `Core.released: Effects`: `TerminalOp::Kill`; for a session, the cancelled replies to its open asks and `AgentOp::Kill` or `AgentOp::Delete`. It also removes the `terminals` and `agents` entries. `handle` appends `released` to its effects where `sync_terminals` and `sync_agents` run today, so the effect order is unchanged.
- What the sync passes do besides finding dead buffers stays: `sync_terminals` keeps sizing terminals to their windows, `sync_session_lists` keeps re-rendering lists whose rows' states changed.

*Alternative:* move `Session` into `Kind::Session`. Rejected: every handler that needs the session and another buffer (the stderr log, a tool's output buffer) would fight the borrow checker. *Alternative:* leave file state flat. Rejected only because the request covers it; it is isolated so it can be dropped at review without touching the rest.

### D2. Parse large buffers through an effect (#2)

- **The cut is 64 KiB of text** (`Rope::len_bytes`), a constant in `syntax.rs`. At about 2 ms per 1,000 lines for a whole reparse (the 2026-10-25 scan), a buffer under the cut costs a few ms in the worst case. Buffers under it keep today's path, so the existing syntax, display and frame tests, whose fixtures are small, pass unchanged.
- **Asking.** `Effect::Parse { buffer, parse }` carries an opaque `Parse` holding a `Syntax::snapshot()` and the rope clone, with the hand-written `PartialEq` and `Debug` that `Request` has. `Parse::run(self) -> Msg` calls the existing `Syntax::reparse` and returns `Msg::Parsed { buffer, parse }`. It lives in the core, so the core still names no runtime; the runner returns it as `Run::Blocking`.
- **One outstanding parse per buffer.** `Syntax` gains `parsing: bool` and `since: Vec<InputEdit>`. `Syntax::edit` pushes onto `since` while `parsing`. A new step in the `handle` tail, where `ask_heads` runs, sends a parse for each large buffer that is `stale` and not `parsing`.
- **Arriving.** `Msg::Parsed` for a live buffer replaces `tree` and `injections` with the result's (its own `locals` already saw every edit and need no copy), then applies each edit in `since` to them with the existing `Syntax::edit` path, clears `since` and `parsing`, and leaves `stale` set if `since` was not empty, so the tail asks again. A result for a killed buffer is dropped. No tick comparison is needed: the edits are replayed, so a late tree is never thrown away and typing cannot starve the colours.
- **Drawing meanwhile.** `Buffer::reparse` does nothing for a large buffer; `highlights` already works on an edited tree. `syntax_snapshot` (swiper previews) hands out the edited tree for a large buffer.
- **Opening.** `Syntax::for_file` on a large rope returns a `Syntax` with an empty tree, `stale` set, so the first frame is uncoloured and the tail asks for the parse. An empty tree is `parser.parse("", None)`, which costs nothing.
- `aiditor-headless` runs `Parse::run` inline, as it runs file operations.
- **The locals cache keeps its index.** Timing after the move showed an edit frame on a 100k-line Rust file spending 10 of its 13 ms building two maps of every fragment in the locals cache, once for reuse and once to prune `facts` and `resolved`. The cache keeps that index instead: an edit removes the edited ids, a rebuild inserts the fragments it makes, and only old fragments the new tree did not reuse are pruned, so the work follows the edit, not the file.

*Alternative:* parse everything through the effect. Rejected: every file would open uncoloured for a frame, and every test that checks colours after a key would need a parse round trip. *Alternative:* try a synchronous parse with a 1 ms timeout first, as Zed does. Rejected: the core reads no clock. *Alternative:* drop a result whose tick is old. Rejected: under steady typing no result would ever be current.

### D1. One ordered block list and one function that writes a block (#1)

```rust
struct Block { start: MarkerId, end: MarkerId, kind: BlockKind }
enum BlockKind { Line, Message(Message), Thought(Thought), Tool(ToolCallId), Plan, Ask(Ask), Answered(Answered), Group(Group), Working }
```

- `Session.blocks: Vec<Block>` is the transcript in order. Blocks never reorder, so their start markers ascend and `block_at` is a binary search by position, then, inside a `Group`, the same search over its rows (`Group.rows: Vec<Block>`). The precedence `block_at` has today (ask, paste, answer, tool, thought, group) falls out of the nesting, except pastes, which live in the prompt area and keep `paste_at`.
- Tool data stays in `Session.tools: HashMap<ToolCallId, Tool>`, as `Row::Tool` refers to it today; `Tool`, `Ask`, `Answered`, `Thought` and `Group` lose their `start` and `end` fields to their `Block`.
- **Rendering is a pure function per kind:** `fn render(&self, block, ctx) -> Rendered`, where `Rendered { from: usize, text: String, props: Vec<(Range<usize>, Prop)>, meta: Option<String> }`. `from` is how many chars from the block's start are kept. Most kinds return `from: 0`. A streaming message returns its stable point and an open tool call past the 20-line cut returns the cut, which keeps the two fast paths of the 2026-11-09 scan.
- **One function writes it:** `Session::write(&mut self, buffer, at: BlockPath, rendered)` replaces `start + from .. end` with `text`, removes the properties in that range, adds `props` shifted, sets the end marker and rewrites the meta line under the block. It replaces the marker repair now spread over `output`, `show_tool`, `append_tool_output`, `redraw_question`, `trim_message`, `close_wait`, `retime` and `show_plan`.
- **A debug check keeps it honest:** in debug builds, after `write`, rendering the block with `from: 0` must equal the buffer's text and properties over the block, the way `render::check_message` checks a streamed message today. Every existing agent test then also tests the fast paths.
- **Order of migration:** one kind per step, each with all agent tests passing: `Line` and `Message` (with `Stream`), `Plan` and `Working`, `Tool`, `Ask` and `Answered`, then `Thought` and `Group` with the run that forms a group. A kind not yet moved keeps its old collection; `block_at` asks the list first and falls back to the old probes until the last kind moves, when the fallback and the old collections are deleted.

*Alternative:* render the whole transcript from the model and diff it into the buffer. Rejected: a frame with new chunks would cost the transcript's length; the stable point exists to avoid that. *Alternative:* draw blocks straight into display lines without a rope. Rejected: motion, search, copy, folds and swiper all read the rope.

### D5. Measure the tail before changing it (#5)

A new `hot_paths` case, `handle-tail/terminal-output-200-file-buffers`, handles a one-byte `Msg::TerminalOutput` in a core with 200 visited files over two watched projects with 2,000 indexed directories. It measures the message, which is nearly all tail.

- At or under 20 µs median (about 3% of the 0.6 ms a 64 KiB terminal batch takes to parse): record the number in `docs/refactoring-opportunities.md` and drop #5.
- Over 20 µs: profile which pass costs it. If it is `watch_set` (`watching.rs:78`), keep the last set in `Watch` with a `dirty` flag that the inputs' setters raise (a buffer's file state through D3's `FileState`, project and workspace changes, an accepted index walk). A debug assertion compares the kept set with a fresh one after every message, so a setter that forgets the flag fails the test suite. Another pass over the line gets the same treatment only if the profile names it.

*Alternative:* add dirty flags now. Rejected: the passes are a few linear scans, nothing has measured them as a cost, and a missed flag would be a stale watch set.

## Risks / Trade-offs

- [#1 rewrites the bookkeeping of 3,000 lines the session tests depend on] → one kind per step, the debug equality check after every write, and no test edited except those that reach into a removed field.
- [A stale tree draws wrong colours for a few ms in large buffers] → accepted and specified; text the edit did not touch keeps its colours.
- [`Syntax` must be `Send` to cross to the service] → a compile-time assertion in `syntax.rs`; `Tree` and `Parser` are `Send`, and the grammars are statics.
- [`Run::Blocking` leaves scheduling to each caller, so callers can differ] → three callers, each one line; the alternative hid the difference `file_sync.rs` needs.
- [The effect order at the end of `handle` could shift when the sync passes stop pushing kill effects] → `released` is appended at the same place, and the existing terminal and agent tests assert effect order.
- [The `FileState` move touches about 150 sites] → purely mechanical, last in its group, its own commit, droppable.
- [The 64 KiB cut puts two parse paths in the code] → both end in the same `Syntax::reparse`; a test parses the same edits both ways and compares the colours.
- [`syntax-edit-frame` and `revert-apply-frame` numbers at 10k and 100k lines drop because the parse left the measured frame] → the report says so and a new case times `Parse::run` alone.

## Migration Plan

Five steps in the order #4, #3, #2, #1, #5, each a set of commits that builds and passes on its own. Rollback is reverting a step's commits; later steps depend on earlier ones only as noted in Goals. No data format, setting or file on disk changes.
