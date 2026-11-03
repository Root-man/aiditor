# Spec Delta

## ADDED Requirements

### Requirement: Attach a disk path to the prompt
`agent-attach-file` SHALL read `Attach: ` over paths anywhere on disk, using find-file's starting directory, asynchronous filesystem completion, annotations and directory editing. The session SHALL remain visible without a file preview. A picked file, image or directory SHALL insert its existing chip at point in the editable prompt, with point after it and surrounding text preserved. From the transcript, it SHALL append at the prompt's end. Cancellation SHALL preserve the prompt and its point, including when output arrives during picking. Sending SHALL use existing chip resource-link and image behavior.

#### Scenario: An external file amid text
- **GIVEN** the prompt contains `compare  please` with point between the spaces
- **WHEN** `agent-attach-file` picks `/outside/a.rs`
- **THEN** the session shows `compare <file chip> please`, and sending includes the resource link `file:///outside/a.rs` between the text blocks

#### Scenario: An image
- **GIVEN** a session prompt
- **WHEN** `agent-attach-file` picks `/screenshots/view.png`
- **THEN** an image chip is inserted, and sending carries the existing image content for that path

#### Scenario: From the transcript
- **GIVEN** point is in the transcript and the prompt contains `work`
- **WHEN** `agent-attach-file` picks a file
- **THEN** its chip is appended after `work` and point follows it

#### Scenario: Cancel after output
- **GIVEN** the attachment picker opened amid prompt text
- **WHEN** agent output arrives above the prompt and `C-g` is pressed
- **THEN** the original prompt and its relative point are preserved

### Requirement: Navigate or attach a directory
In `Attach: `, `RET` on a selected directory SHALL enter it and keep the picker open. `C-RET` SHALL attach the selected file or directory as it is, closing the picker. With no selection, `C-RET` SHALL attach the typed path, with a trailing slash denoting a directory, so the current directory can be attached. `RET` on a file SHALL attach it. The extra binding SHALL be local to the attachment picker; other filesystem pickers SHALL keep their current behavior.

#### Scenario: Enter then attach a directory
- **GIVEN** the attachment picker selects directory `/outside/screens/`
- **WHEN** `RET` is pressed, then `C-RET` with no selection
- **THEN** `RET` lists that directory's entries and `C-RET` inserts its directory chip, sent as `file:///outside/screens/`

#### Scenario: Attach the selected directory directly
- **GIVEN** directory `/outside/empty/` is selected in `Attach: `
- **WHEN** `C-RET` is pressed
- **THEN** its directory chip is inserted without entering it

#### Scenario: Attach a selected file
- **GIVEN** a file candidate is selected in `Attach: `
- **WHEN** `C-RET` is pressed
- **THEN** its file chip is inserted
