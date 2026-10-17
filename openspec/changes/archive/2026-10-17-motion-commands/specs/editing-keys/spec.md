# Spec Delta

## REMOVED Requirements

### Requirement: Arrow keys move point
**Reason**: The arrow keys are now bindings of the motion commands (`forward-char`, `backward-char`, `next-line`, `previous-line`), which keep a goal column, take the prefix argument and move to the buffer's start or end on the first or last line.
**Migration**: See the `motion` capability; rebind `<left>`, `<right>`, `<up>` and `<down>` in `keymap.toml` to change them.
