# Proposal

## Why

The 2026-11-01 scan at `6c0ef19` found four synchronous display costs, a recovery-write failure that prevents retries, and obsolete assertions that stop the benchmark suite. These affect editing responsiveness and crash protection, and prevent measuring fixes from an unchanged checkout.

## What Changes

- **Finding 1 — local highlighting:** retain reusable local-binding analysis across edits and avoid one root highlight query per definition, preserving scope, shadowing, injection and theme semantics.
- **Finding 2 — preview overlays:** keep full-line matching semantics while storing and converting only matches needed by the displayed spans in Swiper and project grep.
- **Finding 3 — hidden streams:** render a dirty streamed message on frames that show its buffer; retain pending styling while hidden and preserve finalization at block boundaries.
- **Finding 4 — autosave:** confirm a snapshot only after its recovery file is persisted, report failures and retry without requiring another edit. Handle edits and buffer cleanup while writes are pending.
- **Finding 5 — Swiper rows:** bound result text and syntax painting to the available columns before passing rows to GPUI, keeping full source snapshots for filtering, preview and acceptance.
- **Finding 6 — benchmark contracts:** correct padded Swiper counts and initially unselected finder counts, retaining the existing fixtures, validation and sampling definitions.
- Add focused behavioral regression checks and measurements for these cases, and update the relevant architecture/performance documentation during implementation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `auto-save`: acknowledge persisted recovery snapshots, retry failed writes, and preserve cleanup and replacement-copy ordering with asynchronous replies.
- `session-buffers`: qualify once-per-frame streamed-message rendering by whether the buffer is shown, while retaining immediate text insertion and completed-block correctness.
- `line-search`: define width-bounded result presentation while retaining full-line matching, navigation, snapshot syntax and current theme behavior.

Local highlighting and preview-overlay optimizations implement the existing `syntax-highlighting`, `line-search` and `project-grep` contracts. Benchmark repairs are tooling changes; neither needs an invented capability.

## Impact

- Core: `syntax.rs`, `buffer.rs`, `display.rs`, `swiper.rs`, `grep.rs`, `minibuffer.rs`, `agents.rs`, `frame.rs`, `autosave.rs`, `lib.rs` and `harness.rs`, with nearby tests.
- Services/frontend: recovery worker and its callers, headless recovery loop, frontend effect/reply handling and relevant GPUI tests. Internal recovery message/operation shapes change; on-disk recovery format stays compatible.
- Validation: core benchmark sources, focused service/crash-recovery tests, frontend tests, `docs/editor.md` and `docs/performance.md`.
- No new dependencies, settings, commands, thread architecture, rich-session redesign or GitHub issue changes. The two lower-impact candidates omitted from the scan are outside this change.
- Evidence: `docs/measurements/2026-11-01-main-benchmarks.md` and its saved benchmark patch, plus the focused probes recorded in this change's design. Performance measurements are observations, not promises of a particular speedup.
