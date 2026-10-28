## ADDED Requirements

### Requirement: The session buffer is named after the session's title
A `session_info_update` whose `title` is a string SHALL rename the session buffer to `*<agent>: <title>*<<workspace>>`, made unique, where `<title>` is the title's first line with surrounding whitespace trimmed, cut to its first 40 chars with `…` appended when longer. A title that is empty after trimming, or a `title` that is null, SHALL rename the buffer back to `*<agent>*<<workspace>>`, made unique. An update without `title` SHALL leave the name alone. `<agent>` and `<workspace>` SHALL be the names the buffer was started with. Every titled update SHALL rename again; an update after the session has ended SHALL be ignored.

#### Scenario: The agent titles the session
- **WHEN** the session started as `*claude*<main>` receives a `session_info_update` with title `Fruit list`
- **THEN** the buffer is named `*claude: Fruit list*<main>`

#### Scenario: A later title renames again
- **WHEN** a session titled `Reply with a list` receives a `session_info_update` with title `Two-fruit markdown list`
- **THEN** the buffer is named `*codex: Two-fruit markdown list*<main>`

#### Scenario: A long or multi-line title
- **WHEN** the title is a 60-char first line followed by a second line
- **THEN** the name holds the first line's first 40 chars followed by `…`, and no text from the second line

#### Scenario: An update without a title
- **WHEN** a `session_info_update` carries only `_meta`
- **THEN** the buffer keeps its name

#### Scenario: Clearing the title
- **WHEN** a session named `*claude: Fruit list*<main>` receives a `session_info_update` with title null
- **THEN** the buffer is named `*claude*<main>`

#### Scenario: Two sessions with one title
- **WHEN** two claude sessions in `main` both receive the title `Fruit list`
- **THEN** one buffer is named `*claude: Fruit list*<main>` and the other `*claude: Fruit list*<main><2>`

### Requirement: Session buffer names belong to the agent
A session buffer's name SHALL change only by its session's title. Renaming the workspace SHALL NOT rename it, and any command that renames buffers by hand SHALL refuse a session buffer.

#### Scenario: Renaming the workspace
- **WHEN** workspace `main` holding `*claude: Fruit list*<main>` is renamed to `work`
- **THEN** the session buffer is still named `*claude: Fruit list*<main>`
