# Spec Delta

## MODIFIED Requirements

### Requirement: Killing a buffer
`kill-buffer` (`C-x k`) SHALL read a buffer name, completing over the listed buffers with the current one first, and kill that buffer. Every window showing it SHALL show the most recent other listed buffer of the current workspace, or `*Messages*`, instead, or a new empty `*scratch*` when there is none. A buffer of another workspace SHALL NOT replace it. A name no buffer has SHALL fail with `No such buffer <name>`.

#### Scenario: Kill the current buffer
- **WHEN** a file is visited from `*scratch*` and `C-x k RET` is typed
- **THEN** the file's buffer is gone and `*scratch*` is current

#### Scenario: The last buffer
- **WHEN** `C-x k RET` is typed with only `*scratch*`
- **THEN** a new empty `*scratch*` is current

#### Scenario: Unknown name
- **WHEN** `C-x k nope RET` is typed
- **THEN** the echo area shows `No such buffer nope` and no buffer is killed

#### Scenario: Another workspace's buffer is not shown
- **WHEN** a file is visited in one workspace, a new workspace is made, and its scratch buffer is killed
- **THEN** the window shows a new `*scratch*` of the new workspace, not the other workspace's file
