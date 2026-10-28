//! Agent sessions: each one an ACP agent process started in a workspace, with a session buffer,
//! comint-style, and a read-only buffer for its stderr.

use agent_client_protocol_schema::v1::{
    AgentRequest, ClientResponse, ContentBlock, Error, NewSessionResponse, PermissionOption,
    PermissionOptionKind, Plan, PlanEntry, PromptRequest, RequestId, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionConfigId, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelect, SessionConfigSelectOptions, SessionConfigValueId, SessionNotification,
    SessionUpdate, SetSessionConfigOptionRequest, StopReason, ToolCall, ToolCallContent,
    ToolCallId, ToolCallUpdate, ToolCallUpdateFields,
};
use alacritty_terminal::vte::ansi::{Attr, Color, Handler, Processor};
use serde_json::{Map, Value};

use std::collections::HashMap;
use std::ops::Range;

use crate::buffer::{self, Buffer, BufferId, Decoration, Face, InsertionType, MarkerId, Prop};
use crate::command::Args;
use crate::markdown;
use crate::project::ProjectId;
use crate::settings::Theme;
use crate::{AgentId, AgentOp, Core, Ctx, Effect, Effects, Request};

/// The prompt line's text, just before the prompt area.
const PROMPT: &str = "> ";
/// The session buffer's major mode.
pub(crate) const MODE: &str = "agent-session-mode";

pub(crate) struct Session {
    /// The registry name it was started as.
    agent: String,
    /// The workspace name it was started in; workspace renames don't change it.
    workspace: String,
    /// The session buffer, the session's handle: killing it stops the agent.
    pub(crate) buffer: BufferId,
    /// Its stderr log.
    log: BufferId,
    project: ProjectId,
    /// The spawn, held while the project's environment is being captured.
    held: Option<AgentOp>,
    /// The `session/new` answer, once the agent is ready, its config options the latest reported.
    pub(crate) session: Option<NewSessionResponse>,
    /// The mode value last sent with `session/set_config_option`, for a refusal's note.
    mode_sent: Option<String>,
    /// Whether the agent failed to start or exited; its buffers stay until killed.
    ended: bool,
    /// `session/update`s in order, for the session buffer.
    // shortcut: tool calls' raw input and output are kept here and merged in `tools`, so twice; drop rendered updates when sessions get long.
    pub(crate) updates: Vec<SessionNotification>,
    /// A `session/prompt` waits for its answer.
    turn: bool,
    /// The last block above the prompt line, while an agent message or thought streams into it.
    stream: Option<Stream>,
    /// The tool calls shown, by id.
    tools: HashMap<ToolCallId, Tool>,
    /// The plan's block: where it starts and its length in chars, the blank line after it not
    /// counted.
    plan: Option<(MarkerId, usize)>,
    /// Permission requests waiting for an answer, oldest first.
    asks: Vec<Ask>,
}

fn session_name(agent: &str, workspace: &str, title: Option<&str>) -> String {
    let title = title
        .unwrap_or_default()
        .lines()
        .next()
        .unwrap_or_default()
        .trim();
    if title.is_empty() {
        return format!("*{agent}*<{workspace}>");
    }
    let mut chars = title.chars();
    let mut title: String = chars.by_ref().take(40).collect();
    if chars.next().is_some() {
        title.push('…');
    }
    format!("*{agent}: {title}*<{workspace}>")
}

/// The session's mode: the first config option of category `mode`, when it is a select.
fn mode_option(
    options: &[SessionConfigOption],
) -> Option<(&SessionConfigId, &SessionConfigSelect)> {
    let option =
        (options.iter()).find(|o| o.category == Some(SessionConfigOptionCategory::Mode))?;
    match &option.kind {
        SessionConfigKind::Select(select) => Some((&option.id, select)),
        _ => None,
    }
}

/// The mode option's current value, for the mode line.
fn mode(options: &[SessionConfigOption]) -> Option<String> {
    mode_option(options).map(|(_, select)| select.current_value.to_string())
}

/// A select's values, its groups flattened.
fn values(select: &SessionConfigSelect) -> Vec<&SessionConfigValueId> {
    match &select.options {
        SessionConfigSelectOptions::Ungrouped(options) => {
            options.iter().map(|o| &o.value).collect()
        }
        SessionConfigSelectOptions::Grouped(groups) => (groups.iter())
            .flat_map(|g| &g.options)
            .map(|o| &o.value)
            .collect(),
        _ => Vec::new(),
    }
}

/// A tool call's block: a header line, then its content when it has any.
struct Tool {
    /// The call, with every update merged in.
    call: ToolCall,
    /// Where the block starts.
    start: MarkerId,
    /// The header's and the whole block's lengths in chars, the blank line after it not counted.
    header: usize,
    len: usize,
    /// Whether its content shows.
    open: bool,
    /// The terminal it announced, whose output it shows.
    term: Option<Term>,
    /// Its terminal changed since the block was last rendered.
    dirty: bool,
    /// The faces of the SGR escapes in its content, from the content's start.
    faces: Faces,
}

/// Faces on char ranges.
type Faces = Vec<(Range<usize>, Face)>;

/// A command's terminal, from the `_meta` extension (#100).
struct Term {
    id: String,
    /// Every chunk, raw.
    output: String,
    /// `exit <code>` or `signal <name>` once it failed.
    exit: Option<String>,
}

/// The terminal a call announces: its `_meta.terminal_info`, else its first terminal item.
fn terminal_id(call: &ToolCall) -> Option<String> {
    let info = (call.meta.as_ref()).and_then(|m| m.get("terminal_info")?["terminal_id"].as_str());
    let item = call.content.iter().find_map(|item| match item {
        ToolCallContent::Terminal(t) => Some(&*t.terminal_id.0),
        _ => None,
    });
    info.or(item).map(Into::into)
}

/// The shown call whose terminal `value`'s `terminal_id` names.
fn terminal_tool<'a>(
    tools: &'a mut HashMap<ToolCallId, Tool>,
    value: &Value,
) -> Option<&'a mut Tool> {
    let id = value["terminal_id"].as_str()?;
    (tools.values_mut()).find(|t| t.term.as_ref().is_some_and(|t| t.id == id))
}

/// A permission request's block, waiting for its answer.
struct Ask {
    request: RequestId,
    /// The tool call's title, for the block and its answer line.
    title: String,
    options: Vec<PermissionOption>,
    /// Where the block starts and its length in chars, the blank line after it not counted.
    start: MarkerId,
    len: usize,
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

    /// Merges a `tool_call_update` into its call, or starts a call from one with a title.
    fn tool_update(&mut self, id: AgentId, update: ToolCallUpdate) {
        let known = (self.agents.get(&id))
            .and_then(|s| s.tools.get(&update.tool_call_id))
            .map(|t| t.call.clone());
        let call = match known {
            Some(mut call) => {
                call.update(update.fields);
                call
            }
            None => match ToolCall::try_from(update) {
                Ok(call) => call,
                Err(_) => return,
            },
        };
        self.show_tool(id, call);
    }

    /// Puts `call` above session `id`'s prompt line as a new block, or rewrites its block in
    /// place, keeping whether it is folded.
    fn show_tool(&mut self, id: AgentId, call: ToolCall) {
        let known =
            (self.agents.get(&id)).is_some_and(|s| s.tools.contains_key(&call.tool_call_id));
        if !known {
            self.end_stream(id);
        }
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let term = (session.tools.get_mut(&call.tool_call_id))
            .and_then(|t| t.term.take())
            .or_else(|| {
                terminal_id(&call).map(|id| Term {
                    id,
                    output: String::new(),
                    exit: None,
                })
            });
        let header = tool_header(&call, term.as_ref()).0;
        let (content, faces) = tool_content(&call, term.as_ref(), &self.settings.theme);
        let text = match content.is_empty() {
            true => header.clone(),
            false => format!("{header}\n{content}"),
        };
        let (header, len) = (header.chars().count(), text.chars().count());
        let tool = match session.tools.get_mut(&call.tool_call_id) {
            Some(tool) => {
                let start = buffer.marker(tool.start).expect("live while shown");
                rewrite(buffer, start..start + tool.len, &text);
                (tool.call, tool.header, tool.len) = (call, header, len);
                (tool.term, tool.dirty, tool.faces) = (term, false, faces);
                tool
            }
            None => {
                let end = buffer.prompt_start() - PROMPT.len();
                buffer.insert_output(end, &format!("{text}\n\n"));
                let tool = Tool {
                    start: (buffer.add_marker(end, InsertionType::Stay)).expect("in the text"),
                    call,
                    header,
                    len,
                    open: false,
                    term,
                    dirty: false,
                    faces,
                };
                session
                    .tools
                    .entry(tool.call.tool_call_id.clone())
                    .or_insert(tool)
            }
        };
        render_tool(buffer, tool, &self.settings.theme);
    }

    /// Appends the command output in `meta` to the terminals it names and records their exit,
    /// for their calls to show with the next frame; drops what names no shown call's terminal.
    fn terminal_meta(&mut self, id: AgentId, meta: Option<&Map<String, Value>>) {
        let (Some(session), Some(meta)) = (self.agents.get_mut(&id), meta) else {
            return;
        };
        for key in ["terminal_output", "terminal_output_delta"] {
            let Some(value) = meta.get(key) else {
                continue;
            };
            if let (Some(tool), Some(data)) = (
                terminal_tool(&mut session.tools, value),
                value["data"].as_str(),
            ) {
                (tool.term.as_mut().expect("found by it").output).push_str(data);
                tool.dirty = true;
            }
        }
        if let Some(value) = meta.get("terminal_exit")
            && let Some(tool) = terminal_tool(&mut session.tools, value)
        {
            tool.term.as_mut().expect("found by it").exit =
                match (value["signal"].as_str(), value["exit_code"].as_i64()) {
                    (Some(signal), _) => Some(format!("signal {signal}")),
                    (None, Some(code)) if code != 0 => Some(format!("exit {code}")),
                    _ => None,
                };
            tool.dirty = true;
        }
    }

    /// Puts `plan` above session `id`'s prompt line as its plan block, or rewrites that block in
    /// place.
    fn show_plan(&mut self, id: AgentId, plan: &Plan) {
        if self.agents.get(&id).is_some_and(|s| s.plan.is_none()) {
            self.end_stream(id);
        }
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let rows = plan_rows(plan);
        let text = rows_text(&rows);
        let len = text.chars().count();
        let start = match &mut session.plan {
            Some((marker, old)) => {
                let start = buffer.marker(*marker).expect("live while shown");
                rewrite(buffer, start..start + *old, &text);
                *old = len;
                start
            }
            None => {
                let end = buffer.prompt_start() - PROMPT.len();
                buffer.insert_output(end, &format!("{text}\n\n"));
                let marker = buffer.add_marker(end, InsertionType::Stay);
                session.plan = Some((marker.expect("in the text"), len));
                end
            }
        };
        render_rows(buffer, start, &rows, &self.settings.theme);
    }

    /// Puts permission request `ask` above session `id`'s prompt line as a block of its own,
    /// pending until answered.
    fn show_ask(&mut self, id: AgentId, request: RequestId, ask: RequestPermissionRequest) {
        self.end_stream(id);
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let call = &ask.tool_call;
        let shown = session.tools.get(&call.tool_call_id).map(|t| &t.call.title);
        let title = (call.fields.title.as_ref().or(shown))
            .map_or_else(|| call.tool_call_id.to_string(), |t| first_line(t));
        let rows = ask_rows(&title, &ask.options);
        let text = rows_text(&rows);
        let end = buffer.prompt_start() - PROMPT.len();
        buffer.insert_output(end, &format!("{text}\n\n"));
        render_rows(buffer, end, &rows, &self.settings.theme);
        session.asks.push(Ask {
            request,
            title,
            options: ask.options,
            start: (buffer.add_marker(end, InsertionType::Stay)).expect("in the text"),
            len: text.chars().count(),
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

    /// Renders each streamed block and each call whose terminal changed, at most once per frame.
    pub(crate) fn render_streams(&mut self) {
        let ids: Vec<_> = self.agents.keys().copied().collect();
        for id in ids {
            self.render_stream(id);
            let dirty: Vec<_> = (self.agents[&id].tools.values())
                .filter(|t| t.dirty)
                .map(|t| t.call.clone())
                .collect();
            for call in dirty {
                self.show_tool(id, call);
            }
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

    /// Changes each colour session buffers took from the current theme's `dim`, `code_block`,
    /// `text.title`, `text.literal`, `text.uri` and ANSI colours to `theme`'s, keeping colours the
    /// current theme lacks.
    pub(crate) fn retheme_sessions(&mut self, theme: &Theme) {
        let old = &self.settings.theme;
        let syntax = ["text.title", "text.literal", "text.uri"].map(|name| {
            let color = |t: &Theme| crate::syntax::color(&t.syntax, name);
            (color(old), color(theme))
        });
        let pairs: Vec<(u32, u32)> = [(old.dim, theme.dim), (old.code_block, theme.code_block)]
            .map(|(o, n)| (Some(o), Some(n)))
            .into_iter()
            .chain(syntax)
            .chain((old.ansi.into_iter().zip(theme.ansi)).map(|(o, n)| (Some(o), Some(n))))
            .filter_map(|pair| match pair {
                (Some(o), Some(n)) if o != n => Some((o, n)),
                _ => None,
            })
            .collect();
        if pairs.is_empty() {
            return;
        }
        // shortcut: two names sharing an old colour but given different new ones both take the first's; store theme names in faces if themes ever collide.
        let map = |c| pairs.iter().find(|&&(o, _)| o == c).map_or(c, |&(_, n)| n);
        for session in self.agents.values_mut() {
            if let Some(b) = self.buffers.get_mut(session.buffer) {
                b.map_colors(map);
            }
            // A call re-renders its faces when it folds and opens.
            for (_, face) in session.tools.values_mut().flat_map(|t| &mut t.faces) {
                (face.fg, face.bg) = (face.fg.map(map), face.bg.map(map));
            }
        }
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
        let (project, log, title, workspace) = (
            workspace.project,
            format!("*{name} stderr*<{}>", workspace.name),
            session_name(name, &workspace.name, None),
            workspace.name.clone(),
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
            workspace,
            buffer,
            log,
            project,
            held: Some(spawn),
            session: None,
            mode_sent: None,
            ended: false,
            updates: Vec::new(),
            turn: false,
            stream: None,
            tools: HashMap::new(),
            plan: None,
            asks: Vec::new(),
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
                let options = response.config_options.as_deref().unwrap_or_default();
                let option = mode_option(options);
                let values = option.map(|(_, select)| values(select)).unwrap_or_default();
                let modes = match values.is_empty() {
                    true => "none".into(),
                    false => (values.iter().map(|v| v.to_string()))
                        .collect::<Vec<_>>()
                        .join(", "),
                };
                if let Some(b) = core.buffers.get_mut(session.buffer) {
                    b.agent_mode = mode(options);
                }
                let default =
                    (core.settings.agents.get(&agent)).and_then(|a| a.default_mode.clone());
                let (mut set, mut note) = (None, None);
                match (default, option) {
                    (Some(default), Some((config, select)))
                        if values.iter().any(|v| *v.0 == *default) =>
                    {
                        if *select.current_value.0 != *default {
                            let request = SetSessionConfigOptionRequest::new(
                                response.session_id.clone(),
                                config.clone(),
                                default.as_str(),
                            );
                            set = Some(AgentOp::SetConfigOption { id, request });
                            session.mode_sent = Some(default.clone());
                        }
                    }
                    (Some(default), _) => note = Some(format!("{agent} has no mode {default}")),
                    (None, _) => {}
                }
                session.session = Some(response);
                core.echo(format!("{agent} session ready (modes: {modes})"));
                if let Some(note) = note {
                    core.log(&note);
                }
                self.effects.extend(set.map(Effect::Agent));
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

    /// Keeps the update, puts a text message or thought chunk, a tool call or the plan above the
    /// prompt line, and follows mode changes.
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
        let tool = match &update.update {
            SessionUpdate::ToolCall(call) => Some(Ok(call.clone())),
            SessionUpdate::ToolCallUpdate(update) => Some(Err(update.clone())),
            _ => None,
        };
        let meta = match &update.update {
            SessionUpdate::ToolCall(call) => call.meta.clone(),
            SessionUpdate::ToolCallUpdate(update) => update.meta.clone(),
            _ => None,
        };
        let plan = match &update.update {
            SessionUpdate::Plan(plan) => Some(plan.clone()),
            _ => None,
        };
        if let SessionUpdate::ConfigOptionUpdate(options) = &update.update {
            if let Some(b) = core.buffers.get_mut(session.buffer) {
                b.agent_mode = mode(&options.config_options);
            }
            if let Some(ready) = &mut session.session {
                ready.config_options = Some(options.config_options.clone());
            }
        }
        if let SessionUpdate::SessionInfoUpdate(info) = &update.update
            && let Some(title) = info.title.as_opt_deref()
        {
            let name = session_name(&session.agent, &session.workspace, title);
            core.buffers.rename(session.buffer, &name);
        }
        session.updates.push(update);
        if let Some(plan) = plan {
            core.show_plan(id, &plan);
        }
        if let Some((text, thought)) = text {
            core.output(id, &text, Some(thought));
        }
        match tool {
            // A `tool_call` replaces the call with its id.
            Some(Ok(call)) => core.show_tool(id, call),
            // An update carrying only terminal `_meta` waits for the frame.
            Some(Err(update)) if update.fields != ToolCallUpdateFields::default() => {
                core.tool_update(id, update)
            }
            _ => {}
        }
        core.terminal_meta(id, meta.as_ref());
    }

    /// Shows a permission request from a ready session and cancels one from an ended or killed
    /// session; refuses it before the session is ready, and every other request.
    pub(crate) fn agent_request(&mut self, id: AgentId, request: RequestId, body: Request) {
        let core = &mut *self.core;
        let session = core.agents.get(&id);
        let ready = session.is_some_and(|s| !s.ended && s.session.is_some());
        // An unknown id is a session forgotten when its buffer was killed.
        let gone = session.is_none_or(|s| s.ended);
        let result = match *body.0 {
            AgentRequest::RequestPermissionRequest(ask) if ready => {
                return core.show_ask(id, request, ask);
            }
            AgentRequest::RequestPermissionRequest(_) if gone => {
                Ok(ClientResponse::RequestPermissionResponse(
                    RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled),
                ))
            }
            _ => Err(Error::method_not_found()),
        };
        (self.effects).push(Effect::Agent(AgentOp::Reply {
            id,
            request,
            result,
        }));
    }

    /// Answers session `id`'s pending request `at` with its option `option`, or `cancelled`,
    /// leaving its answer line in place of its block.
    fn answer_ask(&mut self, id: AgentId, at: usize, option: Option<usize>) {
        let core = &mut *self.core;
        let session = core
            .agents
            .get_mut(&id)
            .expect("a pending request's session");
        let ask = session.asks.remove(at);
        let (outcome, line) = match option.map(|i| &ask.options[i]) {
            Some(o) => (
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    o.option_id.clone(),
                )),
                format!(
                    "Chose {} ({}) for {}",
                    first_line(&o.name),
                    acp_name(o.kind),
                    ask.title
                ),
            ),
            None => (RequestPermissionOutcome::Cancelled, cancelled(&ask)),
        };
        if let Some(b) = core.buffers.get_mut(session.buffer) {
            settle(b, &ask, &line);
        }
        let response = RequestPermissionResponse::new(outcome);
        self.effects.push(Effect::Agent(AgentOp::Reply {
            id,
            request: ask.request,
            result: Ok(ClientResponse::RequestPermissionResponse(response)),
        }));
    }

    /// Answers the current session's oldest pending request with its first option of `kind`.
    fn answer_oldest(&mut self, kind: PermissionOptionKind) -> Result<(), String> {
        let (id, session) = self.core.current_session()?;
        let ask = session
            .asks
            .first()
            .ok_or("No permission request is pending")?;
        let option = (ask.options.iter().position(|o| o.kind == kind))
            .ok_or_else(|| format!("No {} option for {}", acp_name(kind), ask.title))?;
        self.answer_ask(id, 0, Some(option));
        Ok(())
    }

    /// `agent-allow-once`: answers the oldest pending permission request with its first
    /// `allow_once` option.
    pub(crate) fn agent_allow_once(&mut self) -> Result<(), String> {
        self.answer_oldest(PermissionOptionKind::AllowOnce)
    }

    /// `agent-reject-once`: answers the oldest pending permission request with its first
    /// `reject_once` option.
    pub(crate) fn agent_reject_once(&mut self) -> Result<(), String> {
        self.answer_oldest(PermissionOptionKind::RejectOnce)
    }

    /// The session, pending request and option whose option line point is on.
    fn option_at_point(&self) -> Option<(AgentId, usize, usize)> {
        let core = &*self.core;
        let (point, buffer) = (core.point(), core.windows.window.buffer());
        let b = core.buffers.get(buffer)?;
        let (&id, session) = core.agents.iter().find(|(_, s)| s.buffer == buffer)?;
        session.asks.iter().enumerate().find_map(|(at, ask)| {
            let start = b.marker(ask.start)?;
            if !(start..=start + ask.len).contains(&point) {
                return None;
            }
            let line = b.text().slice(start..point).chars().filter(|&c| c == '\n');
            line.count().checked_sub(1).map(|option| (id, at, option))
        })
    }

    /// `agent-send-input`: on a pending permission request's option line, answers it with that
    /// option; elsewhere sends the prompt area's text as a `session/prompt`, leaving it above a
    /// new prompt line.
    pub(crate) fn agent_send_input(&mut self) -> Result<(), String> {
        if let Some((id, at, option)) = self.option_at_point() {
            self.answer_ask(id, at, Some(option));
            return Ok(());
        }
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

    /// `agent-toggle-tool-call`: shows or hides the content of the tool call whose header point
    /// is on, or hides that of the open call point is in, moving point to its header.
    pub(crate) fn agent_toggle_tool_call(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let point = core.point();
        let buffer = core.windows.window.buffer();
        let session = (core.agents.values_mut()).find(|s| s.buffer == buffer);
        let b = core
            .buffers
            .get_mut(buffer)
            .expect("current buffer is live");
        let tool = (session.into_iter().flat_map(|s| s.tools.values_mut()))
            .find(|t| {
                let end = if t.open { t.len } else { t.header };
                (b.marker(t.start)).is_some_and(|s| (s..=s + end).contains(&point))
            })
            .ok_or("No tool call here")?;
        // Without content there is nothing to show, so it stays folded for when content comes.
        tool.open = !tool.open && tool.len > tool.header;
        let start = b.marker(tool.start).expect("live while shown");
        b.remove_properties(start..start + tool.len);
        render_tool(b, tool, &core.settings.theme);
        if point > start + tool.header {
            return self.goto(start);
        }
        Ok(())
    }

    /// `agent-cancel`: sends `session/cancel` while a turn runs, then answers the pending
    /// permission requests `cancelled`, as ACP requires.
    pub(crate) fn agent_cancel(&mut self) -> Result<(), String> {
        let (id, session) = self.core.current_session()?;
        let ready = session.session.as_ref().filter(|_| session.turn);
        let asks = session.asks.len();
        let session = ready.ok_or("No turn is running")?.session_id.clone();
        (self.effects).push(Effect::Agent(AgentOp::Cancel { id, session }));
        for _ in 0..asks {
            self.answer_ask(id, 0, None);
        }
        Ok(())
    }

    /// `agent-cycle-mode`: sets the session's mode option to its next value, wrapping around.
    pub(crate) fn agent_cycle_mode(&mut self) -> Result<(), String> {
        let (id, session) = self.core.current_session()?;
        let agent = &session.agent;
        let ready = match (&session.session, session.ended) {
            (_, true) => Err(format!("{agent} session has ended")),
            (None, _) => Err(format!("{agent} is not ready")),
            (Some(ready), _) => Ok(ready),
        }?;
        let options = ready.config_options.as_deref().unwrap_or_default();
        let no_modes = || format!("{agent} has no modes");
        let (config, select) = mode_option(options).ok_or_else(no_modes)?;
        let values = values(select);
        // shortcut: cycles from the reported mode, so presses before the answer send the same
        // value and a refusal names the last one sent; track values in flight if a slow agent
        // shows it.
        let at = values.iter().position(|v| **v == select.current_value);
        let next =
            (values.get(at.map_or(0, |i| i + 1) % values.len().max(1))).ok_or_else(no_modes)?;
        let request = SetSessionConfigOptionRequest::new(
            ready.session_id.clone(),
            config.clone(),
            (*next).clone(),
        );
        session.mode_sent = Some(next.to_string());
        (self.effects).push(Effect::Agent(AgentOp::SetConfigOption { id, request }));
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

    /// Shows the mode the agent's answer to `session/set_config_option` reports, or notes why it
    /// refused.
    pub(crate) fn agent_configured(
        &mut self,
        id: AgentId,
        result: Result<Vec<SessionConfigOption>, String>,
    ) {
        let core = &mut *self.core;
        let Some(session) = core.agents.get_mut(&id).filter(|s| !s.ended) else {
            return;
        };
        match result {
            Ok(options) => {
                if let Some(b) = core.buffers.get_mut(session.buffer) {
                    b.agent_mode = mode(&options);
                }
                if let Some(ready) = &mut session.session {
                    ready.config_options = Some(options);
                }
            }
            Err(err) => {
                let (agent, value) = (&session.agent, session.mode_sent.as_deref());
                let note = format!("{agent} could not set mode {}: {err}", value.unwrap_or(""));
                core.log(&note);
            }
        }
    }

    /// Ends the session after its agent exited, noting a crash, and releases the agent's handle.
    pub(crate) fn agent_exited(&mut self, id: AgentId, code: u32, signal: Option<String>) {
        let core = &mut *self.core;
        let Some(session) = core.agents.get_mut(&id) else {
            return;
        };
        session.turn = false;
        // The agent is gone with its requests, so only the blocks change.
        if let Some(b) = core.buffers.get_mut(session.buffer) {
            for ask in std::mem::take(&mut session.asks) {
                settle(b, &ask, &cancelled(&ask));
            }
        }
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

/// Replaces `range` of `buffer`'s output with `text`, removing its properties, line by line so
/// point on an unchanged line stays.
fn rewrite(buffer: &mut Buffer, range: Range<usize>, text: &str) {
    let old = buffer.text().slice(range.clone()).to_string();
    buffer.remove_properties(range.clone());
    for (r, new) in buffer::diff(&old, text).into_iter().rev() {
        buffer.replace_output(range.start + r.start..range.start + r.end, &new);
    }
}

/// The ACP name of an enum value, as `in_progress`.
fn acp_name(value: impl serde::Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        _ => "?".into(),
    }
}

/// `text`'s first line, followed by ` …` when it has more.
fn first_line(text: &str) -> String {
    let mut line = text.lines().next().unwrap_or_default().to_string();
    if text.trim_end().contains('\n') {
        line.push_str(" …");
    }
    line
}

/// A tool call's header line, `<title> (<kind>, <status>)` with the title's first line and its
/// terminal's exit after the status, and the title part's length in chars.
fn tool_header(call: &ToolCall, term: Option<&Term>) -> (String, usize) {
    let title = first_line(&call.title);
    let (kind, status) = (acp_name(call.kind), acp_name(call.status));
    let suffix = match term.and_then(|t| t.exit.as_ref()) {
        Some(exit) => format!(" ({kind}, {status}, {exit})"),
        None => format!(" ({kind}, {status})"),
    };
    (format!("{title}{suffix}"), title.chars().count())
}

/// A plan entry's line, `<content> (<status>, <priority>)` with the content's first line, and
/// the content part's length in chars.
fn plan_line(entry: &PlanEntry) -> (String, usize) {
    let content = first_line(&entry.content);
    let suffix = format!(
        " ({}, {})",
        acp_name(&entry.status),
        acp_name(&entry.priority)
    );
    (format!("{content}{suffix}"), content.chars().count())
}

/// The plan block's rows: `Plan`, then a line per entry.
fn plan_rows(plan: &Plan) -> Vec<(String, usize)> {
    let rows = plan.entries.iter().map(plan_line);
    std::iter::once(("Plan".to_string(), 4))
        .chain(rows)
        .collect()
}

/// A permission request's rows: `Permission: <title>`, then a numbered line per option,
/// `<n>. <name> (<kind>)`.
fn ask_rows(title: &str, options: &[PermissionOption]) -> Vec<(String, usize)> {
    let header = format!("Permission: {title}");
    let rows = options.iter().enumerate().map(|(i, o)| {
        let name = format!("{}. {}", i + 1, first_line(&o.name));
        let len = name.chars().count();
        (format!("{name} ({})", acp_name(o.kind)), len)
    });
    let len = header.chars().count();
    std::iter::once((header, len)).chain(rows).collect()
}

/// A block's text from its rows, a line each.
fn rows_text(rows: &[(String, usize)]) -> String {
    let lines: Vec<_> = rows.iter().map(|(text, _)| text.as_str()).collect();
    lines.join("\n")
}

/// Renders the block of `rows` at `start`, whose properties were removed: a rule beside each
/// line and each row's text past its length before the suffix dim.
fn render_rows(buffer: &mut Buffer, start: usize, rows: &[(String, usize)], theme: &Theme) {
    let rule = Prop::Line(Decoration::Rule(theme.dim));
    let dim = Prop::Face(Face {
        fg: Some(theme.dim),
        ..Face::default()
    });
    let mut props = Vec::new();
    let mut line = 0;
    for (text, content) in rows {
        let len = text.chars().count();
        props.push((line..line + 1, rule.clone()));
        if *content < len {
            props.push((line + content..line + len, dim.clone()));
        }
        line += len + 1;
    }
    for (r, prop) in props {
        (buffer.add_property(start + r.start..start + r.end, prop)).expect("inside the block");
    }
}

/// The line a permission request's block becomes when cancelled or dropped.
fn cancelled(ask: &Ask) -> String {
    format!("Cancelled permission for {}", ask.title)
}

/// Replaces `ask`'s block with `line`, which stays plain.
fn settle(buffer: &mut Buffer, ask: &Ask, line: &str) {
    let start = buffer.marker(ask.start).expect("live while pending");
    rewrite(buffer, start..start + ask.len, line);
    buffer.remove_marker(ask.start);
}

/// Everything a tool call carries, uncut, as markdown: its raw input, its content items without
/// ANSI escapes, and its terminal's output, or its raw output when no item carries the result;
/// and the faces of the escapes.
fn tool_content(call: &ToolCall, term: Option<&Term>, theme: &Theme) -> (String, Faces) {
    let pretty = |v: &Value| serde_json::to_string_pretty(v).unwrap_or_default();
    let (mut content, mut faces) = (String::new(), Vec::new());
    // Appends a part whose faces start `at` chars into it.
    let mut push = |part: String, part_faces: Faces, at: usize| {
        if !content.is_empty() {
            content.push_str("\n\n");
        }
        let at = content.chars().count() + at;
        faces.extend((part_faces.into_iter()).map(|(r, f)| (at + r.start..at + r.end, f)));
        content.push_str(&part);
    };
    if let Some(input) = &call.raw_input {
        push(
            format!("Input:\n{}", fenced("json", &pretty(input))),
            vec![],
            0,
        );
    }
    for item in &call.content {
        if let (ToolCallContent::Terminal(t), Some(term)) = (item, term)
            && *t.terminal_id.0 == *term.id
        {
            continue;
        }
        let (part, part_faces) = match item {
            ToolCallContent::Content(c) => match &c.content {
                // Codex sends a read or list command's output here, colored.
                ContentBlock::Text(text) => coloured(&text.text, theme),
                ContentBlock::ResourceLink(link) => (link.uri.clone(), vec![]),
                other => match serde_json::to_value(other) {
                    Ok(v) => (format!("[{}]", v["type"].as_str().unwrap_or("?")), vec![]),
                    Err(_) => ("[?]".into(), vec![]),
                },
            },
            ToolCallContent::Diff(d) => {
                let old = d.old_text.as_deref().unwrap_or_default();
                let path = d.path.display().to_string();
                let diff = similar::TextDiff::from_lines(old, &d.new_text);
                let diff = diff.unified_diff().header(&path, &path).to_string();
                (format!("{path}\n{}", fenced("diff", &diff)), vec![])
            }
            ToolCallContent::Terminal(t) => (format!("Terminal {}", t.terminal_id), vec![]),
            _ => ("[?]".into(), vec![]),
        };
        push(part, part_faces, 0);
    }
    if let Some(term) = term {
        if !term.output.is_empty() {
            let (output, output_faces) = coloured(&term.output, theme);
            let block = fenced("", &output);
            // The faces start after `Output:` and the fence line.
            let at = "Output:\n".len() + block.find('\n').expect("fenced") + 1;
            push(format!("Output:\n{block}"), output_faces, at);
        }
    } else if let (true, Some(output)) = (call.content.is_empty(), &call.raw_output) {
        let text = match output {
            Value::String(text) => text.clone(),
            // Claude's adapter passes a tool result's text blocks through.
            Value::Array(items)
                if !items.is_empty() && items.iter().all(|i| i["text"].is_string()) =>
            {
                let texts: Vec<_> = items.iter().filter_map(|i| i["text"].as_str()).collect();
                texts.join("\n")
            }
            other => pretty(other),
        };
        push(format!("Output:\n{}", fenced("", &text)), vec![], 0);
    }
    (content.trim_end_matches('\n').to_string(), faces)
}

/// `text` without ANSI escape sequences or carriage returns, and the faces its SGR escapes give
/// its chars: the theme's 16 colours, bold and underline.
// shortcut: `\r` is dropped, not taken as a return to the line's start; overwrite the line if progress output shows garbled.
fn coloured(text: &str, theme: &Theme) -> (String, Faces) {
    struct Sgr<'a> {
        text: String,
        len: usize,
        face: Face,
        faces: Faces,
        ansi: &'a [u32; 16],
    }
    impl Handler for Sgr<'_> {
        fn input(&mut self, c: char) {
            self.text.push(c);
            if self.face != Face::default() {
                match self.faces.last_mut() {
                    Some((r, f)) if r.end == self.len && *f == self.face => r.end += 1,
                    _ => self.faces.push((self.len..self.len + 1, self.face)),
                }
            }
            self.len += 1;
        }
        fn put_tab(&mut self, count: u16) {
            (0..count).for_each(|_| self.input('\t'));
        }
        // A face never runs across a line's start, where a code line's decoration begins.
        fn linefeed(&mut self) {
            self.text.push('\n');
            self.len += 1;
        }
        fn terminal_attribute(&mut self, attr: Attr) {
            let colour = |c: Color| match c {
                Color::Named(n) if (n as usize) < 16 => Some(self.ansi[n as usize]),
                Color::Indexed(i) if i < 16 => Some(self.ansi[i as usize]),
                _ => None,
            };
            match attr {
                Attr::Reset => self.face = Face::default(),
                Attr::Foreground(c) => self.face.fg = colour(c),
                Attr::Background(c) => self.face.bg = colour(c),
                Attr::Bold => self.face.bold = true,
                Attr::CancelBold | Attr::CancelBoldDim => self.face.bold = false,
                Attr::Underline => self.face.underline = true,
                Attr::CancelUnderline => self.face.underline = false,
                _ => {}
            }
        }
    }
    let mut sgr = Sgr {
        text: String::new(),
        len: 0,
        face: Face::default(),
        faces: Vec::new(),
        ansi: &theme.ansi,
    };
    <Processor>::new().advance(&mut sgr, text.as_bytes());
    (sgr.text, sgr.faces)
}

/// `text` in a fenced code block whose fence is longer than any run of backticks in it.
fn fenced(lang: &str, text: &str) -> String {
    let run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(run.max(2) + 1);
    let text = text.strip_suffix('\n').unwrap_or(text);
    format!("{fence}{lang}\n{text}\n{fence}")
}

/// Renders a tool call's block, whose properties were removed: its header with a rule beside it,
/// then its content as markdown with a rule beside each line, or folded behind ` …`. No property
/// contains another's end, so starts and ends ascend.
fn render_tool(buffer: &mut Buffer, tool: &Tool, theme: &Theme) {
    let start = buffer.marker(tool.start).expect("live while shown");
    let (header, title) = tool_header(&tool.call, tool.term.as_ref());
    let rule = Prop::Line(Decoration::Rule(theme.dim));
    let dim = Face {
        fg: Some(theme.dim),
        ..Face::default()
    };
    // A rule on one char touches only its line and sorts before the faces on it.
    let mut props = vec![(0..1, rule.clone())];
    let title: String = header.chars().take(title).collect();
    props.extend(markdown::render(&title, false, theme));
    props.push((title.chars().count()..tool.header, Prop::Face(dim)));
    let body = tool.header + 1..tool.len;
    if tool.open && !body.is_empty() {
        let content = buffer
            .text()
            .slice(start + body.start..start + body.end)
            .to_string();
        let shifted = |(r, p): (Range<usize>, Prop)| (body.start + r.start..body.start + r.end, p);
        props.extend(
            markdown::render(&content, false, theme)
                .into_iter()
                .map(shifted),
        );
        // Sorted with the markdown faces by range; where both apply, the later sorted one wins.
        let faces = tool.faces.iter().map(|(r, f)| (r.clone(), Prop::Face(*f)));
        props.extend(faces.map(shifted));
        let mut line = body.start;
        for l in content.split('\n') {
            props.push((line..line + 1, rule.clone()));
            line += l.chars().count() + 1;
        }
    } else if !body.is_empty() {
        props.push((tool.header..tool.len, Prop::Invisible(" …".into())));
    }
    props.sort_by_key(|(r, _)| (r.start, r.end));
    for (r, prop) in props {
        (buffer.add_property(start + r.start..start + r.end, prop)).expect("inside the block");
    }
}

#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::v1::{
        ConfigOptionUpdate, ContentChunk, PlanEntryPriority, PlanEntryStatus,
        SessionConfigSelectOption, SessionUpdate, ToolCallStatus, ToolCallUpdateFields, ToolKind,
    };

    use super::*;
    use crate::Msg;
    use crate::buffer::Prop;
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
                AgentOp::Reply {
                    id,
                    request,
                    result,
                } => {
                    let answer = match result {
                        Ok(ClientResponse::RequestPermissionResponse(r)) => match r.outcome {
                            RequestPermissionOutcome::Selected(s) => s.option_id.to_string(),
                            _ => "cancelled".into(),
                        },
                        Ok(other) => format!("{other:?}"),
                        Err(err) => err.message,
                    };
                    format!("reply {} {request} {answer}", id.0)
                }
                AgentOp::Prompt { id, request } => {
                    let [ContentBlock::Text(text)] = &request.prompt[..] else {
                        panic!("one text block: {request:?}");
                    };
                    format!("prompt {} {} {:?}", id.0, request.session_id, text.text)
                }
                AgentOp::SetConfigOption { id, request } => {
                    let value = request.value.as_value_id().unwrap();
                    let (session, config) = (request.session_id, request.config_id);
                    format!("set {} {session} {config}={value}", id.0)
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

    /// Config options whose `mode` select is `current`, with values `auto` and `plan`.
    fn options(current: &str) -> Vec<SessionConfigOption> {
        let values = ["auto", "plan"].map(|v| SessionConfigSelectOption::new(v, v));
        let mode = SessionConfigOption::select("mode", "Mode", current.to_owned(), values.to_vec());
        let model = SessionConfigOption::select("model", "Model", "m", values[..1].to_vec());
        vec![model, mode.category(SessionConfigOptionCategory::Mode)]
    }

    fn ready(h: &mut Harness, id: u64) {
        let response = NewSessionResponse::new("s").config_options(options("auto"));
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

    fn info(h: &mut Harness, id: u64, info: Value) {
        update(
            h,
            id,
            SessionUpdate::SessionInfoUpdate(serde_json::from_value(info).unwrap()),
        );
    }

    #[test]
    fn a_session_title_names_the_shown_buffer() {
        let mut h = session();
        info(
            &mut h,
            0,
            serde_json::json!({"title": " Fruit list \nMore"}),
        );
        assert_eq!(name(&h), "*claude: Fruit list*<work>");
    }

    #[test]
    fn each_title_renames_the_session_buffer() {
        let mut h = work();
        h.keys("M-x agent-start RET codex RET");
        ready(&mut h, 0);
        for title in [
            "Reply with a list",
            "Two-fruit markdown list",
            "Two-fruit markdown list",
        ] {
            info(&mut h, 0, serde_json::json!({"title": title}));
            assert_eq!(name(&h), format!("*codex: {title}*<work>"));
        }
    }

    #[test]
    fn long_titles_use_forty_chars_of_the_first_line() {
        let mut h = session();
        for (title, shown) in [
            (
                format!(" {} \nSecond line", "é".repeat(60)),
                format!("{}…", "é".repeat(40)),
            ),
            ("🦀".repeat(40), "🦀".repeat(40)),
        ] {
            info(&mut h, 0, serde_json::json!({"title": title}));
            assert_eq!(name(&h), format!("*claude: {shown}*<work>"));
        }
    }

    #[test]
    fn a_meta_only_update_keeps_the_session_title() {
        let mut h = session();
        info(&mut h, 0, serde_json::json!({"title": "Fruit list"}));
        info(&mut h, 0, serde_json::json!({"_meta": {"status": "idle"}}));
        assert_eq!(name(&h), "*claude: Fruit list*<work>");
    }

    #[test]
    fn null_and_empty_titles_restore_the_base_name() {
        let mut h = session();
        for title in [
            Value::Null,
            serde_json::json!(""),
            serde_json::json!(" \t\nSecond line"),
        ] {
            info(&mut h, 0, serde_json::json!({"title": "Fruit list"}));
            info(&mut h, 0, serde_json::json!({"title": title}));
            assert_eq!(name(&h), "*claude*<work>");
        }
    }

    #[test]
    fn sessions_with_the_same_title_get_unique_names() {
        let mut h = session();
        let first = h.core.agents[&AgentId(0)].buffer;
        h.keys("M-x agent-start RET claude RET");
        ready(&mut h, 1);
        info(&mut h, 0, serde_json::json!({"title": "Fruit list"}));
        info(&mut h, 1, serde_json::json!({"title": "Fruit list"}));
        assert_eq!(name(&h), "*claude: Fruit list*<work><2>");
        h.core.switch_to(first);
        assert_eq!(name(&h), "*claude: Fruit list*<work>");
    }

    #[test]
    fn titles_keep_the_workspace_name_the_session_started_with() {
        let mut h = session();
        info(
            &mut h,
            0,
            serde_json::json!({"title": "Fruit *list* <today>"}),
        );
        h.core.rename_workspace("renamed").unwrap();
        assert_eq!(name(&h), "*claude: Fruit *list* <today>*<work>");
        info(&mut h, 0, serde_json::json!({"title": "New title"}));
        assert_eq!(name(&h), "*claude: New title*<work>");
        info(&mut h, 0, serde_json::json!({"title": null}));
        assert_eq!(name(&h), "*claude*<work>");
    }

    #[test]
    fn an_ended_session_keeps_its_last_title() {
        let mut h = session();
        info(&mut h, 0, serde_json::json!({"title": "Fruit list"}));
        exited(&mut h, 0, 0);
        info(&mut h, 0, serde_json::json!({"title": "Late title"}));
        assert_eq!(name(&h), "*claude: Fruit list*<work>");
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

    /// The settings with `dim` and `code_block` changed, reloaded.
    fn retheme(h: &mut Harness) -> (u32, u32) {
        let settings = "[theme]\ndim = 0x010203\ncode_block = 0x040506";
        h.send(Msg::SettingsLoaded(Ok(Some(settings.into()))));
        (0x010203, 0x040506)
    }

    #[test]
    fn a_theme_reload_rethemes_completed_replies() {
        let mut h = session();
        h.keys("a RET");
        thought(&mut h, "hmm");
        chunk(&mut h, "see `x`");
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.core.frame();
        let (dim, code) = retheme(&mut h);
        let all = props(&h);
        let face = |text: &str| match all.iter().find(|(t, _)| t == text) {
            Some((_, Prop::Face(f))) => *f,
            other => panic!("{text}: {other:?} in {all:?}"),
        };
        assert_eq!(face("hmm").fg, Some(dim));
        assert_eq!(face("x").bg, Some(code));
    }

    #[test]
    fn a_theme_reload_rethemes_a_tool_calls_rule() {
        let mut h = session();
        tool(&mut h, ls());
        let (dim, _) = retheme(&mut h);
        let rules: Vec<_> = (props(&h).into_iter())
            .filter_map(|(_, p)| match p {
                Prop::Line(Decoration::Rule(c)) => Some(c),
                _ => None,
            })
            .collect();
        assert!(
            !rules.is_empty() && rules.iter().all(|&c| c == dim),
            "{rules:?}"
        );
    }

    #[test]
    fn an_unchanged_theme_leaves_sessions_alone() {
        let mut h = session();
        h.keys("a RET");
        chunk(&mut h, "see `x` *y*");
        h.core.frame();
        tool(&mut h, ls());
        let before = props(&h);
        h.send(Msg::SettingsLoaded(Ok(None)));
        assert_eq!(props(&h), before);
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

    fn tool(h: &mut Harness, call: ToolCall) {
        update(h, 0, SessionUpdate::ToolCall(call));
    }

    fn tool_update(h: &mut Harness, id: &'static str, fields: ToolCallUpdateFields) {
        update(
            h,
            0,
            SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(id, fields)),
        );
    }

    /// The text of each display line of a frame `rows` lines high.
    fn shown(h: &mut Harness, rows: usize) -> Vec<String> {
        let height = (rows + 1) as f32 * h.core.settings().line_height();
        let frame = h.frame(800., height);
        (frame.windows[0].display.lines.iter())
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    /// An `execute` call `t` titled `` `ls` `` with the input `{"command":"ls"}`.
    fn ls() -> ToolCall {
        ToolCall::new("t", "`ls`")
            .kind(ToolKind::Execute)
            .raw_input(serde_json::json!({"command": "ls"}))
    }

    const LS_INPUT: &str = "Input:\n```json\n{\n  \"command\": \"ls\"\n}\n```";

    #[test]
    fn a_tool_call_starts_folded_and_updates_in_place() {
        let mut h = session();
        h.keys("a RET");
        chunk(&mut h, "Let me look.");
        tool(&mut h, ls());
        assert_eq!(
            h.core.text(),
            format!("> a\n\nLet me look.\n\n`ls` (execute, pending)\n{LS_INPUT}\n\n> ")
        );
        assert_eq!(
            shown(&mut h, 10)[..5],
            ["> a", "", "Let me look.", "", "ls (execute, pending) …"]
        );
        // A message after the call, then updates: the call's block changes where it is.
        chunk(&mut h, "Done.");
        let status = |s| ToolCallUpdateFields::new().status(s);
        tool_update(&mut h, "t", status(ToolCallStatus::InProgress));
        tool_update(
            &mut h,
            "t",
            status(ToolCallStatus::Completed).raw_output(serde_json::json!("a.rs\nb.rs\n")),
        );
        assert_eq!(
            h.core.text(),
            format!(
                "> a\n\nLet me look.\n\n`ls` (execute, completed)\n{LS_INPUT}\n\n\
                 Output:\n```\na.rs\nb.rs\n```\n\nDone.\n\n> "
            )
        );
        assert_eq!(
            shown(&mut h, 10)[4..7],
            ["ls (execute, completed) …", "", "Done."]
        );
        // Props stay in order, so the display can bisect them.
        let props: Vec<_> = h.core.current().properties().map(|(r, _)| r).collect();
        assert!(
            props
                .windows(2)
                .all(|w| w[0].start <= w[1].start && w[0].end <= w[1].end)
        );
        // The header carries the rule, the suffix is dim.
        let frame = h.frame(800., 600.);
        let header = &frame.windows[0].display.lines[4];
        let dim = h.core.settings().theme.dim;
        assert_eq!(header.decorations, [Decoration::Rule(dim)]);
        let suffix = header
            .spans
            .iter()
            .find(|s| s.text.contains("execute"))
            .unwrap();
        assert_eq!(suffix.face.fg, Some(dim));
    }

    #[test]
    fn a_tool_call_without_content_is_its_header() {
        let mut h = session();
        tool(&mut h, ToolCall::new("t", "Think"));
        assert_eq!(h.core.text(), "Think (other, pending)\n\n> ");
        assert_eq!(shown(&mut h, 5)[0], "Think (other, pending)");
        // An update for an unknown call without a title shows nothing.
        let fields = ToolCallUpdateFields::new().status(ToolCallStatus::Failed);
        tool_update(&mut h, "u", fields.clone());
        assert_eq!(h.core.text(), "Think (other, pending)\n\n> ");
        // With a title it starts a block.
        tool_update(&mut h, "u", fields.title("Grep".to_string()));
        assert!(h.core.text().ends_with("Grep (other, failed)\n\n> "));
    }

    #[test]
    fn tab_on_a_header_shows_and_hides_its_content() {
        let mut h = session();
        tool(&mut h, ls());
        h.keys("TAB");
        assert_eq!(h.core.message(), Some("No tool call here"));
        h.keys("M-< C-e TAB");
        let open = [
            "ls (execute, pending)",
            "Input:",
            "",
            "{",
            "  \"command\": \"ls\"",
            "}",
            "",
            "",
            "> ",
        ];
        assert_eq!(shown(&mut h, 10)[..9], open);
        let frame = h.frame(800., 600.);
        let lines = &frame.windows[0].display.lines;
        let rule = Decoration::Rule(h.core.settings().theme.dim);
        assert!(
            (0..7).all(|i| lines[i].decorations.contains(&rule)),
            "{lines:?}"
        );
        assert!(!lines[7].decorations.contains(&rule));
        // An update keeps it open.
        let output = ToolCallUpdateFields::new().raw_output(serde_json::json!("a.rs"));
        tool_update(&mut h, "t", output);
        assert_eq!(shown(&mut h, 12)[8..12], ["Output:", "", "a.rs", ""]);
        // TAB inside the open content folds it, point going to the header.
        h.keys("C-n C-n C-n TAB");
        assert_eq!(shown(&mut h, 5)[..3], ["ls (execute, pending) …", "", "> "]);
        assert_eq!(h.core.point(), 0);
    }

    #[test]
    fn line_motion_and_scrolling_pass_a_folded_call() {
        let mut h = session();
        let output: String = (0..50).map(|i| format!("line {i}\n")).collect();
        tool(&mut h, ls().raw_output(serde_json::json!(output)));
        // The window keeps the header in view above the prompt line.
        assert_eq!(shown(&mut h, 5)[..3], ["ls (execute, pending) …", "", "> "]);
        h.keys("M-< C-n");
        assert_eq!(h.core.current().line_col(h.core.point()), Ok((61, 0)));
        h.keys("C-p");
        assert_eq!(h.core.point(), 0);
        h.keys("TAB C-n");
        assert_eq!(h.core.current().line_col(h.core.point()), Ok((1, 0)));
        // Open, all of the output shows.
        let lines = shown(&mut h, 70);
        assert!((0..50).all(|i| lines.contains(&format!("line {i}"))));
    }

    #[test]
    fn a_repeated_tool_call_replaces_it_and_point_stays_on_its_header() {
        let mut h = session();
        tool(&mut h, ls());
        h.keys("M-< C-e TAB");
        tool(&mut h, ls().status(ToolCallStatus::Completed));
        assert_eq!(h.core.text().matches("`ls`").count(), 1);
        assert_eq!(shown(&mut h, 3)[..2], ["ls (execute, completed)", "Input:"]);
        assert_eq!(h.core.current().line_col(h.core.point()).unwrap().0, 0);
        // TAB on a call without content leaves it folded.
        tool(&mut h, ToolCall::new("e", "Empty"));
        h.keys("M-> C-p C-p TAB");
        let content = ToolCallUpdateFields::new().raw_output(serde_json::json!("x"));
        tool_update(&mut h, "e", content);
        assert!(shown(&mut h, 20).contains(&"Empty (other, pending) …".to_string()));
    }

    #[test]
    fn tool_content_is_uncut_in_long_enough_fences() {
        let mut call = ToolCall::new("t", "Run\nmore");
        assert_eq!(
            tool_header(&call, None),
            ("Run … (other, pending)".into(), 5)
        );
        // A list of text blocks shows as their text; other output as JSON.
        let theme = crate::settings::Settings::default().theme;
        let content = |call: &ToolCall| tool_content(call, None, &theme).0;
        call.raw_output = Some(serde_json::json!([{"type": "text", "text": "x ``` y"}]));
        assert_eq!(content(&call), "Output:\n````\nx ``` y\n````");
        call.raw_output = Some(serde_json::json!({"code": 0}));
        assert_eq!(content(&call), "Output:\n```\n{\n  \"code\": 0\n}\n```");
        // Content items replace the raw output.
        call.content = vec![
            "**hi**\u{1b}[0m".into(),
            ToolCallContent::Diff(
                agent_client_protocol_schema::v1::Diff::new("a.rs", "b\n")
                    .old_text("a\n".to_string()),
            ),
        ];
        assert_eq!(
            content(&call),
            "**hi**\n\na.rs\n```diff\n--- a.rs\n+++ a.rs\n@@ -1 +1 @@\n-a\n+b\n```"
        );
        // A coloured text item's faces land on its chars after the parts before it.
        call.raw_input = Some(serde_json::json!(1));
        call.content = vec!["\u{1b}[34mfile\u{1b}[0m".into()];
        let blue = Face {
            fg: Some(theme.ansi[4]),
            ..Face::default()
        };
        assert_eq!(
            tool_content(&call, None, &theme),
            (
                "Input:\n```json\n1\n```\n\nfile".into(),
                vec![(22..26, blue)]
            )
        );
    }

    #[test]
    fn sgr_escapes_become_faces() {
        let theme = crate::settings::Settings::default().theme;
        let fg = |i: usize| Face {
            fg: Some(theme.ansi[i]),
            ..Face::default()
        };
        let underlined = Face {
            underline: true,
            ..fg(10)
        };
        let bold = Face {
            bold: true,
            ..underlined
        };
        assert_eq!(
            coloured("\u{1b}[1;4;92mok\u{1b}[22mx\u{1b}[0my", &theme),
            ("okxy".into(), vec![(0..2, bold), (2..3, underlined)])
        );
        // Colours beyond the 16 show in the default colour.
        let beyond = "\u{1b}[38;5;196ma\u{1b}[38;2;1;2;3mb\u{1b}[38;5;4mc";
        assert_eq!(
            coloured(beyond, &theme),
            ("abc".into(), vec![(2..3, fg(4))])
        );
        // A face stops at each line's end; `39` and `49` reset the colours.
        let on_red = Face {
            bg: Some(theme.ansi[1]),
            ..fg(2)
        };
        let bg = |i: usize| Face {
            bg: Some(theme.ansi[i]),
            ..Face::default()
        };
        assert_eq!(
            coloured("\u{1b}[48;5;4ma\u{1b}[101mb", &theme),
            ("ab".into(), vec![(0..1, bg(4)), (1..2, bg(9))])
        );
        assert_eq!(
            coloured(
                "\u{1b}[32;41ma\r\nb\u{1b}[39;49mc\u{1b}]0;title\u{7}",
                &theme
            ),
            ("a\nbc".into(), vec![(0..1, on_red), (2..3, on_red)])
        );
    }

    /// An `execute` call `t` titled `make`, announcing terminal `t` in its content and `_meta`.
    fn command() -> ToolCall {
        serde_json::from_value(serde_json::json!({
            "toolCallId": "t", "title": "make", "kind": "execute", "status": "in_progress",
            "content": [{"type": "terminal", "terminalId": "t"}],
            "_meta": {"terminal_info": {"terminal_id": "t"}},
        }))
        .unwrap()
    }

    /// Sends call `t` an update with `fields` and `meta`.
    fn meta_update(h: &mut Harness, fields: ToolCallUpdateFields, meta: Value) {
        let update = ToolCallUpdate::new("t", fields).meta(meta.as_object().cloned());
        self::update(h, 0, SessionUpdate::ToolCallUpdate(update));
    }

    fn output(h: &mut Harness, key: &str, terminal: &str, data: &str) {
        let meta = serde_json::json!({key: {"terminal_id": terminal, "data": data}});
        meta_update(h, ToolCallUpdateFields::new(), meta);
    }

    #[test]
    fn terminal_output_streams_into_its_call_once_per_frame() {
        let mut h = session();
        tool(&mut h, command());
        assert_eq!(h.core.text(), "make (execute, in_progress)\n\n> ");
        output(&mut h, "terminal_output_delta", "t", "\u{1b}[31mre");
        output(&mut h, "terminal_output", "t", "d\u{1b}[0m\r\n");
        output(&mut h, "terminal_output", "other", "lost");
        assert_eq!(h.core.text(), "make (execute, in_progress)\n\n> ");
        h.frame(800., 600.);
        assert_eq!(
            h.core.text(),
            "make (execute, in_progress)\nOutput:\n```\nred\n```\n\n> "
        );
        // Its colour shows once the call is open.
        h.keys("M-< C-e TAB");
        let red = Face {
            fg: Some(h.core.settings().theme.ansi[1]),
            ..Face::default()
        };
        assert!(props(&h).contains(&("red".into(), Prop::Face(red))));
    }

    #[test]
    fn a_coloured_text_item_shows_its_colour_when_open() {
        let mut h = session();
        let call = ToolCall::new("x", "ls").content(vec!["\u{1b}[34mfile\u{1b}[0m".into()]);
        tool(&mut h, call);
        h.keys("M-< C-e TAB");
        assert_eq!(h.core.text(), "ls (other, pending)\nfile\n\n> ");
        let blue = Face {
            fg: Some(h.core.settings().theme.ansi[4]),
            ..Face::default()
        };
        assert!(props(&h).contains(&("file".into(), Prop::Face(blue))));
    }

    #[test]
    fn a_theme_reload_rethemes_command_output() {
        let mut h = session();
        tool(&mut h, command());
        output(&mut h, "terminal_output", "t", "\u{1b}[31mred");
        h.frame(800., 600.);
        h.keys("M-< C-e TAB");
        let ansi = (1..=16)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        h.send(Msg::SettingsLoaded(Ok(Some(format!(
            "[theme]\nansi = [{ansi}]"
        )))));
        let red = Prop::Face(Face {
            fg: Some(2),
            ..Face::default()
        });
        assert!(props(&h).contains(&("red".into(), red.clone())));
        // Folding and opening the call again keeps the new colour.
        h.keys("TAB M-< C-e TAB");
        assert!(props(&h).contains(&("red".into(), red)));
    }

    #[test]
    fn a_failed_command_shows_its_exit_in_the_header() {
        let mut h = session();
        tool(&mut h, command());
        let header = |h: &mut Harness| {
            h.frame(800., 600.);
            h.core.text().lines().next().unwrap().to_string()
        };
        let exit = |code: Value, signal: Value| {
            serde_json::json!({"terminal_exit":
                {"terminal_id": "t", "exit_code": code, "signal": signal}})
        };
        let failed = ToolCallUpdateFields::new().status(ToolCallStatus::Failed);
        meta_update(&mut h, failed, exit(2.into(), Value::Null));
        assert_eq!(header(&mut h), "make (execute, failed, exit 2)");
        meta_update(
            &mut h,
            ToolCallUpdateFields::new(),
            exit(Value::Null, "SIGTERM".into()),
        );
        assert_eq!(header(&mut h), "make (execute, failed, signal SIGTERM)");
        let completed = ToolCallUpdateFields::new().status(ToolCallStatus::Completed);
        meta_update(&mut h, completed, exit(0.into(), Value::Null));
        assert_eq!(header(&mut h), "make (execute, completed)");
    }

    #[test]
    fn a_call_started_from_an_update_opens_its_terminal() {
        let mut h = session();
        let meta = serde_json::json!({"terminal_info": {"terminal_id": "t"}});
        meta_update(&mut h, ToolCallUpdateFields::new().title("make"), meta);
        output(&mut h, "terminal_output", "t", "ok\n");
        h.frame(800., 600.);
        assert_eq!(
            h.core.text(),
            "make (other, pending)\nOutput:\n```\nok\n```\n\n> "
        );
    }

    #[test]
    fn terminal_info_alone_opens_a_terminal() {
        let mut h = session();
        let mut call = command();
        call.content.clear();
        tool(&mut h, call);
        output(&mut h, "terminal_output", "t", "ok\n");
        h.frame(800., 600.);
        assert_eq!(
            h.core.text(),
            "make (execute, in_progress)\nOutput:\n```\nok\n```\n\n> "
        );
    }

    fn plan(h: &mut Harness, entries: &[(&str, PlanEntryStatus, PlanEntryPriority)]) {
        let entries = (entries.iter())
            .map(|(content, status, priority)| {
                PlanEntry::new(*content, priority.clone(), status.clone())
            })
            .collect();
        update(h, 0, SessionUpdate::Plan(Plan::new(entries)));
    }

    #[test]
    fn a_new_plan_replaces_the_old_one_in_place() {
        use PlanEntryPriority::*;
        use PlanEntryStatus::*;
        let mut h = session();
        chunk(&mut h, "Planning.");
        plan(
            &mut h,
            &[
                ("Read the code", InProgress, High),
                ("Write tests", Pending, Medium),
            ],
        );
        let first = "Plan\nRead the code (in_progress, high)\nWrite tests (pending, medium)";
        assert_eq!(h.core.text(), format!("Planning.\n\n{first}\n\n> "));
        let rule = Prop::Line(Decoration::Rule(h.core.settings().theme.dim));
        let rules = |h: &Harness| {
            (props(h).into_iter())
                .filter(|(_, p)| *p == rule)
                .map(|(t, _)| t)
                .collect::<Vec<_>>()
        };
        assert_eq!(rules(&h), ["P", "R", "W"]);
        assert!(props(&h).iter().any(|(t, _)| t == " (pending, medium)"));
        // Point on the plan's first entry stays there while a later entry changes.
        h.keys("M-< C-n C-n C-n");
        let point = h.core.point();
        chunk(&mut h, "Done.");
        plan(
            &mut h,
            &[
                ("Read the code", InProgress, High),
                ("Write tests", Completed, Medium),
                ("Ship\nit", Pending, Low),
            ],
        );
        assert_eq!(h.core.point(), point);
        let second = "Plan\nRead the code (in_progress, high)\nWrite tests (completed, medium)\nShip … (pending, low)";
        assert_eq!(
            h.core.text(),
            format!("Planning.\n\n{second}\n\nDone.\n\n> ")
        );
        assert_eq!(rules(&h), ["P", "R", "W", "S"]);
        plan(&mut h, &[]);
        assert_eq!(h.core.text(), "Planning.\n\nPlan\n\nDone.\n\n> ");
        assert_eq!(rules(&h), ["P"]);
    }

    #[test]
    fn the_mode_line_shows_the_current_mode() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        assert!(!h.display(5).mode_line.contains('['));
        ready(&mut h, 0);
        assert!(h.display(5).mode_line.contains(" [auto]"));
        let options = ConfigOptionUpdate::new(options("plan"));
        update(&mut h, 0, SessionUpdate::ConfigOptionUpdate(options));
        let line = h.display(5).mode_line;
        assert!(line.contains(" [plan]") && !line.contains("auto"), "{line}");
    }

    #[test]
    fn a_session_starts_in_the_default_mode() {
        let mut h = work();
        h.core
            .settings
            .agents
            .get_mut("claude")
            .unwrap()
            .default_mode = Some("plan".into());
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        ready(&mut h, 0);
        assert_eq!(ops(&mut h), ["set 0 s mode=plan"]);
        assert!(h.display(5).mode_line.contains(" [auto]"));
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(options("plan")),
        });
        assert!(h.display(5).mode_line.contains(" [plan]"));
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Err("no".into()),
        });
        assert!(messages(&h).contains("claude could not set mode plan: no\n"));
        assert!(h.display(5).mode_line.contains(" [plan]"));
    }

    #[test]
    fn options_without_a_mode_select_are_no_mode() {
        let mut h = work();
        h.core
            .settings
            .agents
            .get_mut("claude")
            .unwrap()
            .default_mode = Some("plan".into());
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        // A mode-category boolean is not a mode.
        let flag = SessionConfigOption::boolean("fast", "Fast", true)
            .category(SessionConfigOptionCategory::Mode);
        let mut options = options("auto");
        options.pop();
        options.push(flag);
        let response = NewSessionResponse::new("s").config_options(options.clone());
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(response),
        });
        assert!(ops(&mut h).is_empty());
        let note = "claude session ready (modes: none)\nclaude has no mode plan\n";
        assert!(messages(&h).contains(note));
        assert!(!h.display(5).mode_line.contains('['));
        // An answer or update without a mode option clears the mode.
        ready(&mut h, 0);
        assert!(h.display(5).mode_line.contains(" [auto]"));
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(options),
        });
        assert!(!h.display(5).mode_line.contains('['));
    }

    #[test]
    fn a_current_or_unknown_default_mode_sends_nothing() {
        let mut h = work();
        h.core
            .settings
            .agents
            .get_mut("claude")
            .unwrap()
            .default_mode = Some("auto".into());
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        ready(&mut h, 0);
        assert!(ops(&mut h).is_empty());
        h.core
            .settings
            .agents
            .get_mut("claude")
            .unwrap()
            .default_mode = Some("nope".into());
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        ready(&mut h, 1);
        assert!(ops(&mut h).is_empty());
        assert!(
            messages(&h)
                .contains("claude session ready (modes: auto, plan)\nclaude has no mode nope\n")
        );
        assert!(h.display(5).mode_line.contains(" [auto]"));
        assert_eq!(messages(&h).matches("has no mode").count(), 1);
    }

    #[test]
    fn s_tab_cycles_through_every_mode_and_back() {
        let mut h = session();
        let configured = |h: &mut Harness, result| {
            h.send(Msg::AgentConfigured {
                id: AgentId(0),
                result,
            });
        };
        // Mid-turn too.
        h.keys("a RET");
        ops(&mut h);
        h.keys("S-TAB");
        assert_eq!(ops(&mut h), ["set 0 s mode=plan"]);
        configured(&mut h, Ok(options("plan")));
        assert!(h.display(5).mode_line.contains(" [plan]"));
        h.keys("S-TAB");
        assert_eq!(ops(&mut h), ["set 0 s mode=auto"]);
        configured(&mut h, Ok(options("auto")));
        assert!(h.display(5).mode_line.contains(" [auto]"));
        // The agent's own change is where the next cycle starts.
        let plan = ConfigOptionUpdate::new(options("plan"));
        update(&mut h, 0, SessionUpdate::ConfigOptionUpdate(plan));
        h.keys("S-TAB");
        assert_eq!(ops(&mut h), ["set 0 s mode=auto"]);
        configured(&mut h, Err("no".into()));
        assert!(messages(&h).contains("claude could not set mode auto: no\n"));
        assert!(h.display(5).mode_line.contains(" [plan]"));
    }

    #[test]
    fn s_tab_needs_a_ready_session_with_modes() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        h.keys("S-TAB");
        assert_eq!(h.core.message(), Some("claude is not ready"));
        h.send(Msg::AgentStarted {
            id: AgentId(0),
            result: Ok(NewSessionResponse::new("s")),
        });
        h.keys("S-TAB");
        assert_eq!(h.core.message(), Some("claude has no modes"));
        assert!(ops(&mut h).is_empty());
        exited(&mut h, 0, 1);
        ops(&mut h);
        h.keys("S-TAB");
        assert_eq!(h.core.message(), Some("claude session has ended"));
        assert!(ops(&mut h).is_empty());
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
        // A request the agent sent before it died is cancelled.
        ask(&mut h, 1, "c", Some("C"));
        assert_eq!(ops(&mut h), ["reply 0 1 cancelled"]);
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

    /// Sends session 0 permission request `request` for tool call `call` (titled `title`), with
    /// options `allow` (`allow_once`), `always` (`allow_always`) and `reject` (`reject_once`).
    fn ask(h: &mut Harness, request: i64, call: &str, title: Option<&str>) {
        use PermissionOptionKind::*;
        let fields = ToolCallUpdateFields::new().title(title.map(String::from));
        let options = [
            ("allow", "Yes", AllowOnce),
            ("always", "Always", AllowAlways),
        ]
        .into_iter()
        .chain([("reject", "No", RejectOnce)])
        .map(|(id, name, kind)| PermissionOption::new(id, name, kind))
        .collect();
        let ask = RequestPermissionRequest::new(
            "s",
            ToolCallUpdate::new(call.to_string(), fields),
            options,
        );
        h.send(Msg::AgentRequest {
            id: AgentId(0),
            request: RequestId::Number(request),
            body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
        });
    }

    /// The block `ask` shows for a request titled `title`.
    fn asked(title: &str) -> String {
        format!(
            "Permission: {title}\n1. Yes (allow_once)\n2. Always (allow_always)\n3. No (reject_once)"
        )
    }

    #[test]
    fn a_permission_request_shows_inline_and_waits() {
        let mut h = session();
        h.keys("a RET");
        chunk(&mut h, "Editing.");
        ops(&mut h);
        ask(&mut h, 4, "c", Some("Edit a.txt"));
        assert!(ops(&mut h).is_empty(), "nothing answered yet");
        let block = asked("Edit a.txt");
        assert_eq!(h.core.text(), format!("> a\n\nEditing.\n\n{block}\n\n> "));
        let theme = &h.core.settings().theme;
        let rule = Prop::Line(Decoration::Rule(theme.dim));
        let dim = Prop::Face(Face {
            fg: Some(theme.dim),
            ..Face::default()
        });
        let props: Vec<_> = (props(&h).into_iter())
            .filter(|(_, p)| *p == rule || *p == dim)
            .map(|(t, _)| t)
            .collect();
        assert_eq!(
            props,
            [
                "P",
                "1",
                " (allow_once)",
                "2",
                " (allow_always)",
                "3",
                " (reject_once)"
            ]
        );
    }

    #[test]
    fn a_title_comes_from_the_request_the_shown_call_or_the_id() {
        let mut h = session();
        let call = ToolCall::new("c", "Read\nmore");
        update(&mut h, 0, SessionUpdate::ToolCall(call));
        ask(&mut h, 1, "c", None);
        ask(&mut h, 2, "d", None);
        let text = h.core.text();
        assert!(text.contains("Permission: Read …\n"), "{text}");
        assert!(text.contains("Permission: d\n"), "{text}");
    }

    #[test]
    fn the_keys_answer_the_oldest_request_first() {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        ask(&mut h, 1, "a", Some("A"));
        ask(&mut h, 2, "b", Some("B"));
        ask(&mut h, 3, "c", Some("C"));
        h.keys("C-c C-y");
        assert_eq!(ops(&mut h), ["reply 0 1 allow"]);
        h.keys("C-c C-n");
        assert_eq!(ops(&mut h), ["reply 0 2 reject"]);
        let c = asked("C");
        assert_eq!(
            h.core.text(),
            format!(
                "> a\n\nChose Yes (allow_once) for A\n\nChose No (reject_once) for B\n\n{c}\n\n> "
            )
        );
        h.keys("C-c C-y");
        assert_eq!(ops(&mut h), ["reply 0 3 allow"]);
        h.keys("C-c C-y");
        assert_eq!(h.core.message(), Some("No permission request is pending"));
        assert!(ops(&mut h).is_empty());
    }

    #[test]
    fn ret_on_an_option_line_picks_it() {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        ask(&mut h, 1, "a", Some("A"));
        ask(&mut h, 2, "b", Some("B"));
        // The newer request's second option, then the older one's header, which sends nothing.
        h.keys("C-p C-p C-p RET");
        assert_eq!(ops(&mut h), ["reply 0 2 always"]);
        assert!(
            h.core
                .text()
                .contains("Chose Always (allow_always) for B\n")
        );
        h.keys("M-< C-n C-n RET");
        assert_eq!(h.core.message(), Some("claude is busy"));
        h.keys("C-n C-n C-n RET");
        assert_eq!(ops(&mut h), ["reply 0 1 reject"]);
        assert_eq!(
            h.core.text(),
            "> a\n\nChose No (reject_once) for A\n\nChose Always (allow_always) for B\n\n> "
        );
    }

    #[test]
    fn a_missing_option_kind_answers_nothing() {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        let options = vec![PermissionOption::new(
            "always",
            "Always",
            PermissionOptionKind::AllowAlways,
        )];
        let ask = RequestPermissionRequest::new(
            "s",
            ToolCallUpdate::new("c", ToolCallUpdateFields::new().title("Run".to_string())),
            options,
        );
        h.send(Msg::AgentRequest {
            id: AgentId(0),
            request: RequestId::Number(1),
            body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
        });
        h.keys("C-c C-y");
        assert_eq!(h.core.message(), Some("No allow_once option for Run"));
        h.keys("C-c C-n");
        assert_eq!(h.core.message(), Some("No reject_once option for Run"));
        assert!(ops(&mut h).is_empty());
    }

    #[test]
    fn cancelling_answers_pending_requests_and_ends_the_turn_and_an_exit_drops_them() {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        ask(&mut h, 1, "a", Some("A"));
        ask(&mut h, 2, "b", Some("B"));
        h.keys("C-c C-c");
        assert_eq!(
            ops(&mut h),
            ["cancel 0 s", "reply 0 1 cancelled", "reply 0 2 cancelled"]
        );
        let cancelled = "Cancelled permission for A\n\nCancelled permission for B";
        assert_eq!(h.core.text(), format!("> a\n\n{cancelled}\n\n> "));
        prompted(&mut h, Ok(StopReason::Cancelled));
        let ended = format!("> a\n\n{cancelled}\n\nTurn ended: cancelled\n\n> ");
        assert_eq!(h.core.text(), ended);
        h.keys("b RET");
        ops(&mut h);
        ask(&mut h, 3, "c", Some("C"));
        exited(&mut h, 0, 1);
        assert_eq!(ops(&mut h), ["kill 0"]);
        assert!(h.core.text().ends_with("Cancelled permission for C\n\n> "));
        h.keys("C-c C-y");
        assert_eq!(h.core.message(), Some("No permission request is pending"));
        // A request after the exit is cancelled and not shown.
        let text = h.core.text();
        ask(&mut h, 4, "d", Some("D"));
        assert_eq!(ops(&mut h), ["reply 0 4 cancelled"]);
        assert_eq!(h.core.text(), text);
    }

    #[test]
    fn other_requests_and_unready_sessions_are_refused_and_ended_ones_cancelled() {
        use agent_client_protocol_schema::v1::CreateTerminalRequest;
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        ask(&mut h, 1, "c", Some("Early"));
        assert_eq!(ops(&mut h), ["reply 0 1 Method not found"]);
        ready(&mut h, 0);
        ops(&mut h);
        let terminal = CreateTerminalRequest::new("s", "ls");
        h.send(Msg::AgentRequest {
            id: AgentId(0),
            request: RequestId::Number(2),
            body: Request(Box::new(AgentRequest::CreateTerminalRequest(terminal))),
        });
        assert_eq!(ops(&mut h), ["reply 0 2 Method not found"]);
        exited(&mut h, 0, 0);
        ops(&mut h);
        ask(&mut h, 3, "c", Some("Late"));
        assert_eq!(ops(&mut h), ["reply 0 3 cancelled"]);
        assert_eq!(h.core.text(), "> ");
    }
}
