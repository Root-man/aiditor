## MODIFIED Requirements

### Requirement: The major mode names the file's language
The major mode SHALL be the name of the language the file is highlighted in, and `Fundamental` for a file with no grammar. For Rust, Elixir and HEEx, and JavaScript, TypeScript and TSX, it SHALL append the declared Rust, Elixir or Node.js version respectively when found above the file. Search SHALL start in the file's directory and ascend to the filesystem root, checking `.tool-versions`, `mise.toml`, then `rust-toolchain.toml` at each level; the first entry for the relevant tool SHALL win. An unrelated tool or unreadable/malformed file SHALL allow searching to continue. With no usable version, the segment SHALL show the language alone. Only file contents SHALL be read: no compiler, version manager, template or command SHALL be executed to resolve a version.

#### Scenario: A Rust file
- **GIVEN** `a.rs` has no pin above it
- **WHEN** it is shown
- **THEN** the major mode is `Rust`

#### Scenario: No grammar
- **GIVEN** `a.txt` has no grammar
- **WHEN** it is shown
- **THEN** the major mode is `Fundamental`

#### Scenario: Supported pin formats
- **GIVEN** a Rust or Elixir file below `.tool-versions` or `mise.toml` declaring its tool version, or a Rust file below `rust-toolchain.toml` declaring `toolchain.channel`
- **WHEN** it is shown
- **THEN** the language segment includes the declared version, such as `Rust 1.99.0` or `Elixir 1.20.2-otp-26`

#### Scenario: JavaScript runtime
- **GIVEN** JavaScript, TypeScript or TSX files below a `nodejs` entry in `.tool-versions` or a `node`/`nodejs` entry in `mise.toml`
- **WHEN** they are shown
- **THEN** their language names are followed by that runtime version

#### Scenario: Nested pins and ties
- **GIVEN** a pin at a parent directory and a nearer pin for the same tool
- **WHEN** a file under the nearer pin is shown
- **THEN** the nearer pin wins, with ties in one directory resolved in the filename order above

#### Scenario: Multiple versions and system selection
- **GIVEN** an entry declares several versions or selects `system`
- **WHEN** its file is shown
- **THEN** only the first version is used, and `system` shows the language alone without falling back to a parent pin

#### Scenario: Unsupported or invalid value
- **GIVEN** a pin value is empty, contains whitespace/control characters, or is a dynamic mise template
- **WHEN** its file is shown
- **THEN** that entry supplies no displayed version

## ADDED Requirements

### Requirement: Language versions follow disk changes
Open file buffers in every workspace SHALL refresh their language versions when a candidate pin file changes, is created, deleted or atomically replaced, or when the watcher requests a rescan. This SHALL work with project watching disabled and outside project roots, including creation of a nearer pin. Replies from a superseded lookup or for a killed buffer SHALL be ignored. The version SHALL use the language segment's existing faces and fitting rules.

#### Scenario: Editing a shared pin
- **GIVEN** two workspaces have open files using the same pin
- **WHEN** another program edits or atomically replaces that pin
- **THEN** both buffers' language segments follow the new version without reopening files

#### Scenario: A nearer pin appears and disappears
- **GIVEN** an open file inherits a parent pin and project watching is disabled
- **WHEN** a nearer pin is created and then deleted
- **THEN** the segment first uses the nearer version and then returns to the parent's version

#### Scenario: Lost events or late replies
- **GIVEN** a version lookup is in flight
- **WHEN** a pin change or rescan starts a newer lookup and the older reply arrives last
- **THEN** the newer version remains displayed

#### Scenario: Inactive and narrow windows
- **GIVEN** a file with a pinned version
- **WHEN** its window is inactive or too narrow for its whole mode line
- **THEN** the version is dimmed with the rest of the inactive mode line or truncated under the existing group fitting rules
