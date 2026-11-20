# Configuration

aiditor reads three kinds of TOML file from its config dir. Every file is optional: the bundled
defaults apply where yours says nothing. `C-x C-r` (`reload-settings`) rereads all of them; a bad
entry is reported in the echo line and `*Messages*`, and the rest still applies.

| File | What it holds | Bundled defaults |
| --- | --- | --- |
| `settings.toml` | Fonts, display, notifications, agents, themes | [`crates/aiditor-core/settings.toml`](../crates/aiditor-core/settings.toml) |
| `keymap.toml` | Key bindings | [`keymap.toml`](../crates/aiditor-core/keymap.toml), then [`keymap-macos.toml`](../crates/aiditor-core/keymap-macos.toml) or [`keymap-linux.toml`](../crates/aiditor-core/keymap-linux.toml) |
| `projects/<name>.toml` | One project each | None |

## Where files live

| | macOS | Linux |
| --- | --- | --- |
| Config dir | `~/Library/Application Support/aiditor` | `$XDG_CONFIG_HOME/aiditor` (`~/.config/aiditor`) |
| Data dir | `~/Library/Application Support/aiditor` | `$XDG_DATA_HOME/aiditor` (`~/.local/share/aiditor`) |

`aiditor --config-dir DIR` reads config from `DIR` instead.

The data dir holds state, not config: `recovery/` (auto-save files), `sessions.toml` (pinned agent
sessions) and `shell/zsh` (the shell integration terminals load). Don't edit these.

## settings.toml

Copy any key from the bundled file and change it. Tables merge key by key; any other value replaces
the default.

| Key | Default | What it does |
| --- | --- | --- |
| `font_family` | Menlo on macOS, Noto Sans Mono on Linux | The monospace font |
| `font_size` | `14.0` | Text size in px, 6 to 72; the text-scale commands zoom from it |
| `line_height` | `1.4` | Line height as a multiple of `font_size`, 1 to 3 |
| `workspace_font_size` | `13.0` | The workspace modeline's size, 6 to 72; zooms with `font_size` |
| `option_as_meta` | `true` | macOS: Option is Meta. `false` leaves it for special characters and dead keys. Linux's Alt is always Meta |
| `line_numbers` | `"absolute"` | Line numbers in files, `*scratch*` and `*Messages*`: `"off"`, `"absolute"` or `"relative"` to point's line |
| `hl_line` | `true` | Highlight point's line in the selected window |
| `notifications` | `true` | System notifications for agent events and long terminal commands |
| `terminal_notify_after` | `10` | Seconds a terminal command runs before its end notifies; `0` never |
| `max_watched_files` | `10000` | Most files plus directories a project may have and still be watched for external changes; a project file can override it |
| `grep_workers` | `1` | Files project grep searches at once, at least 1 |
| `paste_fold_lines` | `20` | A paste into a session prompt longer than this many lines folds into a chip |
| `table_width` | `100` | The widest a session draws a Markdown table, in chars; wider ones wrap their cells |
| `enabled_agents` | unset (all) | Agents offered in every project, by name; a project file's list replaces it |
| `theme` | `"default"` | Which `[themes.<name>]` table to draw |

### Agents

Each `[agents.<name>]` table starts an [ACP](https://agentclientprotocol.com) adapter:

| Key | What it does |
| --- | --- |
| `command` | The adapter's executable, looked up on `PATH`, or a full path. Required |
| `args` | Its arguments |
| `env` | Variables set on top of the environment captured from your login shell; a project's `env` wins over these |
| `default_mode` | The session mode to switch to once the agent lists its modes, such as `"auto"` or `"plan"` |

Only Claude Code is bundled:

```toml
[agents.claude]
command = "claude-agent-acp"
default_mode = "auto"
env = { CLAUDE_CODE_ENABLE_TASKS = "1", CLAUDE_CODE_ENABLE_TODO_TOOLS = "1" }
```

Add Codex, or change one key of Claude's:

```toml
[agents.codex]
command = "codex-acp"

[agents.claude]
command = "/opt/homebrew/bin/claude-agent-acp"
```

### Themes

`theme` names a `[themes.<name>]` table, drawn merged over `themes.default`, so a theme lists only
what it changes. A theme has three parts:

- `ansi`: the 16 terminal colours as `0xRRGGBB` numbers: black, red, green, yellow, blue, magenta,
  cyan, white, then their bright forms.
- `faces`: the editor's named faces (`default`, `cursor`, `region`, `mode-line`, ...). The bundled
  file lists every one with a comment.
- `syntax`: faces by tree-sitter capture name. A name without an entry takes its longest dotted
  prefix's (`function.method` falls back to `function`).

A face has `fg`, `bg` and `box` (colours as `0xRRGGBB`) and `bold`, `italic`, `underline` and
`strike` (booleans). Anything it leaves unset comes from the face underneath.

```toml
theme = "mine"

[themes.mine.faces]
default = { fg = 0xebdbb2, bg = 0x282828 }
region = { bg = 0x504945 }

[themes.mine.syntax]
comment = { fg = 0x928374, italic = true }
```

## keymap.toml

Each table is a keymap: `[global]`, a named prefix such as `[project-prefix]`, or a mode such as
`[vterm-mode]` or `[agent-session-mode]`. The bundled `keymap.toml` lists every table. Your entries
bind over the bundled ones.

```toml
[global]
"C-c g" = "project-grep"    # bind a key sequence to a command (any name M-x offers)
"C-x u" = false             # unbind
"C-c p" = "project-prefix"  # a value naming a table makes that key a prefix for it
"C-x p" = false

[vterm-mode]
"C-c C-y" = "vterm-copy-mode"
```

Keys are written as in Emacs's `kbd`: modifiers `C-`, `M-`, `S-` and `s-` (Super/Command), the
named keys `RET`, `TAB`, `SPC`, `DEL`, `ESC`, `<left>`, `<right>`, `<up>`, `<down>`, `<home>`,
`<end>`, `<prior>`, `<next>`, and sequences separated by spaces.

## projects/&lt;name&gt;.toml

A project is a primary repo plus other roots. Find-file, grep and every agent session see the same
roots. The project commands (`C-x p c` to create, `C-x p a` to add a root, ...) write these files;
you can also edit them by hand and press `C-x C-r`.

```toml
name = "aiditor"
default_agent = "claude"        # optional: the agent agent-start selects first
watch = true                    # optional: false stops watching the roots for external changes
max_watched_files = 20000       # optional: overrides settings.toml
enabled_agents = ["claude"]     # optional: replaces settings.toml's list
ignore = ["*", "!notes/"]       # optional: gitignore patterns, on top of .gitignore and .ignore

[env]                           # optional: set for the project's terminals and agents
MIX_ENV = "dev"

[primary]                       # agents start here; its AGENTS.md / CLAUDE.md apply
path = "/Users/me/Repos/aiditor"

[[roots]]                       # zero or more
path = "/Users/me/Repos/zed"
```

Roots within one project must not overlap. Different projects may share roots.

## Command line

```
aiditor [--config-dir DIR] [--debug] [FILE | FOLDER]
aiditor --wait [+LINE[:COL]] FILE...
```

- With no argument, aiditor opens the workspace list of your saved projects.
- A file or folder opens in the first project with a root that holds it. If no project has one, it
  opens in an unsaved project at the enclosing git repo or folder.
- `--debug` shows frame timing stats.
- `--wait` opens files in the running aiditor and waits until you finish with `C-x #`. Terminals
  inside aiditor set `$EDITOR` to it, so `git commit` edits its message in an aiditor buffer.
- `RUST_LOG` sets the log filter, `info` by default. Warnings and errors also go to `*Messages*`.
