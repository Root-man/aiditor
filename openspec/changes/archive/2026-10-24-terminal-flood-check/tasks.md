## 1. Benchmarks

- [x] 1.1 Add `terminal-output/64k-batch-of-yes`, `terminal-output/64k-batch-of-cat` and `terminal-display/50x120-screen` to `crates/aiditor-core/benches/core.rs`; verify with `python3 scripts/bench_local.py`

## 2. Drain yields

- [x] 2.1 Yield to the UI executor after each message in `Editor::drain`; verify with a GPUI test where a producer keeps a channel full and another foreground task still runs

## 3. Measure and record

- [x] 3.1 Run `yes` and `cat` of a large file in a terminal (as its `SHELL`, since typing into a terminal waits for char mode) while typing in a file buffer, with `--debug`, on Linux; record the bench numbers and the measured key/frame times in docs/editor.md under Output floods
- [x] 3.2 If typing still lags, add a roadmap task to move the emulator into the terminal task (roadmap skill); otherwise note in docs/editor.md that it stays in the core
- [x] 3.3 Move the macOS run of 3.1 to the M2 gate (#92, roadmap.yaml)
