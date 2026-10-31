# Proposal

## Why

Project grep can show only `[searching]` until almost all work is finished: it walks roots serially and withholds ready hits behind earlier files, making expensive macOS file opens especially visible. The Legacy crap investigation found 7,225 match-free files before the first useful root; parallel discovery reduced the measured four-worker first-hit time from 399 ms to 28 ms without changing security settings.

## What Changes

- Search across roots with the existing worker budget and deliver available hits without waiting for earlier roots or files.
- **BREAKING:** Append hits in arrival order and retain the first 10,000 received by the collector. Cross-file order and capped membership may vary between runs; do not re-sort on completion. The user explicitly selected this behavior.
- Flush the first hit immediately and subsequent pending hits on an approximately 50 ms deadline independent of further matches or file completion.
- Preserve search coverage, per-file line order, hit representation, cancellation, exact cap, prompt status and stable selection.
- Bound active search workers and application result buffering; use the existing traversal library without building a separate complete file inventory. Its traversal frontier remains library-managed rather than claiming a fixed total path-memory bound.
- Replace investigation-only code with focused service/core regression tests and a sparse late-root benchmark. Remove the temporary probes and Defender decoder, and condense the investigation documentation while retaining aggregate evidence.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-grep`: Replace ordered delivery with arrival-order streaming and capped membership, independent batch deadlines, parallel root discovery, and explicit worker/result-buffer bounds.

## Impact

- `crates/aiditor-services/src/grep.rs`: traversal, worker coordination, batching, cancellation and their tests.
- `crates/aiditor-services/src/files.rs`: reuse root-relative ignore and file-selection policy for multi-root search while preserving index/watcher behavior.
- `crates/aiditor-core/src/grep.rs`: verify arrival-order batches preserve selection, preview and search status; retain the existing message interface and append behavior.
- `crates/aiditor-services/benches/services.rs`, `docs/editor.md` and `docs/performance.md`: persistent regression coverage, benchmark instructions and documented ordering.
- Remove `crates/aiditor-services/examples/{grep_probe,open_probe}.rs` and `scripts/analyze_mde_trace.py` after transferring useful checks and recording final measurements. Retain the three existing aggregate JSON evidence files; leave raw archives and unrelated temporary files alone.

No new dependency, external `rg` process, content index/cache, worker-setting change, security-policy change or unrelated headless-runner rewrite is proposed. The fix improves scheduling and time to visible hits; it does not remove host-level Endpoint Security authorization latency.
