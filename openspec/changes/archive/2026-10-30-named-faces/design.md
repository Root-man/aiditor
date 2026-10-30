# Design

## Context

`Settings.theme` is a struct of `u32`s. `buffer::Face` holds resolved colours, also in session buffers, whose properties `retheme_sessions` remaps on reload. `Decoration` carries colours and is only used by sessions. `docs/editor.md` (Themes and fonts) settles the file format.

## Goals / Non-Goals

**Goals:** the format decided in #345, faces read by name, sessions restyled by redisplay alone.

**Non-Goals:** drawing what the faces are for. A face with nothing drawn yet (`match`, `line-number`, `status-*` and so on) is defined and waits for its issue. Only the old keys' uses move: `default`, `cursor`, `region`, `completion-selected` (the selected candidate, which used `region`), `mode-line`, `mode-line-inactive`, `workspace-bar`, `separator`, `session-thought`, `session-block`, `session-code`, `link`. Fonts stay as they are.

## Decisions

**A `FaceName` enum, kebab-case through serde, with `faces: BTreeMap<FaceName, Face>` and `Theme::face(name)`.** Names are checked by the compiler, and an unknown key fails to deserialize with no extra validation. A struct of 41 fields couldn't be stored in a property by name; strings would lose both checks. `buffer::Face` itself deserializes (`deny_unknown_fields`, `default`), so a face in the file and in a span are one type. `Face::over` holds the "unset takes what is underneath" rule, and replaces the merge written out in `display.rs` and `markdown.rs`.

**`Settings` keeps `theme: Theme`, resolved in `parse`.** The file's `theme` key is `theme_name`, `themes` stays raw `toml::Table`s, and `Theme` is `#[serde(skip)]`. `parse` merges every table over `default` with the existing `merge` and deserializes it, so a typo in a theme that isn't selected is still rejected, then keeps the selected one. No caller of `settings.theme` has to learn about names. The old `[theme]` table fails as a map where a string is wanted.

**Sessions store `Prop::Named(Vec<FaceRef>)`, Emacs's list of faces.** `FaceRef` is `Face(FaceName)`, `Syntax(&'static str)` (`text.title`), `Fg(u8)`/`Bg(u8)` (ANSI indices) or `Bold`/`Italic`/`Underline`. The list is resolved at each redisplay, later entries over earlier. Keeping names layered, not one merged face, is what lets an overlapping heading in a thought take both faces' attributes from whichever theme is current. `Prop::Face` stays for resolved colours: terminal copy mode has true-colour cells no name can hold.

**`Decoration` holds a `FaceName`**: the frontend resolves it when painting, `Background` from `bg` and `Rule` from `fg`. Display model tests stop depending on colours, and no second "resolved" enum is needed.

**Inline code is `session-code` alone**, as frame 6 draws it, with no background, in the bundled theme too: a theme merged over `default` can't unset an attribute, so a bundled `bg` would show through every theme that sets only `fg`.

**`dim` maps to `session-thought` everywhere it was used in sessions**, including tool and plan suffixes and rules, as the issue says, and to `separator` for the workspace modeline's ` › `, whose design colour that is.

**`Theme::foreground()`, `background()` and `cursor()`** return `default`'s and `cursor`'s colours for the terminal and frontend. Merging only adds keys, so the bundled values can't be removed.

## Risks / Trade-offs

- A theme can't turn off `bold`, `italic` or `underline` by layering a face over another: they add, as `Face` always did. Within a theme's own table, `italic = false` over the merged default works.
- A theme can't unset an attribute it inherits from `themes.default`, so a bundled face must not set what other themes want left unset.
- Gruvbox's italic `session-thought` also slants tool suffixes until #365 redraws them.
- Terminal copy mode resolves colours when entered, so a theme change shows on the next entry, as today.

## Migration Plan

The old and new formats reject each other: a build from before this change reports an error on the new file and keeps defaults. The user's `settings.toml` is rewritten once, on this branch's testing, with a backup. Rollback is restoring that file.
