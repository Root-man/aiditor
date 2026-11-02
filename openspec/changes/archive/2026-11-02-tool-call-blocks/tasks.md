# Tasks

## 1. Header

- [x] 1.1 Replace `tool_header` with the fold, status, icon, verb and target line, using the primary root kept on the session; render its faces and icon in `render_tool`; verify with core tests of folded, running and failed headers
- [x] 1.2 Put the exit, in `status-failed`, in the header's right side after the waiting counter and before the duration; verify with the failed-command test
- [x] 1.3 Change the fold marker on `TAB`, keeping point; verify with the toggle tests

## 2. Open bodies

- [x] 2.1 Show `$ <command>` and the output in one fenced block, drop `Input:` for commands and diffs; verify with `tool_content` tests and the folded/open agreement test
- [x] 2.2 Fold a command's output past 20 lines behind `… N more lines` when open, rendering markdown on the visible text only; verify with tests of 50, 20 and streaming lines and of property order
- [x] 2.3 Colour diffs with the `diff-*` faces; verify with a test of an open edit

## 3. Full output

- [x] 3.1 `RET` on a header opens or refreshes the call's output buffer; verify with tests of a command, a diff, repeat, killed buffer, no output and RET elsewhere

## 4. Finish

- [x] 4.1 Update the tests in `aiditor-services` that read headers and run the workspace's checks
- [x] 4.2 Record the design in `docs/editor.md`'s session buffer and agent command output sections
