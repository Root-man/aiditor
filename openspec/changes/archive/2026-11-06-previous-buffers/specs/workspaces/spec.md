# Spec Delta

## MODIFIED Requirements

### Requirement: A killed buffer is replaced in every workspace's windows
Killing a buffer SHALL replace it in the windows of every workspace that shows it, not only the current one. Each such window SHALL show its own most recent previous buffer that is still live and belongs to that workspace, or is `*Messages*`; with none, the replacement SHALL be that workspace's most recent other listed buffer, or `*Messages*`.

#### Scenario: A window of a workspace that is not current
- **WHEN** a window of workspace `a` shows a buffer of workspace `b`, `b` is current, and that buffer is killed
- **THEN** switching to `a` shows a buffer of `a` or `*Messages*` in that window

#### Scenario: The workspace list leaves a split as it was
- **WHEN** workspace `x` has two windows, one showing file `a` and the other a more recently shown file `b`, `C-x t l` is typed in the window showing `a`, another workspace is chosen with `RET`, and `x` is switched back to
- **THEN** one window of `x` shows `a` and the other shows `b`
