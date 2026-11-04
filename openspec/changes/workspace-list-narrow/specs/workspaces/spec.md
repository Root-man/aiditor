# Spec Delta

## ADDED Requirements

### Requirement: Workspace list fits each window
The workspace list SHALL fit its rows independently to each window's current text width. Narrow windows SHALL omit workspace and buffer where columns before shortening names, collapse state words to glyphs in the same status faces, and retain project primary roots. Long names SHALL end in `…`; project roots SHALL lose their start to `…`, sharing available room with their names by the shared mode-line fitting rule. Existing fold markers, indentation, current marker and selected-row decoration SHALL remain. At extreme widths all row text SHALL stay within the available columns.

The list mode line SHALL omit inventory counts as a whole before shortening its name, retaining the selectable position using the shared mode-line fitting rules. Widening SHALL restore omitted details. Fitting SHALL run on width changes without refreshing inventory, state or folds, mutating snapshot text, or changing navigation/activation identities.

#### Scenario: Half of a 900px frame
- **GIVEN** a workspace list in one half of a 900px frame, containing project roots, a long current workspace name and active states
- **WHEN** its window is drawn
- **THEN** it shows project roots, names ending in ellipsis when needed, state glyphs, selected/current decoration and a mode line with its name and selectable position, as in frame 13c

#### Scenario: Resize a retained snapshot
- **GIVEN** a list with folded rows, a selected workspace and metadata captured before an underlying buffer changes
- **WHEN** its width narrows and widens without refreshing
- **THEN** rows refit and restore details from the same snapshot while folds, selection and inventory remain unchanged

#### Scenario: Different widths for the same list
- **GIVEN** the same list buffer is visible in two windows of different widths
- **WHEN** the frame renders
- **THEN** each window fits its own rows and mode line without affecting the other's presentation

#### Scenario: Very small widths
- **GIVEN** long Unicode names, project roots and multiple buffer states
- **WHEN** the list is rendered at several widths down to zero columns
- **THEN** each row fits its columns, shortening names at the end and roots at the start wherever room permits
