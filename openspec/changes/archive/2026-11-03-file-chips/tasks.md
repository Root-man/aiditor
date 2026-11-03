# Tasks

## 1. Display

- [x] 1.1 Add `strike` to `Face` and a `FaceRef::Strike` layer; draw it in the frontend; settings and frontend tests cover it
- [x] 1.2 Add `Prop::Display(pieces)`, drawn like a fold's placeholder with each piece's faces; display tests cover spans, cursor and right alignment

## 2. Chips in the core

- [x] 2.1 The chip table (interned by path, `U+F0000 + i`), kinds by extension, and `paint` over a range; prompt area repainted on frame when its tick moved; harness tests: drawn, one char for `C-f`/`DEL`, undo restores
- [x] 2.2 Send: split the prompt into text, resource-link and image blocks; echo painted; harness test checks the request's blocks
- [x] 2.3 Services: fill image data from the file (base64) or answer the prompt with an error; service test
- [x] 2.4 Replay: join user chunks by message id; links and image chunks become chips; harness test
- [x] 2.5 `RET` on a chip visits a file or opens `find-file` in a directory; echo the path while point is on a chip; harness tests
- [x] 2.6 Missing state via `FileOp::Check` on new paths and watcher reports; repaint struck; harness test

## 3. Docs

- [x] 3.1 Record the behaviour in `docs/editor.md`'s ACP section
