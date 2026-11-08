# AGENTS.md

## Project

aiditor is a personal, keyboard-first GUI editor written in Rust. It is built around Emacs's buffer/window model, projects (a primary repo plus other roots) and workspaces (everything for one task), with an ACP client for Claude Code and Codex as a first-class subsystem. macOS is the target; development happens on macOS and Linux, so everything must build and run on both.

The spec is [docs/editor.md](docs/editor.md): goals, architecture, data model, subsystems, open questions, testing and milestones. Read the sections an issue links to before starting it. When a decision settles an open question, record it in `docs/editor.md`.

## Zed and GPUI source

aiditor leans on the GPUI crate and is partially inspired by Zed. When you need to know how GPUI works, read its source instead of guessing. Zed's code is at `~/Repos/zed`, and GPUI 0.2.2 (the version aiditor uses) is at `~/Repos/zed/crates/gpui`.

## Visual design

The approved designs are in [docs/design](docs/design), exported from the [aiditor visual direction canvas](https://claude.ai/artifact/VyRd2AK1RfiVhuBx6nA1KP) (the remote copy needs a claude.ai login; agents read the local files). Each frame is an HTML file whose markup and CSS give the exact colours, sizes and spacing; they don't render without the canvas runtime, so read them as source. [canvas.json](docs/design/canvas.json) lists the frames in order. Follow them for any visual work (epic #343). When the canvas changes, re-export the changed files. Frames:

- **1 · Editor** ([Main.dc.html](docs/design/Main.dc.html)): a file buffer with the line number gutter, point's line, the workspace modeline, the mode line and the echo line.
- **2 · Splits** ([Splits.dc.html](docs/design/Splits.dc.html)): three windows, with active and inactive mode lines and the 1px divider.
- **3 · Project grep** ([Grep.dc.html](docs/design/Grep.dc.html)): hits grouped by file, match highlighting in the previewed buffer, the fixed-width count and the searching state.
- **4 · find-file** ([FindFile.dc.html](docs/design/FindFile.dc.html)): the `*/16` count, directories in their own face, and annotations for mode bits, size and age.
- **5 · Terminal** ([Terminal.dc.html](docs/design/Terminal.dc.html)): test output, with the exit status and input mode in the mode line.
- **6 · Agent session, monospace** ([Session.dc.html](docs/design/Session.dc.html)): the interactive session buffer: the summary rule and ruled prompt between turns, messages, timings to the second and the working timer, collapsed tool groups and tool-call blocks, the permission request, file chips and the growing prompt.
- **6b · Session prompt** ([SessionPrompt.dc.html](docs/design/SessionPrompt.dc.html)): `@` references, `/` commands, `agent-attach-file` and folded pastes.
- **6c · File chips** ([FileLinks.dc.html](docs/design/FileLinks.dc.html)): the chip's kinds and states, and file links in agent prose.
- **6d · Questions: choices** ([QuestionChoices.dc.html](docs/design/QuestionChoices.dc.html)): one choice, several choices and previews.
- **6e · Questions: several questions and forms** ([QuestionForms.dc.html](docs/design/QuestionForms.dc.html)): folded questions, own answer, every form field type and minibuffer checks.
- **6f · Questions: URL, non-blocking, answered** ([QuestionStates.dc.html](docs/design/QuestionStates.dc.html)): URL requests, questions beside a running turn, answered rows and a question while you type.
- **6g · Telling turns apart and mode line colours** ([SessionTurns.dc.html](docs/design/SessionTurns.dc.html)): six ways to separate turns (frame 6 uses D and F), and the item-quality colours for model and effort and the green/yellow/red `ctx`.
- **7 · Agent session, rich** ([SessionRich.dc.html](docs/design/SessionRich.dc.html)): a sketch for M7, with proportional prose and cards. Not approved; see below.
- **8 · Welcome dashboard** ([Dashboard.dc.html](docs/design/Dashboard.dc.html)): a sketch for M7. Not approved; see below.
- **9 · Mode line study** ([ModeLines.dc.html](docs/design/ModeLines.dc.html)): the workspace modeline and every buffer type's mode line, where each segment lives, and the rules.
- **10 · Gruvbox light** ([Light.dc.html](docs/design/Light.dc.html)): the light theme, with swiper.
- **11 · Narrow frame** ([Narrow.dc.html](docs/design/Narrow.dc.html)): truncation and the `↪` continuation marker at 900×600.
- **12 · Faces and spacing** ([Faces.dc.html](docs/design/Faces.dc.html)): every named face in both themes, the spacing scale and type sizes.
- **13 · Workspace list** ([WorkspaceList.dc.html](docs/design/WorkspaceList.dc.html)): `*Workspaces*` with every project, `TAB` folding, name, where and state columns, and key hints in the echo line.
- **13b · Workspace list study** ([WorkspaceListStudy.dc.html](docs/design/WorkspaceListStudy.dc.html)): every row kind and state, where each piece comes from, keys and rules.
- **13c · Workspace list in a split** ([WorkspaceListNarrow.dc.html](docs/design/WorkspaceListNarrow.dc.html)): the list in a narrow window, beside a file.
- **13d · Workspace list, Gruvbox light** ([WorkspaceListLight.dc.html](docs/design/WorkspaceListLight.dc.html)): the folded list in the light theme, with point away from the current workspace.

Frames 2, 5, 6b, 7, 10 and 11 still draw the older workspace modeline and mode lines without `│` separators; frames 1 and 9 are the reference for those. Frames 6d, 6e, 6f and 6g still draw a prompt's and a message's time at the right of its first line; frame 6 is the reference: small, on the blank line under the block.

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
