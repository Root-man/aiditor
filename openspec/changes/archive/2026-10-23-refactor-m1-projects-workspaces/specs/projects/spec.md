# Spec Delta

## MODIFIED Requirements

### Requirement: Projects load at launch and on C-x C-r
The editor SHALL read every `*.toml` file in the projects directory at launch and again when `C-x C-r` reloads settings, reconciling the saved projects it had by their project-file paths. A valid replacement at the same path SHALL update the same project even when its name changes. Ad-hoc projects SHALL stay. A saved project whose file is absent or invalid SHALL retain its last valid definition while a live workspace is bound to it; otherwise it SHALL be removed. A retained definition SHALL keep its file association and last-loaded text for normal checked project edits. Invalid files SHALL still report their errors. A missing projects directory at launch SHALL mean no saved projects and no error. During reload it SHALL retain only saved definitions still bound to live workspaces, in addition to ad-hoc projects.

#### Scenario: No projects directory
- **GIVEN** the editor is starting
- **WHEN** the config dir has no `projects` directory
- **THEN** there are no saved projects and no error is shown

#### Scenario: Reload picks up a new file
- **GIVEN** the editor is running
- **WHEN** a project file is added and `C-x C-r` is pressed
- **THEN** the new project is loaded

#### Scenario: Reload keeps ad-hoc projects
- **GIVEN** the editor was started with a folder no saved project contains, giving an ad-hoc project
- **WHEN** `C-x C-r` is pressed
- **THEN** the ad-hoc project is still there and its workspace is still bound to it

#### Scenario: A bound saved file disappears
- **GIVEN** a workspace is bound to a saved project
- **WHEN** the project's file is removed and settings are reloaded
- **THEN** that workspace retains the last valid project roots and settings, and its project commands remain usable

#### Scenario: A bound file becomes invalid
- **GIVEN** a workspace is bound to a valid saved project
- **WHEN** its file becomes invalid and settings are reloaded
- **THEN** the load error is reported, the bound project retains its last valid definition, and an edit that conflicts with the existing disk text is refused

#### Scenario: The project name changes in the same file
- **GIVEN** a saved project has an existing workspace and index
- **WHEN** its TOML name changes at the same project-file path and settings are reloaded
- **THEN** the workspace remains bound to that project, its project label uses the new name, and its index is refreshed under the same ownership

#### Scenario: A project file is renamed
- **GIVEN** a workspace is bound to `old.toml`
- **WHEN** that file is moved to `new.toml` and settings are reloaded
- **THEN** the existing workspace retains the last valid `old.toml` definition and `new.toml` loads as a distinct project

#### Scenario: A retained definition becomes unbound
- **GIVEN** a saved file was removed and its definition is retained for one workspace
- **WHEN** that workspace is killed
- **THEN** the retained definition and its index are released while unrelated projects and workspaces remain usable

### Requirement: Edits are checked and keep the rest of the file
An edit to a project that has a file SHALL rewrite that file, keeping its comments, key order and every part the edit does not change. An edit SHALL be refused, with the file unchanged and the reason in the echo area, when the result would be rejected as a project file (an overlapping root, a missing directory, a role on more than one line) or when an existing file on disk differs from the text last loaded or written. A file that has been deleted SHALL be recreated with the checked edited definition, since no existing disk content is lost. A refused edit SHALL leave the loaded project unchanged. A disk conflict SHALL direct the user to reload with `C-x C-r`; other failures SHALL retain their specific cause. Project creation and saving an ad-hoc project SHALL continue to refuse an existing target file.

#### Scenario: Comments survive
- **GIVEN** a project file has a comment line
- **WHEN** a root is added
- **THEN** the written file still has the comment

#### Scenario: Overlapping root
- **GIVEN** a project has a primary root
- **WHEN** `project-add-root` adds a directory inside it
- **THEN** the file is unchanged and the echo area says the path is inside the primary

#### Scenario: Changed on disk
- **GIVEN** the project file changed on disk after it was loaded
- **WHEN** a role is then set
- **THEN** the file keeps the disk version and the echo area says it changed on disk and directs the user to `C-x C-r`

#### Scenario: Deleted project file
- **GIVEN** a loaded project's file was deleted and its roots remain valid
- **WHEN** a role is set
- **THEN** the file is recreated with the edited definition and preserved comments, and the loaded project reflects the successful write

#### Scenario: A file appears during creation
- **GIVEN** a create-only project write was requested while its path was absent
- **WHEN** another editor write creates that path before the create-only write checks it
- **THEN** the create-only operation reports that the file exists and preserves its content

## ADDED Requirements

### Requirement: Project names do not redirect existing ownership
Loaded projects SHALL retain identity independently of their display name for the editor session. Saving an ad-hoc project SHALL preserve its existing workspace and index ownership. Duplicate names SHALL keep first-match command resolution in deterministic load order: retained ad-hoc entries, valid saved files in file-name order, then saved definitions retained only for bound workspaces. A duplicate or renamed project SHALL NOT redirect an existing workspace, index or pending operation to another project. A stale project-write reply SHALL NOT overwrite a newer reconciled definition; a successful superseded write SHALL trigger a fresh read of its project file.

#### Scenario: Another file takes the same name
- **GIVEN** a workspace is bound to `b.toml` and another valid file `a.toml` declares the same name
- **WHEN** settings are reloaded
- **THEN** name-based commands resolve the first match while the existing workspace and its index remain owned by `b.toml`

#### Scenario: Saving an ad-hoc project
- **GIVEN** an ad-hoc project has two workspaces and an index
- **WHEN** `project-save` succeeds
- **THEN** both workspaces and the index still belong to that project, now associated with its saved file

#### Scenario: A superseded write completes
- **GIVEN** a project write is pending and a newer definition has already been reconciled
- **WHEN** the pending write replies successfully
- **THEN** its old definition does not replace the newer in-memory definition and a fresh read reconciles the actual file content
