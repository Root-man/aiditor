//! Agent sessions: each one an ACP agent process started in a workspace, with a session buffer,
//! comint-style, and a read-only buffer for its stderr.

use agent_client_protocol_schema::v1::{
    ContentBlock, NewSessionResponse, PromptRequest, SessionNotification, SessionUpdate, StopReason,
};

use std::ops::Range;

use crate::buffer::{Buffer, BufferId, InsertionType, MarkerId};
use crate::command::Args;
use crate::markdown;
use crate::project::ProjectId;
use crate::settings::Theme;
use crate::{AgentId, AgentOp, Core, Ctx, Effect, Effects};

/// The prompt line's text, just before the prompt area.
const PROMPT: &str = "> ";
/// The session buffer's major mode.
pub(crate) const MODE: &str = "agent-session-mode";

pub(crate) struct Session {
    /// The registry name it was started as.
    agent: String,
    /// The session buffer, the session's handle: killing it stops the agent.
    pub(crate) buffer: BufferId,
    /// Its stderr log.
    log: BufferId,
    project: ProjectId,
    /// The spawn, held while the project's environment is being captured.
    held: Option<AgentOp>,
    /// The `session/new` answer, once the agent is ready.
    pub(crate) session: Option<NewSessionResponse>,
    /// Whether the agent failed to start or exited; its buffers stay until killed.
    ended: bool,
    /// `session/update`s in order, for the session buffer.
    pub(crate) updates: Vec<SessionNotification>,
    /// A `session/prompt` waits for its answer.
    turn: bool,
    /// The last block above the prompt line, while an agent message or thought streams into it.
    stream: Option<Stream>,
}

/// A block being streamed.
struct Stream {
    /// Where the block starts.
    start: MarkerId,
    thought: bool,
    /// Chunks arrived since it was last rendered.
    dirty: bool,
}

impl Core {
    /// Puts `text` above session `id`'s prompt line: as a new block followed by a blank line, or,
    /// for a chunk of the kind streaming (`Some(thought)`), at the end of the streamed block. A
    /// chunk's block is rendered as markdown by the next frame.
    fn output(&mut self, id: AgentId, text: &str, chunk: Option<bool>) {
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let end = buffer.prompt_start() - PROMPT.len();
        if let Some(stream) = &mut session.stream
            && Some(stream.thought) == chunk
        {
            buffer.insert_output(end - "\n\n".len(), text);
            stream.dirty = true;
            return;
        }
        self.end_stream(id);
        let session = self.agents.get_mut(&id).expect("found above");
        let buffer = self.buffers.get_mut(session.buffer).expect("found above");
        buffer.insert_output(end, &format!("{text}\n\n"));
        session.stream = chunk.map(|thought| Stream {
            start: buffer
                .add_marker(end, InsertionType::Stay)
                .expect("in the text"),
            thought,
            dirty: true,
        });
    }

    /// Renders session `id`'s streamed block if chunks arrived since it was last rendered.
    fn render_stream(&mut self, id: AgentId) {
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let (Some(stream), Some(buffer)) =
            (&mut session.stream, self.buffers.get_mut(session.buffer))
        else {
            return;
        };
        if std::mem::take(&mut stream.dirty) {
            let start = buffer.marker(stream.start).expect("live while streaming");
            let end = buffer.prompt_start() - PROMPT.len() - "\n\n".len();
            render(buffer, start..end, stream.thought, &self.settings.theme);
        }
    }

    /// Renders and ends session `id`'s stream, if any.
    fn end_stream(&mut self, id: AgentId) {
        self.render_stream(id);
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        if let (Some(stream), Some(buffer)) =
            (session.stream.take(), self.buffers.get_mut(session.buffer))
        {
            buffer.remove_marker(stream.start);
        }
    }

    /// Renders each streamed block that changed, at most once per frame.
    pub(crate) fn render_streams(&mut self) {
        let ids: Vec<_> = self.agents.keys().copied().collect();
        for id in ids {
            self.render_stream(id);
        }
    }

    /// The session whose buffer the selected window shows.
    fn current_session(&mut self) -> Result<(AgentId, &mut Session), String> {
        let buffer = self.windows.window.buffer();
        (self.agents.iter_mut())
            .find(|(_, s)| s.buffer == buffer)
            .map(|(&id, s)| (id, s))
            .ok_or_else(|| "Not in an agent session buffer".into())
    }

    /// The agent names `agent-start` completes over, in registry order.
    pub(crate) fn agent_names(&self) -> Vec<String> {
        self.settings.agents.keys().cloned().collect()
    }

    /// The current project's `default_agent` among `agent_names`, else the first.
    pub(crate) fn default_agent_index(&self) -> usize {
        let project = self.current_workspace().project;
        let default = (self.project_at(project).ok())
            .and_then(|at| self.projects[at].project.default_agent.as_ref());
        default
            .and_then(|name| self.settings.agents.keys().position(|n| n == name))
            .unwrap_or(0)
    }

    /// Whether any agent session hasn't ended: waiting, starting or ready.
    pub(crate) fn live_agents(&self) -> bool {
        self.agents.values().any(|s| !s.ended)
    }

    /// Whether buffer `id` is the session buffer of a session that hasn't ended.
    pub(crate) fn live_agent(&self, id: BufferId) -> bool {
        self.agents.values().any(|s| s.buffer == id && !s.ended)
    }

    /// Stops the agents of sessions whose session buffer was killed and forgets them.
    pub(crate) fn sync_agents(&mut self, effects: &mut Effects) {
        let buffers = &self.buffers;
        self.agents.retain(|&id, session| {
            let live = buffers.get(session.buffer).is_some();
            if !live {
                effects.push(Effect::Agent(AgentOp::Kill { id }));
            }
            live
        });
    }
}

impl Ctx<'_> {
    /// `agent-start`: starts the named agent in the current workspace's project, showing a new
    /// session buffer, with a new log buffer for its stderr.
    pub(crate) fn agent_start(&mut self, args: Args) -> Result<(), String> {
        let core = &mut *self.core;
        let name = &args.strings[0];
        let agent = (core.settings.agents.get(name))
            .ok_or_else(|| format!("{name:?} is not a registered agent"))?
            .clone();
        let workspace = core.current_workspace();
        let (project, log, title) = (
            workspace.project,
            format!("*{name} stderr*<{}>", workspace.name),
            format!("*{name}*<{}>", workspace.name),
        );
        let loaded = &core.projects[core.project_at(project)?].project;
        // Under the captured environment, which `release_agents` puts first.
        let env = (agent.env.into_iter()).chain(loaded.env.clone()).collect();
        let cwd = loaded.primary.path.clone();
        let id = AgentId(core.next_agent);
        core.next_agent += 1;
        let log = core.buffers.create(&log);
        core.buffers.get_mut(log).expect("just created").read_only = true;
        let buffer = core.buffers.create(&title);
        let b = core.buffers.get_mut(buffer).expect("just created");
        b.append(PROMPT);
        let start = PROMPT.len();
        b.prompt = Some((b.add_marker(start, InsertionType::Stay)).expect("after the prompt"));
        b.set_marker(b.point, start).expect("after the prompt");
        b.major_mode = MODE;
        core.switch_to(buffer);
        let spawn = AgentOp::Spawn {
            id,
            command: agent.command,
            args: agent.args,
            cwd,
            env,
        };
        let session = Session {
            agent: name.clone(),
            buffer,
            log,
            project,
            held: Some(spawn),
            session: None,
            ended: false,
            updates: Vec::new(),
            turn: false,
            stream: None,
        };
        core.agents.insert(id, session);
        core.echo(format!("Starting {name}..."));
        self.release_agents(project);
        Ok(())
    }

    /// Sends the held spawns of `project`'s sessions, the captured environment first, unless a
    /// capture is still running.
    pub(crate) fn release_agents(&mut self, project: ProjectId) {
        let Some(vars) = self.core.captured(project) else {
            return;
        };
        let sessions = self.core.agents.values_mut();
        for session in sessions.filter(|s| s.project == project) {
            if let Some(AgentOp::Spawn {
                id,
                command,
                args,
                cwd,
                env,
            }) = session.held.take()
            {
                let env = vars.iter().cloned().chain(env).collect();
                self.effects.push(Effect::Agent(AgentOp::Spawn {
                    id,
                    command,
                    args,
                    cwd,
                    env,
                }));
            }
        }
    }

    pub(crate) fn agent_started(
        &mut self,
        id: AgentId,
        result: Result<NewSessionResponse, String>,
    ) {
        let core = &mut *self.core;
        let Some(session) = core.agents.get_mut(&id).filter(|s| !s.ended) else {
            return;
        };
        let agent = session.agent.clone();
        match result {
            Ok(response) => {
                let modes: Vec<_> = (response.modes.iter())
                    .flat_map(|m| &m.available_modes)
                    .map(|m| m.id.to_string())
                    .collect();
                let modes = match modes.is_empty() {
                    true => "none".into(),
                    false => modes.join(", "),
                };
                session.session = Some(response);
                core.echo(format!("{agent} session ready (modes: {modes})"));
            }
            Err(err) => {
                session.ended = true;
                // Stops an agent that lives on after an error answer, and releases its handle.
                self.effects.push(Effect::Agent(AgentOp::Kill { id }));
                core.echo(format!("{agent} session failed to start: {err}"));
            }
        }
    }

    pub(crate) fn agent_stderr(&mut self, id: AgentId, line: String) {
        let core = &mut *self.core;
        let buffer = (core.agents.get(&id)).and_then(|s| core.buffers.get_mut(s.log));
        if let Some(buffer) = buffer {
            buffer.append(&format!("{line}\n"));
        }
    }

    /// Keeps the update, and puts a text message or thought chunk above the prompt line.
    pub(crate) fn agent_update(&mut self, id: AgentId, update: SessionNotification) {
        let core = &mut *self.core;
        let Some(session) = core.agents.get_mut(&id).filter(|s| !s.ended) else {
            return;
        };
        let text = match &update.update {
            SessionUpdate::AgentMessageChunk(chunk) => Some((chunk, false)),
            SessionUpdate::AgentThoughtChunk(chunk) => Some((chunk, true)),
            _ => None,
        }
        .and_then(|(chunk, thought)| match &chunk.content {
            ContentBlock::Text(text) => Some((text.text.clone(), thought)),
            _ => None,
        });
        session.updates.push(update);
        if let Some((text, thought)) = text {
            core.output(id, &text, Some(thought));
        }
    }

    /// `agent-send-input`: sends the prompt area's text as a `session/prompt`, leaving it above
    /// a new prompt line.
    pub(crate) fn agent_send_input(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let buffer = core.windows.window.buffer();
        let (id, session) = core.current_session()?;
        let agent = &session.agent;
        let ready = match (&session.session, session.ended, session.turn) {
            (_, true, _) => Err(format!("{agent} session has ended")),
            (None, ..) => Err(format!("{agent} is not ready")),
            (_, _, true) => Err(format!("{agent} is busy")),
            (Some(ready), ..) => Ok(ready.session_id.clone()),
        };
        let session_id = ready?;
        let b = core
            .buffers
            .get_mut(buffer)
            .expect("current buffer is live");
        let start = b.prompt_start();
        let input = b.text().slice(start..).to_string();
        if input.trim().is_empty() {
            return Ok(());
        }
        core.end_stream(id);
        let b = core
            .buffers
            .get_mut(buffer)
            .expect("current buffer is live");
        b.append(&format!("\n\n{PROMPT}"));
        let theme = &core.settings.theme;
        render(b, start..start + input.chars().count(), false, theme);
        let end = b.len_chars();
        b.set_marker(b.prompt.expect("a session buffer has a prompt"), end)?;
        // The old prompt area's edits would replay into read-only text.
        b.clear_undo();
        let session = core.agents.get_mut(&id).expect("found above");
        session.turn = true;
        let request = PromptRequest::new(session_id, vec![input.into()]);
        self.effects
            .push(Effect::Agent(AgentOp::Prompt { id, request }));
        self.goto(end)
    }

    /// `agent-cancel`: sends `session/cancel` while a turn runs.
    pub(crate) fn agent_cancel(&mut self) -> Result<(), String> {
        let (id, session) = self.core.current_session()?;
        let ready = session.session.as_ref().filter(|_| session.turn);
        let session = ready.ok_or("No turn is running")?.session_id.clone();
        (self.effects).push(Effect::Agent(AgentOp::Cancel { id, session }));
        Ok(())
    }

    /// Ends the turn, with a line saying why unless it ended normally.
    pub(crate) fn agent_prompted(&mut self, id: AgentId, result: Result<StopReason, String>) {
        let core = &mut *self.core;
        let Some(session) = core.agents.get_mut(&id).filter(|s| s.turn) else {
            return;
        };
        session.turn = false;
        let line = match result {
            Ok(StopReason::EndTurn) => return,
            Ok(StopReason::MaxTokens) => "Turn ended: max_tokens".into(),
            Ok(StopReason::MaxTurnRequests) => "Turn ended: max_turn_requests".into(),
            Ok(StopReason::Refusal) => "Turn ended: refusal".into(),
            Ok(StopReason::Cancelled) => "Turn ended: cancelled".into(),
            Ok(reason) => format!("Turn ended: {reason:?}"),
            Err(err) => format!("Error: {err}"),
        };
        core.output(id, &line, None);
    }

    /// Ends the session after its agent exited, noting a crash, and releases the agent's handle.
    pub(crate) fn agent_exited(&mut self, id: AgentId, code: u32, signal: Option<String>) {
        let core = &mut *self.core;
        let Some(session) = core.agents.get_mut(&id) else {
            return;
        };
        session.turn = false;
        if !std::mem::replace(&mut session.ended, true) {
            let agent = &session.agent;
            let line = match signal {
                Some(signal) => format!("{agent} session killed by {signal}"),
                None => format!("{agent} session exited with code {code}"),
            };
            core.log(&line);
        }
        self.effects.push(Effect::Agent(AgentOp::Kill { id }));
    }
}

/// Renders `range` of `buffer` as markdown, replacing its properties.
fn render(buffer: &mut Buffer, range: Range<usize>, thought: bool, theme: &Theme) {
    buffer.remove_properties(range.clone());
    let text = buffer.text().slice(range.clone()).to_string();
    for (r, prop) in markdown::render(&text, thought, theme) {
        (buffer.add_property(range.start + r.start..range.start + r.end, prop))
            .expect("inside the rendered range");
    }
}

#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::v1::{
        ContentChunk, SessionMode, SessionModeState, SessionUpdate,
    };

    use super::*;
    use crate::Msg;
    use crate::buffer::{Decoration, Face, Prop};
    use crate::harness::{Harness, listed, name};
    use crate::settings::Agent;

    /// A harness in workspace `work` on a project at `/aiditor-test/app`, with agents `claude`
    /// (`env` `A=2`) and `codex`.
    fn work() -> Harness {
        let mut h = Harness::new(&[("app/src/main.rs", "fn main() {}\n")]);
        h.core.rename_workspace("work").unwrap();
        h.core.projects[0].project.primary.path = h.path("app");
        let agent = |command: &str, env: &[(&str, &str)]| Agent {
            command: command.into(),
            args: vec!["--acp".into()],
            env: (env.iter()).map(|&(k, v)| (k.into(), v.into())).collect(),
            default_mode: None,
        };
        let agents = &mut h.core.settings.agents;
        agents.insert("claude".into(), agent("claude-agent-acp", &[("A", "2")]));
        agents.insert("codex".into(), agent("codex-acp", &[]));
        h
    }

    /// The agent ops since the last call, as text.
    fn ops(h: &mut Harness) -> Vec<String> {
        (h.agent_ops.drain(..))
            .map(|op| match op {
                AgentOp::Spawn {
                    id,
                    command,
                    cwd,
                    env,
                    ..
                } => {
                    let env: Vec<_> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
                    format!(
                        "spawn {} {command} {} {}",
                        id.0,
                        cwd.display(),
                        env.join(",")
                    )
                }
                AgentOp::Reply { id, .. } => format!("reply {}", id.0),
                AgentOp::Prompt { id, request } => {
                    let [ContentBlock::Text(text)] = &request.prompt[..] else {
                        panic!("one text block: {request:?}");
                    };
                    format!("prompt {} {} {:?}", id.0, request.session_id, text.text)
                }
                AgentOp::Cancel { id, session } => format!("cancel {} {session}", id.0),
                AgentOp::Kill { id } => format!("kill {}", id.0),
            })
            .collect()
    }

    /// The request number of the last capture started.
    fn capture(h: &mut Harness) -> u64 {
        (h.effects.iter().rev())
            .find_map(|effect| match effect {
                Effect::CaptureEnvironment { request, .. } => Some(*request),
                _ => None,
            })
            .expect("a capture started")
    }

    fn messages(h: &Harness) -> String {
        let id = h.core.buffers.find("*Messages*").unwrap();
        h.core.buffers.get(id).unwrap().text().to_string()
    }

    fn log_text(h: &Harness, name: &str) -> String {
        let id = h.core.buffers.find(name).unwrap();
        h.core.buffers.get(id).unwrap().text().to_string()
    }

    fn ready(h: &mut Harness, id: u64) {
        let modes = vec![
            SessionMode::new("auto", "Auto"),
            SessionMode::new("plan", "Plan"),
        ];
        let response = NewSessionResponse::new("s").modes(SessionModeState::new("auto", modes));
        h.send(Msg::AgentStarted {
            id: AgentId(id),
            result: Ok(response),
        });
    }

    fn exited(h: &mut Harness, id: u64, code: u32) {
        h.send(Msg::AgentExited {
            id: AgentId(id),
            code,
            signal: None,
        });
    }

    #[test]
    fn the_default_agent_is_preselected_and_spawned_in_the_primary_repo() {
        let mut h = work();
        h.core.projects[0].project.default_agent = Some("codex".into());
        h.keys("M-x agent-start RET");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Agent: ");
        assert_eq!(
            listed(&mut h),
            (vec!["claude".into(), "codex".into()], Some(1))
        );
        h.keys("RET");
        let app = h.path("app");
        assert_eq!(
            ops(&mut h),
            [format!("spawn 0 codex-acp {} ", app.display())]
        );
        assert_eq!(h.core.message(), Some("Starting codex..."));
        // The session buffer is shown, the log isn't.
        assert_eq!(name(&h), "*codex*<work>");
        assert_eq!((h.core.text(), h.core.point()), ("> ".into(), 2));
        let session = h.core.current();
        assert!(!session.read_only && session.workspace == Some(h.core.workspace()));
        let log = h.core.buffers.find("*codex stderr*<work>").unwrap();
        let log = h.core.buffers.get(log).unwrap();
        assert!(log.read_only && log.workspace == Some(h.core.workspace()));
    }

    #[test]
    fn the_environment_layers_in_order() {
        let mut h = work();
        h.core.capture_environments();
        h.send(Msg::Tick);
        let request = capture(&mut h);
        h.send(Msg::EnvironmentCaptured {
            project: ProjectId(0),
            request,
            result: Ok(vec![("PATH".into(), "/a".into()), ("A".into(), "1".into())]),
            direnv: None,
        });
        h.core.projects[0]
            .project
            .env
            .insert("B".into(), "3".into());
        h.keys("M-x agent-start RET claude RET");
        let app = h.path("app");
        assert_eq!(
            ops(&mut h),
            [format!(
                "spawn 0 claude-agent-acp {} PATH=/a,A=1,A=2,B=3",
                app.display()
            )]
        );
    }

    #[test]
    fn an_unknown_agent_starts_nothing() {
        let mut h = work();
        h.call(
            "agent-start",
            Args {
                strings: vec!["gemini".into()],
                ..Args::default()
            },
        );
        assert_eq!(
            h.core.message(),
            Some("\"gemini\" is not a registered agent")
        );
        assert!(ops(&mut h).is_empty());
        assert!(h.core.agents.is_empty());
    }

    #[test]
    fn the_spawn_waits_for_the_capture() {
        let mut h = work();
        h.core.capture_environments();
        h.keys("M-x agent-start RET claude RET");
        assert!(ops(&mut h).is_empty());
        let request = capture(&mut h);
        h.send(Msg::EnvironmentCaptured {
            project: ProjectId(0),
            request,
            result: Ok(vec![("PATH".into(), "/opt/bin".into())]),
            direnv: None,
        });
        let app = h.path("app");
        assert_eq!(
            ops(&mut h),
            [format!(
                "spawn 0 claude-agent-acp {} PATH=/opt/bin,A=2",
                app.display()
            )]
        );
    }

    #[test]
    fn a_failed_capture_spawns_without_its_variables() {
        let mut h = work();
        h.core.capture_environments();
        h.keys("M-x agent-start RET claude RET");
        let request = capture(&mut h);
        h.send(Msg::EnvironmentCaptured {
            project: ProjectId(0),
            request,
            result: Err("timed out".into()),
            direnv: None,
        });
        let app = h.path("app");
        assert_eq!(
            ops(&mut h),
            [format!("spawn 0 claude-agent-acp {} A=2", app.display())]
        );
    }

    #[test]
    fn a_session_killed_while_waiting_never_spawns() {
        let mut h = work();
        h.core.capture_environments();
        h.keys("M-x agent-start RET claude RET");
        let session = h.core.buffers.find("*claude*<work>").unwrap();
        h.core.kill_buffer(session);
        h.send(Msg::Tick);
        let request = capture(&mut h);
        h.send(Msg::EnvironmentCaptured {
            project: ProjectId(0),
            request,
            result: Ok(Vec::new()),
            direnv: None,
        });
        // The kill of a never-spawned agent is ignored by the service.
        assert_eq!(ops(&mut h), ["kill 0"]);
    }

    #[test]
    fn stderr_goes_to_the_log_and_ready_to_messages() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        for line in ["hello", "world"] {
            h.send(Msg::AgentStderr {
                id: AgentId(0),
                line: line.into(),
            });
        }
        assert_eq!(log_text(&h, "*claude stderr*<work>"), "hello\nworld\n");
        ready(&mut h, 0);
        assert!(messages(&h).contains("claude session ready (modes: auto, plan)\n"));
        // It replaces `Starting claude...` in the echo area.
        assert_eq!(
            h.core.message(),
            Some("claude session ready (modes: auto, plan)")
        );
        h.keys("M-x agent-start RET claude RET");
        h.send(Msg::AgentStarted {
            id: AgentId(1),
            result: Ok(NewSessionResponse::new("t")),
        });
        assert!(messages(&h).contains("claude session ready (modes: none)\n"));
    }

    #[test]
    fn a_failed_start_is_noted_and_keeps_the_log() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Err("claude-agent-acp: No such file or directory".into()),
        });
        let note = "claude session failed to start: claude-agent-acp: No such file or directory";
        assert_eq!(h.core.message(), Some(note));
        assert!(messages(&h).contains(note));
        assert_eq!(ops(&mut h), ["kill 0"]);
        assert!(!h.core.live_agents());
        // Late stderr still reaches the log, and the exit adds no crash note.
        h.send(Msg::AgentStderr {
            id: AgentId(0),
            line: "auth failed".into(),
        });
        exited(&mut h, 0, 1);
        assert_eq!(log_text(&h, "*claude stderr*<work>"), "auth failed\n");
        assert!(!messages(&h).contains("exited"));
    }

    #[test]
    fn a_crash_ends_only_its_own_session() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET M-x agent-start RET claude RET");
        ready(&mut h, 0);
        ready(&mut h, 1);
        ops(&mut h);
        exited(&mut h, 0, 3);
        assert!(messages(&h).contains("claude session exited with code 3\n"));
        // Only the crashed agent's handle is released.
        assert_eq!(ops(&mut h), ["kill 0"]);
        assert!(h.core.live_agents());
        let second = h.core.buffers.find("*claude*<work><2>").unwrap();
        assert!(h.core.live_agent(second));
        let first = h.core.buffers.find("*claude*<work>").unwrap();
        assert!(!h.core.live_agent(first) && h.core.buffers.get(first).is_some());
        h.send(Msg::AgentExited {
            id: AgentId(1),
            code: 1,
            signal: Some("Killed: 9".into()),
        });
        assert!(messages(&h).contains("claude session killed by Killed: 9\n"));
    }

    fn update(h: &mut Harness, id: u64, update: SessionUpdate) {
        h.send(Msg::AgentUpdate {
            id: AgentId(id),
            update: Box::new(SessionNotification::new("s", update)),
        });
    }

    fn chunk(h: &mut Harness, text: &str) {
        update(
            h,
            0,
            SessionUpdate::AgentMessageChunk(ContentChunk::new(text.into())),
        );
    }

    fn prompted(h: &mut Harness, result: Result<StopReason, &str>) {
        h.send(Msg::AgentPrompted {
            id: AgentId(0),
            result: result.map_err(String::from),
        });
    }

    /// A ready `claude` session in `*claude*<work>`, shown.
    fn session() -> Harness {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ready(&mut h, 0);
        ops(&mut h);
        h
    }

    #[test]
    fn message_chunks_stream_above_the_prompt_and_updates_are_kept() {
        let mut h = session();
        let before = log_text(&h, "*claude stderr*<work>");
        chunk(&mut h, "Hi");
        let thought = SessionUpdate::AgentThoughtChunk(ContentChunk::new("hmm".into()));
        update(&mut h, 0, thought);
        chunk(&mut h, " there");
        // Unknown agents' updates are dropped.
        update(
            &mut h,
            9,
            SessionUpdate::AgentMessageChunk(ContentChunk::new("x".into())),
        );
        // The thought is a block of its own, between two message blocks.
        assert_eq!(h.core.text(), "Hi\n\nhmm\n\n there\n\n> ");
        assert_eq!(h.core.point(), h.core.text().chars().count());
        assert_eq!(h.core.agents[&AgentId(0)].updates.len(), 3);
        assert_eq!(log_text(&h, "*claude stderr*<work>"), before);
    }

    #[test]
    fn a_prompt_is_sent_and_its_reply_streams_above_the_prompt() {
        let mut h = session();
        h.keys("h e l l o RET");
        assert_eq!(ops(&mut h), [r#"prompt 0 s "hello""#]);
        assert_eq!(h.core.text(), "> hello\n\n> ");
        assert_eq!(h.core.point(), 11);
        chunk(&mut h, "Hi");
        chunk(&mut h, " there");
        prompted(&mut h, Ok(StopReason::EndTurn));
        assert_eq!(h.core.text(), "> hello\n\nHi there\n\n> ");
        // The turn ended, so the next prompt goes out.
        h.keys("a g a i n RET");
        assert_eq!(ops(&mut h), [r#"prompt 0 s "again""#]);
        chunk(&mut h, "Again");
        assert_eq!(
            h.core.text(),
            "> hello\n\nHi there\n\n> again\n\nAgain\n\n> "
        );
    }

    /// Each property of the current buffer, with the text it covers.
    fn props(h: &Harness) -> Vec<(String, Prop)> {
        let b = h.core.current();
        (b.properties())
            .map(|(r, p)| (b.text().slice(r).to_string(), p.clone()))
            .collect()
    }

    fn thought(h: &mut Harness, text: &str) {
        update(
            h,
            0,
            SessionUpdate::AgentThoughtChunk(ContentChunk::new(text.into())),
        );
    }

    #[test]
    fn a_streamed_block_renders_when_the_frame_is_built() {
        let mut h = session();
        h.keys("a RET");
        chunk(&mut h, "**a");
        chunk(&mut h, "b**");
        assert_eq!(h.core.text(), "> a\n\n**ab**\n\n> ");
        assert!(props(&h).is_empty(), "nothing rendered before the frame");
        h.core.frame();
        let bold = Face {
            bold: true,
            ..Face::default()
        };
        let hidden = Prop::Invisible(String::new());
        let first = vec![
            ("**".into(), hidden.clone()),
            ("ab".into(), Prop::Face(bold)),
            ("**".into(), hidden.clone()),
        ];
        assert_eq!(props(&h), first);
        // The next reply renders on its own; the first keeps its properties.
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.keys("b RET");
        chunk(&mut h, "*c*");
        assert_eq!(props(&h), first);
        h.core.frame();
        let italic = Face {
            italic: true,
            ..Face::default()
        };
        let all = [
            first.clone(),
            vec![
                ("*".into(), hidden.clone()),
                ("c".into(), Prop::Face(italic)),
                ("*".into(), hidden.clone()),
            ],
        ]
        .concat();
        assert_eq!(props(&h), all);
        h.core.frame();
        assert_eq!(props(&h), all, "a frame without chunks changes nothing");
    }

    #[test]
    fn a_finished_block_renders_before_the_next_starts() {
        let mut h = session();
        chunk(&mut h, "`x`");
        thought(&mut h, "hmm");
        // The message rendered when the thought began; the thought waits for the frame.
        assert_eq!(props(&h).len(), 3);
        h.core.frame();
        let dim = Face {
            fg: Some(h.core.settings().theme.dim),
            ..Face::default()
        };
        assert_eq!(props(&h).last(), Some(&("hmm".into(), Prop::Face(dim))));
    }

    #[test]
    fn the_echoed_prompt_renders_as_markdown() {
        let mut h = session();
        h.keys("* * h i * * RET");
        assert_eq!(h.core.text(), "> **hi**\n\n> ");
        let bold = Face {
            bold: true,
            ..Face::default()
        };
        assert!(props(&h).contains(&("hi".into(), Prop::Face(bold))));
    }

    #[test]
    fn rendered_replies_show_without_markup_and_keep_it_in_the_text() {
        let mut h = session();
        chunk(
            &mut h,
            "# Title\n\nSome **bold** [docs](https://x.dev).\n\n```rust\nfn main() {}\n```",
        );
        let frame = h.frame(800., 600.);
        let lines = &frame.windows[0].display.lines;
        let shown: Vec<String> = (lines.iter())
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert_eq!(
            shown[..7],
            ["Title", "", "Some bold docs.", "", "", "fn main() {}", ""]
        );
        let code = Decoration::Background(h.core.settings().theme.code_block);
        assert!((4..7).all(|i| lines[i].decorations == [code]));
        assert!(lines[3].decorations.is_empty());
        // Swiper matches the hidden markup, and copying yields it.
        h.keys("C-s * * b");
        assert_eq!(listed(&mut h).0, ["3 Some **bold** [docs](https://x.dev)."]);
        h.keys("C-g M-< C-n C-n C-SPC C-e M-w");
        assert_eq!(
            h.clipboard.as_deref(),
            Some("Some **bold** [docs](https://x.dev).")
        );
    }

    #[test]
    fn ret_sends_only_a_ready_idle_session_with_input() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        h.keys("x RET");
        assert_eq!(h.core.message(), Some("claude is not ready"));
        ready(&mut h, 0);
        h.keys("RET");
        assert_eq!(ops(&mut h), [r#"prompt 0 s "x""#]);
        h.keys("y RET");
        assert_eq!(h.core.message(), Some("claude is busy"));
        assert!(ops(&mut h).is_empty());
        assert_eq!(h.core.text(), "> x\n\n> y");
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.keys("DEL SPC C-j RET");
        assert!(ops(&mut h).is_empty());
        exited(&mut h, 0, 1);
        h.keys("RET");
        assert_eq!(h.core.message(), Some("claude session has ended"));
    }

    #[test]
    fn c_j_and_s_ret_insert_newlines() {
        let mut h = session();
        h.keys("a C-j b S-RET c");
        assert_eq!(h.core.text(), "> a\nb\nc");
        assert!(ops(&mut h).is_empty());
    }

    #[test]
    fn a_turn_that_ends_otherwise_says_so() {
        let mut h = session();
        h.keys("a RET");
        prompted(&mut h, Ok(StopReason::Refusal));
        assert_eq!(h.core.text(), "> a\n\nTurn ended: refusal\n\n> ");
        h.keys("b RET");
        chunk(&mut h, "B");
        prompted(&mut h, Err("Internal error"));
        assert!(
            h.core
                .text()
                .ends_with("> b\n\nB\n\nError: Internal error\n\n> ")
        );
        // A late answer, with no turn running, adds nothing.
        prompted(&mut h, Ok(StopReason::MaxTokens));
        assert!(h.core.text().ends_with("Error: Internal error\n\n> "));
    }

    #[test]
    fn c_c_c_c_cancels_a_running_turn() {
        let mut h = session();
        h.keys("C-c C-c");
        assert_eq!(h.core.message(), Some("No turn is running"));
        h.keys("a RET C-c C-c");
        assert_eq!(ops(&mut h), [r#"prompt 0 s "a""#, "cancel 0 s"]);
        prompted(&mut h, Ok(StopReason::Cancelled));
        assert_eq!(h.core.text(), "> a\n\nTurn ended: cancelled\n\n> ");
    }

    #[test]
    fn the_transcript_is_read_only_and_the_prompt_area_edits() {
        let mut h = session();
        h.keys("a RET");
        chunk(&mut h, "Hi");
        let transcript = h.core.text();
        h.keys("M-< z");
        assert_eq!(h.core.message(), Some("Text is read-only"));
        h.keys("M-> DEL");
        assert_eq!(h.core.message(), Some("Text is read-only"));
        assert_eq!(h.core.text(), transcript);
        h.keys("d r a f t DEL");
        chunk(&mut h, " there");
        assert_eq!(h.core.text(), "> a\n\nHi there\n\n> draf");
        assert_eq!(h.core.point(), h.core.text().chars().count());
        h.keys("C-/");
        assert_eq!(h.core.text(), "> a\n\nHi there\n\n> draft");
    }

    #[test]
    fn killing_a_workspace_stops_its_agents_without_a_crash_note() {
        let mut h = work();
        let work = h.core.workspace();
        h.new_workspace("fix", "home");
        h.keys("M-x agent-start RET claude RET");
        ready(&mut h, 0);
        ops(&mut h);
        let fix = h.core.workspace();
        h.core.switch_workspace(work).unwrap();
        h.core.kill_workspace(fix).unwrap();
        h.send(Msg::Tick);
        assert_eq!(ops(&mut h), ["kill 0"]);
        h.send(Msg::AgentExited {
            id: AgentId(0),
            code: 0,
            signal: Some("Terminated: 15".into()),
        });
        assert!(!messages(&h).contains("Terminated"));
        assert!(!h.core.live_agents());
    }

    #[test]
    fn killing_the_session_buffer_asks_first_and_the_log_does_not() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        fn kill(h: &mut Harness, name: &str) {
            let strings = vec![name.into()];
            h.call(
                "kill-buffer",
                Args {
                    strings,
                    ..Args::default()
                },
            );
        }
        kill(&mut h, "*claude stderr*<work>");
        assert!(h.core.minibuffer().is_none());
        assert!(h.core.buffers.find("*claude stderr*<work>").is_none());
        assert!(ops(&mut h).is_empty());
        // Stderr without a log goes nowhere.
        h.send(Msg::AgentStderr {
            id: AgentId(0),
            line: "lost".into(),
        });
        kill(&mut h, "*claude*<work>");
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Buffer *claude*<work> has a running process; kill it? "
        );
        assert_eq!(listed(&mut h).0, ["no", "yes"]);
        h.keys("yes RET");
        assert!(h.core.buffers.find("*claude*<work>").is_none());
        assert_eq!(ops(&mut h), ["kill 0"]);
    }

    #[test]
    fn quitting_asks_about_live_agents() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ready(&mut h, 0);
        ops(&mut h);
        h.keys("C-x C-c");
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Active processes exist; kill them and exit anyway? "
        );
        h.keys("RET");
        assert!(!h.quit && ops(&mut h).is_empty());
        h.keys("C-x C-c yes RET");
        assert!(h.quit);
        // An ended session doesn't ask.
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        exited(&mut h, 0, 1);
        h.keys("C-x C-c");
        assert!(h.quit);
    }
}
