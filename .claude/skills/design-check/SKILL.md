---
name: design-check
description: Run aiditor on macOS, put it in the state a docs/design frame shows, screenshot it, render the frame, and report the differences. Each difference is checked against docs/editor.md and the code before it is called a bug. Use for "compare with the design", "check frame N", "visual test against the canvas", /design-check or $design-check.
argument-hint: "<frame number or file, e.g. 1 or Main.dc.html>"
---

# Design check

Compare the running app against one frame in `docs/design` (the list is in `AGENTS.md`). The deliverable is a list of real gaps. A list of pixel differences is not the deliverable.

## The main rule: a difference is not a bug until you have explained it

Most differences come from the test setup, not the app. A frame shows a lived-in state: a git repo, a toolchain file, several workspaces, a saved buffer, a message in the echo line. A throwaway fixture has none of these, so the app correctly leaves those segments out. Before you list a difference, decide which bucket it belongs in:

1. **Setup**: the fixture or session lacks the state behind the segment. Find the code that produces the segment and check what it needs. Then either add that state to the fixture and re-run, or put the difference under "not compared".
2. **Intended**: `docs/editor.md`, an issue, or a code comment or test says the app does this on purpose. Quote the source.
3. **Gap**: neither of the above. Only these go in the findings, each with the file and line that would change.

To check a difference, grep `crates/` for the segment, face or text (for example `⎇`, `FaceName::`, the word in the mode line). Read the builder and its tests. The tests in `crates/aiditor-core/src/display.rs` usually show which inputs make each segment appear. Known cases:

| What the frame shows | Where it comes from | What the fixture needs |
|---|---|---|
| Version after the language name (`Rust 1.99.0`) | `language_version.rs` reads `.tool-versions`, `mise.toml` or `rust-toolchain.toml` | One of those files in the project root |
| `⎇ main` in the mode line | VCS head of the buffer's repository | `git init` and one commit in the fixture |
| Several workspaces with state dots in the workspace modeline | The open workspaces and their state | More workspaces. With one workspace, `project › project` is correct, because a workspace is named after its project by default. |
| Dim path prefix before the bold file name | Path relative to the project root | The file in a subdirectory of the project, not at its root |
| `●` modified marker | Unsaved edits | An edit (then save or revert it, see Cleanup) |
| Echo text (`Wrote …`) | The last command | Run that command (for example save) |
| `Top`, `Bot` or `All` | Buffer length compared with the window height | A file longer than the window |
| No padding above line 1 | Intended: it saves space | Nothing |
| macOS title bar, clipped last line | The window is not fullscreen; every frame assumes fullscreen | Go fullscreen (step 3) |

Add new cases here when you find them.

## Steps

1. **Read the frame.** Read `docs/design/<Frame>.dc.html` as source. Note its size (`data-props` `$preview`), every segment, colours, sizes and the state it shows. Read the parts of `docs/editor.md` it touches.
2. **Build a fixture that reproduces the frame's state** in the scratchpad, not in `/tmp` or the repo:
   - Extract the buffer text from the frame's line `<div>`s. Strip the tags and unescape the entities, and drop the fake cursor span (`<span class="pt"> </span>`): it is not text, and keeping it adds a trailing space.
   - Recreate the path, for example `benches/src/support.rs` under a project root.
   - Add the state from the table above: `git init` plus a commit on the frame's branch, a `.tool-versions`, and so on.
3. **Launch with the user's default config** (no `--config-dir` unless asked), after you check that no other aiditor is running (`pgrep -x aiditor`). Another instance takes the focus and the keystrokes. Then:
   - `cargo build -p aiditor`, start `target/debug/aiditor <fixture file>` in the background, and record the PID.
   - Find the window with a Swift script that calls `CGWindowListCopyWindowInfo` and filters by PID. The process also owns a small 500×500 blank window; take the big one.
   - Bring it to the front: `tell application "System Events" to set frontmost of process "aiditor" to true`. Then go fullscreen: `tell process "aiditor" to set value of attribute "AXFullScreen" of window 1 to true`. Wait about 2 s for the animation to finish. Fullscreen follows the display size, so compare proportions and spacing rather than absolute positions when the sizes differ from the frame.
   - If osascript reports an Accessibility error, stop and tell the user: the terminal needs Accessibility and Screen Recording permissions.
4. **Drive it to the frame's state** with `osascript … keystroke` or `key code`. Send one key per call, with short sleeps in between.
   - Take a screenshot first and look at it. A startup prompt (recovery, an error in the echo line) takes the keys; deal with it before you go on.
   - After each step, check the mode line (`line:col`) to confirm the keys landed.
5. **Capture** with `screencapture -o -l <id> <scratchpad>/app.png`.
6. **Render the frame.** Copy it to the scratchpad and remove every `<script>…</script>` with a multiline regex (Python `re.S`). Do not use a sed range: the first `<script>` closes on its own line, and a range deletes the whole body. Render with `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --user-data-dir=<scratchpad>/chrome --hide-scrollbars --force-device-scale-factor=2 --window-size=<w>,<h> --screenshot=<scratchpad>/design.png file://…`. Open the PNG. If it is blank, the HTML is broken; check that `class="fr"` survived. Do not blame Chrome.
7. **Compare** the two images region by region: workspace modeline, text area (gutter, point's line, cursor, syntax faces), mode line and echo line. Put every difference into one of the three buckets above, by reading the spec and the code. Re-run with a better fixture when that settles a question.
8. **Clean up**: leave no unsaved edits. If you edited a buffer, save it or undo back to clean before you kill the process; otherwise the next start shows a recovery prompt. Then kill the PID, confirm with `pgrep`, and delete the fixture and Chrome profile. Keep `app.png` and `design.png`.

## Report

- One line: does the frame match, apart from the gaps listed.
- **Gaps**: a table with columns region | design | app | why it is a gap (spec or code reference) | where to fix.
- **Setup or intended**: one line each, with the reason (missing fixture state, or a quote from the spec).
- **Not compared**: whatever you could not reproduce or check, for example exact hex values or fonts.
- The full paths to `app.png` and `design.png`.

Do not edit repo files during a check.
