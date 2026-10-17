# Spec Delta

## Purpose

Opening a file named on the command line into a buffer and writing the buffer back to that file, without losing edits when the write fails.

## ADDED Requirements

### Requirement: A file named on the command line opens in a buffer
When the app is started with a file path, it SHALL read that file and make a buffer visiting it current, with point at the start and the buffer unmodified.

#### Scenario: Existing file
- **WHEN** the app starts with the path of a file containing `hello`
- **THEN** the current buffer is named after the file, holds `hello`, visits that path and is unmodified

#### Scenario: Missing file
- **WHEN** the app starts with a path where no file exists
- **THEN** the current buffer is empty, visits that path and is unmodified, and nothing is written to disk until it is saved

#### Scenario: Unreadable file
- **WHEN** the file exists but cannot be read as UTF-8 text
- **THEN** the app exits with an error message naming the file and does not open a window

### Requirement: C-x C-s saves the current buffer to its file
`C-x C-s` SHALL write the current buffer's text to the file it visits. The buffer SHALL become unmodified only after the write succeeds.

#### Scenario: Save after editing
- **WHEN** a buffer visiting a file is edited and `C-x C-s` is pressed
- **THEN** the file on disk holds the buffer's text and the buffer is unmodified

#### Scenario: Failed write keeps the edits
- **WHEN** the write fails, for example because the file's directory does not exist
- **THEN** the error is logged and the buffer stays modified with its text unchanged

#### Scenario: Buffer without a file
- **WHEN** `C-x C-s` is pressed in a buffer that visits no file
- **THEN** nothing is written and the buffer stays as it was

#### Scenario: C-x followed by another key
- **WHEN** `C-x` is followed by any key other than `C-s`
- **THEN** neither key edits the buffer and nothing is saved
