# Tasks

## 1. Markers

- [x] 1.1 Add `MarkerId`, `InsertionType` and the marker map to `Buffer`, with `add_marker`, `marker`, `set_marker` and `remove_marker`; verify with unit tests for add, read, out-of-range and remove
- [x] 1.2 Make `insert` and `delete` move markers after a successful edit; verify with unit tests for markers before, at and after inserts and deletes for both insertion types, a refused edit, and a proptest that a marker on a char follows it through random edits elsewhere
- [x] 1.3 Record in `docs/editor.md` open questions that a window's point and mark follow edits as markers in the buffer; verify the open question no longer lists it as undecided
