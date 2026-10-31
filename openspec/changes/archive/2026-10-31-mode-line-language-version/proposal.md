# Proposal

## Why

The file mode line names the language but omits the version pinned by its project. Issue #357 adds that context without running a compiler or version manager.

## What Changes

- Append the nearest declared tool version to the file's language segment.
- Read `.tool-versions`, `mise.toml` and `rust-toolchain.toml` above the file through the file service.
- Refresh every affected open buffer when a pin changes, including outside watched project roots.
- Keep the language alone when no usable pin exists.
- Color the language name and version together using an overridable face for each supported language family; inactive windows keep their shared dim face.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `display-model`: the language segment includes a declared version, language color and follows changes on disk.
- `settings`: language-family faces can be overridden through the existing theme tables.

## Impact

Core buffer metadata, file operations and their test harness, the file service, watcher integration tests, and `docs/editor.md`. Reuse the existing TOML parser and mode-line renderer; add no dependency or command execution.
