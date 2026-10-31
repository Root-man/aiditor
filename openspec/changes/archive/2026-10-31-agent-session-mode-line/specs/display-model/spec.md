## MODIFIED Requirements

### Requirement: Mode line segments use colour only for state
In the selected window, `**` and `●` SHALL be in `status-modified`; `%%`, `⊘`, `[deleted]` and `[conflict]` in `status-failed`; `UTF-8 CRLF` and `char` in `status-waiting`; `copy` in `status-info`; a running command in `status-running`; `exit <n>` in `status-failed`; the branch in `vcs-branch` and its ` ±` in `status-modified`; a file's or shell's directory, major mode, region size, position and `[unwatched]` in `mode-line-dim`; a file's name and a terminal's name in `mode-line-buffer-id`. A session model and effort SHALL use `mode-line-dim`, its title `mode-line-buffer-id`, context usage `mode-line-dim`, and mode and lifecycle status their specified risk and status faces. Other text SHALL be in the `mode-line` face, and every segment's face SHALL sit over it.

#### Scenario: A modified file in the selected window
- **WHEN** the selected window's buffer is modified
- **THEN** `●` has the `status-modified` foreground and the file's name is bold in the `mode-line` foreground
