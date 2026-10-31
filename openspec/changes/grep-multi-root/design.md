# Design

## Context

The core owns grep query state and supplies ordered root paths. The service currently submits every root to one parallel walker; ready hits stream through a bounded channel to a deadline-driven collector. Handler-provided minibuffer choices already use fuzzy completion instead of the command's hit filter.

## Goals / Non-Goals

**Goals:** implement root priority without buffering another root's results or changing the hit protocol.

**Non-Goals:** multiple root selection, a separate command, deterministic file ordering or new configuration.

## Decisions

- Assign finder names in project order before promoting the most specific containing root. Buffers without a visited file use project order. This keeps duplicate names stable across searches.
- Keep root-picker state in the grep session and suspend query polling until a root is accepted. Use handler choices for fuzzy completion and clear the accepted root argument before reading the query, preserving the existing hit-acceptance path and query history. Any prefix invokes the picker, following commands that treat prefix presence as a mode switch.
- Run one parallel walk per root, sequentially, using the existing shared result channel and collector across roots. This guarantees root order with bounded storage and preserves deadline flushing and the global cap. Concurrent traversal with buffered later-root hits would require unbounded buffering or more coordination.
- Pass the searched root explicitly to file search so overlapping roots retain correct relative paths and root-specific ignore behavior.

## Risks / Trade-offs

- A slow first root delays subsequent roots; this is the intended priority policy in #340 and supersedes the previous cross-root concurrency contract. Files within that root remain concurrent.
- Repeated root walks add setup cost; no new cache or tuning is needed for this scope.
- Nested roots can search the same physical file through multiple roots, as independent configured roots; each traversal applies its own relative ignores and labels.

## Verification fixture correction

Workspace checks exposed the bundled-keybindings GUI test leaving the default home project active. Its project commands could launch a home-directory index walk, delaying runtime shutdown indefinitely. Bind that test to its existing temporary file's directory; this changes only the test fixture and keeps the key-routing assertions intact.
