## ADDED Requirements

### Requirement: File chips
A chip SHALL be one char of a session buffer's text naming a file, a directory or an image, drawn as a space, its icon (`▤` file, `▣` image, `▦` directory), a space, the path's last component and a space, in the boxed `attachment` face. Motion, deletion, kill, yank and undo SHALL treat it as one char, and a yanked chip SHALL keep its path.

#### Scenario: Drawn as icon and name
- **WHEN** a chip for `/p/src/main.rs` is in the prompt
- **THEN** it is drawn as ` ▤ main.rs ` in the `attachment` face

#### Scenario: One char
- **WHEN** point is before a chip and `C-f` runs, then `DEL`
- **THEN** point moves past the chip, then the chip is deleted whole

#### Scenario: Undo brings it back
- **WHEN** a deleted chip's deletion is undone
- **THEN** the chip is drawn again as before

### Requirement: Chips are sent as resource links and images
Sending a prompt with chips SHALL send its text as text blocks split at the chips, runs of only whitespace left out, and each chip in its place: a file or directory as a `resource_link` with the `file://` URI of its path, a directory's ending in `/`, and an image (`png`, `jpg`, `jpeg`, `gif`, `webp`) as image content with the file's data, read when it is sent. An image that can't be read SHALL end the turn with `Error: <reason>`.

#### Scenario: A file in the middle
- **WHEN** the prompt is `look at <chip /p/a.rs> please` and is sent
- **THEN** the request's blocks are the text `look at `, a resource link `a.rs` to `file:///p/a.rs` and the text ` please`

#### Scenario: An image
- **WHEN** the prompt holds a chip for `/p/cat.png` and is sent
- **THEN** the request carries image content of type `image/png` with the file's bytes

### Requirement: Chips in sent and replayed messages
The echoed prompt SHALL show its chips as the prompt did. In a replayed user message, each `[@name](file://path)` SHALL show as a chip for `path`, a directory when it ends in `/`; each `@path` word at the start or after whitespace, as the Claude Code CLI writes a mention, SHALL show as a chip when `path` is a file in the project's file index, relative to any root or absolute under one; and an image block SHALL show as an `▣ image` chip. Replayed user chunks with the same message id SHALL join one block.

#### Scenario: Echo
- **WHEN** a prompt with a chip is sent
- **THEN** the echo above the new prompt line shows the same chip

#### Scenario: Replay
- **WHEN** a loaded session replays the chunks `look at `, `[@a.rs](file:///p/a.rs)` and ` please` with one message id
- **THEN** one block shows `> look at  ▤ a.rs  please`, the middle a chip

#### Scenario: A CLI mention
- **WHEN** a loaded session replays `@src/a.rs vs @src/gone.rs`, and only `src/a.rs` is in the index
- **THEN** `@src/a.rs` shows as a chip and `@src/gone.rs` stays text

### Requirement: RET and the echo on a chip
`RET` with point on a chip SHALL visit its file in another window, splitting a lone one, or for a directory open `find-file` in it, sending nothing. A chip without a path SHALL echo `No file`. While point is on a chip, the echo line SHALL show its full path when the command left no other message.

#### Scenario: Visiting
- **WHEN** point is on a chip for `/p/a.rs` in the prompt and `RET` is typed
- **THEN** `/p/a.rs` shows in another window and the prompt is not sent

#### Scenario: The path on the echo line
- **WHEN** point moves onto a chip for `/p/a.rs`
- **THEN** the echo line shows `/p/a.rs`

### Requirement: Missing files are struck through
A chip whose file is gone from disk SHALL be drawn struck through, its icon in `status-failed`, wherever it shows. The state SHALL be checked when its path is first seen and when the file watcher reports it, and SHALL clear when the file is back.

#### Scenario: Deleted
- **WHEN** a check answers that a chip's file is gone
- **THEN** the chip is struck through with its icon in `status-failed`
