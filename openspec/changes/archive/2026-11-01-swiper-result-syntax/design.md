# Design

## Context

See proposal.md for motivation. Swiper retains a flat string and LF-delimited byte ranges, while preview and acceptance follow the live buffer. Syntax highlights return source-character ranges; the frontend's text runs use UTF-8 byte lengths. Custom-filter completion rows currently have plain string labels and empty fuzzy-match highlights.

## Goals / Non-Goals

**Goals:** Keep immutable text and parsed syntax together, preserve full parsing context, and limit painting queries to displayed result ranges. Reuse the syntax engine and frontend text-run renderer.

**Non-Goals:** Change search matching or navigation; add search-term emphasis in the list; colour project-grep result text; introduce configuration or dependencies.

## Decisions

- Snapshot the current parsed syntax trees when Swiper opens, after applying pending source edits. Clone tree handles and injection context with an independent syntax cache, retaining a cheap Rope clone for character/byte translation. The original parser and trees remain editable without changing the snapshot. Parsing every line independently loses context; reparsing the whole file at prompt startup duplicates work.
- Query the frozen syntax for the sorted source ranges of visible result rows only. Convert returned character ranges to candidate-local UTF-8 byte ranges after the line-number prefix. Plain buffers carry empty syntax face lists.
- Add per-candidate syntax faces to the completion display. Generalize the existing completion text-run helper to consume ranged faces; ordinary fuzzy completion supplies its existing match face. The selected row's background overrides syntax backgrounds while preserving foreground and font styles.
- Resolve faces through the current theme at display time so reloads require no new snapshot. Keep existing plain labels, filtering, preview, acceptance and cleanup paths.

## Risks / Trade-offs

- Snapshot text differs from the live source after a revert -> keep parsed trees and coordinate translation frozen with the listed text; test a pending edit before opening and changes during the prompt.
- Unicode and line-number prefixes shift byte offsets -> convert using the frozen Rope and test multibyte text plus padded prefixes.
- Snapshot trees and local caches retain memory for the prompt -> release them through the existing prompt lifetime; do not retain all-file rendered faces.
- Selection can be obscured by a configured syntax background -> explicitly apply the selection background to the text runs on the selected row.
