# Design

## Context

The file service already answers `FileOp`s on Tokio's blocking pool (`editor.rs`, `aiditor-headless`), and `find-file` drops listings the text has left. The core can't depend on `aiditor-services`, so the test harness carries out each `FileOp` itself.

## Goals / Non-Goals

**Goals:** one walk per request, off the core thread; the result stored per project for #70 to read.

**Non-Goals:** streaming results while the walk runs; building the index without the command; refresh on file events (#80); project `ignore` patterns (#190); persisting the index.

## Decisions

- **`ignore` over shelling out to `fd`/`rg`, `nucleo` for matching later, not fzf.** It is ripgrep's walker, already picked in `docs/editor.md`, cross-platform and in-process. fzf only matches, needs an external binary, and can't drive our minibuffer.
- **In memory, keyed by project name on `Core`.** A walk of a big repo takes well under a second, so an on-disk cache only adds staleness. A map on `Core` keeps `Loaded`, which also travels in `Msg`s, free of derived state. Entries of projects that vanish on reload stay until the next index of that name; they are small.
- **Cache, not walk-per-open (option B).** `$HOME`-sized projects with watching off (#191) would re-walk for seconds every time.
- **Walker settings:** `hidden(false)`, `require_git(false)`, a `filter_entry` dropping `.git`, the default of not following links; symlinks are kept when they stat as files. Paths are sorted so tests and the empty-pattern finder order are stable.
- **One message per project, a generation per request.** `Core` counts requests; the reply carries its number and is dropped unless it is the project's latest. That covers a walk from before `project-add-root` finishing after one from after it.
- **Sequential walk.** Simpler than `build_parallel`; marked `shortcut:` to switch if large repos feel slow.
- **Root errors only.** A root that can't be read (`fs::metadata`) is reported; unreadable entries inside are skipped, as `rg` keeps going.

## Risks / Trade-offs

- [The harness walks with `std::fs` and no gitignore] → the ignore rules are tested in `aiditor-services` against the real walk; core tests cover storage, messages and generations.
- [Pushing 100k paths into nucleo on the core thread in #70] → measured there; the injector is `Send`, so it can be fed off-thread.
