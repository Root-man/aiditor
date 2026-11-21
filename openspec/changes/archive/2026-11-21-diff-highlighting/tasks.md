# Tasks

## 1. Core

- [x] 1.1 `FaceRef::Is(Face)` and `Buffer::set_grammar(path)`
- [x] 1.2 The index side buffer takes the file's grammar; the old side is a hidden buffer with it, killed with the diff
- [x] 1.3 Removed rows take the old buffer's syntax faces over `diff-removed`; redraw on its `Parsed` and on `SettingsLoaded`

## 2. Tests

- [x] 2.1 Core tests: old and new sides tokenising differently (a removed line inside an old block comment), added lines' faces, the staged side, a language without a grammar, a large old side coloured when its parse arrives

## 3. Docs

- [x] 3.1 `docs/editor.md`: highlighting in the diff view as built
