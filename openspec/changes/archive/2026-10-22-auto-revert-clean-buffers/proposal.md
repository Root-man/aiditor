# Proposal

## Why

A file open in two workspaces has two buffers, kept in sync through the disk. Agents, `git checkout` and the other workspace's saves change files under open buffers. The watcher (#80) already checks each buffer visiting a reported path, but only for deletion. Issue [#81](https://github.com/Root-man/aiditor/issues/81) makes a clean buffer follow its file: the buffer that saved sees its own hash and stays put, and every other clean buffer of that file reverts, in every workspace.

## What Changes

- When the per-buffer check finds the file present with a hash other than the buffer's disk hash, an unmodified buffer is reverted with the diff revert (#79). Point, mark and markers stay with their text, the old text is one undo group, and the buffer ends unmodified with the file's hash.
- An equal hash does nothing, so the saving buffer is left alone.
- A modified buffer is left as it is, its disk hash included, so `C-x C-s` still refuses to overwrite the change. Marking it conflicted is #83.
- A clean buffer in the deleted state whose file comes back reverts to it, as does a clean buffer of a file that didn't exist when visited. Its save is then no longer refused.
- Defaults picked:
  - An auto-revert is silent: no echo, since it often happens in a workspace the user isn't looking at.
  - A failed auto-revert (the file turned binary or non-UTF-8, or was deleted between the check and the read) echoes its error, as `revert-buffer` does, through the same reply handler.
- Not included:
  - the conflict state and its choices (#83);
  - the diff buffer (#82);
  - an echo or `*Messages*` line per auto-revert.

## Capabilities

### New Capabilities

### Modified Capabilities
- `file-watching`: a check also compares the file's hash with the buffer's, reverting a clean buffer whose file changed.

## Impact

- `aiditor-core`:
  - `watching.rs`: `on_checked` moves from `Core` to `Ctx`, so it can ask the file service for a revert;
  - `buffer.rs`: `Buffers::iter_mut` yields ids too;
  - `lib.rs`: the `Msg::Checked` dispatch.
- `aiditor-services`: none; `FileOp::Check` and `FileOp::Revert` are already answered.
- `docs/editor.md`: a "Built in #81" note under File watching, and the "Not yet" line of #80 updated.
