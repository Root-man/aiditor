# Design

## Context

Transcripts are JSON lines `{"from": "client" | "agent", "message": ...}` (#286). The client sends `initialize`, `session/new`, then each `session/prompt` after the previous turn ends; the agent sends ext notifications such as `_auth/status_update`, `session/update`s, `session/request_permission` (recorded id `0`) and responses. Client request ids are fresh UUIDs on every run. The agent service's `Client` handles `session/update` typed and passes agent requests to the core, which refuses them.

## Goals / Non-Goals

**Goals:**
- Replay what the adapters really sent through the real agent service and core.
- A failure that names the message the replay didn't expect.

**Non-Goals:**
- Checking request params or the client's answers beyond method and id; later epics add their own assertions (session buffer snapshots, permission answers) to the same test.
- Replaying timing: messages are written as fast as the client reads them.

## Decisions

- **Replay lives in the fake agent, as plain JSON lines.** `AIDITOR_FAKE_AGENT_REPLAY` picks the mode before the ACP builder starts, as the script is picked by `AIDITOR_FAKE_AGENT`; an env variable fits registry entries' `env` the way the script does. The ACP crate's typed agent would reject or reshape what the adapters send (ext notifications, fields the schema lacks), so replay reads and writes raw lines with `std::io`, synchronously: write every agent line up to the next client line, read one line, check it, repeat.
- **Matching.** A request or notification matches on `method`; a response matches on `id`, which is the recorded agent request's id since replay sends that id unchanged. A matched client request maps its recorded id to the live one, and agent responses with a mapped id get the live one. `{cwd}` is replaced with the live `session/new` `cwd`, JSON-escaped, in the line's text, so it reaches paths inside strings too.
- **Failing.** A mismatch writes `unexpected client message: <line>` and `expected: <recorded line>` (or `expected: the end`) to stderr and exits 1; the agent service passes stderr to the core as `AgentStderr` before `AgentExited`, and the test collects stderr and puts it in every panic it raises.
- **The test drives the core.** As `agents.rs`'s `Live` does: settings register `[agents.replay]` with the fake and the variable, `M-x agent-start`, then each recorded prompt's first text block is inserted and sent with `RET`; agent messages are pumped into the core and its `Effect::Agent` ops run on `Agents`. It counts `AgentUpdate`s and `AgentRequest`s against the transcript's `session/update`s and agent requests, and checks each `AgentPrompted` against the recorded `stopReason`. The unexpected-message test sends `C-c C-c` right after the first prompt, so the core sends `session/cancel` before the replay's next expected `session/prompt`, and expects the panic with `#[should_panic]`.

## Risks / Trade-offs

- Matching by method alone lets a client that sends wrong params pass; params are left to later, targeted assertions.
- A client that waits for something the transcript never sends hangs until the test's 10 s message timeout, which fails with the stderr so far.
