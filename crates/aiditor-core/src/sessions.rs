//! `*Sessions*`: every registered agent's past sessions in the current project, from
//! `session/list`.

use agent_client_protocol_schema::v1::{SessionId, SessionInfo};
use jiff::{Span, Timestamp, Zoned};

use crate::buffer::{Buffer, BufferId, Kind};
use crate::display::{ModeLine, WindowDisplay};
use crate::project::ProjectId;
use crate::settings::FaceName;
use crate::{AgentOp, Core, Ctx, Open};

const MODE: &str = "agent-sessions-mode";
/// The widest a title shows, cut to one less and `…` past it.
const TITLE_WIDTH: usize = 60;

/// The echo line's actions, as the workspace list's.
pub(crate) const HINTS: &[(&[&str], &str)] = &[
    (&["agent-sessions-next", "agent-sessions-previous"], "move"),
    (&["agent-sessions-visit"], "switch"),
    (&["agent-sessions-toggle"], "fold"),
    (&["agent-sessions"], "refresh"),
    (&["agent-sessions-quit"], "quit"),
];

/// A group of sessions by activity, in the list's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    // Nothing is pinned or archived until pins and archived listings exist.
    #[allow(dead_code)]
    Pinned,
    Today,
    Yesterday,
    Week,
    Older,
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
}

/// A line point can be on: a group's heading or a session's row.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    Heading(Group),
    /// The agent's index and the session.
    Session(usize, SessionId),
}

/// A sessions buffer's state.
pub(crate) struct SessionList {
    project: ProjectId,
    /// One per registered agent, in registry order.
    agents: Vec<Listing>,
    /// Each heading's and shown session row's line, in order.
    rows: Vec<(usize, Row)>,
    /// Every listed session, folded or shown, with its state as rendered, to rebuild when one
    /// changes.
    listed: Vec<(usize, SessionId, Option<&'static str>)>,
    folded: Vec<Group>,
    /// The row point was last on, kept while no rows are listed.
    at: Option<Row>,
    /// The mode line's project, session count and open count.
    summary: String,
}

struct Listing {
    name: String,
    /// The latest request's number; answers to others are dropped.
    request: u64,
    /// The list request, held while the project's environment is being captured.
    held: Option<AgentOp>,
    answer: Option<Result<Vec<SessionInfo>, String>>,
}

/// `updated` read as RFC 3339, whatever its offset or fraction.
fn timestamp(updated: Option<&str>) -> Option<Timestamp> {
    updated?.parse().ok()
}

impl SessionList {
    /// Puts the list's mode line, with point on `line`, on `buffer`'s `display`.
    pub(crate) fn decorate(&self, buffer: &Buffer, display: &mut WindowDisplay, line: usize) {
        let sessions: Vec<_> = (self.rows.iter())
            .filter(|(_, row)| matches!(row, Row::Session(..)))
            .map(|(line, _)| *line)
            .collect();
        let position = sessions.partition_point(|l| *l <= line).max(1);
        let segment = |text: &str, face| vec![(text.to_string(), face)];
        display.mode_line = ModeLine {
            left: vec![
                segment(&buffer.name, FaceName::ModeLine),
                segment(&self.summary, FaceName::ModeLineDim),
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
        let mut lines = Vec::new();
        let mut sessions = Vec::new();
        for (index, listing) in list.agents.iter().enumerate() {
            match &listing.answer {
                None => lines.push(format!("Listing {}...", listing.name)),
                // On one line, so each row's line number holds.
                Some(Err(err)) => {
                    let err = err.split_whitespace().collect::<Vec<_>>().join(" ");
                    lines.push(format!("{}: {err}", listing.name));
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
                let (group, time) = Group::of(time, &now);
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
        let titles = entries
            .iter()
            .map(|e| e.3.chars().count())
            .max()
            .unwrap_or(0);
        let agents = (entries.iter())
            .map(|e| list.agents[e.1].name.chars().count())
            .max()
            .unwrap_or(0);
        let mut rows = Vec::new();
        for members in entries.chunk_by(|a, b| a.0 == b.0) {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            let group = members[0].0;
            let folded = list.folded.contains(&group);
            let mark = if folded { "▸" } else { "▾" };
            rows.push((lines.len(), Row::Heading(group)));
            lines.push(format!("{mark} {} {}", group.name(), members.len()));
            if folded {
                continue;
            }
            for (_, agent, session, title, time, state) in members {
                let name = &list.agents[*agent].name;
                let state = state.map_or_else(String::new, |s| format!("  {s}"));
                let row = format!("  {title:<titles$}  {name:<agents$}  {time:>10}{state}");
                rows.push((lines.len(), Row::Session(*agent, session.clone())));
                lines.push(row.trim_end().to_string());
            }
        }
        let all_answered = list.agents.iter().all(|a| a.answer.is_some());
        if entries.is_empty() && all_answered {
            lines.push("No sessions".into());
        }
        let listed: Vec<_> = (entries.iter())
            .map(|(_, agent, session, .., state)| (*agent, session.clone(), *state))
            .collect();
        let open = listed.iter().filter(|(.., state)| state.is_some()).count();
        let project =
            (self.project_at(list.project)).map_or("", |i| &self.projects[i].project.name);
        let summary = format!("{project} · {} sessions · {open} open", listed.len());
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
        let at = match rows.is_empty() {
            true => at,
            false => (rows.iter())
                .find(|(l, _)| Some(*l) == target)
                .map(|(_, r)| r.clone()),
        };
        let first = target.is_some()
            && (target == first_session || target == rows.first().map(|(l, _)| *l));
        let buffer = self.buffers.get_mut(id).expect("checked above");
        let list = buffer.session_list_mut().expect("checked above");
        (list.rows, list.listed, list.at, list.summary) = (rows, listed, at, summary);
        buffer.set_text(&lines.join("\n"));
        let point = match target {
            Some(line) => buffer.char_idx(line, 0).expect("a row is a line"),
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
        if core.settings.agents.is_empty() {
            return Err("No agents registered".into());
        }
        let workspace = core.current_workspace()?;
        let (project, name) = (workspace.project, format!("*Sessions*<{}>", workspace.name));
        let loaded = &core.projects[core.project_at(project)?].project;
        let cwd = loaded.primary.path.clone();
        let mut agents = Vec::new();
        for (name, agent) in &core.settings.agents {
            core.list_requests += 1;
            let request = core.list_requests;
            // Under the captured environment, which `release_agents` puts first.
            let env = (agent.env.clone().into_iter())
                .chain(loaded.env.clone())
                .collect();
            let op = AgentOp::List {
                request,
                command: agent.command.clone(),
                args: agent.args.clone(),
                cwd: cwd.clone(),
                env,
            };
            agents.push(Listing {
                name: name.clone(),
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
            agents,
            rows: Vec::new(),
            listed: Vec::new(),
            folded: vec![Group::Archived],
            at,
            summary: String::new(),
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
            Some((id, listing))
        });
        if let Some((id, listing)) = found {
            listing.answer = Some(result);
            core.render_sessions(id);
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
        let line = target.0;
        let first = line == first.0 || first_session.is_some_and(|r| r.0 == line);
        // The lines above the first row are otherwise out of reach once scrolled past.
        if first {
            self.core.windows.window.set_top(0);
        }
        self.goto(self.core.current().char_idx(line, 0)?)
    }

    /// `agent-sessions-visit`: shows the session on point's row, loading it in the current
    /// workspace unless it is open.
    pub(crate) fn sessions_visit(&mut self) -> Result<(), String> {
        let list = self.session_rows()?;
        let line = self.core.current().line_col(self.core.point())?.0;
        let (agent, session) = (list.rows.iter())
            .find_map(|(l, row)| match row {
                Row::Session(agent, session) if *l == line => Some((agent, session)),
                _ => None,
            })
            .ok_or("Select a session row")?;
        let listing = &list.agents[*agent];
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
        if self.core.session_state(&listing.name, session) == Some("[loading]") {
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

    fn body(h: &Harness) -> String {
        let id = h.core.buffers.find("*Sessions*<work>").expect("listed");
        h.core.buffers.get(id).unwrap().text().to_string()
    }

    /// The last line, the only row of a one-session list.
    fn last(h: &Harness) -> String {
        body(h).lines().last().unwrap().into()
    }

    fn line(h: &Harness) -> String {
        let (line, _) = h.core.current().line_col(h.point()).unwrap();
        h.text().lines().nth(line).unwrap_or_default().into()
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
    fn no_agents_lists_nothing() {
        let mut h = work();
        h.core.settings.agents.clear();
        h.keys("M-x agent-sessions RET");
        assert_eq!(h.core.message(), Some("No agents registered"));
        assert!(h.core.buffers.find("*Sessions*<work>").is_none());
    }

    #[test]
    fn sessions_of_both_agents_show_most_recent_first() {
        let h = listed();
        assert_eq!(
            body(&h),
            "▾ Today 3\n\
            \x20 Fix it  claude       10:00\n\
            \x20 Port    codex        09:00\n\
            \x20 Old     claude       08:00"
        );
        assert_eq!(line(&h), "  Fix it  claude       10:00");
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
             ▾ Older 3\n\
            \x20 Good  claude  2026-10-14\n\
            \x20 none  claude\n\
            \x20 Bad   claude"
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
            Some("[loading]")
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
        assert!(body(&h).starts_with("▾ Today 2\n  New   claude  "));
        assert!(line(&h).contains("Port"));
    }

    #[test]
    fn rows_are_moved_over_and_q_kills_the_list() {
        let mut h = listed();
        h.keys("n n n p");
        assert!(line(&h).contains("Port"));
        h.keys("p");
        assert!(line(&h).contains("Fix it"));
        h.keys("p p");
        assert_eq!(line(&h), "▾ Today 3");
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
        assert_eq!(line(&h), "▾ Older 20");
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
            "▾ Today 3\n\
            \x20 Mine   claude       10:00  [idle]\n\
            \x20 Mine   codex        10:00\n\
            \x20 Other  claude       09:00"
        );
        h.keys("RET");
        assert_eq!(h.core.windows.window.buffer(), session);
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
        assert_eq!(body(&h), "▾ Today 1\n  Mine  codex       10:00");
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
        assert!(body(&h).starts_with("▾ Today 3\n  Fix it  claude       10:00  [loading]\n"));
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
        assert!(body(&h).starts_with("▾ Today 3\n  Fix it  claude       10:00  [idle]\n"));
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
        assert!(body(&h).starts_with("▾ Today 3\n  Fix it  claude       10:00  [idle]\n"));
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
        assert_eq!(line(&h), "  c1  claude              [idle]");
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
        assert_eq!(last(&h), "  c1  claude              [idle]");
        h.core.switch_to(session);
        h.keys("a RET");
        assert_eq!(last(&h), "  c1  claude              [working]");
        let option = PermissionOption::new("allow", "Yes", PermissionOptionKind::AllowOnce);
        let call = ToolCallUpdate::new("t", ToolCallUpdateFields::new());
        let ask = RequestPermissionRequest::new("c1", call, vec![option]);
        h.send(Msg::AgentRequest {
            id: AgentId(0),
            request: RequestId::Number(1),
            body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
        });
        assert_eq!(last(&h), "  c1  claude              [waiting]");
        h.keys("C-c C-y");
        assert_eq!(last(&h), "  c1  claude              [working]");
        h.send(Msg::AgentPrompted {
            id: AgentId(0),
            result: Ok(StopReason::EndTurn),
        });
        assert_eq!(last(&h), "  c1  claude              [idle]");
        h.send(Msg::AgentExited {
            id: AgentId(0),
            code: 1,
            signal: None,
        });
        assert_eq!(last(&h), "  c1  claude              [ended]");
        h.core.switch_to(session);
        h.keys("C-x k RET");
        assert!(h.core.buffers.get(session).is_none());
        assert_eq!(last(&h), "  c1  claude");
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
            "▾ Today 3\n\
            \x20 Fix it   claude       10:00\n\
            \x20 Port     codex        09:00\n\
            \x20 Old      claude       08:00\n\
            \n\
            ▾ Yesterday 1\n\
            \x20 Late     claude       23:00\n\
            \n\
            ▾ Older 1\n\
            \x20 Ancient  claude  2026-10-14"
        );
        h.keys("n n n");
        assert_eq!(line(&h), "▾ Yesterday 1");
        h.keys("n n n n");
        assert!(line(&h).contains("Ancient"));
        h.keys("p");
        assert_eq!(line(&h), "▾ Older 1");
    }

    #[test]
    fn tab_folds_a_group_and_g_resets_the_folds() {
        let mut h = grouped();
        h.keys("TAB");
        // Only headings fold.
        assert!(line(&h).contains("Fix it"));
        h.keys("p TAB");
        assert_eq!(line(&h), "▸ Today 3");
        assert!(body(&h).starts_with("▸ Today 3\n\n▾ Yesterday 1\n"));
        h.keys("TAB");
        assert_eq!(line(&h), "▾ Today 3");
        assert!(body(&h).starts_with("▾ Today 3\n  Fix it "));
        h.keys("TAB g");
        let requests = lists(&mut h);
        for (request, ..) in requests {
            let x1 = info("x1", Some("Port"), Some("2026-10-28T09:00:00Z"));
            answer(&mut h, request, Ok(vec![x1]));
        }
        assert!(body(&h).starts_with("▾ Today 2\n"));
    }

    #[test]
    fn point_keeps_its_heading_and_a_folded_session_goes_to_its_heading() {
        let mut h = grouped();
        h.keys("n n n");
        assert_eq!(line(&h), "▾ Yesterday 1");
        let mut sessions = vec![info("c0", Some("New"), Some("2026-10-28T11:00:00Z"))];
        sessions.extend([
            info("c1", Some("Fix it"), Some("2026-10-28T10:00:00Z")),
            info("y1", Some("Late"), Some("2026-10-27T23:00:00Z")),
        ]);
        answer(&mut h, 3, Ok(sessions.clone()));
        assert_eq!(line(&h), "▾ Yesterday 1");
        h.keys("M-< n");
        assert!(line(&h).contains("New"));
        // Folded from its heading, an answer keeps point there.
        h.keys("p TAB");
        answer(&mut h, 3, Ok(sessions));
        assert_eq!(line(&h), "▸ Today 3");
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
        assert_eq!(line(&h), "▸ Today 3");
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
        assert_eq!(row, format!("  {}…  claude       10:00", "x".repeat(59)));
        assert_eq!(row.chars().position(|c| c == 'c'), Some(64));
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
        let right: Vec<String> = mode_line.right.iter().map(segment).collect();
        assert_eq!(right, ["2/3"]);
        let hints: Vec<_> = (frame.key_hints.iter())
            .map(|(k, a)| (k.as_str(), *a))
            .collect();
        assert_eq!(
            hints,
            [
                ("n/p", "move"),
                ("RET", "switch"),
                ("TAB", "fold"),
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
            "▾ Yesterday 1\n\
            \x20 Sun  claude       10:00\n\
            \n\
            ▾ Older 1\n\
            \x20 Sat  claude  2026-10-24"
        );
        h.keys("g");
        for (request, ..) in lists(&mut h) {
            answer(&mut h, request, Ok(Vec::new()));
        }
        let frame = h.frame(1440., 900.);
        assert!(frame.windows[0].display.mode_line.right.is_empty());
    }
}
