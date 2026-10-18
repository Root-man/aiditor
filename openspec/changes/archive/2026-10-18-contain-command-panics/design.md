# Design

## Context

Every command, whether from a key, `M-x` or a prompt's answer, reaches its handler through one line in `Ctx::try_call`. `*Messages*` is written by `Core::log`; the echo line by `Core::echo`. The core is a plain crate with no GPUI.

## Goals / Non-Goals

**Goals:** contain panics in command handlers with a backtrace in `*Messages*`; fail any `panic = "abort"` build.

**Non-Goals:** wrapping non-command core calls in the frontend (service answers, timers, rendering); repairing state a panicking command left behind.

## Decisions

- **Wrap the handler call in `try_call`.** One spot covers every entry. Nested calls (a command calling another) nest their own catch, so the innermost failing command is the one named. `AssertUnwindSafe` because the state may be inconsistent by design, and the log says so.
- **Backtrace through a chained panic hook.** `catch_unwind` returns only the payload, so a hook installed once (`std::sync::Once`) stores `"{info}\n{Backtrace::force_capture()}"` in a thread-local and then calls the previous hook, keeping stderr output and test-harness behaviour. After a caught panic the core takes that string; the payload is the fallback if it is missing. Alternative: install the hook in the frontend; rejected because the core tests could not see the backtrace.
- **Echo short, log long.** The echo line gets `<name> panicked; its state may be inconsistent (see *Messages*)`, which `echo` also logs; the panic and backtrace follow it in `*Messages*` only.
- **`#[cfg(panic = "abort")] compile_error!` in the core** instead of a CI grep of `Cargo.toml`: it also catches `-C panic=abort` from `RUSTFLAGS` or `.cargo/config.toml` and fails locally as well as in CI. Cargo builds tests and benches with unwinding regardless, so `cargo test` never trips it; `cargo build`/`clippy` do. CI builds only the dev profile, so a CI step also greps `Cargo.toml`, the crate manifests and `.cargo/` for `panic = "abort"`, covering `[profile.release]`.

## Risks / Trade-offs

- [The hook is process-global] → it chains to the previous one and only adds a thread-local write.
- [A panic in non-command code still ends the process, and aborts on macOS inside the key callback] → spec notes it; a follow-up wraps the frontend's calls.
- [A command's state may be left inconsistent] → said in the log; recovery files from #50 still guard unsaved text.
