# Design

## Context

The core constructs language segments from buffer syntax. The file service performs blocking reads, and `WatchSet.files` adds parent-directory watches even outside project roots. Existing git metadata follows this path; version lookups can use the same effect dispatch without new services or frontend layout.

## Goals / Non-Goals

**Goals:** Read declared versions asynchronously and keep all open copies current.

**Non-Goals:** Resolve installed versions, environment overrides, mise includes, plugin aliases beyond node/nodejs, lockfiles, or additional pin filenames. No new dependencies or settings.

## Decisions

- Put pure candidate enumeration and parsing beside core metadata, reusing core's installed TOML parser. The file service reads candidates in order. This keeps filesystem access out of the core without another dependency.
- Default precedence follows the issue's filename order within each directory, nearest directory first. Only entries for the file's tool count, so an unrelated nearer pin doesn't hide an ancestor's language entry.
- Map the currently highlighted runtime languages: Rust to rust, Elixir/HEEx to elixir, JavaScript/TypeScript/TSX to nodejs (`node` also accepted in mise). Configuration and prose modes have no tool version.
- Support mise string, first array item, and table `version`; rustup `toolchain.channel`; and the first whitespace-delimited asdf version, with comments stripped. Show literal values only. An explicit `system`, empty/unsafe value or template stops inheritance but supplies no displayed suffix. A malformed file or absent tool entry allows fallback.
- Watch candidate filenames in every ancestor, including missing files. This notices new nearer pins and deletion fallback even with `watch = false`. Reuse directory watches instead of polling or watching only the selected existing pin.
- Use dedicated `mode-line-<language>` faces for Elixir/HEEx, JavaScript, TypeScript/TSX, Rust, YAML, Markdown and TOML. Color the whole language segment, including an available version; keep Fundamental dim. Reuse the shared inactive-face and fitting logic. User review explicitly overrides the prior state-only colour rule.
- Tag each buffer lookup with a monotonically increasing request number; accept a reply only for that live buffer's latest request. Watcher events invalidate affected candidates or descendants, and rescans invalidate all versions.

## Risks / Trade-offs

- Ancestor candidates add non-recursive directory watches and may report unrelated sibling events. Only matching candidate paths invalidate versions; the project index retains its existing filtering.
- Literal channels such as `stable` remain literal; no installed version is claimed.
- Reads are per buffer, like current git metadata. Shared directory watches are deduplicated; add read caching only if measured usage needs it.

## Migration Plan

No persistent state or format changes. Reverting the change restores language-only segments.
