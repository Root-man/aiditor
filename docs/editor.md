# Editor project — architecture sketch & open questions

Oct 12, 2026 · @Boba

## Goals and principles

Build a personal, keyboard-first GUI editor for macOS in Rust, organised around **workspaces** (a primary repo plus other repos and folders) and **tabs** (everything for one task), with Emacs's buffer/window model as the interaction core. It is a private experiment; if it works, it becomes the editor used for daily work.

**Goals**

- ACP client as a first-class subsystem with built-in session management; Claude Code and Codex from day one.
- Emacs-flavoured keybindings and the Emacs buffer/window model as the default. Exact keys are chosen as each feature is built.
- Workspaces that group several repos and give every agent session the same filesystem scope.
- Tabs that hold all the files, terminals and agent sessions for one task.
- Built-in terminal, where a terminal is just another buffer; several per tab.

**MVP feature set**, mapped from current daily use:

| Today in Emacs | In this editor |
| --- | --- |
| projectile | Workspace commands: find file across all roots, switch workspace, grep the workspace |
| consult | Minibuffer completion for buffers, files, commands and grep hits, with live preview |
| swiper | In-buffer search that lists matching lines in the minibuffer |
| Buffer and window navigation | `C-x b`, `C-x o`, splits, one window configuration per tab |
| Multiple cursors | Multiple selections in the core model, not an add-on |
| In-buffer editing | Emacs motion, mark, kill ring, registers, keyboard macros |
| dired (rarely used) | After MVP |

Languages with first-class support: Elixir, Rust, JS/TS and Markdown.

**Non-goals**

- Elisp compatibility or running Emacs packages.
- A git UI or Magit replacement. Git runs in terminal buffers, and `git diff` in a terminal is how agent changes are reviewed.
- LSP and git worktree workflows in the MVP.
- Linux, Windows, a TUI frontend, plugins, collaboration, remote editing.

**Design principles**

1. **Everything is a buffer.** Files, terminals, agent sessions and the message log share one model, one buffer list and one window system.
2. **Everything is a command.** Keys and `M-x` call the same named commands, so every feature is reachable both ways.
3. **Every layer is testable on its own**, and the whole app is testable end to end (see Testing).
4. **One owner of state.** A single-threaded core loop owns editor state; I/O (agents, PTYs, file watching) runs in tasks that talk to it by messages.
5. **Agents use their own tooling, scoped by the workspace.** The editor gives each agent a working directory and extra roots, then shows what it does: its file changes appear in buffers, its terminals are terminal buffers.

## Architecture at a glance

Four layers: a thin frontend, one core loop that owns all state, async services that do I/O, and the outside processes they talk to.

&#91;embedded content: architecture · frontend, core loop, async services, outside processes\]

Arrows between layers are message channels, never shared mutable state. Dashed boxes run outside the editor. Agents reach the workspace through their own tools, scoped by `cwd` and the extra roots; there is no editor-side MCP server.

## Core data model

A workspace defines the filesystem scope; a tab is the unit of work inside it. Every tab belongs to exactly one workspace, and every terminal and agent session belongs to exactly one tab.

| Entity | What it is | Owns / refers to | Survives restart |
| --- | --- | --- | --- |
| Workspace | A primary repo plus additional repos and folders | Roots, tabs, default agent | Yes, a TOML file in the editor's config dir, outside the repos |
| Primary repo | The root whose agent config is used: AGENTS.md, CLAUDE.md, MCP server list, agent settings | Path, role line | In the workspace file |
| Additional root | Another repo or folder the agent gets filesystem access to | Path, role line | In the workspace file |
| Tab | Everything for one task, tied to one workspace | Window configuration, tab buffer list, terminals, agent session buffers | No |
| Buffer | Text plus metadata, independent of any window | Rope, undo history, major and minor modes, markers, file binding or kind | No |
| Buffer kind | File, Terminal, AgentSession, Messages, Scratch | Kind-specific payload (PTY handle, session id) | No |
| Window | A viewport onto one buffer | Buffer id, point, mark, extra cursors, scroll position | No |
| Window configuration | Split tree of windows (`C-x 2` / `C-x 3`) | Windows, split ratios, selected window | No |
| Frame | A macOS window | Shows one tab at a time | No |
| Agent session | One ACP conversation with one agent process | Agent, workspace, ACP session id, transcript, mode | Yes: index plus transcript, resumable |

A buffer never knows which windows show it, and a window holds only a buffer id. That separation keeps `C-x b`, `C-x 4 b` and per-tab window configurations cheap.

Each workspace has its own buffers; tabs in the same workspace share them. A file open in two workspaces has two buffers, kept in sync through the disk: when the file changes on disk (a save from the other workspace, an agent edit, a `git checkout` in a terminal), every buffer visiting it reverts automatically.

If the file changes on disk while its buffer has unsaved edits, the editor does not revert. It marks the buffer as conflicted, shows that in the mode line, and asks you to resolve it the next time you focus the buffer or try to save it. The choices are:

- **Keep mine**: overwrite the disk version on save.
- **Take disk**: revert to the disk version; your unsaved text stays in undo history.
- **Compare**: open a diff of buffer against disk and merge by hand.

Every save also checks the file's hash on disk against the last version the buffer loaded or saved, so a change the watcher missed is still caught before it can be overwritten.

## Tabs

A tab is everything related to one task: its file buffers, terminal buffers and agent sessions, laid out in its own window configuration. Opening a tab asks for a workspace, and the tab stays bound to it.

- **Ownership.** Terminals and agent session buffers are created in a tab and stay there. File buffers can appear in several tabs of the same workspace.
- **New tab.** Starts with one empty buffer.
- **Closing a tab** kills every buffer that belongs to it, terminals and session buffers included. The agent sessions stay in the session index and can be resumed in another tab of the same workspace.
- **Outside files.** A tab can open files from outside its workspace. What the agent may touch is set by the workspace roots and the session's approval mode, not by what is open.
- **Lifetime.** Tabs live until closed or until the app quits; they are not restored on the next launch.
- **Scoped switching.** `C-x b` and the consult-style switcher list the current tab's buffers first, with a key to widen to all buffers.
- **Commands.** New tab (prompts for workspace), switch, next/previous, rename, close, list tabs. All available as keys and through `M-x`.
- **Display.** A frame shows one tab at a time; a tab strip shows the tab names and their workspaces.

Emacs has part of this. Since Emacs 27, `tab-bar-mode` gives each tab its own window configuration, but buffers stay global. Packages such as tabspaces and perspective.el add per-tab buffer lists, and tabspaces ties tabs to projects. This design builds that combination in and adds ownership of terminals and agent sessions.

## Commands and keymaps

Copy Emacs's model, not just its key chords: named interactive commands, layered keymaps resolved by a key-sequence trie, and a minibuffer for every prompt. Bindings are Emacs-flavoured; exact keys are settled as each feature is built.

- **Command registry.** Each command has a name (`find-file`, `tab-new`, `agent-send-region`), an argument spec (prefix arg, region, prompted string), and a handler `fn(&mut Ctx, Args) -> Result`. `M-x` lists them all with fuzzy completion.
- **Keymap layering**, highest priority first: transient map (after `C-x`, during isearch) → minor-mode maps of the current buffer → major-mode map → global map.
- **Key sequences** resolve through a trie, so prefix keys (`C-x`, `C-c`, `C-x 4`) work naturally, with a which-key style hint after a short delay.
- **Prefix argument** (`C-u`, `M-<digit>`) is passed to every command.
- **Minibuffer** is a real one-line buffer with its own keymap, history rings and a vertical fuzzy completion list. It hosts the consult-style commands (buffers, files across workspace roots, workspace grep, `M-x`) with live preview, and the swiper-style in-buffer search.
- **Multiple cursors** are part of the window's selection state, so every editing command runs once per cursor and the whole run is one undo group.
- **Kill ring, mark ring, registers and keyboard macros** belong in the core, because they depend on the command loop.
- **Undo** is linear with a redo stack, per buffer, and grouped per command. An agent write and an auto-revert from disk are each one undo group, so either can be undone in one step.
- **Prefixes.** One prefix for tab commands and one for agent commands (new session, send region or buffer, switch session, cycle mode, cancel turn).

Keymaps and all other settings live in TOML files, reloadable at runtime. There is no scripting language.

## ACP integration and session management

The editor is an ACP **client**. Each agent session spawns its own agent process (Claude Code through `claude-agent-acp`, Codex through `codex-acp`), talks JSON-RPC over stdio, and lives in the tab it was created in; after that tab closes, it can be resumed in another tab of the same workspace.

**Workspace scope, the same way the CLIs and desktop apps do it.** The editor does not run an MCP server. Since June 2026 ACP has a stable way to pass extra roots: `session/new` takes `cwd` as the primary working directory and `additionalDirectories` as further absolute paths, which the agent treats as part of its filesystem scope. The client may only send them when the agent advertises `sessionCapabilities.additionalDirectories` ([ACP session setup](https://agentclientprotocol.com/protocol/v1/session-setup)).

- `cwd` = the workspace's primary repo. The agent loads its own config from there: AGENTS.md or CLAUDE.md, MCP servers, settings. The editor passes no MCP servers of its own.
- `additionalDirectories` = every other repo and folder in the workspace. Both the Claude Code and Codex adapters advertise support for it.
- The first prompt of a session carries a short preamble: the list of roots with their role lines from the workspace file.
- Every prompt attaches the current buffer as a file reference the agent reads itself: the file buffer last focused in the tab, plus the line range of the region when active.

**Components**

- **Agent registry.** Claude Code (claude-agent-acp) and Codex (the maintained @agentclientprotocol/codex-acp) in TOML: command, args, env, and a default mode per agent.
- **One process per session.** A crash ends only that session; the agent's stderr goes to a log buffer.
- **Session manager.** Keeps an index (session id, agent, tab, title, last activity, status) and the transcript. Sessions are the only state that survives a restart: a resume command lists the workspace's past sessions and continues the chosen one in the current tab with `session/resume` when advertised, otherwise `session/load`; both adapters support resuming across processes ([ConnectOnion's adapter matrix](https://docs.connectonion.com/useful-tools/acp-agent)).
- **Session buffer.** A rich chat view like the Claude Code and Codex apps: rendered markdown, collapsible tool calls, the agent's plan, the current mode, and a multi-line prompt area at the bottom with Emacs editing. Underneath the rendering is a text projection, so swiper search, mark and kill, and copying all work with Emacs keys.
- **Status.** Expect two or three concurrent sessions. Mode-line status, a dashboard and notifications come after the MVP.

**Modes and auto-approve.** Both agents expose their approval levels over ACP as session modes. The Claude adapter advertises `auto`, `default`, `acceptEdits`, `plan`, `dontAsk` and `bypassPermissions` ([block/buzz#2885](https://github.com/block/buzz/pull/2885)), with `bypassPermissions` offered only when bypass is enabled in the adapter. The maintained Codex adapter uses `read-only`, `agent` and `agent-full-access`, and can start in a chosen mode via `INITIAL_AGENT_MODE` ([openab#1353](https://github.com/openabdev/openab/pull/1353)). Claude Code starts in auto and Codex in agent, both set in config, and a key cycles modes per session; any permission request that still arrives is answered inline in the session buffer.

**Client-side callbacks**

| Agent asks for | Editor does |
| --- | --- |
| `fs/read_text_file` | Read from the open buffer if there is one, else from disk |
| `fs/write_text_file` | Apply directly. If the buffer has unsaved edits, save them first; then write the buffer and save it to disk as one undo group |
| `terminal/create` and friends | Spawn a regular terminal buffer in the session's tab |
| `session/request_permission` | Prompt inline in the session buffer |

Agents also change files without asking the editor, through shell commands or their own patch tools. The file watcher catches those and auto-reverts the affected buffers.

## Subsystems

**Terminal.** Built on `alacritty_terminal` for VT emulation and `portable-pty` for the PTY. A tab usually holds several shells, and terminals an agent creates are regular terminal buffers in its tab. Each terminal has two modes, as in vterm: *char mode* sends almost every key to the shell (only an escape prefix stays with the editor), *copy mode* turns the scrollback into a read-only buffer where Emacs motion, search and kill work. Shell-integration escape codes (OSC 133 prompt marks, OSC 7 cwd) give jump-to-prompt and per-command output capture.

**Language support.** Tree-sitter for highlighting, folding and structural motion in Elixir, Rust, JS/TS and Markdown. LSP comes after the MVP, one server per root and language.

**Search and navigation.** File finder and grep across all workspace roots (ripgrep's library crates), fuzzy matching in the minibuffer, swiper-style line search in the buffer, and jump history.

**Git.** Nothing built in for the MVP: the git CLI runs in terminal buffers, and `git diff` there is how agent changes are reviewed.

**Persistence.** Only agent sessions survive a restart: the session index (SQLite) and transcripts (JSONL per session) live under the macOS Application Support dir. Tabs, buffers and terminals start fresh on every launch. Workspace definitions and settings are TOML in the config dir.

**Frontend.** A native macOS GUI built on GPUI. The core produces a display model: per window, the visible lines with styled spans, cursors and overlays; the frontend draws it and feeds input events back. Keeping that boundary means the frontend stays thin and the core is testable without it.

**Runtime.** Tokio. The core loop is single-threaded, owns all editor state, and processes one message at a time: key events, agent updates, PTY output, file-watch events. Each I/O source is a task holding a channel to the core.

## Testing

Each layer is tested on its own, and the assembled app is tested end to end with a fake agent.

| Layer | How it is tested |
| --- | --- |
| Text model: rope, undo, markers, multiple cursors | Unit tests plus property tests (`proptest`) on random edit sequences and undo/redo round trips |
| Commands and keymaps | Headless core driven by scripted key sequences; assertions on buffer text and point, snapshots of the display model (`insta`) |
| Workspaces, tabs, session index | Temp-dir fixtures; tab lifecycle (closing kills its buffers); session index round trips |
| File sync | External writes to temp repos; check that the right buffers auto-revert |
| ACP client | A fake agent built on the same ACP crate, scripted to stream updates, request permissions, write files and create terminals; recorded transcripts from real Claude Code and Codex sessions replayed against the client |
| Terminal | Byte streams fed into the emulator grid with snapshot assertions; PTY tests against small shell scripts |
| Frontend | Kept thin; a few screenshot tests of rendered windows |
| Whole app | Launch with a test workspace and the fake agent, drive it with a key script, assert on files, buffers and tabs; a manual smoke run against the real agents before each milestone gate |

## Crate shortlist

All MVP choices are settled. Check each crate's current version and maintenance before committing.

| Area | Choice | Alternatives | Notes |
| --- | --- | --- | --- |
| ACP | `agent-client-protocol` | Hand-rolled JSON-RPC | Official Rust crate; also used to build the fake agent for tests |
| Async runtime | `tokio` | — | Everything else assumes it |
| GUI | GPUI | `iced`, `floem`, custom `wgpu` + `cosmic-text` | Decided; Zed's framework, strong on macOS |
| Text storage | `ropey` | `crop` | Rope with line/char indexing; undo built on top |
| Syntax | `tree-sitter` + grammars for Elixir, Rust, JS/TS, Markdown | `syntect` | Also gives structural motion |
| Fuzzy matching | `nucleo` | `fuzzy-matcher` | Helix's matcher, fast on large lists |
| Workspace search | `ignore` + `grep-searcher` | Shell out to `rg` | ripgrep's own library crates |
| File watching | `notify` | — | One watcher per root; drives auto-revert |
| PTY | `portable-pty` | `nix` directly | From the WezTerm project |
| VT emulation | `alacritty_terminal` | — | Decided; Zed embeds the same crate |
| Persistence | `rusqlite` + `serde_json` | `redb` | Session index in SQLite, transcripts as JSONL |
| Config | `serde` + `toml` | — | Decided: TOML everywhere |
| Testing | `insta`, `proptest` | — | Snapshots and property tests |
| Diagnostics | `tracing` | — | Piped into a Messages buffer |
| LSP (after MVP) | `lsp-types` + `async-lsp` | — | Not needed for the MVP |

## Milestone path

Build the editor core first and add the agent at M3, so agent features land on a model that already works. Each phase ends with a gate you can test by using it.

&#91;embedded content: milestone path · 6 phases, each with a gate\]

Phases are in order of dependency, not to scale; no dates are set. LSP, diff review, a dashboard with notifications, dired and worktree tasks come after M5.

## Sources

- [ACP session setup, additional workspace roots](https://agentclientprotocol.com/protocol/v1/session-setup)
- [ACP announcement: additional workspace roots stabilized](https://agentclientprotocol.com/announcements/additional-directories-stabilized)
- [block/buzz#2885: modes advertised by claude-agent-acp](https://github.com/block/buzz/pull/2885)
- [openab#1353: Codex ACP adapters and their mode ids](https://github.com/openabdev/openab/pull/1353)
- [ConnectOnion ACP agent: resume support per adapter](https://docs.connectonion.com/useful-tools/acp-agent)
