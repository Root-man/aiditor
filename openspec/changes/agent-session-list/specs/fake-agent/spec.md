## ADDED Requirements

### Requirement: Listing sessions
The script MAY have a `sessions` list of pages, each a list of ACP `SessionInfo` objects. With it, the fake SHALL advertise `sessionCapabilities.list`; without it, it SHALL NOT. `session/list` without a cursor SHALL answer page 0, and with cursor `n` page `n`, keeping only the sessions whose `cwd` is the request's `cwd` (all when the request has none), with `nextCursor` `n+1` when another page follows. A cursor that names no page SHALL be an invalid-params error.

#### Scenario: Two pages
- **WHEN** the script has two pages and the client lists with the `cwd` of their sessions
- **THEN** the first answer holds page 0 with `nextCursor` `1`, and the second page 1 with no `nextCursor`
