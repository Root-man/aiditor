# Tasks

## 1. Effect

- [x] 1.1 Add `Effect::OpenUrl(String)` in `crates/aiditor-core/src/lib.rs` beside `OpenWithSystem`, pass it through in `harness.rs`, ignore it in `aiditor-headless.rs` and the service test supports (`file_sync.rs`, `scenario.rs`, `support/mod.rs`), and handle it with `cx.open_url(&url)` in `crates/aiditor/src/editor.rs`; verify `cargo build --workspace` and `cargo test --workspace --no-run` succeed

## 2. Bare URLs drawn as links

- [x] 2.1 In `markdown::render`, give words starting with `http://` or `https://` the link face alongside `is_file` words; verify with a test in `markdown.rs` that `See https://x.dev/docs.` links `https://x.dev/docs` without the `.` and `ftp://x.dev/file` stays plain

## 3. RET opens the URL

- [x] 3.1 Add `markdown::url_at(line, at)` returning the inline link destination, autolink text or bare `http(s)` word holding byte `at`; verify with unit tests for each kind and for a point outside any link
- [x] 3.2 Factor the "point has the `link` face and is above the prompt" check out of `link_at_point`, add `url_at_point` using it and `url_at`, and call it in `agent_send_input` after chips: push `Effect::OpenUrl` for `http(s)` URLs, else `Err("Not a web link: <url>")`; verify with harness tests in `agents.rs` covering the spec scenarios (markdown link, autolink, bare URL while a turn runs, `file:` link echoes and opens nothing, URL in the prompt area is sent) and that the existing `ret_on_a_message_link_visits_its_line_in_another_window` and tool header link tests still pass

## 4. Docs and check

- [x] 4.1 Add a sentence on URL links and `RET` opening `http(s)` in the browser to the file links note in `docs/editor.md`'s session buffer section; verify by reading the note
- [x] 4.2 Run `cargo test --workspace` and `cargo clippy --workspace`, then on macOS run aiditor, get an agent message with a markdown link and a bare URL, and press `RET` on each; verify the browser opens both and the prompt isn't sent
