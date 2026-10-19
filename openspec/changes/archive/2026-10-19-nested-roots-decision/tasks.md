## 1. Record the decision

- [x] 1.1 Add a "Roots and folders" bullet under Projects in `docs/editor.md` linking #57, #58 and #64, and add the overlap check to the file-format bullet; verify by reading the section
- [x] 1.2 Remove the two questions from Open questions and the Projects bullet that lists them, keeping "switch project"; verify with `grep -n "Nested or overlapping" docs/editor.md` finding nothing
- [x] 1.3 Add open-folder to #64 and save-project for an ad-hoc project to #58 in `docs/roadmap.yaml` and on GitHub; verify with `gh issue view 58` and `gh issue view 64`

## 2. Reject overlapping roots

- [x] 2.1 In `Project::parse`, reject a root equal to or inside an earlier root (component-wise, either direction) with `<inner> is inside <outer>`; a `shortcut:` comment notes symlinks and `..` are not resolved
- [x] 2.2 Unit tests: nested root, duplicate root and a reversed order are rejected, `/r/app` beside `/r/app2` loads; `cargo test -p aiditor-core project`
- [x] 2.3 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
