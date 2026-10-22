# Spec Delta

## ADDED Requirements

### Requirement: Project ignore patterns narrow the watch
The watcher SHALL apply each project's `ignore` patterns to that project's roots as the file index does: changes under directories they leave out, and to existing files they leave out that no buffer visits, SHALL NOT be reported, and on Linux directories they leave out SHALL get no watch. A root shared by projects with different patterns SHALL be watched wherever any of them keeps a directory.

#### Scenario: An ignored file
- **WHEN** a project's `ignore` is `["*.log"]` and `a.log` and `b.rs` change under its root
- **THEN** reports name `b.rs` but not `a.log`

#### Scenario: Allow-list
- **WHEN** a project's `ignore` is `["*", "!notes/"]` and files change in `src/` and `notes/` under its root
- **THEN** reports name the file in `notes/` only, and on Linux only the root and `notes/` (with its subdirectories) are watched
