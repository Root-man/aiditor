# Design

## Context

See proposal.md for the coverage gap. `Watcher::spawn` delivers `Msg::FilesChanged`; `files::run` carries out Core's file effects. Core's public workspace, launch, open, key and current-buffer APIs are sufficient to assemble these without exposing internals. CI already runs `cargo test` on Ubuntu and macOS.

## Goals / Non-Goals

**Goals:** Exercise real native notifications and file I/O through Core across workspace boundaries, including fallback watches.

**Non-Goals:** Add a second application runtime, test GPUI rendering, simulate OS queue overflow, or duplicate the lower-level unit suites.

## Decisions

- Keep the driver inside one services integration-test file. A channel queues native reports; Core runs on the test thread, and real `files::run` replies are delivered after each effect batch as in the application. A production runtime abstraction or extension of the synthetic Core harness would add scope.
- Initialize temporary Git roots and use Core's computed watch sets, including its real index preflight. Use public workspace APIs to inspect each copy, rather than exposing private buffer storage for tests.
- Establish readiness by repeatedly modifying a visited probe until a native report names it. Use bounded receive/predicate waits for outcomes; avoid startup sleeps or synthesized change messages. Keep filenames beneath canonical temporary roots to avoid macOS path aliases.
- Assert observable text, saved/dirty/conflict state, disk hashes, ticks, and undo behavior. A subsequent write after an atomic replacement checks that watching survives replacement.
- Treat watch-disabled and zero-limit projects as two inputs to the same fallback scenario. No configuration defaults change. Spec deltas are deliberately skipped because every assertion covers existing behavior.

## Risks / Trade-offs

- Native delivery is asynchronous -> bounded readiness and outcome waits produce explicit failures instead of depending on scheduling.
- Native watcher tests cannot receive macOS FSEvents inside this filesystem sandbox -> run native tests with native filesystem access; CI runners already provide it.
- The test loop runs file operations synchronously -> production asynchronous reply races remain covered by focused Core tests, while this suite verifies the assembled disk-sync path.

## Migration Plan

No migration. The new integration target is discovered by the existing workspace test command on both CI operating systems.
