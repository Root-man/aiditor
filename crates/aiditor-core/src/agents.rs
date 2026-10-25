//! Agent sessions: each one an ACP agent process started in a workspace, with a read-only buffer
//! for its stderr.

use agent_client_protocol_schema::v1::{NewSessionResponse, SessionNotification};

use crate::buffer::BufferId;
use crate::command::Args;
use crate::project::ProjectId;
use crate::{AgentId, AgentOp, Core, Ctx, Effect, Effects};

pub(crate) struct Session {
    /// The registry name it was started as.
    agent: String,
    /// Its stderr log, and its handle until the session buffer exists: killing it stops the agent.
    pub(crate) buffer: BufferId,
    project: ProjectId,
    /// The spawn, held while the project's environment is being captured.
    held: Option<AgentOp>,
    /// The `session/new` answer, once the agent is ready.
    pub(crate) session: Option<NewSessionResponse>,
    /// Whether the agent failed to start or exited; its log buffer stays until killed.
    ended: bool,
    /// `session/update`s in order, for the session buffer.
    pub(crate) updates: Vec<SessionNotification>,
}

impl Core {
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

    /// Whether buffer `id` is the log of a session that hasn't ended.
    pub(crate) fn live_agent(&self, id: BufferId) -> bool {
        self.agents.values().any(|s| s.buffer == id && !s.ended)
    }

    /// Stops the agents of sessions whose log buffer was killed and forgets them.
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
    /// `agent-start`: starts the named agent in the current workspace's project, with a new log
    /// buffer for its stderr.
    pub(crate) fn agent_start(&mut self, args: Args) -> Result<(), String> {
        let core = &mut *self.core;
        let name = &args.strings[0];
        let agent = (core.settings.agents.get(name))
            .ok_or_else(|| format!("{name:?} is not a registered agent"))?
            .clone();
        let workspace = core.current_workspace();
        let (project, log) = (
            workspace.project,
            format!("*{name} stderr*<{}>", workspace.name),
        );
        let loaded = &core.projects[core.project_at(project)?].project;
        // Under the captured environment, which `release_agents` puts first.
        let env = (agent.env.into_iter()).chain(loaded.env.clone()).collect();
        let cwd = loaded.primary.path.clone();
        let id = AgentId(core.next_agent);
        core.next_agent += 1;
        let buffer = core.buffers.create(&log);
        core.buffers
            .get_mut(buffer)
            .expect("just created")
            .read_only = true;
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
            project,
            held: Some(spawn),
            session: None,
            ended: false,
            updates: Vec::new(),
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
                core.log(&format!("{agent} session ready (modes: {modes})"));
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
        let buffer = (core.agents.get(&id)).and_then(|s| core.buffers.get_mut(s.buffer));
        if let Some(buffer) = buffer {
            buffer.append(&format!("{line}\n"));
        }
    }

    pub(crate) fn agent_update(&mut self, id: AgentId, update: SessionNotification) {
        if let Some(session) = self.core.agents.get_mut(&id).filter(|s| !s.ended) {
            session.updates.push(update);
        }
    }

    /// Ends the session after its agent exited, noting a crash, and releases the agent's handle.
    pub(crate) fn agent_exited(&mut self, id: AgentId, code: u32, signal: Option<String>) {
        let core = &mut *self.core;
        let Some(session) = core.agents.get_mut(&id) else {
            return;
        };
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

#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::v1::{
        ContentChunk, SessionMode, SessionModeState, SessionUpdate,
    };

    use super::*;
    use crate::Msg;
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
        // Not shown: the window stays on the file.
        assert_ne!(name(&h), "*codex stderr*<work>");
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
        let log = h.core.buffers.find("*claude stderr*<work>").unwrap();
        h.core.kill_buffer(log);
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
        let second = h.core.buffers.find("*claude stderr*<work><2>").unwrap();
        assert!(h.core.live_agent(second));
        let first = h.core.buffers.find("*claude stderr*<work>").unwrap();
        assert!(!h.core.live_agent(first) && h.core.buffers.get(first).is_some());
        h.send(Msg::AgentExited {
            id: AgentId(1),
            code: 1,
            signal: Some("Killed: 9".into()),
        });
        assert!(messages(&h).contains("claude session killed by Killed: 9\n"));
    }

    #[test]
    fn updates_are_kept_without_changing_buffers() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ready(&mut h, 0);
        let before = log_text(&h, "*claude stderr*<work>");
        let chunk = SessionUpdate::AgentMessageChunk(ContentChunk::new("hi".into()));
        let update = SessionNotification::new("s", chunk);
        h.send(Msg::AgentUpdate {
            id: AgentId(0),
            update: Box::new(update.clone()),
        });
        // Unknown agents' updates are dropped.
        h.send(Msg::AgentUpdate {
            id: AgentId(9),
            update: Box::new(update.clone()),
        });
        assert_eq!(h.core.agents[&AgentId(0)].updates, [update]);
        assert_eq!(log_text(&h, "*claude stderr*<work>"), before);
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
    fn killing_the_log_buffer_asks_first() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        h.call(
            "kill-buffer",
            Args {
                strings: vec!["*claude stderr*<work>".into()],
                ..Args::default()
            },
        );
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Buffer *claude stderr*<work> has a running process; kill it? "
        );
        assert_eq!(listed(&mut h).0, ["no", "yes"]);
        h.keys("yes RET");
        assert!(h.core.buffers.find("*claude stderr*<work>").is_none());
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
