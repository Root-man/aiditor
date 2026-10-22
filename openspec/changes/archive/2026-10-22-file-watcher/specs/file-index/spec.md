# Spec Delta

## ADDED Requirements

### Requirement: Creates and deletes refresh the index
When the watcher reports a path under a root of an indexed project that exists but is not in the index, or that is gone while the index holds it or files under it, the project SHALL be walked again without anything echoed, and its index replaced by the walk's answer as with `project-index`. A reported path that the index already holds and that still exists (a write) SHALL NOT start a walk, and a file the index lacks SHALL start a walk only the first time it is reported, so writes to a file the walk ignores start none. When the watcher reports lost events, every indexed project SHALL be walked again the same way.

#### Scenario: A created file
- **WHEN** a file is created under the root of an indexed project and the watcher reports it
- **THEN** the next `C-x p f` lists it, and the echo area is unchanged

#### Scenario: A deleted file
- **WHEN** an indexed file is deleted and the watcher reports it
- **THEN** the next `C-x p f` no longer lists it

#### Scenario: A write
- **WHEN** an indexed file is written and the watcher reports it
- **THEN** no walk starts

#### Scenario: An ignored file
- **WHEN** a visited file the walk ignores is reported twice
- **THEN** only the first report starts a walk

#### Scenario: Lost events
- **WHEN** the watcher reports that events were lost
- **THEN** every indexed project is walked again without anything echoed
