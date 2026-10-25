# Design

## Context

Services run on the Tokio runtime and message the core (`aiditor-services/src/lib.rs`). `Terminals` is the closest sibling: the app calls `run(op)` from the UI thread, a spawn returns a per-terminal bounded receiver the app drains, and `kill_all` blocks once for a 100 ms grace on quit. The threading spike (#95) chose `agent-client-protocol` 3.5.0 on Tokio, spawning with `tokio::process` and wrapping its pipes with `tokio-util`'s compat layer into `ByteStreams`, killing the process group ourselves.

## Goals / Non-Goals

**Goals:**
- Spawn, `initialize`, `session/new`, stderr, exit, request/reply plumbing and kill, tested against a real child process.

**Non-Goals:**
- A user command, log buffers, Messages notes (#280).
- `session/update` notifications (ignored by the crate when unhandled; #280 keeps them).
- `additionalDirectories`, prompts, modes, permissions, terminals, `session/load`.

## Decisions

- **Core types.** `AgentId(u64)`; `AgentOp::{Spawn { id, command, args, cwd, env }, Reply { id, request, result }, Kill { id }}`; `Effect::Agent(AgentOp)`; `Msg::{AgentStarted { id, result: Result<NewSessionResponse, String> }, AgentStderr { id, line }, AgentRequest { id, request: RequestId, body: Request }, AgentExited { id, code, signal }}`. Schema types are carried as they are (v1). `Reply`'s result is `Result<ClientResponse, acp::Error>`. Exit mirrors `TerminalExited` (`code: u32`, `signal: Option<String>`).
- **`Request` newtype.** The schema's `AgentRequest` has no `PartialEq`, and `Msg` derives it for ~20 test comparisons. `Request(Box<AgentRequest>)` compares by its derived `Debug` text, which shows every field; the box keeps `Msg` small (clippy's `large_enum_variant`). Dropping `PartialEq` from `Msg` was the alternative, rewriting those tests with `matches!`.
- **Environment as one ordered list.** `env` is applied after the editor's own environment, in order, so later pairs win. The core will concatenate captured, registry and project variables (#280); the service needs no notion of layers. `std::process::Command` resolves the program on the child's `PATH` when the env sets it, which is what finds `claude-agent-acp` on the captured `PATH`.
- **Process.** `tokio::process::Command` with piped stdio, `process_group(0)` so the agent leads its own group, `kill_on_drop(true)` as a safety net. Spawning happens synchronously in `run`, so the handle holds the pid at once and there is no "starting" state to race with a kill. A spawn error is sent on the fresh receiver.
- **Per-agent bounded channel**, like terminals: one agent's stderr flood can't fill the shared service channel.
- **Tasks per agent.** One task drives the connection: `Client.builder().on_receive_request(AgentRequest → keep responder, send Msg::AgentRequest).connect_with(ByteStreams, main)`, where `main` sends `initialize` then `session/new`, sends `AgentStarted`, then awaits `incoming_closed()`. Any error before the started message becomes `AgentStarted(Err)`. A second task reads stderr lines, waits for the child and for the connection task to end, then sends `AgentExited`, so the exit follows the stderr and any failed start.
- **Pending requests** live in an `Arc<Mutex<HashMap<RequestId, Responder<ClientResponse>>>>` shared by the handler and the handle; `Reply` removes and answers. `Responder::respond_with_result` is sync, so the UI thread can call it.
- **Core rejection.** `Msg::AgentRequest` makes the core push `AgentOp::Reply` with `Error::method_not_found()`; the other agent messages are ignored until #280.
- **Kill.** `killpg(pid, SIGTERM)`, then a thread sleeps the 100 ms grace and sends SIGKILL; `kill_all` signals all, sleeps once, SIGKILLs all. Removing the handle drops pending responders.
- **Test agent** is a binary in `aiditor-services` (`src/bin/aiditor-test-agent.rs`), next to `aiditor-headless`, so integration tests reach it with `CARGO_BIN_EXE_aiditor-test-agent`. It reports what it received on stderr (`cwd=…`, `fs=… terminal=…`, `env A=…`), answers `initialize` and `session/new` with a session id and two modes, and acts on `AIDITOR_TEST_AGENT`: `request` sends `session/request_permission` after the session opens and reports the answer on stderr; `exit3` writes two stderr lines and exits 3; `trap` ignores SIGTERM.

## Risks / Trade-offs

- A reaped agent's group id could be reused within the 100 ms grace before SIGKILL — the same shortcut `Terminals` takes.
- Unix only (`killpg`, `process_group`), as is the rest of the services crate.
