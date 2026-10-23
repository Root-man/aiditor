## MODIFIED Requirements

### Requirement: Project watches have a filtered size limit
Before adding project root watches, the editor SHALL count files and kept directories, including each root, after all index ignore filters across every project root. If either count exceeds the effective `max_watched_files`, it SHALL contribute no root watches, show `[unwatched]`, refresh its index only on demand or reload, and keep visited files watched through their parent directories. The echo area and `*Messages*` SHALL explain the project, counts, limit and setting name. An unchanged refusal SHALL NOT be reported on every message. Index results and reloads SHALL reevaluate eligibility. A re-walk of the same roots and ignore patterns SHALL keep the previous counts, and so the root watches and mode line, until it answers; changed roots or patterns SHALL leave the project unwatched until their walk answers.

#### Scenario: Threshold and explanation
- **GIVEN** a project with two kept files and one directory, and limit one
- **WHEN** its workspace opens and the count completes
- **THEN** the project is unwatched and the echo and `*Messages*` explain the counts and `max_watched_files = 1`

#### Scenario: At the limit
- **GIVEN** a project with two kept files and one directory, and limit two
- **WHEN** its workspace opens and the count completes
- **THEN** its roots are watched

#### Scenario: Ignored trees
- **GIVEN** a project whose ignored tree exceeds its limit but whose kept files and directories each fit
- **WHEN** its count completes
- **THEN** the kept roots are watched

#### Scenario: Empty directories and zero
- **GIVEN** a project with more kept directories than the limit, even with no files
- **WHEN** its count completes
- **THEN** it is unwatched; zero also refuses an empty root because the root itself is counted

#### Scenario: A limit crossing
- **GIVEN** an eligible project gains files over its limit
- **WHEN** a watcher-triggered reindex finishes
- **THEN** root watches stop, visited buffers still revert, and a manual reindex after reducing the files can resume root watching

#### Scenario: An edit or reload keeps the watches
- **GIVEN** a watched project, or one refused over its limit
- **WHEN** a role edit or `C-x C-r` re-walks its unchanged roots and patterns
- **THEN** its root watches and mode line stay as they were throughout, and an unchanged refusal is not reported again
