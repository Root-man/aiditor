# Design

## Context

`crates/aiditor-services/src/bin/aiditor-test-agent.rs` is an ACP agent on `agent-client-protocol`'s `Agent` builder, chosen by `AIDITOR_TEST_AGENT`. The agent service (`agents.rs`) only spawns, opens a session, forwards requests and updates, and kills; it sends no prompts yet.

## Goals / Non-Goals

**Goals:** one script format that covers every current behavior plus prompts, modes and cancel; the service tests use it.

**Non-Goals:** terminals (`terminal/create`), `session/load`, recorded-transcript replay, whole-app scenarios. Each lands with the work that needs it.

## Decisions

- **JSON, not TOML.** Updates and permission requests are ACP JSON, so the script embeds them verbatim and deserializes them into the schema's own types at load. Any update the schema has works without code, and a typo fails the load (exit 2) instead of a turn.
- **`prompts` is a list of turns, not one list repeated.** Tests of a conversation need different turns; a test that wants the same turn twice writes it twice.
- **Steps run in a spawned connection task** (`ConnectionTo::spawn`), so the dispatch loop stays free to route the permission answer and `session/cancel`. Each turn gets a `tokio_util` `CancellationToken`; `session/cancel` cancels the current one, and `sleep` and `permission` race it with `select!`.
- **Session id stays fixed** (`fake-session`), so scripts never write it; the fake inserts it.
- **`ignoreSigterm` is a step**, replacing `trap`; the test waits for a following `stderr` step so the kill comes after it.
- **Exit 3 runs after `session/new`**: there is no pre-session phase; the ported test filters for its two lines rather than expecting them alone.
- **The fake is tested directly** in `tests/fake_agent.rs` with the crate's `Client` builder, since the service can't prompt yet.

## Risks / Trade-offs

- [`exit` drops ACP messages still queued for stdout] → documented; scripts `sleep` first when it matters.
- [Steps after a turn's prompt answer are not possible] → not needed yet; a later `session` phase or background steps can be added with the work that needs them.
