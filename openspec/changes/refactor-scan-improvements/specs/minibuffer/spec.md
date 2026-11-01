# Spec Delta

## MODIFIED Requirements

### Requirement: A preview can show a file not yet visited
A preview SHALL be able to name a file. A file visited in the current workspace SHALL show its buffer. Any other file SHALL be read and prepared as text off the core thread into a hidden, read-only preview buffer, the window keeping what it showed until the read answers. Only the current preview request of the same prompt session SHALL be allowed to install its result; a reply from a closed prompt or an obsolete selection SHALL be dropped, even if a later prompt selects the same path. A file that is missing, binary, unreadable, not a regular file, or larger than 10 MiB SHALL show the window as it was, and a device or pipe SHALL NOT be read.

#### Scenario: Previewing an unvisited file
- **GIVEN** a previewing prompt is open
- **WHEN** it selects a file no buffer visits
- **THEN** once it is read, the window shows its text, and no buffer list offers the preview buffer

#### Scenario: A late read
- **GIVEN** a selected file is being read for preview
- **WHEN** the selection moves on before the read answers
- **THEN** that answer is dropped and the window displays only an eligible current preview

#### Scenario: The same path in a later prompt
- **GIVEN** a preview read belongs to a prompt that has closed
- **WHEN** a new prompt selects the same path before the old read answers
- **THEN** the new prompt uses its own current request and the old result leaves its window unchanged

#### Scenario: Selecting a device
- **GIVEN** `C-x C-f` lists `/dev/`
- **WHEN** `zero` is selected
- **THEN** no bytes are read from it and the window shows what it showed before the prompt

#### Scenario: Selecting a named pipe
- **GIVEN** a previewing prompt lists a named pipe and a text file
- **WHEN** the pipe is selected, then the text file
- **THEN** the window shows the text file's text

#### Scenario: A file over the limit
- **WHEN** a previewing prompt selects a file larger than 10 MiB that no buffer visits
- **THEN** the file is not read for the preview, the window shows what it showed before and `RET` still visits it

### Requirement: Preview requests coalesce to the current selection
The editor SHALL keep at most one preview file read active and one latest pending request, including across prompt closures and reopenings. A selection change SHALL replace an obsolete pending request. Closing a prompt SHALL cancel its queued request and invalidate its active result. Preparation of preview text SHALL leave the editor able to process input. Existing preview restoration, visited-buffer reuse, full-file display of files within the preview limit, and normal file visitation on `RET` SHALL remain in effect.

#### Scenario: Moving quickly through candidates
- **GIVEN** file `a` is being read and file `b` is queued for preview
- **WHEN** the selection advances to file `c` before `a` finishes
- **THEN** the pending request becomes `c`, and `c` is read after `a` finishes

#### Scenario: Input during preview preparation
- **GIVEN** a large file is being prepared for preview
- **WHEN** the user types another query or presses `C-g`
- **THEN** the input is handled before preview preparation completes

#### Scenario: Accepting after a preview
- **GIVEN** a hidden preview shows a selected file
- **WHEN** the user presses `RET`
- **THEN** the origin is restored, the hidden preview is released, and the ordinary visit reads the selected file according to file-visiting rules
