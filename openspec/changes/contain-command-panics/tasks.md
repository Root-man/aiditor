# Tasks

## 1. Contain panics

- [x] 1.1 In `crates/aiditor-core/src/lib.rs`, run the handler in `Ctx::try_call` under `catch_unwind` with a once-installed chained panic hook that captures the panic and a backtrace; echo `<name> panicked; its state may be inconsistent (see *Messages*)` and log the details. Verify with a harness test that a panicking command logs `boom`, its location and a backtrace to `*Messages*`, and that typing `a` afterwards inserts it.
- [x] 1.2 Add `#[cfg(panic = "abort")] compile_error!` to the core. Verify `cargo build` with `CARGO_PROFILE_DEV_PANIC=abort` fails with that error and builds without it.
- [x] 1.3 Record what was built in `docs/editor.md` Crash safety (handler wrapped; frontend wrapper not yet). Verify by reading the section.
- [x] 1.4 Add a CI step that fails when a manifest or `.cargo/` sets `panic = "abort"`. Verify the grep passes on the tree and fails with `[profile.release] panic = "abort"` appended to `Cargo.toml`.

## Workflow follow-up

- Verify, commit, archive and ship per the implement skill.
