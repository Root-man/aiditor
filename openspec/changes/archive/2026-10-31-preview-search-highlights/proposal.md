# Proposal

## Why

Project grep and Swiper preview a hit without showing where the other matches are. #339 makes the search visible in the previewed window and distinguishes the selected match.

## What Changes

- Overlay `match` on every matching visible text span and `match-current` on the first match on the selected hit's line.
- Use each command's search semantics and smart case, including Swiper's exclusions.
- Limit overlays to the actual preview window and the lifetime of the prompt, without text properties.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-grep`: transient match faces in the selected hit's preview.
- `line-search`: the same transient faces with Swiper matching rules.

## Impact

Core display/frame/minibuffer, Swiper and grep matchers, display benchmarks and tests, and editor documentation. The core will reuse the existing grep-regex/grep-matcher dependencies for exact grep semantics. No new theme faces or persistent buffer changes.
