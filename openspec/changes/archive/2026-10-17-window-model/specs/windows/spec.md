# Spec Delta

## Purpose

Windows are viewports onto buffers: each holds point, mark and a scroll position, and keeps them right as the buffer is edited and point moves.

## ADDED Requirements

### Requirement: A window shows one buffer
The frame SHALL have one window. It SHALL show the current buffer, and visiting a file SHALL show the new buffer in it with point at the start and the scroll position at the first line.

#### Scenario: Visiting a file
- **WHEN** a file is visited after typing in `*scratch*`
- **THEN** the window shows the file's buffer with point at 0

### Requirement: Point and mark follow edits
A window's point and mark SHALL be positions in its buffer that follow edits: text inserted or deleted before them moves them, text inserted exactly at them leaves them before it unless the edit is a command typing at point, and a deletion around them moves them to its start. The mark SHALL be unset until a command sets it.

#### Scenario: An edit before point moves point
- **WHEN** point is at 3 in `abcdef` and `XY` is inserted at 1 without moving point
- **THEN** point is 5

#### Scenario: A deletion around point
- **WHEN** point is at 3 in `abcdef` and 1..5 is deleted
- **THEN** point is 1

#### Scenario: Mark follows edits
- **WHEN** the mark is set at 4 in `abcdef` and 0..2 is deleted
- **THEN** the mark is at 2

### Requirement: Scrolling keeps point visible
Given how many lines fit in the window, the window's first line SHALL move only as far as needed for point's line to be visible: down when point is below the last visible line, up when it is above the first.

#### Scenario: Point moves below the window
- **WHEN** the window fits 3 lines, shows from line 0, and point moves to line 5
- **THEN** the window shows from line 3

#### Scenario: Point moves back up
- **WHEN** the window shows from line 3 and point moves to line 1
- **THEN** the window shows from line 1

#### Scenario: Point stays on screen
- **WHEN** point moves within the visible lines
- **THEN** the first line does not change
