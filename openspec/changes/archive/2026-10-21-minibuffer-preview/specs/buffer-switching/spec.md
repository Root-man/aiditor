# Spec Delta

## ADDED Requirements

### Requirement: Switching to a buffer previews it
`switch-to-buffer` SHALL preview the selected buffer in the selected window while its minibuffer is open, at the point the buffer remembers. A typed name no buffer has SHALL preview nothing. `switch-to-buffer-other-window` and `kill-buffer` SHALL NOT preview.

#### Scenario: The previous buffer shows at once
- **WHEN** a file is visited from `*scratch*` and `C-x b` is typed
- **THEN** the selected window shows `*scratch*` while the minibuffer is open

#### Scenario: Aborting leaves the buffer order alone
- **WHEN** files `a` then `b` are visited from `*scratch*`, `C-x b C-n C-g` is typed, then `C-x b`
- **THEN** `b` is shown again after `C-g`, and the list still holds `a`, `*scratch*`, `b` in that order, with `*Messages*`, made by `Quit`, before `b`
