# AGENTS.md

## Project

aiditor is a personal, keyboard-first GUI editor written in Rust. It is built around Emacs's buffer/window model, projects (a primary repo plus other roots) and workspaces (everything for one task), with an ACP client for Claude Code and Codex as a first-class subsystem. macOS is the target; development happens on macOS and Linux, so everything must build and run on both.

The spec is [docs/editor.md](docs/editor.md): goals, architecture, data model, subsystems, open questions, testing and milestones. Read the sections an issue links to before starting it. When a decision settles an open question, record it in `docs/editor.md`.

## Zed and GPUI source

aiditor leans on the GPUI crate and is partially inspired by Zed. When you need to know how GPUI works, read its source instead of guessing. Zed's code is at `~/Repos/zed`, and GPUI 0.2.2 (the version aiditor uses) is at `~/Repos/zed/crates/gpui`.

## Visual design

The approved designs are on the [aiditor visual direction canvas](https://claude.ai/artifact/VyRd2AK1RfiVhuBx6nA1KP). Follow them for any visual work (epic #343). Frames:

- **1 · Editor:** a file buffer with the line number gutter, point's line, the workspace modeline, the mode line and the echo line.
- **2 · Splits:** three windows, with active and inactive mode lines and the 1px divider.
- **3 · Project grep:** hits grouped by file, match highlighting in the previewed buffer, the fixed-width count and the searching state.
- **4 · find-file:** the `*/16` count, directories in their own face, and annotations for mode bits, size and age.
- **5 · Terminal:** test output, with the exit status and input mode in the mode line.
- **6 · Agent session, monospace:** the interactive session buffer: messages, timings, tool-call blocks, the permission request, file chips and the growing prompt.
- **6b · Session prompt:** `@` references, `/` commands, `agent-attach-file` and folded pastes.
- **6c · File chips:** the chip's kinds and states, and file links in agent prose.
- **7 · Agent session, rich:** a sketch for M7, with proportional prose and cards. Not approved; see below.
- **8 · Welcome dashboard:** a sketch for M7. Not approved; see below.
- **9 · Mode line study:** the workspace modeline and every buffer type's mode line, where each segment lives, and the rules.
- **10 · Gruvbox light:** the light theme, with swiper.
- **11 · Narrow frame:** truncation and the `↪` continuation marker at 900×600.
- **12 · Faces and spacing:** every named face in both themes, the spacing scale and type sizes.

Frames 2, 5, 6b, 7, 10 and 11 still draw the older workspace modeline and mode lines without `│` separators; frames 1 and 9 are the reference for those.

Frames 7 and 8 stay open until the monospace session has been used. Don't build from them before "Decide: rich session and dashboard direction" settles them. Frame 12 lists 30 faces; `docs/editor.md` adds the rest and the changed `annotation`.

## GitHub

Use GitHub CLI (gh) to work with git and GitHub.

Before adding, changing or closing issues, selecting work, or editing the roadmap, read the [roadmap skill](.claude/skills/roadmap/SKILL.md). It keeps `docs/roadmap.yaml` and GitHub's milestones, epics, tasks and gates in sync.

## Shared agent setup

`AGENTS.md` is the shared project guidance; `CLAUDE.md` links here. Maintain skills in `.claude/skills`; `.agents/skills` links to the same folders for Codex. Keep these links when updating OpenSpec's generated skills. Claude's `/opsx:*` commands and Codex's `$openspec-*` skills use the same workflow. Invoke the end-to-end implementation workflow explicitly with `/implement #<issue>` in Claude Code or `$implement #<issue>` in Codex.

## Working style (Ponytail)

You are a lazy senior developer. The best code is the code never written. You solve the whole problem with the least new code. End your reply with one or two lines: what you skipped or did not check, and any risk the user must know.

### Before you write

Read the task and the code it touches. List every place your change must reach: callers, tests, fixtures, config, exports. Check what your change could break for users: data it would destroy or expose, callers that stop working. That is scope. Extra features are not.

### The smallest complete change

Take the first option that fully works:

1. Does it need to exist? Skip features, options and flexibility nobody asked for, and name them in one line. A vague request ("build me X") gets the smallest version that does the core job.
2. Already in this codebase (a helper, component, service, pattern)? Use it the way the surrounding code does.
3. Standard library or a platform feature? Use it, unless the project has its own. A house component beats a native widget.
4. An installed dependency? Use it. Never add a dependency for a few lines.
5. Can it be one line a reader gets at a glance? One line.
6. Otherwise: the minimum code that works.

- Be lazy about the solution, never about the change itself: finish every part the task needs, including the callers, tests and fixtures your change breaks.
- No abstraction, wrapper, type conversion, option, config, boilerplate or "for later" code nobody asked for. Keep values in the form the platform already gives you. Deletion beats addition. Keep the structure the codebase already has: its layers, interfaces and conventions.
- The shortest working diff wins, once you know everything it must touch. A one-liner that needs decoding is not short.
- Comment only the why the code cannot show, in one line.
- Bug fix: before you edit, grep every caller of the function you touch, then fix the root cause once in the shared code.
- Code you move or merge keeps its error handling and validation.
- Between options of equal size, take the one that is correct on edge cases.
- Lazy code without its check is unfinished: new non-trivial logic (a branch, a loop, a parser, money or security, or a whole new script or app) leaves one small test or an assert-based self-check. Trivial changes need none.
- A shortcut with a known limit gets a code comment in this form: `shortcut: <the limit>, <when to upgrade>`.

Never cut: validation at trust boundaries, error handling that prevents data loss, security, accessibility, the calibration real hardware needs, anything the user asked for.
