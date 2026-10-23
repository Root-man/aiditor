# Spec Delta

## MODIFIED Requirements

### Requirement: The newest rebuild wins
When a project is indexed again before an earlier walk is done, the earlier walk's answer SHALL be dropped, whenever it arrives. At most one walk SHALL run for a project at a time; requests received during it SHALL coalesce into one latest pending refresh, started when the active walk finishes. The pending refresh SHALL use the latest requested roots and ignore patterns. A user-requested completion announcement SHALL survive intervening quiet refresh requests and be emitted once for the final accepted result. An answer for a project no longer loaded or retained for a live workspace SHALL be dropped. Same-scope rebuilds SHALL preserve the last accepted counts and watch eligibility until an accepted answer arrives.

#### Scenario: Out of order
- **GIVEN** a project has accepted a newer requested rebuild
- **WHEN** an obsolete earlier reply is delivered late
- **THEN** the index holds the newer rebuild's files and nothing is echoed for the obsolete reply

#### Scenario: A burst while a walk runs
- **GIVEN** one project walk is running
- **WHEN** several creates, deletes and explicit rebuild requests arrive before it finishes
- **THEN** one pending refresh represents those requests, starts after the active walk completes, and supplies the final accepted index

#### Scenario: A user request survives quiet invalidation
- **GIVEN** `project-index` requested an announced rebuild
- **WHEN** watcher events request quiet rebuilds before it finishes
- **THEN** the final accepted result produces one `Indexed <n> files in <project>` completion with the usual singular and error formatting

#### Scenario: Scope changes while queued
- **GIVEN** a project walk runs for roots and patterns that are then changed twice
- **WHEN** the active walk finishes and the pending refresh starts
- **THEN** that refresh uses the latest roots and patterns, and only its accepted counts restore root-watch eligibility

## ADDED Requirements

### Requirement: Ignore-file changes refresh the index policy
A reported change to a `.gitignore` or `.ignore` under a watched project's roots SHALL refresh that project's index and watch coverage even when the ignore file itself is excluded from ordinary candidates. Previous suppression of paths the walk ignored SHALL NOT prevent reevaluation under the changed rules. Explicit indexing and configuration reload SHALL reread ignore files and git's global excludes. Projects with watching disabled SHALL keep their existing on-demand refresh policy.

#### Scenario: A previously excluded tree is included
- **GIVEN** a watched project's `.ignore` excludes `notes/` and a visited file there was already reported and suppressed
- **WHEN** the rule is removed and the ignore-file change is reported
- **THEN** the refreshed index includes the kept files in `notes/` and subsequent relevant creates can refresh it again

#### Scenario: Global excludes change before a manual rebuild
- **GIVEN** git's global excludes have changed since the project's previous walk
- **WHEN** the user runs `project-index`
- **THEN** its accepted files and watch counts reflect the current global excludes
