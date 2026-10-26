# ACP transcripts

Real sessions with `claude-agent-acp` (`claude.jsonl`) and `codex-acp` (`codex.jsonl`), recorded with aiditor's client capabilities, for tests that replay what the adapters really send. Each line is one JSON-RPC message as `aiditor-acp-record` logs it: `{"from": "client" | "agent", "message": <the message>}`.

Both sessions run in a scratch project holding `notes.md` and send the same prompts: a markdown reply, reading `notes.md`, appending a line with the file editing tool. Every agent request is refused with `method_not_found`, as the core does until permissions are answered.

- **Claude Code** (claude-agent-acp 0.95.0) starts in `default`, so its edit asks permission and is refused. It is then asked to enter plan mode, which it reports as `current_mode_update`. A project's `.claude/settings.json` can't start it in `acceptEdits`, because the adapter ignores an escalating `defaultMode` from an untrusted project.
- **Codex** (codex-acp 2.2.2) starts in `workspace-write` (`INITIAL_AGENT_MODE`), so its edit goes through and `curl` asks permission; the refusal ends that turn `cancelled`. In its default `agent` mode a reviewer model approves instead of the client. Codex can't change its own mode, so it records no mode change.

## Recording again

Log in to both agents first. The adapters must be current: codex-acp 1.13 bundled a Codex too old for the account's model, and every turn failed.

```sh
cargo test -p aiditor-services --test transcripts -- --ignored --nocapture
```

`record_claude` and `record_codex` each overwrite their file. Add `ASDF_NODEJS_VERSION=...` or similar if an adapter needs it on `PATH`. Real agents aren't deterministic, so check that the turns did what the prompts asked, and look in the transcript for errors such as `429 Too Many Requests` before committing.

## Scrubbing

The drivers scrub as they write:

- The scratch project becomes `{cwd}` and `$HOME` becomes `{home}`.
- Every email address becomes `user@example.com` (both adapters report the account's).
- Each `available_commands_update` keeps only its first 3 commands. The rest name the user's own skills and their organization's apps.

`committed_transcripts_parse_and_are_scrubbed` fails if a home path (also in streamed text, which can split a path across chunks), an email or a longer command list is left. Before committing, also look for what no rule catches: your user name, account and organization ids, tokens (`eyJ`, `sk-`, `Bearer`), and private paths outside the home directory:

```sh
grep -nE "$USER|eyJ|sk-|Bearer|/private/|/var/folders" crates/aiditor-services/tests/transcripts/*.jsonl
```

## Recording from the editor

The recorder works in front of any agent. Add a registry entry to `settings.toml`, start it with `M-x agent-start`, and scrub the file by hand:

```toml
[agents.claude-record]
command = "aiditor-acp-record"   # target/debug/aiditor-acp-record, or on PATH
args = ["/tmp/claude.jsonl", "--", "claude-agent-acp"]
```
