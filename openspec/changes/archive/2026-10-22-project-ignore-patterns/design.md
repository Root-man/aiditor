# Design

## Context

The file index, project grep and the watcher all walk roots with one builder, `files::walker` in `aiditor-services` (the `ignore` crate's `WalkBuilder` with a `.git` filter). The core sends roots as plain paths in `FileOp::Index`, `GrepSearch` and `WatchSet`. The core has no `ignore` crate; `aiditor-services`' `projects::check` already adds the filesystem checks to `Project::parse`, and every project load and write goes through it.

## Goals / Non-Goals

**Goals:** one place decides what the patterns leave out, used by all three walkers; an allow-list written the way the issue writes it (`*`, `!dir/`) works.

**Non-Goals:** per-root patterns; patterns that re-include gitignored files; a project command to edit the patterns (the file is edited by hand, `C-x p o`).

## Decisions

- **Top-level `ignore`, matched per root.** One list for the project, each pattern relative to each root, as a `.ignore` at each root would be. Per-root lists were the alternative; the issue names one key and the motivating case (`$HOME`) has one root.
- **Applied as a walk filter on top of the ignore files.** The walker's `filter_entry` drops entries the patterns ignore, so an entry is kept only when both the ignore files and the patterns keep it. `WalkBuilder::add_ignore` (lowest precedence, so `.gitignore` negations would override it) and overrides (inverted, whitelist-first, and able to re-include gitignored files) both had the wrong precedence.
- **The last pattern matching the path or a directory above it decides.** Each pattern is compiled into its own `Gitignore` matcher; for an entry, patterns are tried last to first with `matched_path_or_any_parents` on the root-relative path, and the first that matches decides (ignore or keep). So `*`, `!notes/` keeps `notes/a.md`: `!notes/` matches its parent and comes after `*`. Plain gitignore semantics would leave `notes/`'s files out (`*` matches them by name, and git can't re-include under a re-included directory's own matches), contradicting the issue's allow-list; a later `notes/*.tmp` still excludes again. Patterns that only exclude behave exactly as in a `.gitignore`.
- **Checked in `aiditor-services`' project check.** `projects::check` compiles every pattern and rejects the file with `ignore pattern <p>: <error>`. Keeping the check next to the path checks avoids an `ignore` dependency in the core. The walkers compile again at walk time; a failure there can't happen past the check, and drops that pattern.
- **Ops carry the patterns.** `FileOp::Index` and `GrepSearch` (one project each) get an `ignore` field; `WatchSet.roots` becomes `(root, patterns)`, since a root shared across projects may have different patterns. The watcher keys its roots by that pair; a directory any of them keeps is watched, and listings for a parent are the union over the roots keeping it.
- **A reload re-walks the indexes.** `C-x C-r` used to leave indexes as they were, so a pattern edited by hand would stay invisible to the finder until `C-x p i`; the reload now re-walks each indexed project quietly, as a create or delete does. The watch set already follows reloads.
- **Project commands leave the key alone.** `project::sync` only sets the keys it knows, so `ignore` and its comments stay through `C-x p a` and friends.

## Risks / Trade-offs

- [The allow-list rule differs from git's for negated directories] → documented in `docs/editor.md`; a `.gitignore`-style list with only excludes behaves as in git.
- [N patterns × path depth matchings per walked entry] → patterns are few and pruned directories are never entered; fine for `$HOME` narrowed to a few directories.
- [Patterns changed by hand take effect on reload] → `C-x C-r` reloads projects, and the next watch set and index walk use the new patterns, as with roots.
