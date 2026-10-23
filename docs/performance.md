# Core performance benchmarks

The initial suite measures CPU work in `aiditor-core`, without the GUI or disk I/O:

| Benchmark | One operation |
| --- | --- |
| `buffer-load/100k-lines` | Create a buffer and its Rope from an existing UTF-8 string, then drop it |
| `edit-undo/100k-lines` | Insert one Unicode character at line 50,000, then undo it |
| `display/50-rows-in-100k-lines` | Build 50 visible display-model lines at line 50,000, including their drop |
| `swiper/two-queries-in-100k-lines` | Replace the open search query with `needle`, then `needle !skip`, including filtering and preview |

All cases use the same deterministic 100,000-line text, containing Unicode and an excluded line every fifth line. Swiper opening and fixture construction are outside the measured query loop. Observable self-checks verify line counts, displayed text, search counts and edit/undo restoration.

Each case warms up by doubling its iteration count until a batch takes at least 10 ms, then measures 21 batches with that iteration count. Results are the median, minimum and maximum of those batch averages, in nanoseconds per operation. These ranges are not per-event latency percentiles or statistical confidence intervals. No timing threshold fails ordinary CI.

## Run locally

On macOS or Linux, using the Rust version in `.tool-versions`:

```sh
cargo bench --locked -p aiditor-core --bench core
```

To save tab-separated results as well as print them, set `BENCH_OUTPUT` to an absolute path:

```sh
BENCH_OUTPUT=/tmp/aiditor-benchmarks.tsv cargo bench --locked -p aiditor-core --bench core
```

This uses Cargo's optimized bench profile and no additional dependencies. Ordinary `cargo test` does not run this target; the existing `cargo clippy --all-targets` CI step compiles it on Linux and macOS. The older ignored Swiper baseline test remains available for its more detailed query matrix.

## Run in GitHub Actions

After `benchmarks.yml` is merged into the default branch, open **Actions → Benchmarks → Run workflow**, select the branch to measure, and optionally supply its PR number. This is a manually started workflow, without a separate environment approval gate. It does not run on pushes or PR events.

The CLI equivalent is:

```sh
gh workflow run benchmarks.yml --ref your-branch -f pr=123
```

To exercise the comparison, run the workflow twice on the same branch with the same PR number: the first run establishes the baseline and the second compares against it when the environment metadata matches.

Omit `-f pr=123` for a summary without a PR comment. The tested commit is the selected branch's commit when dispatched. Fork PRs are not supported: the optional PR must be open, in this repository, and still have that tested commit as its head when reporting. Invalid or stale targets fail reporting after retaining the summary; they receive no comment.

The workflow uses `ubuntu-24.04`, builds only the core benchmark target, and caches its build artifacts separately from ordinary CI. The benchmark job has read-only repository permissions. A separate reporting job can read Actions artifacts and write PR comments. No hosted workflow is dispatched by local development commands.

Every successful benchmark job uploads `results.json` as `core-benchmarks`, retained for 90 days (subject to repository retention policy). The report always goes in the reporting job's summary; providing a PR number also creates a new PR comment. Re-running a workflow can create another comment.

The baseline is the previous successful `Benchmarks` workflow run on the **same branch**, excluding the current and later run IDs. It is not the PR's base branch. Failed runs are skipped. The first run, a missing/expired artifact, or changed benchmark definitions/environment establishes a new baseline with no percentage comparison. Other API or malformed-artifact errors fail reporting rather than silently claiming there is no baseline.

Comparisons require matching runner label, architecture, CPU model, full Rust compiler version and a fingerprint of the benchmark source and workspace build profiles. Results also record the tested commit, run ID and runner image version; dependency or production-code changes can therefore be measured without changing the benchmark fingerprint. The runner image version is informational and does not prevent comparisons. Positive changes mean slower; negative changes mean faster.

Hosted runners vary even with matching metadata. Treat changes as clues, repeat suspect measurements, and confirm meaningful regressions on consistent hardware. These benchmarks do not measure GUI rendering, input latency, filesystem services or terminal throughput.

## Reporting checks

The report helper uses Python's standard library and GitHub CLI. Its recording command is Linux CI plumbing, reading CPU information from `/proc/cpuinfo`; local benchmarking does not need it. Ordinary CI runs its small unit suite:

```sh
python3 -m unittest discover -s scripts -p test_benchmarks.py
```
