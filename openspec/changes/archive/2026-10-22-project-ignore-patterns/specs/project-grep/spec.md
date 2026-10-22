# Spec Delta

## MODIFIED Requirements

### Requirement: What a search covers
A search SHALL look in the same files the file index lists for each root, the project's `ignore` patterns applied, roots in the order given and files in path order. It SHALL treat a file as binary at its first NUL byte and stop searching it there, as ripgrep does, so a small binary file gives no hits. It SHALL skip files it cannot read and a root it cannot read.

#### Scenario: Ignored and binary files
- **WHEN** a root has `.gitignore` with `target/`, and `target/out`, `src/a.rs` and a file `blob` with a NUL byte all hold `needle`
- **THEN** the search finds `needle` in `src/a.rs` only

#### Scenario: Order
- **WHEN** roots `/r/app` and `/r/lib` both hold matching files
- **THEN** every hit under `/r/app` arrives before any hit under `/r/lib`, each root's in path order

#### Scenario: Project ignore patterns
- **WHEN** the project's `ignore` is `["*", "!notes/"]` and `notes/n.md` and `src/a.rs` both hold `needle`
- **THEN** the search finds `needle` in `notes/n.md` only
