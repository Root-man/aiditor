//! `*Sessions*`: every registered agent's past sessions in the current project, from
//! `session/list`.

use std::path::{Path, PathBuf};

use agent_client_protocol_schema::v1::{SessionId, SessionInfo};
use jiff::{Span, Timestamp, Zoned};
use serde::{Deserialize, Serialize};

use crate::buffer::{Buffer, BufferId, Decoration, FaceRef, Kind, Prop};
use crate::command::{Args, Preview};
use crate::display::{ModeLine, Segment, WindowDisplay};
use crate::grep::Searched;
use crate::project::{Project, ProjectId};
use crate::settings::{Agent, FaceName};
use crate::{AgentId, AgentOp, Core, Ctx, Effect, Open};

const MODE: &str = "agent-sessions-mode";
/// The widest a title shows, cut to one less and `…` past it.
const TITLE_WIDTH: usize = 60;

/// The echo line's actions, as the workspace list's.
pub(crate) const HINTS: &[(&[&str], &str)] = &[
    (&["agent-sessions-next", "agent-sessions-previous"], "move"),
    (&["agent-sessions-visit"], "switch"),
    (&["agent-sessions-toggle"], "fold"),
    (&["agent-sessions-search"], "search"),
    (&["agent-sessions-pin"], "pin/unpin"),
    (&["agent-sessions-archive"], "archive"),
    (&["agent-sessions"], "refresh"),
    (&["agent-sessions-quit"], "quit"),
];

/// A group of sessions by activity, in the list's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Pinned,
    Today,
    Yesterday,
    Week,
    Older,
    // Nothing is archived until archived listings exist.
    #[allow(dead_code)]
    Archived,
}

impl Group {
    fn name(self) -> &'static str {
        match self {
            Group::Pinned => "Pinned",
            Group::Today => "Today",
            Group::Yesterday => "Yesterday",
            Group::Week => "This week",
            Group::Older => "Older",
            Group::Archived => "Archived",
        }
    }

    /// The group of a session updated at `time`, reckoned from `now`, and its time as shown.
    fn of(time: Option<Timestamp>, now: &Zoned) -> (Group, String) {
        let Some(time) = time else {
            return (Group::Older, String::new());
        };
        let time = time.to_zoned(now.time_zone().clone());
        let (date, today) = (time.date(), now.date());
        let offset = today.weekday().to_monday_zero_offset();
        let monday = today.checked_sub(Span::new().days(offset)).ok();
        let (group, format) = if date >= today {
            (Group::Today, "%H:%M")
        } else if today.yesterday().ok() == Some(date) {
            (Group::Yesterday, "%H:%M")
        } else if monday.is_some_and(|monday| date >= monday) {
            (Group::Week, "%a %H:%M")
        } else {
            (Group::Older, "%Y-%m-%d")
        };
        (group, time.strftime(format).to_string())
    }

    fn face(self) -> Vec<FaceRef> {
        match self {
            Group::Pinned => vec![FaceRef::Face(FaceName::SessionListPinned)],
            Group::Archived => vec![FaceRef::Face(FaceName::Annotation), FaceRef::Bold],
            _ => vec![FaceRef::Face(FaceName::SessionListGroup)],
        }
    }
}

/// A pinned session: an agent's session id in the project whose primary root is `cwd`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Pin {
    agent: String,
    cwd: PathBuf,
    session: String,
}

/// `sessions.toml`.
#[derive(Serialize, Deserialize)]
struct PinFile {
    #[serde(default)]
    pin: Vec<Pin>,
}

/// The pins in `sessions.toml`'s text, none without a file.
pub(crate) fn parse_pins(text: Option<&str>) -> Result<Vec<Pin>, String> {
    let file = toml::from_str::<PinFile>(text.unwrap_or_default());
    file.map(|f| f.pin).map_err(|e| e.message().to_string())
}

/// An agent's glyph and face; other agents get neither, the glyph's columns kept.
fn agent_look(name: &str) -> (&'static str, Option<FaceName>) {
    match name {
        "claude" => ("✻ ", Some(FaceName::AgentClaude)),
        "codex" => ("◆ ", Some(FaceName::AgentCodex)),
        _ => ("  ", None),
    }
}

/// An open session's state: its words and face.
type State = (&'static str, FaceName);

/// A line point can be on: a group's heading or a session's row.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    Heading(Group),
    /// The agent's index and the session.
    Session(usize, SessionId),
}

impl Row {
    /// Point's column on it: a heading's name, a row's title.
    fn column(&self) -> usize {
        match self {
            Row::Heading(_) => 2,
            Row::Session(..) => 4,
        }
    }
}

/// A sessions buffer's state.
pub(crate) struct SessionList {
    project: ProjectId,
    /// The primary root the agents were asked in, which pins are kept under.
    cwd: PathBuf,
    /// One per registered agent, in registry order.
    agents: Vec<Listing>,
    /// Each heading's and shown session row's line, in order.
    rows: Vec<(usize, Row)>,
    /// Every listed session, folded or shown, with its state as rendered, to rebuild when one
    /// changes.
    listed: Vec<(usize, SessionId, Option<State>)>,
    folded: Vec<Group>,
    /// The row point was last on, kept while no rows are listed.
    at: Option<Row>,
    /// The mode line's project, session count and open count.
    summary: Segment,
}

struct Listing {
    name: String,
    /// The latest request's number; answers to others are dropped.
    request: u64,
    /// The list request, held while the project's environment is being captured.
    held: Option<AgentOp>,
    answer: Option<Result<Vec<SessionInfo>, String>>,
}

/// A list request of `agent` in `project`'s primary root, deleting `delete` first.
fn list_op(
    agent: &Agent,
    request: u64,
    project: &Project,
    delete: Option<(SessionId, Option<AgentId>)>,
) -> AgentOp {
    AgentOp::List {
        request,
        command: agent.command.clone(),
        args: agent.args.clone(),
        cwd: project.primary.path.clone(),
        // Under the captured environment, which `release_agents` puts first.
        env: (agent.env.clone().into_iter())
            .chain(project.env.clone())
            .collect(),
        delete,
    }
}

/// The selected session hit's row in the sessions buffer the prompt was opened from, if shown.
pub(crate) fn search_preview(core: &Core, _label: &str) -> Option<Preview> {
    let hit = core.grep.hit_at(core.prompts.selected_index()?)?;
    let session = core.grep.session_of(hit)?;
    let buffer = core.buffers.get(core.origin()?.buffer())?;
    let row = Row::Session(session.agent, session.id.clone());
    let line = (buffer.session_list()?.rows.iter())
        .find(|(_, r)| *r == row)?
        .0;
    buffer.char_idx(line, row.column()).ok().map(Preview::Point)
}

/// `updated` read as RFC 3339, whatever its offset or fraction.
fn timestamp(updated: Option<&str>) -> Option<Timestamp> {
    updated?.parse().ok()
}

impl SessionList {
    /// Puts the selected row's background and the list's mode line, with point on `line` (from
    /// 1), on `buffer`'s `display`.
    pub(crate) fn decorate(&self, buffer: &Buffer, display: &mut WindowDisplay, line: usize) {
        let lines = &mut display.lines;
        if let Some(number) = (display.cursor)
            .map(|(l, _)| lines[l].number)
            .filter(|n| self.rows.iter().any(|(line, _)| line == n))
        {
            for line in lines.iter_mut().filter(|l| l.number == number) {
                (line.decorations).push(Decoration::Background(FaceName::CompletionSelected));
            }
        }
        let sessions: Vec<_> = (self.rows.iter())
            .filter(|(_, row)| matches!(row, Row::Session(..)))
            .map(|(line, _)| *line)
            .collect();
        let position = sessions.partition_point(|l| *l <= line).max(1);
        let segment = |text: &str, face| vec![(text.to_string(), face)];
        display.mode_line = ModeLine {
            left: vec![
                segment(&buffer.name, FaceName::ModeLine),
                self.summary.clone(),
            ],
            right: (!sessions.is_empty())
                .then(|| {
                    segment(
                        &format!("{position}/{}", sessions.len()),
                        FaceName::ModeLine,
                    )
                })
                .into_iter()
                .collect(),
            path: None,
        };
    }
}

impl Core {
    fn pinned(&self, agent: &str, cwd: &Path, session: &SessionId) -> bool {
        (self.pins.iter()).any(|p| p.agent == agent && p.cwd == cwd && *p.session == *session.0)
    }

    /// Writes the pins and rebuilds the live sessions buffers listed in `cwd`.
    fn pins_changed(&mut self, cwd: &Path, effects: &mut Vec<Effect>) {
        match toml::to_string(&PinFile {
            pin: self.pins.clone(),
        }) {
            Ok(text) => effects.push(Effect::WritePins(text)),
            Err(err) => self.report(format!("sessions.toml: {err}")),
        }
        let lists: Vec<_> = (self.buffers.recent())
            .filter(|(_, b)| b.session_list().is_some_and(|l| l.cwd == cwd))
            .map(|(id, _)| id)
            .collect();
        for id in lists {
            self.render_sessions(id);
        }
    }

    /// Takes the held list requests of `project`'s live sessions buffers, `vars` first in each
    /// environment.
    pub(crate) fn release_lists(
        &mut self,
        project: ProjectId,
        vars: &[(String, String)],
    ) -> Vec<AgentOp> {
        let lists = (self.buffers.iter_mut())
            .filter_map(|(_, b)| b.session_list_mut())
            .filter(|l| l.project == project);
        let held = lists
            .flat_map(|l| &mut l.agents)
            .filter_map(|a| a.held.take());
        held.map(|mut op| {
            if let AgentOp::List { env, .. } = &mut op {
                env.splice(0..0, vars.iter().cloned());
            }
            op
        })
        .collect()
    }

    /// Rebuilds the live sessions buffers whose sessions' states changed since they were rendered.
    pub(crate) fn sync_session_lists(&mut self) {
        let stale: Vec<_> = (self.buffers.recent())
            .filter_map(|(id, b)| Some((id, b.session_list()?)))
            .filter(|(_, list)| {
                (list.listed.iter()).any(|(agent, session, state)| {
                    self.session_state(&list.agents[*agent].name, session) != *state
                })
            })
            .map(|(id, _)| id)
            .collect();
        for id in stale {
            self.render_sessions(id);
        }
    }

    /// Drops `agent`'s session `id`, being deleted, from the sessions buffers' answers.
    // shortcut: assumes the agent can delete it; `g` lists it again if it can't.
    pub(crate) fn forget_listed(&mut self, agent: &str, id: &SessionId) {
        let changed: Vec<_> = (self.buffers.iter_mut())
            .filter_map(|(buffer, b)| {
                let list = b.session_list_mut()?;
                let listing = list.agents.iter_mut().find(|a| a.name == agent)?;
                let Some(Ok(sessions)) = &mut listing.answer else {
                    return None;
                };
                let listed = sessions.len();
                sessions.retain(|s| s.session_id != *id);
                (sessions.len() != listed).then_some(buffer)
            })
            .collect();
        for buffer in changed {
            self.render_sessions(buffer);
        }
    }

    /// Rebuilds sessions buffer `id`'s text from its answers and folds, keeping point on its row.
    // shortcut: groups are reckoned at each rebuild, so a list left open past midnight keeps its
    // old groups until an answer, a state change, a fold or `g`; rebuild on the clock if that bites.
    fn render_sessions(&mut self, id: BufferId) {
        let Some(buffer) = self.buffers.get(id) else {
            return;
        };
        let Some(list) = buffer.session_list() else {
            return;
        };
        // Point where a window shows the buffer, else the buffer's own.
        let saved = self.workspaces.iter().flat_map(|w| &w.windows);
        let windows = std::iter::once(&self.windows).chain(saved);
        let point = (windows.flat_map(|w| std::iter::once(&w.window).chain(w.others.values())))
            .find(|w| w.buffer() == id)
            .map_or_else(
                || buffer.marker(buffer.point).unwrap_or(0),
                |w| w.point(&self.buffers),
            );
        let line = buffer.line_col(point).map_or(0, |(line, _)| line);
        let at = match list.rows.iter().find(|(l, _)| *l == line) {
            Some((_, row)) => Some(row.clone()),
            None if list.rows.is_empty() => list.at.clone(),
            None => None,
        };
        // Each line's pieces of text, each with the property it carries.
        let mut lines: Vec<Vec<(String, Option<Prop>)>> = Vec::new();
        let plain = |text: String| vec![(text, None)];
        let face = |face| Some(Prop::Named(vec![FaceRef::Face(face)]));
        let mut sessions = Vec::new();
        for (index, listing) in list.agents.iter().enumerate() {
            match &listing.answer {
                None => lines.push(plain(format!("Listing {}...", listing.name))),
                // On one line, so each row's line number holds.
                Some(Err(err)) => {
                    let err = err.split_whitespace().collect::<Vec<_>>().join(" ");
                    lines.push(plain(format!("{}: {err}", listing.name)));
                }
                Some(Ok(infos)) => sessions.extend(infos.iter().map(|info| {
                    let time = timestamp(info.updated_at.as_deref());
                    (index, info, time)
                })),
            }
        }
        // Stable: equal times keep registry order, then the agent's.
        sessions.sort_by_key(|&(.., time)| std::cmp::Reverse(time));
        let now = (self.wall_clock)().to_zoned(self.time_zone.clone());
        let mut entries: Vec<_> = (sessions.into_iter())
            .map(|(agent, info, time)| {
                let (mut group, time) = Group::of(time, &now);
                if self.pinned(&list.agents[agent].name, &list.cwd, &info.session_id) {
                    group = Group::Pinned;
                }
                let mut title = (info.title.as_deref())
                    .and_then(|t| t.lines().next())
                    .filter(|t| !t.is_empty())
                    .map_or_else(|| info.session_id.to_string(), str::to_string);
                if title.chars().count() > TITLE_WIDTH {
                    title = title.chars().take(TITLE_WIDTH - 1).chain(['…']).collect();
                }
                let state = self.session_state(&list.agents[agent].name, &info.session_id);
                (group, agent, info.session_id.clone(), title, time, state)
            })
            .collect();
        // Stable: each group keeps the time order.
        entries.sort_by_key(|e| e.0);
        let states = (entries.iter())
            .filter_map(|e| e.5.map(|(state, _)| state.chars().count()))
            .max();
        let agents = (entries.iter())
            .map(|e| list.agents[e.1].name.chars().count())
            .max()
            .unwrap_or(0);
        let mut rows = Vec::new();
        for members in entries.chunk_by(|a, b| a.0 == b.0) {
            if !lines.is_empty() {
                lines.push(Vec::new());
            }
            let group = members[0].0;
            let folded = list.folded.contains(&group);
            let mark = if folded { "▸ " } else { "▾ " };
            rows.push((lines.len(), Row::Heading(group)));
            lines.push(vec![
                (mark.into(), face(FaceName::Annotation)),
                (group.name().into(), Some(Prop::Named(group.face()))),
                ("  ".into(), None),
                (members.len().to_string(), face(FaceName::SessionListCount)),
            ]);
            if folded {
                continue;
            }
            let time_face = match group {
                Group::Today => FaceName::SessionListToday,
                _ => FaceName::ModeLineDim,
            };
            for (_, agent, session, title, time, state) in members {
                let name = &list.agents[*agent].name;
                let (glyph, agent_face) = agent_look(name);
                let pad = |text: &str, width: usize| {
                    " ".repeat(width.saturating_sub(text.chars().count()))
                };
                // The space after the title puts the rest at the window's right edge, as frame
                // 14's columns; the padding after a shorter agent or state keeps them lined up.
                let mut row = vec![
                    (format!("    {title}"), None),
                    (" ".into(), Some(Prop::AlignRight)),
                    (format!("{glyph}{name}"), agent_face.and_then(face)),
                    (format!("{}  {}", pad(name, agents), pad(time, 10)), None),
                    (time.clone(), face(time_face)),
                ];
                if let Some(states) = states {
                    let (text, state_face) = state.unwrap_or(("", FaceName::Default));
                    row.push((format!("  {}", pad(text, states)), None));
                    row.push((text.into(), face(state_face)));
                }
                rows.push((lines.len(), Row::Session(*agent, session.clone())));
                lines.push(row);
            }
        }
        let all_answered = list.agents.iter().all(|a| a.answer.is_some());
        if entries.is_empty() && all_answered {
            lines.push(plain("No sessions".into()));
        }
        let listed: Vec<_> = (entries.iter())
            .map(|(_, agent, session, .., state)| (*agent, session.clone(), *state))
            .collect();
        let open = listed.iter().filter(|(.., state)| state.is_some()).count();
        let project =
            (self.project_at(list.project)).map_or("", |i| &self.projects[i].project.name);
        let open_face = match open {
            0 => FaceName::ModeLineDim,
            _ => FaceName::StatusRunning,
        };
        let summary = vec![
            (
                format!("{project} · {} sessions · ", listed.len()),
                FaceName::ModeLineDim,
            ),
            (format!("{open} open"), open_face),
        ];
        let shown = |row: &Row| rows.iter().find(|(_, r)| r == row).map(|(l, _)| *l);
        let first_session = (rows.iter())
            .find(|(_, r)| matches!(r, Row::Session(..)))
            .map(|(l, _)| *l);
        let target = (at.as_ref())
            .and_then(|at| {
                shown(at).or_else(|| match at {
                    // Folded away: its group's heading.
                    Row::Session(agent, session) => (entries.iter())
                        .find(|e| (e.1, &e.2) == (*agent, session))
                        .and_then(|e| shown(&Row::Heading(e.0))),
                    Row::Heading(_) => None,
                })
            })
            .or(first_session)
            .or(rows.first().map(|(l, _)| *l));
        let target_row = (rows.iter()).find(|(l, _)| Some(*l) == target);
        let column = target_row.map_or(0, |(_, r)| r.column());
        let at = match rows.is_empty() {
            true => at,
            false => target_row.map(|(_, r)| r.clone()),
        };
        let first = target.is_some()
            && (target == first_session || target == rows.first().map(|(l, _)| *l));
        let buffer = self.buffers.get_mut(id).expect("checked above");
        let list = buffer.session_list_mut().expect("checked above");
        (list.rows, list.listed, list.at, list.summary) = (rows, listed, at, summary);
        let text: Vec<String> = (lines.iter())
            .map(|line| line.iter().map(|(text, _)| text.as_str()).collect())
            .collect();
        buffer.remove_properties(0..buffer.len_chars());
        buffer.set_text(&text.join("\n"));
        let mut at = 0;
        for line in lines {
            for (text, prop) in line {
                let end = at + text.chars().count();
                if let Some(prop) = prop.filter(|_| end > at) {
                    (buffer.add_property(at..end, prop)).expect("in the text");
                }
                at = end;
            }
            at += 1;
        }
        let point = match target {
            Some(line) => buffer.char_idx(line, column).expect("a row is that wide"),
            None => buffer.len_chars(),
        };
        buffer.set_marker(buffer.point, point).expect("in the text");
        let windows = (self.windows.windows_mut()).chain(
            self.workspaces
                .iter_mut()
                .flat_map(|w| w.windows.iter_mut())
                .flat_map(|w| w.windows_mut()),
        );
        for window in windows.filter(|w| w.buffer() == id) {
            window
                .set_point(&mut self.buffers, point)
                .expect("in the text");
            if first {
                window.set_top(0);
            }
        }
    }
}

impl Ctx<'_> {
    /// `agent-sessions`: asks every registered agent for its sessions in the current project and
    /// shows them in the workspace's sessions buffer, made or rebuilt.
    pub(crate) fn agent_sessions(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't list sessions in the minibuffer")?;
        let core = &mut *self.core;
        let workspace = core.current_workspace()?;
        let (project, name) = (workspace.project, format!("*Sessions*<{}>", workspace.name));
        let loaded = &core.projects[core.project_at(project)?].project;
        let enabled = core.enabled_agents(loaded);
        if enabled.is_empty() {
            return Err("No agents enabled".into());
        }
        let cwd = loaded.primary.path.clone();
        let mut agents = Vec::new();
        for name in enabled {
            let agent = &core.settings.agents[&name];
            core.list_requests += 1;
            let request = core.list_requests;
            let op = list_op(agent, request, loaded, None);
            agents.push(Listing {
                name,
                request,
                held: Some(op),
                answer: None,
            });
        }
        let here = core.workspace();
        let found = (core.buffers.recent())
            .find(|(_, b)| {
                b.session_list().is_some() && b.workspace.is_some() && b.workspace == here
            })
            .map(|(id, _)| id);
        let id = found.unwrap_or_else(|| {
            let id = core.buffers.create(&name);
            let buffer = core.buffers.get_mut(id).expect("just created");
            (buffer.major_mode, buffer.read_only) = (MODE, true);
            id
        });
        let buffer = core.buffers.get_mut(id).expect("live");
        let at = buffer.session_list_mut().and_then(|l| l.at.take());
        buffer.kind = Kind::SessionList(SessionList {
            project,
            cwd,
            agents,
            rows: Vec::new(),
            listed: Vec::new(),
            folded: vec![Group::Archived],
            at,
            summary: Vec::new(),
        });
        core.switch_to(id);
        core.render_sessions(id);
        self.release_agents(project);
        Ok(())
    }

    /// Keeps an answer to the latest request of a live sessions buffer and rebuilds it.
    pub(crate) fn agent_listed(&mut self, request: u64, result: Result<Vec<SessionInfo>, String>) {
        let core = &mut *self.core;
        let found = (core.buffers.iter_mut()).find_map(|(id, b)| {
            let list = b.session_list_mut()?;
            let listing = list.agents.iter_mut().find(|a| a.request == request)?;
            let (name, cwd) = (listing.name.clone(), list.cwd.clone());
            Some((id, listing, name, cwd))
        });
        let Some((id, listing, agent, cwd)) = found else {
            return;
        };
        let listed = match &result {
            Ok(infos) => Some(
                infos
                    .iter()
                    .map(|i| i.session_id.clone())
                    .collect::<Vec<_>>(),
            ),
            Err(_) => None,
        };
        listing.answer = Some(result);
        let pins = core.pins.len();
        // shortcut: an agent past the 50-page cap drops pins on the cut pages; key off the cap if one comes close.
        if let Some(listed) = listed {
            core.pins.retain(|p| {
                (p.agent != agent || p.cwd != cwd) || listed.iter().any(|s| *s.0 == *p.session)
            });
        }
        match core.pins.len() == pins {
            true => core.render_sessions(id),
            false => core.pins_changed(&cwd, &mut self.effects),
        }
    }

    fn session_rows(&self) -> Result<&SessionList, String> {
        (self.core.current().session_list()).ok_or_else(|| "Not in a sessions buffer".into())
    }

    /// `agent-sessions-next` and `-previous`: to the next or previous heading or session row.
    pub(crate) fn sessions_move(&mut self, forward: bool) -> Result<(), String> {
        let rows = &self.session_rows()?.rows;
        let (first, last) = (rows.first(), rows.last());
        let (Some(first), Some(last)) = (first, last) else {
            return Err("No sessions listed".into());
        };
        let line = self.core.current().line_col(self.core.point())?.0;
        let target = match forward {
            true => rows.iter().find(|r| r.0 > line).unwrap_or(last),
            false => rows.iter().rev().find(|r| r.0 < line).unwrap_or(first),
        };
        let first_session = rows.iter().find(|(_, r)| matches!(r, Row::Session(..)));
        let (line, column) = (target.0, target.1.column());
        let first = line == first.0 || first_session.is_some_and(|r| r.0 == line);
        // The lines above the first row are otherwise out of reach once scrolled past.
        if first {
            self.core.windows.window.set_top(0);
        }
        self.goto(self.core.current().char_idx(line, column)?)
    }

    /// The sessions buffer and the agent's index and session on point's row.
    fn session_row(&self) -> Result<(&SessionList, usize, &SessionId), String> {
        let list = self.session_rows()?;
        let line = self.core.current().line_col(self.core.point())?.0;
        (list.rows.iter())
            .find_map(|(l, row)| match row {
                Row::Session(agent, session) if *l == line => Some((list, *agent, session)),
                _ => None,
            })
            .ok_or_else(|| "Select a session row".into())
    }

    /// `agent-sessions-pin`: pins the session on point's row, or unpins it.
    pub(crate) fn sessions_pin(&mut self) -> Result<(), String> {
        let (list, agent, session) = self.session_row()?;
        let pin = Pin {
            agent: list.agents[agent].name.clone(),
            cwd: list.cwd.clone(),
            session: session.to_string(),
        };
        let core = &mut *self.core;
        match core.pins.iter().position(|p| *p == pin) {
            Some(index) => _ = core.pins.remove(index),
            None => core.pins.push(pin.clone()),
        }
        core.pins_changed(&pin.cwd, &mut self.effects);
        Ok(())
    }

    /// `agent-sessions-archive`: archives the session on point's row, stopping its agent first
    /// if it is open; its row leaves at once and the agent's fresh list replaces its rows.
    pub(crate) fn sessions_archive(&mut self) -> Result<(), String> {
        let (list, agent, session) = self.session_row()?;
        let (project, name) = (list.project, list.agents[agent].name.clone());
        // shortcut: no adapter advertises `session/archive` and codex-acp's `session/delete`
        // archives (`thread/archive`), so only `codex` archives, by deleting; #481 moves to
        // `session/archive`.
        if name != "codex" {
            return Err("Not supported yet".into());
        }
        let session = session.clone();
        let core = &mut *self.core;
        let settings =
            (core.settings.agents.get(&name)).ok_or(format!("{name} is not registered"))?;
        let loaded = &core.projects[core.project_at(project)?].project;
        core.list_requests += 1;
        let stop = core.open_agent(&name, &session);
        let op = list_op(
            settings,
            core.list_requests,
            loaded,
            Some((session.clone(), stop)),
        );
        let request = core.list_requests;
        let list = (core.current_mut().session_list_mut()).expect("checked above");
        let listing = &mut list.agents[agent];
        (listing.request, listing.held) = (request, Some(op));
        core.forget_listed(&name, &session);
        self.release_agents(project);
        Ok(())
    }

    /// `agent-sessions-visit`: shows the session on point's row, loading it in the current
    /// workspace unless it is open.
    pub(crate) fn sessions_visit(&mut self) -> Result<(), String> {
        let (_, agent, session) = self.session_row()?;
        let session = session.clone();
        self.visit_listed(agent, &session)
    }

    /// Shows the agent's listed session, loading it in the current workspace unless it is open.
    fn visit_listed(&mut self, agent: usize, session: &SessionId) -> Result<(), String> {
        let listing = &self.session_rows()?.agents[agent];
        let Some(buffer) = self.core.open_session(&listing.name, session) else {
            let title = match &listing.answer {
                Some(Ok(infos)) => (infos.iter())
                    .find(|i| i.session_id == *session)
                    .and_then(|i| i.title.clone()),
                _ => None,
            };
            let (name, session) = (listing.name.clone(), session.clone());
            return self.start_session(&name, Open::Load(session), title.as_deref());
        };
        let state = self.core.session_state(&listing.name, session);
        if state.is_some_and(|(state, _)| state == "○ loading") {
            let echo = format!("Loading {} session...", listing.name);
            self.core.echo(echo);
            return Ok(());
        }
        let owner = self.core.buffers.get(buffer).and_then(|b| b.workspace);
        if let Some(owner) = owner.filter(|w| Some(*w) != self.core.workspace()) {
            self.core.switch_workspace(owner)?;
        }
        self.core.switch_to(buffer);
        Ok(())
    }

    /// `agent-sessions-search`: asks for a regexp and lists its hits in the listed sessions' own
    /// files as it is typed, as `project-grep`; given a listed hit, shows its session.
    pub(crate) fn sessions_search(&mut self, args: Args) -> Result<(), String> {
        if let Some(label) = args.strings.first() {
            let hit = self.core.grep.hit(label).ok_or("No match")?;
            let session = self.core.grep.session_of(hit).ok_or("No match")?;
            let (agent, id) = (session.agent, session.id.clone());
            return self.visit_listed(agent, &id);
        }
        let list = self.session_rows()?;
        let home = std::env::home_dir().ok_or("No home directory")?;
        // As Claude Code names a project's directory.
        let cwd: String = (list.cwd.to_string_lossy().chars())
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let (mut roots, mut sessions) = (Vec::new(), Vec::new());
        for (agent, listing) in list.agents.iter().enumerate() {
            // shortcut: ignores `CLAUDE_CONFIG_DIR` and `CODEX_HOME`; read them from the agent's
            // environment once someone sets them.
            let root = match listing.name.as_str() {
                "claude" => home.join(".claude/projects").join(&cwd),
                "codex" => home.join(".codex/sessions"),
                _ => continue,
            };
            let Some(Ok(infos)) = &listing.answer else {
                continue;
            };
            let glyph = agent_look(&listing.name).0;
            sessions.extend(infos.iter().map(|info| Searched {
                agent,
                id: info.session_id.clone(),
                header: format!("{glyph}{}", info.title.as_deref().unwrap_or("(untitled)")),
            }));
            roots.push((root, listing.name.clone()));
        }
        if sessions.is_empty() {
            return Err("No sessions to search".into());
        }
        let project = &self.core.projects[self.core.project_at(list.project)?].project;
        let prompt = format!("Search sessions in {}: ", project.name);
        let grep = &self.core.grep;
        self.core.grep = grep.fresh("agent-sessions-search", roots, sessions);
        let command = self.core.commands["agent-sessions-search"];
        self.read_string(command, args, prompt, None)
    }

    /// `agent-sessions-toggle`: folds or unfolds the group whose heading point is on.
    pub(crate) fn sessions_toggle(&mut self) -> Result<(), String> {
        let rows = &self.session_rows()?.rows;
        let line = self.core.current().line_col(self.core.point())?.0;
        let Some(&(_, Row::Heading(group))) = rows.iter().find(|(l, _)| *l == line) else {
            return Ok(());
        };
        let list = (self.core.current_mut().session_list_mut()).expect("checked above");
        match list.folded.iter().position(|g| *g == group) {
            Some(index) => _ = list.folded.remove(index),
            None => list.folded.push(group),
        }
        let id = self.core.windows.window.buffer();
        self.core.render_sessions(id);
        Ok(())
    }

    /// `agent-sessions-quit`: kills the sessions buffer.
    pub(crate) fn sessions_quit(&mut self) -> Result<(), String> {
        self.session_rows()?;
        let id = self.core.windows.window.buffer();
        self.core.kill_buffer(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::v1::{
        AgentRequest, ContentChunk, NewSessionResponse, PermissionOption, PermissionOptionKind,
        RequestId, RequestPermissionRequest, SessionConfigOption, SessionConfigOptionCategory,
        SessionConfigSelectOption, SessionNotification, SessionUpdate, StopReason, ToolCallUpdate,
        ToolCallUpdateFields,
    };
    use jiff::tz::{Offset, TimeZone};

    use super::*;
    use crate::harness::Harness;
    use crate::project::Root;
    use crate::settings::Agent;
    use crate::{AgentId, Effect, Msg, Request};

    /// A harness in workspace `work` on a project at `app`, with agents `claude` (`env` `A=2`)
    /// and `codex`, showing times in UTC at noon on Wednesday 2026-10-28.
    fn work() -> Harness {
        let mut h = Harness::new(&[("app/main.rs", "")]);
        h.core.rename_workspace("work").unwrap();
        h.core.projects[0].project.primary.path = h.path("app");
        h.core.time_zone = TimeZone::UTC;
        h.core.wall_clock = || "2026-10-28T12:00:00Z".parse().unwrap();
        let agent = |command: &str, env: &[(&str, &str)]| Agent {
            command: command.into(),
            args: Vec::new(),
            env: (env.iter()).map(|&(k, v)| (k.into(), v.into())).collect(),
            default_mode: None,
        };
        let agents = &mut h.core.settings.agents;
        agents.insert("claude".into(), agent("claude-agent-acp", &[("A", "2")]));
        agents.insert("codex".into(), agent("codex-acp", &[]));
        h
    }

    /// The list requests since the last call: number, command, cwd and environment.
    fn lists(h: &mut Harness) -> Vec<(u64, String, String, String)> {
        (h.agent_ops.drain(..))
            .filter_map(|op| match op {
                AgentOp::List {
                    request,
                    command,
                    cwd,
                    env,
                    ..
                } => {
                    let env: Vec<_> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
                    Some((request, command, cwd.display().to_string(), env.join(",")))
                }
                _ => None,
            })
            .collect()
    }

    fn info(id: &str, title: Option<&str>, updated: Option<&str>) -> SessionInfo {
        let info = SessionInfo::new(id.to_string(), "/app").title(title.map(String::from));
        info.updated_at(updated.map(String::from))
    }

    fn answer(h: &mut Harness, request: u64, result: Result<Vec<SessionInfo>, &str>) {
        let result = result.map_err(String::from);
        h.send(Msg::AgentListed { request, result });
    }

    /// The text, without the spaces that line up the columns at the end of a line.
    fn body(h: &Harness) -> String {
        let id = h.core.buffers.find("*Sessions*<work>").expect("listed");
        let text = h.core.buffers.get(id).unwrap().text().to_string();
        text.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Each face over the text, with the text it covers, in order; `Bold` added as a face.
    fn faces(h: &Harness) -> Vec<(String, Vec<FaceRef>)> {
        let id = h.core.buffers.find("*Sessions*<work>").expect("listed");
        let b = h.core.buffers.get(id).unwrap();
        (b.properties())
            .filter_map(|(r, p)| match p {
                Prop::Named(refs) => Some((b.text().slice(r).to_string(), refs.clone())),
                _ => None,
            })
            .collect()
    }

    /// The last face over the text and the text it covers, a row's state when it has one.
    fn last_face(h: &Harness) -> (String, FaceName) {
        let (text, refs) = faces(h).pop().unwrap();
        match refs[..] {
            [FaceRef::Face(face)] => (text, face),
            _ => panic!("{refs:?}"),
        }
    }

    /// The last line, the only row of a one-session list.
    fn last(h: &Harness) -> String {
        body(h).lines().last().unwrap().into()
    }

    fn line(h: &Harness) -> String {
        let (line, _) = h.core.current().line_col(h.point()).unwrap();
        h.text()
            .lines()
            .nth(line)
            .unwrap_or_default()
            .trim_end()
            .into()
    }

    /// `agent-sessions` answered: claude with `c1` at 10:00 and `c2` at 08:00, codex with `x1`
    /// at 09:00.
    fn listed() -> Harness {
        let mut h = work();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let at = |hour| Some(format!("2026-10-28T{hour}:00:00Z"));
        let claude = vec![
            info("c1", Some("Fix it"), at("10").as_deref()),
            info("c2", Some("Old"), at("08").as_deref()),
        ];
        answer(&mut h, requests[0].0, Ok(claude));
        answer(
            &mut h,
            requests[1].0,
            Ok(vec![info("x1", Some("Port"), at("09").as_deref())]),
        );
        h
    }

    #[test]
    fn times_show_in_the_given_zone_whatever_their_offset() {
        let zone = TimeZone::fixed(Offset::constant(2));
        let now = timestamp(Some("2026-10-28T20:00:00+02:00")).unwrap();
        let now = now.to_zoned(zone);
        let shown = |s: &str| Group::of(timestamp(Some(s)), &now);
        let today = |time: &str| (Group::Today, time.to_string());
        assert_eq!(shown("2026-10-28T10:00:00Z"), today("12:00"));
        assert_eq!(shown("2026-10-28T12:30:00.123+02:00"), today("12:30"));
        assert_eq!(shown("2026-10-28T05:15:00.5-04:00"), today("11:15"));
        assert_eq!(timestamp(Some("yesterday")), None);
        assert_eq!(timestamp(None), None);
        assert_eq!(Group::of(None, &now), (Group::Older, String::new()));
    }

    #[test]
    fn sessions_fall_in_their_group_at_day_and_week_boundaries() {
        let at = |s: &str| timestamp(Some(s)).unwrap().to_zoned(TimeZone::UTC);
        let of = |now: &Zoned, s: &str| Group::of(timestamp(Some(s)), now);
        let wednesday = at("2026-10-28T00:30:00Z");
        let cases = [
            ("2026-10-28T00:10:00Z", Group::Today, "00:10"),
            // Later than now, as a skewed clock reports it.
            ("2026-10-29T08:00:00Z", Group::Today, "08:00"),
            ("2026-10-27T23:50:00Z", Group::Yesterday, "23:50"),
            ("2026-10-26T23:50:00Z", Group::Week, "Mon 23:50"),
            ("2026-10-26T00:00:00Z", Group::Week, "Mon 00:00"),
            ("2026-10-25T23:59:00Z", Group::Older, "2026-10-25"),
        ];
        for (time, group, shown) in cases {
            assert_eq!(of(&wednesday, time), (group, shown.into()), "{time}");
        }
        let monday = at("2026-10-26T09:00:00Z");
        assert_eq!(of(&monday, "2026-10-25T10:00:00Z").0, Group::Yesterday);
        assert_eq!(of(&monday, "2026-10-24T10:00:00Z").0, Group::Older);
        let sunday = at("2026-11-01T09:00:00Z");
        assert_eq!(
            of(&sunday, "2026-10-26T10:00:00Z"),
            (Group::Week, "Mon 10:00".into())
        );
    }

    #[test]
    fn every_agent_is_asked_in_the_primary_repo_and_shown_listing() {
        let mut h = work();
        h.keys("M-x agent-sessions RET");
        let app = h.path("app").display().to_string();
        assert_eq!(
            lists(&mut h),
            [
                (1, "claude-agent-acp".into(), app.clone(), "A=2".into()),
                (2, "codex-acp".into(), app, String::new())
            ]
        );
        assert_eq!(h.core.current().name, "*Sessions*<work>");
        assert_eq!(h.core.current().major_mode, MODE);
        assert!(h.core.current().read_only);
        assert_eq!(body(&h), "Listing claude...\nListing codex...");
        assert_eq!(
            h.core.current().session_list().unwrap().folded,
            [Group::Archived]
        );
        h.keys("x");
        assert_eq!(h.core.message(), Some("Buffer is read-only"));
    }

    #[test]
    fn the_requests_wait_for_the_capture() {
        let mut h = work();
        h.core.capture_environments();
        h.keys("M-x agent-sessions RET");
        assert!(lists(&mut h).is_empty());
        let request = (h.effects.iter().rev())
            .find_map(|e| match e {
                Effect::CaptureEnvironment { request, .. } => Some(*request),
                _ => None,
            })
            .unwrap();
        h.send(Msg::EnvironmentCaptured {
            project: ProjectId(0),
            request,
            result: Ok(vec![("PATH".into(), "/opt/bin".into())]),
            direnv: None,
        });
        let envs: Vec<_> = lists(&mut h).into_iter().map(|l| l.3).collect();
        assert_eq!(envs, ["PATH=/opt/bin,A=2", "PATH=/opt/bin"]);
    }

    #[test]
    fn a_list_killed_while_waiting_is_never_sent() {
        let mut h = work();
        h.core.capture_environments();
        h.keys("M-x agent-sessions RET C-x k RET");
        let request = (h.effects.iter().rev())
            .find_map(|e| match e {
                Effect::CaptureEnvironment { request, .. } => Some(*request),
                _ => None,
            })
            .unwrap();
        h.send(Msg::EnvironmentCaptured {
            project: ProjectId(0),
            request,
            result: Ok(Vec::new()),
            direnv: None,
        });
        assert!(lists(&mut h).is_empty());
    }

    #[test]
    fn only_enabled_agents_are_asked_for_sessions() {
        let mut h = work();
        h.core.projects[0].project.enabled_agents = Some(vec!["codex".into()]);
        h.keys("M-x agent-sessions RET");
        let commands: Vec<_> = lists(&mut h).into_iter().map(|l| l.1).collect();
        assert_eq!(commands, ["codex-acp"]);
        assert_eq!(body(&h), "Listing codex...");
        h.keys("g");
        let commands: Vec<_> = lists(&mut h).into_iter().map(|l| l.1).collect();
        assert_eq!(commands, ["codex-acp"], "g asks the same agents");
        h.core.projects[0].project.enabled_agents = Some(Vec::new());
        h.keys("M-x agent-sessions RET");
        assert_eq!(h.core.message(), Some("No agents enabled"));
        assert!(lists(&mut h).is_empty());
    }

    #[test]
    fn no_agents_lists_nothing() {
        let mut h = work();
        h.core.settings.agents.clear();
        h.keys("M-x agent-sessions RET");
        assert_eq!(h.core.message(), Some("No agents enabled"));
        assert!(h.core.buffers.find("*Sessions*<work>").is_none());
    }

    #[test]
    fn sessions_of_both_agents_show_most_recent_first() {
        let h = listed();
        assert_eq!(
            body(&h),
            "▾ Today  3\n\
            \x20   Fix it ✻ claude       10:00\n\
            \x20   Port ◆ codex        09:00\n\
            \x20   Old ✻ claude       08:00"
        );
        assert_eq!(line(&h), "    Fix it ✻ claude       10:00");
    }

    #[test]
    fn missing_times_sort_last_blank_and_errors_replace_rows() {
        let mut h = work();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let sessions = vec![
            info("none", None, None),
            info("bad", Some("Bad\nsecond line"), Some("yesterday")),
            info("good", Some("Good"), Some("2026-10-14T10:00:00Z")),
        ];
        answer(&mut h, requests[0].0, Ok(sessions));
        assert_eq!(
            body(&h),
            "Listing codex...\n\
             \n\
             ▾ Older  3\n\
            \x20   Good ✻ claude  2026-10-14\n\
            \x20   none ✻ claude\n\
            \x20   Bad ✻ claude"
        );
        answer(
            &mut h,
            requests[1].0,
            Err("closed: {\n  \"method\": \"initialize\"\n}"),
        );
        assert!(body(&h).starts_with("codex: closed: { \"method\": \"initialize\" }\n\n▾ Older"));
        h.keys("n RET");
        assert_eq!(
            h.core.session_state("claude", &"none".into()),
            Some(("○ loading", FaceName::ModeLineDim))
        );
    }

    #[test]
    fn no_sessions_says_so() {
        let mut h = work();
        h.keys("M-x agent-sessions RET");
        for (request, ..) in lists(&mut h) {
            answer(&mut h, request, Ok(Vec::new()));
        }
        assert_eq!(body(&h), "No sessions");
        h.keys("n");
        assert_eq!(h.core.message(), Some("No sessions listed"));
    }

    #[test]
    fn point_stays_on_its_session_and_g_drops_late_answers() {
        let mut h = listed();
        h.keys("n");
        assert!(line(&h).contains("Port"));
        h.keys("g");
        let new = (3, 4);
        let requests = lists(&mut h);
        assert_eq!(
            requests.iter().map(|r| r.0).collect::<Vec<_>>(),
            [new.0, new.1]
        );
        assert_eq!(body(&h), "Listing claude...\nListing codex...");
        answer(&mut h, 1, Ok(vec![info("late", None, None)]));
        assert_eq!(body(&h), "Listing claude...\nListing codex...");
        let x1 = info("x1", Some("Port"), Some("2026-10-28T09:00:00Z"));
        answer(&mut h, new.1, Ok(vec![x1]));
        let newer = info("c3", Some("New"), Some("2026-10-28T11:00:00Z"));
        answer(&mut h, new.0, Ok(vec![newer]));
        assert!(body(&h).starts_with("▾ Today  2\n    New ✻ claude  "));
        assert!(line(&h).contains("Port"));
    }

    #[test]
    fn rows_are_moved_over_and_q_kills_the_list() {
        let mut h = listed();
        h.keys("n n n p");
        assert!(line(&h).contains("Port"));
        h.keys("p");
        assert!(line(&h).contains("Fix it"));
        // On the title, and on a heading's name.
        assert_eq!(h.core.current().line_col(h.point()).unwrap().1, 4);
        h.keys("p p");
        assert_eq!(line(&h), "▾ Today  3");
        assert_eq!(h.core.current().line_col(h.point()).unwrap().1, 2);
        h.keys("RET");
        assert_eq!(h.core.message(), Some("Select a session row"));
        h.keys("q");
        assert!(h.core.buffers.find("*Sessions*<work>").is_none());
    }

    #[test]
    fn the_first_row_shows_the_header_above_it() {
        let mut h = work();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let many = (0..20)
            .map(|n| info(&format!("c{n}"), None, None))
            .collect();
        answer(&mut h, requests[0].0, Ok(many));
        answer(&mut h, requests[1].0, Ok(Vec::new()));
        for _ in 0..20 {
            h.keys("n");
            h.display(5);
        }
        h.keys("g");
        h.display(5);
        let requests = lists(&mut h);
        let many = (0..20)
            .map(|n| info(&format!("c{n}"), None, None))
            .collect();
        answer(&mut h, requests[1].0, Ok(Vec::new()));
        answer(&mut h, requests[0].0, Ok(many));
        h.display(5);
        for _ in 0..20 {
            h.keys("p");
            h.display(5);
        }
        assert_eq!(line(&h), "▾ Older  20");
        assert_eq!(h.display(5).lines[0].number, 0);
        for _ in 0..20 {
            h.keys("n");
            h.display(5);
        }
        h.keys("g");
        h.display(5);
        answer(&mut h, 6, Ok(vec![info("c0", None, None)]));
        answer(&mut h, 5, Ok(vec![info("c0", None, None)]));
        assert_eq!(h.display(5).lines[0].number, 0);
    }

    #[test]
    fn open_sessions_are_marked_and_ret_shows_them() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        h.agent_ops.clear();
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("c1")),
        });
        let session = h.core.windows.window.buffer();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let sessions = vec![
            info("c1", Some("Mine"), Some("2026-10-28T10:00:00Z")),
            info("c2", Some("Other"), Some("2026-10-28T09:00:00Z")),
        ];
        answer(&mut h, requests[0].0, Ok(sessions.clone()));
        // The same id from another agent is a different session.
        answer(&mut h, requests[1].0, Ok(sessions[..1].to_vec()));
        assert_eq!(
            body(&h),
            "▾ Today  3\n\
            \x20   Mine ✻ claude       10:00  ○ idle\n\
            \x20   Mine ◆ codex        10:00\n\
            \x20   Other ✻ claude       09:00"
        );
        h.keys("RET");
        assert_eq!(h.core.windows.window.buffer(), session);
    }

    /// The list requests that delete since the last call: number, command, session and agent
    /// to stop.
    fn archives(h: &mut Harness) -> Vec<(u64, String, String, Option<AgentId>)> {
        (h.agent_ops.drain(..))
            .filter_map(|op| match op {
                AgentOp::List {
                    request,
                    command,
                    delete: Some((session, stop)),
                    ..
                } => Some((request, command, session.to_string(), stop)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_archives_a_codex_session_and_its_fresh_list_lands() {
        let mut h = listed();
        h.keys("n a");
        let sent = archives(&mut h);
        let [(request, command, session, None)] = &sent[..] else {
            panic!("{sent:?}");
        };
        assert_eq!((command.as_str(), session.as_str()), ("codex-acp", "x1"));
        let rows = "▾ Today  2
    Fix it ✻ claude       10:00
    Old ✻ claude       08:00";
        assert_eq!(body(&h), rows);
        // The answer to the earlier request is dropped, the new one taken.
        let stale = vec![info("x1", Some("Port"), Some("2026-10-28T09:00:00Z"))];
        answer(&mut h, request - 1, Ok(stale));
        assert_eq!(body(&h), rows);
        let fresh = vec![info("x2", Some("New"), Some("2026-10-28T11:00:00Z"))];
        answer(&mut h, *request, Ok(fresh));
        assert!(body(&h).contains("New ◆ codex"), "{}", body(&h));
    }

    #[test]
    fn a_fails_on_claude_sessions_and_headings() {
        let mut h = listed();
        h.keys("a");
        assert_eq!(h.core.message(), Some("Not supported yet"));
        h.keys("p a");
        assert_eq!(h.core.message(), Some("Select a session row"));
        assert!(archives(&mut h).is_empty());
        assert!(body(&h).contains("Port ◆ codex"));
    }

    #[test]
    fn archiving_an_open_session_stops_its_agent_and_keeps_its_buffer() {
        let mut h = work();
        h.keys("M-x agent-start RET codex RET");
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("x1")),
        });
        let session = h.core.windows.window.buffer();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        answer(&mut h, requests[0].0, Ok(Vec::new()));
        let mine = vec![info("x1", Some("Mine"), Some("2026-10-28T10:00:00Z"))];
        answer(&mut h, requests[1].0, Ok(mine));
        h.keys("a");
        let sent = archives(&mut h);
        assert!(matches!(&sent[..], [(.., Some(AgentId(0)))]), "{sent:?}");
        assert!(h.core.buffers.get(session).is_some());
        assert_eq!(body(&h), "No sessions");
    }

    #[test]
    fn a_rolled_back_session_leaves_the_list() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        let started = Ok(NewSessionResponse::new("c1"));
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: started,
        });
        let chunk = ContentChunk::new("yo".into()).message_id("m1");
        let update = SessionNotification::new("c1", SessionUpdate::AgentMessageChunk(chunk));
        h.send(Msg::AgentUpdate {
            id: AgentId(0),
            update: Box::new(update),
        });
        let session = h.core.windows.window.buffer();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let mine = vec![info("c1", Some("Mine"), Some("2026-10-28T10:00:00Z"))];
        answer(&mut h, requests[0].0, Ok(mine.clone()));
        answer(&mut h, requests[1].0, Ok(mine));
        h.core.switch_to(session);
        h.ctx().goto(0).unwrap();
        h.keys("M-x agent-rollback-session RET yes RET");
        h.send(Msg::AgentStarted {
            id: AgentId(1),
            result: Ok(NewSessionResponse::new("f")),
        });
        // Codex's session of the same id is another session.
        assert_eq!(body(&h), "▾ Today  1\n    Mine ◆ codex       10:00");
    }

    /// The listed sessions' search for `needle`, started with `s`.
    fn session_search(h: &mut Harness) -> crate::GrepSearch {
        h.collect_effects = true;
        h.keys("s needle")
            .advance(std::time::Duration::from_millis(200));
        (std::mem::take(&mut h.effects).into_iter())
            .find_map(|e| match e {
                Effect::Grep(crate::GrepOp::Search(op)) => Some(op),
                _ => None,
            })
            .expect("a search starts")
    }

    /// The title on the line point is on in the sessions window under the minibuffer.
    fn previewed(h: &Harness) -> String {
        let window = &h.core.windows.others[&h.core.windows.tree.leaves()[0]];
        let buffer = h.core.buffers.get(window.buffer()).unwrap();
        let line = buffer.line_col(window.point(&h.core.buffers)).unwrap().0;
        let text = buffer.text().line(line).to_string();
        text.split(['✻', '◆']).next().unwrap().trim().into()
    }

    #[test]
    fn s_searches_only_the_listed_sessions_and_lists_their_text_by_session() {
        let mut h = listed();
        let op = session_search(&mut h);
        let mb = h.core.frame().minibuffer.unwrap();
        assert_eq!(mb.prompt, "Search sessions in home: ");
        let home = std::env::home_dir().unwrap();
        let cwd: String = (h.path("app").to_string_lossy().chars())
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let claude = home.join(".claude/projects").join(&cwd);
        let codex = home.join(".codex/sessions");
        assert_eq!(op.roots, [claude.clone(), codex.clone()]);
        assert_eq!(op.names, ["c1", "c2", "x1"]);
        let hit = |root: &PathBuf, path: &str, text: &str| crate::Hit {
            root: root.clone(),
            path: path.into(),
            line: 3,
            col: 0,
            text: text.into(),
        };
        let long = format!(r#"{{"message":{{"content":"{} needle"}}}}"#, "x".repeat(30));
        let hits = vec![
            hit(
                &claude,
                "c2.jsonl",
                r#"{"cwd":"/needle","message":{"content":"hi"}}"#,
            ),
            hit(
                &codex,
                "2026/10/28/rollout-2026-10-28T09-00-00-x1.jsonl",
                r#"{"payload":{"content":[{"type":"input_text","text":"the needle\nnow"}]}}"#,
            ),
            hit(&claude, "c1.jsonl", &long),
        ];
        h.send(Msg::Grepped {
            search: op.id,
            hits: Ok(hits),
            done: true,
        });
        let mb = h.frame(800., 600.).minibuffer.unwrap();
        let cut = format!("…{} needle", "x".repeat(19));
        assert_eq!(mb.candidates, ["the needle now", cut.as_str()]);
        assert_eq!(mb.groups, [(0, "◆ Port".into()), (1, "✻ Fix it".into())]);
        assert!(!mb.numbered, "no line numbers");
        assert_eq!(mb.highlights, [vec![4..10], vec![23..29]]);
        assert_eq!(previewed(&h), "Port");
        h.keys("C-n");
        assert_eq!(previewed(&h), "Fix it");
        h.collect_effects = false;
        h.keys("RET");
        let loads: Vec<_> = (h.agent_ops.drain(..))
            .filter_map(|op| match op {
                AgentOp::Spawn { session, .. } => Some(session),
                _ => None,
            })
            .collect();
        assert_eq!(loads, [Open::Load("c1".into())]);
    }

    #[test]
    fn a_hit_in_a_folded_session_previews_nothing() {
        let mut h = listed();
        h.keys("p TAB");
        let op = session_search(&mut h);
        h.send(Msg::Grepped {
            search: op.id,
            hits: Ok(vec![crate::Hit {
                root: op.roots[0].clone(),
                path: "c1.jsonl".into(),
                line: 0,
                col: 0,
                text: r#"{"text":"a needle"}"#.into(),
            }]),
            done: true,
        });
        assert_eq!(
            h.frame(800., 600.).minibuffer.unwrap().candidates,
            ["a needle"]
        );
        assert!(previewed(&h).starts_with("▸ Today"), "{}", previewed(&h));
    }

    #[test]
    fn s_needs_a_sessions_buffer_with_searchable_sessions() {
        let mut h = work();
        h.keys("M-x agent-sessions-search RET");
        assert_eq!(h.core.message(), Some("Not in a sessions buffer"));
        h.keys("M-x agent-sessions RET");
        h.keys("s");
        assert_eq!(h.core.message(), Some("No sessions to search"));
    }

    #[test]
    fn ret_loads_a_session_not_open_and_then_shows_it() {
        let mut h = listed();
        let agents = &mut h.core.settings.agents;
        agents.get_mut("claude").unwrap().default_mode = Some("plan".into());
        let lib = Root {
            path: h.path("lib"),
        };
        h.core.projects[0].project.roots.push(lib);
        let list = h.core.windows.window.buffer();
        h.keys("RET");
        let spawns: Vec<_> = (h.agent_ops.drain(..))
            .filter_map(|op| match op {
                AgentOp::Spawn {
                    command,
                    cwd,
                    roots,
                    session,
                    ..
                } => Some((command, cwd, roots, session)),
                _ => None,
            })
            .collect();
        let c1 = Open::Load("c1".into());
        let roots = vec![h.path("lib")];
        assert_eq!(
            spawns,
            [("claude-agent-acp".into(), h.path("app"), roots, c1)]
        );
        assert_eq!(h.core.message(), Some("Loading claude session..."));
        // The list stays shown while the history loads.
        assert_eq!(h.core.windows.window.buffer(), list);
        assert!(body(&h).starts_with("▾ Today  3\n    Fix it ✻ claude       10:00  ○ loading\n"));
        let session = h.core.buffers.find("*claude: Fix it*<work>").unwrap();
        let user = SessionUpdate::UserMessageChunk(ContentChunk::new("hi".into()));
        let reply = SessionUpdate::AgentMessageChunk(ContentChunk::new("hello".into()));
        for update in [user, reply] {
            let update = Box::new(SessionNotification::new("c1", update));
            h.send(Msg::AgentUpdate {
                id: AgentId(0),
                update,
            });
        }
        let text = h.core.buffers.get(session).unwrap().text().to_string();
        assert_eq!(text, "> hi\n\nhello\n\n> ");
        h.keys("RET");
        assert_eq!(h.core.windows.window.buffer(), list);
        assert_eq!(h.core.message(), Some("Loading claude session..."));
        assert!(h.agent_ops.is_empty());
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("c1").config_options(vec![mode()])),
        });
        // A loaded session keeps its mode.
        assert!(h.agent_ops.is_empty());
        assert_eq!(
            h.core.message(),
            Some("claude session ready (modes: auto, plan)")
        );
        assert_eq!(h.core.windows.window.buffer(), session);
        h.core.switch_to(list);
        assert!(body(&h).starts_with("▾ Today  3\n    Fix it ✻ claude       10:00  ○ idle\n"));
        h.keys("RET");
        assert_eq!(h.core.windows.window.buffer(), session);
        h.keys("x RET");
        let prompts: Vec<_> = (h.agent_ops.drain(..))
            .filter_map(|op| match op {
                AgentOp::Prompt { request, .. } => Some(request.prompt),
                _ => None,
            })
            .collect();
        assert_eq!(prompts, [vec!["x".into()]]);
    }

    /// A `mode` select at `auto` with values `auto` and `plan`.
    fn mode() -> SessionConfigOption {
        let values = ["auto", "plan"].map(|v| SessionConfigSelectOption::new(v, v));
        let mode = SessionConfigOption::select("mode", "Mode", "auto", values.to_vec());
        mode.category(SessionConfigOptionCategory::Mode)
    }

    #[test]
    fn a_session_loaded_after_leaving_the_list_stays_hidden() {
        let mut h = listed();
        let list = h.core.windows.window.buffer();
        h.keys("RET");
        let other = h.core.buffers.find("*claude stderr*<work>").unwrap();
        h.core.switch_to(other);
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("c1")),
        });
        assert_eq!(h.core.windows.window.buffer(), other);
        assert_eq!(h.core.message(), Some("claude session ready (modes: none)"));
        h.core.switch_to(list);
        assert!(body(&h).starts_with("▾ Today  3\n    Fix it ✻ claude       10:00  ○ idle\n"));
    }

    #[test]
    fn an_open_session_in_another_workspace_switches_to_it() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("c1")),
        });
        let (work, session) = (h.core.workspace().unwrap(), h.core.windows.window.buffer());
        let other = h.core.new_workspace("other", ProjectId(0));
        h.core.switch_workspace(other).unwrap();
        h.agent_ops.clear();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        answer(&mut h, requests[0].0, Ok(vec![info("c1", None, None)]));
        assert_eq!(line(&h), "    c1 ✻ claude              ○ idle");
        h.keys("RET");
        assert_eq!(
            (h.core.workspace().unwrap(), h.core.windows.window.buffer()),
            (work, session)
        );
    }

    #[test]
    fn states_follow_the_session_without_g() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("c1")),
        });
        let session = h.core.windows.window.buffer();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        answer(&mut h, requests[0].0, Ok(vec![info("c1", None, None)]));
        answer(&mut h, requests[1].0, Ok(Vec::new()));
        assert_eq!(last(&h), "    c1 ✻ claude              ○ idle");
        h.core.switch_to(session);
        h.keys("a RET");
        assert_eq!(last(&h), "    c1 ✻ claude              ● working");
        assert_eq!(last_face(&h), ("● working".into(), FaceName::StatusRunning));
        let option = PermissionOption::new("allow", "Yes", PermissionOptionKind::AllowOnce);
        let call = ToolCallUpdate::new("t", ToolCallUpdateFields::new());
        let ask = RequestPermissionRequest::new("c1", call, vec![option]);
        h.send(Msg::AgentRequest {
            id: AgentId(0),
            request: RequestId::Number(1),
            body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
        });
        assert_eq!(
            last(&h),
            "    c1 ✻ claude              ● awaiting permission"
        );
        let waiting = ("● awaiting permission".into(), FaceName::StatusWaiting);
        assert_eq!(last_face(&h), waiting);
        h.keys("C-c C-y");
        assert_eq!(last(&h), "    c1 ✻ claude              ● working");
        h.send(Msg::AgentPrompted {
            id: AgentId(0),
            result: Ok(StopReason::EndTurn),
        });
        assert_eq!(last(&h), "    c1 ✻ claude              ○ idle");
        h.send(Msg::AgentExited {
            id: AgentId(0),
            code: 1,
            signal: None,
        });
        assert_eq!(last(&h), "    c1 ✻ claude              ● exited");
        assert_eq!(last_face(&h), ("● exited".into(), FaceName::StatusFailed));
        h.core.switch_to(session);
        h.keys("C-x k RET");
        assert!(h.core.buffers.get(session).is_none());
        assert_eq!(last(&h), "    c1 ✻ claude");
    }

    /// `listed`, plus claude's `y1` yesterday and `o1` two weeks ago.
    fn grouped() -> Harness {
        let mut h = listed();
        h.keys("g");
        let requests = lists(&mut h);
        let claude = vec![
            info("c1", Some("Fix it"), Some("2026-10-28T10:00:00Z")),
            info("c2", Some("Old"), Some("2026-10-28T08:00:00Z")),
            info("y1", Some("Late"), Some("2026-10-27T23:00:00Z")),
            info("o1", Some("Ancient"), Some("2026-10-14T09:00:00Z")),
        ];
        answer(&mut h, requests[0].0, Ok(claude));
        let codex = vec![info("x1", Some("Port"), Some("2026-10-28T09:00:00Z"))];
        answer(&mut h, requests[1].0, Ok(codex));
        h
    }

    #[test]
    fn groups_show_headings_with_counts_and_moves_cross_them() {
        let mut h = grouped();
        assert_eq!(
            body(&h),
            "▾ Today  3\n\
            \x20   Fix it ✻ claude       10:00\n\
            \x20   Port ◆ codex        09:00\n\
            \x20   Old ✻ claude       08:00\n\
            \n\
            ▾ Yesterday  1\n\
            \x20   Late ✻ claude       23:00\n\
            \n\
            ▾ Older  1\n\
            \x20   Ancient ✻ claude  2026-10-14"
        );
        h.keys("n n n");
        assert_eq!(line(&h), "▾ Yesterday  1");
        h.keys("n n n n");
        assert!(line(&h).contains("Ancient"));
        h.keys("p");
        assert_eq!(line(&h), "▾ Older  1");
    }

    #[test]
    fn tab_folds_a_group_and_g_resets_the_folds() {
        let mut h = grouped();
        h.keys("TAB");
        // Only headings fold.
        assert!(line(&h).contains("Fix it"));
        h.keys("p TAB");
        assert_eq!(line(&h), "▸ Today  3");
        assert!(body(&h).starts_with("▸ Today  3\n\n▾ Yesterday  1\n"));
        h.keys("TAB");
        assert_eq!(line(&h), "▾ Today  3");
        assert!(body(&h).starts_with("▾ Today  3\n    Fix it "));
        h.keys("TAB g");
        let requests = lists(&mut h);
        for (request, ..) in requests {
            let x1 = info("x1", Some("Port"), Some("2026-10-28T09:00:00Z"));
            answer(&mut h, request, Ok(vec![x1]));
        }
        assert!(body(&h).starts_with("▾ Today  2\n"));
    }

    #[test]
    fn point_keeps_its_heading_and_a_folded_session_goes_to_its_heading() {
        let mut h = grouped();
        h.keys("n n n");
        assert_eq!(line(&h), "▾ Yesterday  1");
        let mut sessions = vec![info("c0", Some("New"), Some("2026-10-28T11:00:00Z"))];
        sessions.extend([
            info("c1", Some("Fix it"), Some("2026-10-28T10:00:00Z")),
            info("y1", Some("Late"), Some("2026-10-27T23:00:00Z")),
        ]);
        answer(&mut h, 3, Ok(sessions.clone()));
        assert_eq!(line(&h), "▾ Yesterday  1");
        h.keys("M-< n");
        assert!(line(&h).contains("New"));
        // Folded from its heading, an answer keeps point there.
        h.keys("p TAB");
        answer(&mut h, 3, Ok(sessions));
        assert_eq!(line(&h), "▸ Today  3");
    }

    #[test]
    fn a_session_folded_away_puts_point_on_its_heading() {
        let mut h = grouped();
        let list = h.core.windows.window.buffer();
        h.keys("n");
        assert!(line(&h).contains("Port"));
        let buffer = h.core.buffers.get_mut(list).unwrap();
        buffer.session_list_mut().unwrap().folded.push(Group::Today);
        h.core.render_sessions(list);
        assert_eq!(line(&h), "▸ Today  3");
    }

    #[test]
    fn each_piece_of_a_heading_and_a_row_has_its_face() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("s1")),
        });
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let mine = info("s1", Some("Mine"), Some("2026-10-28T10:00:00Z"));
        let port = info("c1", Some("Port"), Some("2026-10-26T09:00:00Z"));
        answer(&mut h, requests[0].0, Ok(vec![mine]));
        answer(&mut h, requests[1].0, Ok(vec![port]));
        let face = |text: &str, face| (text.to_string(), vec![FaceRef::Face(face)]);
        use FaceName::*;
        // The titles and the spaces between columns have none.
        assert_eq!(
            faces(&h),
            [
                face("▾ ", Annotation),
                face("Today", SessionListGroup),
                face("1", SessionListCount),
                face("✻ claude", AgentClaude),
                face("10:00", SessionListToday),
                face("○ idle", ModeLineDim),
                face("▾ ", Annotation),
                face("This week", SessionListGroup),
                face("1", SessionListCount),
                face("◆ codex", AgentCodex),
                face("Mon 09:00", ModeLineDim),
            ]
        );
        assert_eq!(Group::Pinned.face(), [FaceRef::Face(SessionListPinned)]);
        let archived = [FaceRef::Face(Annotation), FaceRef::Bold];
        assert_eq!(Group::Archived.face(), archived);
        assert_eq!(agent_look("gemini"), ("  ", None));
    }

    #[test]
    fn a_long_title_is_cut() {
        let mut h = work();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let long = "x".repeat(80);
        let at = Some("2026-10-28T10:00:00Z");
        answer(&mut h, requests[0].0, Ok(vec![info("c1", Some(&long), at)]));
        answer(&mut h, requests[1].0, Ok(Vec::new()));
        let row = last(&h);
        assert_eq!(row, format!("    {}… ✻ claude       10:00", "x".repeat(59)));
        assert_eq!(row.chars().position(|c| c == '✻'), Some(65));
    }

    #[test]
    fn the_mode_line_counts_sessions_and_the_echo_line_shows_keys() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("c1")),
        });
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let at = |hour| Some(format!("2026-10-28T{hour}:00:00Z"));
        let claude = vec![
            info("c1", Some("Mine"), at("10").as_deref()),
            info("c2", Some("Other"), at("09").as_deref()),
            info("c3", Some("Third"), at("08").as_deref()),
        ];
        answer(&mut h, requests[0].0, Ok(claude));
        answer(&mut h, requests[1].0, Ok(Vec::new()));
        h.keys("n");
        let frame = h.frame(1440., 900.);
        let mode_line = &frame.windows[0].display.mode_line;
        let segment = |s: &crate::display::Segment| s.iter().map(|(t, _)| t.as_str()).collect();
        let left: Vec<String> = mode_line.left.iter().map(segment).collect();
        assert_eq!(left, ["*Sessions*", "home · 3 sessions · 1 open"]);
        let open = ("1 open".to_string(), FaceName::StatusRunning);
        assert_eq!(mode_line.left[1].last(), Some(&open));
        // The rows end at the window's last column, so times and states line up.
        let display = &frame.windows[0].display;
        let rows: Vec<String> = (display.lines.iter())
            .filter(|l| (1..=3).contains(&l.number))
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert!(rows[0].ends_with("10:00  ○ idle") && rows[1].ends_with("09:00        "));
        let width = rows[0].chars().count();
        assert!(width > 100 && rows.iter().all(|r| r.chars().count() == width));
        let selected = Decoration::Background(FaceName::CompletionSelected);
        let selected: Vec<_> = (display.lines.iter())
            .filter(|l| l.decorations.contains(&selected))
            .map(|l| l.number)
            .collect();
        assert_eq!(selected, [2]);
        let right: Vec<String> = mode_line.right.iter().map(segment).collect();
        assert_eq!(right, ["2/3"]);
        h.keys("p p");
        let frame = h.frame(1440., 900.);
        let display = &frame.windows[0].display;
        let selected = Decoration::Background(FaceName::CompletionSelected);
        let selected: Vec<_> = (display.lines.iter())
            .filter(|l| l.decorations.contains(&selected))
            .map(|l| l.number)
            .collect();
        assert_eq!(selected, [0]);
        let hints: Vec<_> = (frame.key_hints.iter())
            .map(|(k, a)| (k.as_str(), *a))
            .collect();
        assert_eq!(
            hints,
            [
                ("n/p", "move"),
                ("RET", "switch"),
                ("TAB", "fold"),
                ("s", "search"),
                ("P", "pin/unpin"),
                ("a", "archive"),
                ("g", "refresh"),
                ("q", "quit")
            ]
        );
    }

    #[test]
    fn on_a_monday_two_days_back_is_older_and_an_empty_list_has_no_position() {
        let mut h = work();
        h.core.wall_clock = || "2026-10-26T09:00:00Z".parse().unwrap();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let sessions = vec![
            info("s1", Some("Sun"), Some("2026-10-25T10:00:00Z")),
            info("s2", Some("Sat"), Some("2026-10-24T10:00:00Z")),
        ];
        answer(&mut h, requests[0].0, Ok(sessions));
        answer(&mut h, requests[1].0, Ok(Vec::new()));
        assert_eq!(
            body(&h),
            "▾ Yesterday  1\n\
            \x20   Sun ✻ claude       10:00\n\
            \n\
            ▾ Older  1\n\
            \x20   Sat ✻ claude  2026-10-24"
        );
        h.keys("g");
        for (request, ..) in lists(&mut h) {
            answer(&mut h, request, Ok(Vec::new()));
        }
        let frame = h.frame(1440., 900.);
        let mode_line = &frame.windows[0].display.mode_line;
        assert!(mode_line.right.is_empty());
        let none = ("0 open".to_string(), FaceName::ModeLineDim);
        assert_eq!(mode_line.left[1].last(), Some(&none));
    }

    /// A `sessions.toml` pinning each (agent, session) in project `app`.
    fn pins_file(h: &Harness, pins: &[(&str, &str)]) -> String {
        let app = h.path("app").display().to_string();
        (pins.iter())
            .map(|(agent, session)| {
                format!("[[pin]]\nagent = \"{agent}\"\ncwd = \"{app}\"\nsession = \"{session}\"\n")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The pins `sessions.toml`'s last write holds, as (agent, session).
    fn written(h: &Harness) -> Vec<(String, String)> {
        let pins = parse_pins(h.pins.last().map(String::as_str)).unwrap();
        pins.into_iter().map(|p| (p.agent, p.session)).collect()
    }

    #[test]
    fn p_pins_and_unpins_and_point_follows_the_session() {
        let mut h = listed();
        h.keys("n P");
        assert_eq!(
            body(&h),
            "▾ Pinned  1\n\
            \x20   Port ◆ codex        09:00\n\
            \n\
            ▾ Today  2\n\
            \x20   Fix it ✻ claude       10:00\n\
            \x20   Old ✻ claude       08:00"
        );
        assert!(line(&h).contains("Port"));
        assert_eq!(written(&h), [("codex".into(), "x1".into())]);
        let app = h.path("app");
        assert_eq!(h.core.pins[0].cwd, app);
        // After a restart, the file as written pins it again.
        let mut fresh = work();
        fresh.send(Msg::PinsLoaded(Ok(h.pins.last().cloned())));
        fresh.keys("M-x agent-sessions RET");
        let requests = lists(&mut fresh);
        let x1 = info("x1", Some("Port"), Some("2026-10-28T09:00:00Z"));
        answer(&mut fresh, requests[1].0, Ok(vec![x1]));
        assert!(body(&fresh).ends_with("▾ Pinned  1\n    Port ◆ codex       09:00"));
        h.keys("P");
        assert!(body(&h).starts_with("▾ Today  3\n    Fix it "));
        assert!(line(&h).contains("Port"));
        assert_eq!(written(&h), []);
        h.keys("M-< P");
        assert_eq!(h.core.message(), Some("Select a session row"));
        assert_eq!(h.pins.len(), 2);
    }

    #[test]
    fn the_same_id_from_another_agent_is_not_pinned() {
        let mut h = work();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let s1 = vec![info("s1", Some("Same"), Some("2026-10-28T10:00:00Z"))];
        answer(&mut h, requests[0].0, Ok(s1.clone()));
        answer(&mut h, requests[1].0, Ok(s1));
        h.keys("P");
        assert_eq!(
            body(&h),
            "▾ Pinned  1\n\
            \x20   Same ✻ claude       10:00\n\
            \n\
            ▾ Today  1\n\
            \x20   Same ◆ codex        10:00"
        );
    }

    #[test]
    fn loaded_pins_list_under_pinned_with_their_time() {
        let mut h = work();
        let text = pins_file(&h, &[("claude", "o1")]);
        h.send(Msg::PinsLoaded(Ok(Some(text))));
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let old = info("o1", Some("Ancient"), Some("2026-10-14T09:00:00Z"));
        let new = info("c1", Some("Fix it"), Some("2026-10-28T10:00:00Z"));
        answer(&mut h, requests[0].0, Ok(vec![new, old]));
        answer(&mut h, requests[1].0, Ok(Vec::new()));
        assert_eq!(
            body(&h),
            "▾ Pinned  1\n\
            \x20   Ancient ✻ claude  2026-10-14\n\
            \n\
            ▾ Today  1\n\
            \x20   Fix it ✻ claude       10:00"
        );
        let date = faces(&h).into_iter().find(|(t, _)| t == "2026-10-14");
        assert_eq!(date.unwrap().1, [FaceRef::Face(FaceName::ModeLineDim)]);
        // Nothing went, so nothing is written.
        assert!(h.pins.is_empty());
    }

    #[test]
    fn an_unreadable_file_is_reported_and_pins_nothing() {
        let mut h = work();
        h.send(Msg::PinsLoaded(Ok(Some("[[pin]]\nagent = 1".into()))));
        assert!(h.core.message().unwrap().starts_with("sessions.toml: "));
        assert!(h.core.pins.is_empty());
        h.send(Msg::PinsLoaded(Ok(None)));
        assert!(h.core.pins.is_empty());
    }

    #[test]
    fn a_gone_sessions_pin_is_dropped_and_others_are_kept() {
        let mut h = work();
        let mut text = pins_file(&h, &[("claude", "c9"), ("claude", "c1"), ("codex", "x9")]);
        text.push_str("\n[[pin]]\nagent = \"claude\"\ncwd = \"/lib\"\nsession = \"l1\"\n");
        h.send(Msg::PinsLoaded(Ok(Some(text))));
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        // An error keeps codex's pins.
        answer(&mut h, requests[1].0, Err("closed"));
        assert!(h.pins.is_empty());
        let c1 = info("c1", Some("Fix it"), Some("2026-10-28T10:00:00Z"));
        answer(&mut h, requests[0].0, Ok(vec![c1]));
        assert_eq!(h.core.message(), None);
        let kept = [("claude", "c1"), ("codex", "x9"), ("claude", "l1")];
        let kept: Vec<_> = kept.map(|(a, s)| (a.to_string(), s.to_string())).into();
        assert_eq!(written(&h), kept);
        assert!(body(&h).ends_with("▾ Pinned  1\n    Fix it ✻ claude       10:00"));
    }
}
