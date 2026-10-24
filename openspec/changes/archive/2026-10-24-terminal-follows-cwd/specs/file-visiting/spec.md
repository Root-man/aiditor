# Spec Delta

## MODIFIED Requirements

### Requirement: C-x C-f visits a file
`find-file` (`C-x C-f`) SHALL read a file name at the prompt `Find file: `, whose text starts as the current buffer's directory with a trailing `/`: its file's directory, a terminal buffer's directory, or the working directory for any other buffer that visits no file. A relative name SHALL be taken from that directory. It SHALL visit the file in a new current buffer as a file named on the command line is visited, read without blocking the editor.

#### Scenario: Existing file
- **WHEN** a buffer visits `d/a.txt`, `d/b.txt` holds `hi`, and `C-x C-f b.txt RET` is typed
- **THEN** the current buffer is named `b.txt`, holds `hi`, visits `d/b.txt` and is unmodified

#### Scenario: New file
- **WHEN** at the `Find file: ` prompt the name of a file that does not exist is entered
- **THEN** the current buffer is empty, visits that path and is unmodified, and no file is written until it is saved

#### Scenario: The prompt starts at the buffer's directory
- **WHEN** a buffer visits `/x/y/a.txt` and `C-x C-f` is typed
- **THEN** the minibuffer's text is `/x/y/`

#### Scenario: The prompt starts at a terminal's directory
- **WHEN** a terminal's shell ran `cd /tmp` and reported it through OSC 7, and `C-x C-f` is typed in its buffer
- **THEN** the minibuffer's text is `/tmp/`
