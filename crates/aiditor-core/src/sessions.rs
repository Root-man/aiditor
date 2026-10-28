//! `*Sessions*`: every registered agent's past sessions in the current project, from
//! `session/list`.

use agent_client_protocol_schema::v1::{SessionId, SessionInfo};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::buffer::BufferId;
use crate::project::ProjectId;
use crate::{AgentOp, Core, Ctx};

const MODE: &str = "agent-sessions-mode";
const HEADER: &str = "Sessions — RET switch, g refresh, n/p move, q quit";

/// A sessions buffer's state.
pub(crate) struct SessionList {
    project: ProjectId,
    /// One per registered agent, in registry order.
    agents: Vec<Listing>,
    /// Each session row's line, agent index and session.
    rows: Vec<(usize, usize, SessionId)>,
    /// Each row's state as rendered, to rebuild when one changes.
    states: Vec<Option<&'static str>>,
    /// The agent and session point was last on, kept while no rows are listed.
    at: Option<(usize, SessionId)>,
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

/// `time` in `zone` to the minute.
fn local(time: Timestamp, zone: &TimeZone) -> String {
    time.to_zoned(zone.clone())
        .strftime("%Y-%m-%d %H:%M")
        .to_string()
}

impl Core {
    /// Takes the held list requests of `project`'s live sessions buffers, `vars` first in each
    /// environment.
    pub(crate) fn release_lists(
        &mut self,
        project: ProjectId,
        vars: &[(String, String)],
    ) -> Vec<AgentOp> {
        let buffers = &self.buffers;
        self.session_lists
            .retain(|id, _| buffers.get(*id).is_some());
        let lists = (self.session_lists.values_mut()).filter(|l| l.project == project);
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

    /// Rebuilds the live sessions buffers whose rows' states changed since they were rendered.
    pub(crate) fn sync_session_lists(&mut self) {
        let buffers = &self.buffers;
        self.session_lists
            .retain(|id, _| buffers.get(*id).is_some());
        let stale: Vec<_> = (self.session_lists.iter())
            .filter(|(_, list)| {
                let states = (list.rows.iter()).map(|(_, agent, session)| {
                    self.session_state(&list.agents[*agent].name, session)
                });
                !states.eq(list.states.iter().copied())
            })
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            self.render_sessions(id);
        }
    }

    /// Rebuilds sessions buffer `id`'s text from its answers, keeping point on its session.
    fn render_sessions(&mut self, id: BufferId) {
        let (Some(list), Some(buffer)) = (self.session_lists.get(&id), self.buffers.get(id)) else {
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
        let at = match list.rows.iter().find(|(l, ..)| *l == line) {
            Some((_, agent, session)) => Some((*agent, session.clone())),
            None if list.rows.is_empty() => list.at.clone(),
            None => None,
        };
        let mut lines = vec![HEADER.to_string(), String::new()];
        let mut sessions = Vec::new();
        for (index, listing) in list.agents.iter().enumerate() {
            match &listing.answer {
                None => lines.push(format!("Listing {}...", listing.name)),
                Some(Err(err)) => lines.push(format!("{}: {err}", listing.name)),
                Some(Ok(infos)) => sessions.extend(infos.iter().map(|info| {
                    let time = timestamp(info.updated_at.as_deref());
                    (index, info, time)
                })),
            }
        }
        // Stable: equal times keep registry order, then the agent's.
        sessions.sort_by_key(|&(.., time)| std::cmp::Reverse(time));
        let width = (sessions.iter())
            .map(|&(agent, ..)| list.agents[agent].name.chars().count())
            .max()
            .unwrap_or(0);
        let (mut rows, mut states) = (Vec::new(), Vec::new());
        for (agent, info, time) in sessions {
            let name = &list.agents[agent].name;
            let time = time.map_or_else(String::new, |t| local(t, &self.time_zone));
            let state = self.session_state(name, &info.session_id);
            states.push(state);
            let state = state.map_or_else(String::new, |s| format!("{s} "));
            let title = (info.title.as_deref())
                .and_then(|t| t.lines().next())
                .filter(|t| !t.is_empty())
                .map_or_else(|| info.session_id.to_string(), str::to_string);
            rows.push((lines.len(), agent, info.session_id.clone()));
            lines.push(format!("{name:<width$}  {time:<16}  {state}{title}"));
        }
        if rows.is_empty() && list.agents.iter().all(|a| a.answer.is_some()) {
            lines.push("No sessions".into());
        }
        let target = (at.as_ref())
            .and_then(|key| rows.iter().find(|(_, a, s)| (*a, s) == (key.0, &key.1)))
            .or(rows.first())
            .map(|row| row.0);
        let at = match rows.is_empty() {
            true => at,
            false => rows
                .iter()
                .find(|r| Some(r.0) == target)
                .map(|r| (r.1, r.2.clone())),
        };
        let rows_first = rows.first().map(|row| row.0);
        let list = self.session_lists.get_mut(&id).expect("checked above");
        (list.rows, list.states, list.at) = (rows, states, at);
        let buffer = self.buffers.get_mut(id).expect("checked above");
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
        let first = target.is_some() && target == rows_first;
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
        let workspace = core.current_workspace();
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
        let buffers = &core.buffers;
        core.session_lists
            .retain(|id, _| buffers.get(*id).is_some());
        let here = core.workspace();
        let found = (core.session_lists.keys())
            .find(|id| {
                core.buffers
                    .get(**id)
                    .is_some_and(|b| b.workspace == Some(here))
            })
            .copied();
        let id = found.unwrap_or_else(|| {
            let id = core.buffers.create(&name);
            let buffer = core.buffers.get_mut(id).expect("just created");
            (buffer.major_mode, buffer.read_only) = (MODE, true);
            id
        });
        let at = core.session_lists.remove(&id).and_then(|l| l.at);
        let list = SessionList {
            project,
            agents,
            rows: Vec::new(),
            states: Vec::new(),
            at,
        };
        core.session_lists.insert(id, list);
        core.switch_to(id);
        core.render_sessions(id);
        self.release_agents(project);
        Ok(())
    }

    /// Keeps an answer to the latest request of a live sessions buffer and rebuilds it.
    pub(crate) fn agent_listed(&mut self, request: u64, result: Result<Vec<SessionInfo>, String>) {
        let core = &mut *self.core;
        let buffers = &core.buffers;
        core.session_lists
            .retain(|id, _| buffers.get(*id).is_some());
        let found = (core.session_lists.iter_mut()).find_map(|(id, list)| {
            let listing = list.agents.iter_mut().find(|a| a.request == request)?;
            Some((*id, listing))
        });
        if let Some((id, listing)) = found {
            listing.answer = Some(result);
            core.render_sessions(id);
        }
    }

    fn session_rows(&self) -> Result<&SessionList, String> {
        let id = self.core.windows.window.buffer();
        (self.core.session_lists.get(&id)).ok_or_else(|| "Not in a sessions buffer".into())
    }

    /// `agent-sessions-next` and `-previous`: to the next or previous session row.
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
        let (line, first) = (target.0, target.0 == first.0);
        // The header above the first row is otherwise out of reach once scrolled past.
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
        let (_, agent, session) = (list.rows.iter())
            .find(|r| r.0 == line)
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
            return self.start_session(&name, Some((session, title.as_deref())));
        };
        let owner = self.core.buffers.get(buffer).and_then(|b| b.workspace);
        if let Some(owner) = owner.filter(|w| *w != self.core.workspace()) {
            self.core.switch_workspace(owner)?;
        }
        self.core.switch_to(buffer);
        Ok(())
    }

    /// `agent-sessions-quit`: kills the sessions buffer.
    pub(crate) fn sessions_quit(&mut self) -> Result<(), String> {
        self.session_rows()?;
        let id = self.core.windows.window.buffer();
        self.core.session_lists.remove(&id);
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
    use jiff::tz::Offset;

    use super::*;
    use crate::harness::Harness;
    use crate::settings::Agent;
    use crate::{AgentId, Effect, Msg, Request};

    /// A harness in workspace `work` on a project at `app`, with agents `claude` (`env` `A=2`)
    /// and `codex`, showing times in UTC.
    fn work() -> Harness {
        let mut h = Harness::new(&[("app/main.rs", "")]);
        h.core.rename_workspace("work").unwrap();
        h.core.projects[0].project.primary.path = h.path("app");
        h.core.time_zone = TimeZone::UTC;
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

    /// The text after the header and its blank line.
    fn body(h: &Harness) -> String {
        let id = h.core.buffers.find("*Sessions*<work>").expect("listed");
        let text = h.core.buffers.get(id).unwrap().text().to_string();
        text.strip_prefix(&format!("{HEADER}\n\n")).unwrap().into()
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
        let shown = |s: &str| timestamp(Some(s)).map(|t| local(t, &zone));
        assert_eq!(shown("2026-10-28T10:00:00Z").unwrap(), "2026-10-28 12:00");
        assert_eq!(
            shown("2026-10-28T12:30:00.123+02:00").unwrap(),
            "2026-10-28 12:30"
        );
        assert_eq!(
            shown("2026-10-28T05:15:00.5-04:00").unwrap(),
            "2026-10-28 11:15"
        );
        assert_eq!(shown("yesterday"), None);
        assert_eq!(timestamp(None), None);
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
            "claude  2026-10-28 10:00  Fix it\n\
             codex   2026-10-28 09:00  Port\n\
             claude  2026-10-28 08:00  Old"
        );
        assert_eq!(line(&h), "claude  2026-10-28 10:00  Fix it");
    }

    #[test]
    fn missing_times_sort_last_blank_and_errors_replace_rows() {
        let mut h = work();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        let sessions = vec![
            info("none", None, None),
            info("bad", Some("Bad\nsecond line"), Some("yesterday")),
            info("good", Some("Good"), Some("2026-10-28T10:00:00Z")),
        ];
        answer(&mut h, requests[0].0, Ok(sessions));
        assert_eq!(
            body(&h),
            "Listing codex...\n\
             claude  2026-10-28 10:00  Good\n\
             claude                    none\n\
             claude                    Bad"
        );
        answer(&mut h, requests[1].0, Err("codex can't list sessions"));
        assert!(body(&h).starts_with("codex: codex can't list sessions\nclaude  "));
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
        assert!(line(&h).ends_with("Port"));
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
        assert!(body(&h).starts_with("claude  2026-10-28 11:00  New\ncodex "));
        assert!(line(&h).ends_with("Port"));
    }

    #[test]
    fn rows_are_moved_over_and_q_kills_the_list() {
        let mut h = listed();
        h.keys("n n n p");
        assert!(line(&h).ends_with("Port"));
        h.keys("p p");
        assert!(line(&h).ends_with("Fix it"));
        h.keys("M-<").keys("RET");
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
        assert_eq!(line(&h), "claude                    c0");
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
            "claude  2026-10-28 10:00  [idle] Mine\n\
             codex   2026-10-28 10:00  Mine\n\
             claude  2026-10-28 09:00  Other"
        );
        h.keys("RET");
        assert_eq!(h.core.windows.window.buffer(), session);
    }

    #[test]
    fn ret_loads_a_session_not_open_and_then_shows_it() {
        let mut h = listed();
        let agents = &mut h.core.settings.agents;
        agents.get_mut("claude").unwrap().default_mode = Some("plan".into());
        let list = h.core.windows.window.buffer();
        h.keys("RET");
        let spawns: Vec<_> = (h.agent_ops.drain(..))
            .filter_map(|op| match op {
                AgentOp::Spawn {
                    command,
                    cwd,
                    session,
                    ..
                } => Some((command, cwd, session)),
                _ => None,
            })
            .collect();
        let c1 = Some(SessionId::from("c1"));
        assert_eq!(spawns, [("claude-agent-acp".into(), h.path("app"), c1)]);
        assert_eq!(h.core.current().name, "*claude: Fix it*<work>");
        assert_eq!(h.core.message(), Some("Loading claude session..."));
        let session = h.core.windows.window.buffer();
        let user = SessionUpdate::UserMessageChunk(ContentChunk::new("hi".into()));
        let reply = SessionUpdate::AgentMessageChunk(ContentChunk::new("hello".into()));
        for update in [user, reply] {
            let update = Box::new(SessionNotification::new("c1", update));
            h.send(Msg::AgentUpdate {
                id: AgentId(0),
                update,
            });
        }
        assert_eq!(h.text(), "> hi\n\nhello\n\n> ");
        h.keys("RET");
        assert_eq!(h.core.message(), Some("claude is not ready"));
        h.core.switch_to(list);
        assert!(body(&h).starts_with("claude  2026-10-28 10:00  [starting] Fix it\n"));
        h.keys("RET");
        assert_eq!(h.core.windows.window.buffer(), session);
        assert!(h.agent_ops.is_empty());
        let mode = SessionConfigOption::select(
            "mode",
            "Mode",
            "auto",
            vec![
                SessionConfigSelectOption::new("auto", "auto"),
                SessionConfigSelectOption::new("plan", "plan"),
            ],
        );
        let mode = mode.category(SessionConfigOptionCategory::Mode);
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("c1").config_options(vec![mode])),
        });
        // A loaded session keeps its mode.
        assert!(h.agent_ops.is_empty());
        assert_eq!(
            h.core.message(),
            Some("claude session ready (modes: auto, plan)")
        );
        h.core.switch_to(list);
        assert!(body(&h).starts_with("claude  2026-10-28 10:00  [idle] Fix it\n"));
    }

    #[test]
    fn an_open_session_in_another_workspace_switches_to_it() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("c1")),
        });
        let (work, session) = (h.core.workspace(), h.core.windows.window.buffer());
        let other = h.core.new_workspace("other", ProjectId(0));
        h.core.switch_workspace(other).unwrap();
        h.agent_ops.clear();
        h.keys("M-x agent-sessions RET");
        let requests = lists(&mut h);
        answer(&mut h, requests[0].0, Ok(vec![info("c1", None, None)]));
        assert_eq!(line(&h), "claude                    [idle] c1");
        h.keys("RET");
        assert_eq!(
            (h.core.workspace(), h.core.windows.window.buffer()),
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
        assert_eq!(body(&h), "claude                    [idle] c1");
        h.core.switch_to(session);
        h.keys("a RET");
        assert_eq!(body(&h), "claude                    [working] c1");
        let option = PermissionOption::new("allow", "Yes", PermissionOptionKind::AllowOnce);
        let call = ToolCallUpdate::new("t", ToolCallUpdateFields::new());
        let ask = RequestPermissionRequest::new("c1", call, vec![option]);
        h.send(Msg::AgentRequest {
            id: AgentId(0),
            request: RequestId::Number(1),
            body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
        });
        assert_eq!(body(&h), "claude                    [waiting] c1");
        h.keys("C-c C-y");
        assert_eq!(body(&h), "claude                    [working] c1");
        h.send(Msg::AgentPrompted {
            id: AgentId(0),
            result: Ok(StopReason::EndTurn),
        });
        assert_eq!(body(&h), "claude                    [idle] c1");
        h.send(Msg::AgentExited {
            id: AgentId(0),
            code: 1,
            signal: None,
        });
        assert_eq!(body(&h), "claude                    [ended] c1");
        h.core.switch_to(session);
        h.keys("C-x k RET");
        assert!(h.core.buffers.get(session).is_none());
        assert_eq!(body(&h), "claude                    c1");
    }
}
