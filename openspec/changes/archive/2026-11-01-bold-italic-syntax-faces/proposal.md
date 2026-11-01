# Proposal

## Why

Issue #335 requires theme-controlled italic comments and bold keywords, including settings overrides. #332 already migrated the bundled and personal themes to faces and wired rendering; the remaining work is explicit acceptance coverage and an accurate contract.

## What Changes

- Specify syntax faces with bold and italic, attribute-wise overrides, and rejection of bare colours.
- Exercise named-theme inheritance and style removal, plus display and frontend font runs after a settings reload.
- Update the language-support documentation to describe faces rather than colours.
- Retain the bundled palette and the user's already-migrated Gruvbox themes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `syntax-highlighting`: explicitly require bold/italic attributes, per-attribute merging, table-only entries and reload behavior.

## Impact

Settings and display tests in aiditor-core, GPUI editor tests, and docs/editor.md. Reuse the existing Face representation and renderer without new dependencies or migration machinery. No personal configuration writes are needed.
