# Design

## Context

aiditor's agent service (`aiditor_services::agents::Agents`) spawns an agent, sends `initialize` with empty client capabilities and `session/new` with the `cwd`, then prompts. The core answers every agent request with `method_not_found` and has no `session/set_mode` command yet. There is no computer-use tool here, so the GUI cannot be driven to record; the user chose a scripted driver.

## Goals / Non-Goals

**Goals:**
- A transparent recorder usable in front of any agent, from the GUI's registry or from a test.
- One scrubbed transcript per adapter, recorded with exactly the messages aiditor sends.

**Non-Goals:**
- Replaying transcripts against the client (later ACP-client tests).
- Answering permissions or setting modes: recorded as aiditor does them today (refused, none).

## Decisions

- **Line-based std threads, no Tokio.** ACP over stdio is newline-delimited JSON, so the recorder reads lines with `BufRead` in two threads, writes each line through unchanged and appends `{"from", "message"}` to OUT under a mutex, flushed per line so a SIGTERM to the process group loses nothing. The child is not put in its own group, so aiditor's `killpg` reaches both.
- **Driver is an ignored integration test** (`tests/transcripts.rs`, `record_claude` / `record_codex`), run with `cargo test -p aiditor-services --test transcripts -- --ignored`. It uses `Agents` directly: spawn the recorder in front of the adapter, wait for `AgentStarted`, send each prompt as one text block (as `agent-send-input` does), answer every `AgentRequest` with `method_not_found` (as the core does), wait for `AgentPrompted`, then kill. So the wire traffic is aiditor's, without a GUI. Each driver passes `env` the way a registry entry does; the agent's stderr is printed with `--nocapture`.
- **Scratch project.** A temp dir with `notes.md` to read and edit; the prompts are the same for both. Claude starts in `default`, so its edit is the permission request: a project's `.claude/settings.json` can't start it in `acceptEdits`, as the adapter drops an escalating `defaultMode` from an untrusted project, and allow rules there didn't apply either. Codex starts in `workspace-write` (`INITIAL_AGENT_MODE`), so its edit goes through and `curl` asks; in its default `agent` mode a reviewer model approves instead of the client, and `/tmp` is writable in its sandbox.
- **Scrubbing in the driver.** After the session the driver replaces the canonical and as-given temp dir with `{cwd}`, `$HOME` with `{home}` and every email with `user@example.com` (both adapters report the account's), and cuts each `available_commands_update` to its first 3 commands, as the rest name the user's skills and their organization's apps (the user's choice). A test checks the committed files parse and that none of these remain, including in the joined streamed text, since a path once came split across two chunks. Tokens, ids and other private paths are checked by hand with the README's grep.
- **Mode change.** Claude is asked to enter plan mode, which its adapter reports as `current_mode_update`; Codex cannot change its mode by itself, so its transcript has none.

## Risks / Trade-offs

- Real agents are not deterministic; transcripts are a sample, re-recorded by hand when adapters change.
- A refused permission makes the agent's tool fail, so later steps differ from what a client that answers would see; Claude's edit never lands.
- codex-acp 1.13 bundled a Codex too old for the account's model, failing every turn; the README says to update the adapters first.
