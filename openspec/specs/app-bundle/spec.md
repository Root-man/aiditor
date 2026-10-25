# app-bundle Specification

## Purpose
Lets aiditor be built and installed locally as a macOS app bundle or a Linux desktop entry, so it starts from the Dock, Spotlight or a desktop launcher with its own name and icon.

## Requirements

### Requirement: macOS app bundle
On macOS, the bundle script SHALL build the release binary and install `~/Applications/aiditor.app`, replacing an earlier install. The bundle SHALL have an Info.plist with bundle identifier `io.github.root-man.aiditor`, name `aiditor`, the editor's version and an icon, the binary in `Contents/MacOS`, and an ad-hoc code signature. It is not notarized.

#### Scenario: Install the bundle
- **WHEN** the user runs the bundle script on macOS
- **THEN** `~/Applications/aiditor.app` exists, its Info.plist is valid, and `codesign --verify` accepts it

#### Scenario: Start from the Dock or Spotlight
- **WHEN** the user starts aiditor from Spotlight or the Dock
- **THEN** the editor opens and Cmd-Tab shows it with the name `aiditor` and its icon

### Requirement: Linux desktop entry
On Linux, the bundle script SHALL build the release binary, install it as `~/.local/bin/aiditor`, and install `aiditor.desktop` under `~/.local/share/applications` and the icon under `~/.local/share/icons/hicolor`, replacing an earlier install. The entry SHALL start the installed binary by absolute path, and the editor's window SHALL carry app id `aiditor`, matching the entry.

#### Scenario: Start from the launcher
- **WHEN** the user runs the bundle script on Linux and picks aiditor in the desktop launcher
- **THEN** the editor opens and the desktop shows it with the entry's name and icon

### Requirement: Shell start is unchanged
Starting the binary from a shell (`aiditor .`) SHALL behave as before; the bundle adds a way to start it, not a wrapper around it.

#### Scenario: Shell start
- **WHEN** the user runs `aiditor .` in a shell
- **THEN** the binary runs directly with the shell's environment and opens the directory
