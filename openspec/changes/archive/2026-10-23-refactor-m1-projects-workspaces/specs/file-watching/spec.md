# Spec Delta

## ADDED Requirements

### Requirement: Recovery restores directory watch coverage
After lost events, relevant ignore-file changes, or replacement of a watched directory, the editor SHALL reconcile watch coverage with the current kept directories and watch eligibility. Recovery SHALL repair registrations lost when a directory was deleted and recreated at the same path. It SHALL preserve still-valid unaffected registrations and the existing same-scope watch continuity and size-limit rules. A fresh accepted count SHALL decide whether project root coverage continues; visited files SHALL retain their parent-directory fallback. Explicit indexing or configuration reload SHALL also refresh ignore-file-derived coverage.

#### Scenario: A directory creation event was lost
- **GIVEN** a new kept directory exists inside an eligible project but its creation event was lost
- **WHEN** the watcher reports lost events and recovery completes
- **THEN** a subsequent write inside that directory is reported and the project's index includes its files

#### Scenario: A directory is replaced within one debounce interval
- **GIVEN** a kept directory has a native watch
- **WHEN** it is deleted and recreated at the same path before the report is flushed
- **THEN** recovery establishes coverage for the replacement and a later write within it is reported

#### Scenario: An ignore file changes coverage
- **GIVEN** a root's `.gitignore` or `.ignore` excludes a directory
- **WHEN** that rule is removed and the reported change is processed
- **THEN** recovery includes the directory and later changes inside it are reported

#### Scenario: Recovery preserves unaffected watches
- **GIVEN** two eligible roots are watched and one needs directory recovery
- **WHEN** the recovered coverage is installed
- **THEN** the other root retains its valid registration and continues reporting writes throughout

#### Scenario: Recovery crosses the size limit
- **GIVEN** a project was eligible before lost events added more kept files than its limit
- **WHEN** the recovery count completes
- **THEN** its root watches stop, its mode line shows `[unwatched]`, and visited files continue to be watched through their directories

### Requirement: Watch failures remain recoverable and visible
A requested watch SHALL count as installed only after a successful backend registration. Failed or uncertain registration changes SHALL remain pending for reconciliation and retry, even when no further filesystem events arrive. Repeated attempts SHALL use bounded backoff rather than a busy loop. The first failure and changed failures SHALL be reported in the echo area and `*Messages*`, identical repeats SHALL be suppressed, and recovery SHALL be reported once. Coverage no longer requested SHALL cease being retried. Visited files without confirmed root coverage SHALL use their parent-directory fallback.

#### Scenario: Registration fails once
- **GIVEN** an eligible root's first registration fails and the next attempt can succeed
- **WHEN** the retry deadline passes without any filesystem event
- **THEN** registration is retried, a later file change is reported, and the user has received one failure and one recovery message

#### Scenario: A batch outcome is uncertain
- **GIVEN** a watch update fails while committing a batch of changes
- **WHEN** the watcher reconciles that failed update
- **THEN** uncertain registrations are re-established before being treated as confirmed coverage

#### Scenario: A persistent failure
- **GIVEN** registration keeps failing with the same error
- **WHEN** several retry deadlines pass
- **THEN** the pending coverage is retried with bounded backoff while the original failure remains in `*Messages*` without repeated identical entries

#### Scenario: A failed root has a visited file
- **GIVEN** a root registration fails but a visited file's parent directory can be watched
- **WHEN** another program changes that file
- **THEN** the parent-directory fallback reports the change and normal buffer synchronization runs
