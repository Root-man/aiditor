## MODIFIED Requirements

### Requirement: What a search covers
A search SHALL look in the same files the file index lists for each root, with the project's `ignore` patterns applied relative to each root, and, when it names files, only in files whose names contain one of them. Roots SHALL be searched in the supplied order, one at a time. Cross-file order within a root SHALL be unspecified; one file's hits SHALL keep line order. A file SHALL be binary at its first NUL byte and stop there, as ripgrep does. Unreadable files and roots SHALL be skipped.

#### Scenario: Ignored and binary files
- **GIVEN** a root has `.gitignore` with `target/`, and `target/out`, `src/a.rs` and a file `blob` with a NUL byte all hold `needle`
- **WHEN** the root is searched
- **THEN** the search finds `needle` in `src/a.rs` only

#### Scenario: Order
- **GIVEN** roots `/r/app` and `/r/lib`, at least two workers, and matching files in both roots
- **WHEN** the roots are searched in that order
- **THEN** all hits from `/r/app` precede those from `/r/lib`

#### Scenario: Project ignore patterns
- **GIVEN** the project's `ignore` is `["*", "!notes/"]` and each root has matching `notes/n.md` and `src/a.rs`
- **WHEN** all roots are searched
- **THEN** the search finds the matching lines in each root's `notes/n.md` only

#### Scenario: Named files
- **GIVEN** a root holds matching `a/x1.jsonl`, `b/rollout-x2.jsonl` and `c3.jsonl`
- **WHEN** it is searched naming `x1` and `x2`
- **THEN** the search finds the lines in `a/x1.jsonl` and `b/rollout-x2.jsonl` only
