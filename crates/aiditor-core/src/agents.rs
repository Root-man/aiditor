//! Agent sessions: each one an ACP agent process started in a workspace, with a session buffer,
//! comint-style, and a read-only buffer for its stderr.

use agent_client_protocol_schema::v1::{
    AgentRequest, AvailableCommand, AvailableCommandInput, ClientResponse, ContentBlock, Error,
    MessageId, NewSessionResponse, PermissionOption, PermissionOptionKind, Plan, PlanEntry,
    PromptRequest, RequestId, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionConfigId, SessionConfigKind,
    SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelect,
    SessionConfigSelectOption, SessionConfigSelectOptions, SessionConfigValueId, SessionId,
    SessionNotification, SessionUpdate, SetSessionConfigOptionRequest, StopReason, ToolCall,
    ToolCallContent, ToolCallId, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
};
use alacritty_terminal::vte::ansi::{Attr, Color, Handler, Processor};
use serde_json::{Map, Value};

use std::collections::{HashMap, HashSet};
use std::ops::{Range, RangeInclusive};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::buffer::{self, Buffer, BufferId, Decoration, FaceRef, InsertionType, MarkerId, Prop};
use crate::command::Args;
use crate::display::ModeLine;
use crate::markdown;
use crate::project::{Project, ProjectId};
use crate::settings::FaceName;
use crate::{AgentId, AgentOp, Core, Ctx, Effect, Effects, Msg, Request};

mod chips;
mod groups;
mod render;
pub(crate) use chips::{Chips, is_chip};
use groups::*;
use render::*;

/// The prompt line's text, just before the prompt area.
const PROMPT: &str = "> ";
/// Slash commands the editor does itself, left out of the `Command:` prompt: Codex's `plan`, as
/// `S-TAB` cycles modes.
const HIDDEN_COMMANDS: &[&str] = &["plan"];
/// The session buffer's major mode.
pub(crate) const MODE: &str = "agent-session-mode";
/// How often a shown session's working line and waiting counters tick.
const TICK: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Activity {
    Working,
    Failed,
    Waiting,
}

impl Activity {
    pub(crate) fn face(self) -> FaceName {
        match self {
            Self::Working => FaceName::StatusRunning,
            Self::Failed => FaceName::StatusFailed,
            Self::Waiting => FaceName::StatusWaiting,
        }
    }
}

pub(crate) struct Session {
    /// The registry name it was started as.
    agent: String,
    /// The workspace name it was started in; workspace renames don't change it.
    workspace: String,
    title: String,
    usage: Option<(u64, u64)>,
    /// The project's primary root, which tool call paths show relative to.
    root: PathBuf,
    /// The session buffer, the session's handle: killing it stops the agent.
    pub(crate) buffer: BufferId,
    /// Its stderr log.
    log: BufferId,
    project: ProjectId,
    /// The spawn, held while the project's environment is being captured.
    held: Option<AgentOp>,
    /// The `session/new` answer, once the agent is ready, its config options the latest reported.
    pub(crate) session: Option<NewSessionResponse>,
    /// The session it loads instead of opening one, and the buffer it was loaded from, which
    /// shows it once loaded if still selected.
    load: Option<(SessionId, BufferId)>,
    /// The mode value last sent with `session/set_config_option`, for a refusal's note.
    mode_sent: Option<String>,
    /// Whether the agent failed to start or exited; its buffers stay until killed.
    ended: bool,
    /// A `session/prompt` waits for its answer; its working line is the last block.
    turn: Option<Turn>,
    /// The last block above the prompt line, while an agent message or thought streams into it.
    stream: Option<Stream>,
    /// The agent and user messages, in transcript order.
    messages: Vec<Message>,
    /// The tool calls shown, by id.
    tools: HashMap<ToolCallId, Tool>,
    /// The plan's block: where it starts and its length in chars, the blank line after it not
    /// counted.
    plan: Option<(MarkerId, usize)>,
    /// Permission requests waiting for an answer, oldest first.
    asks: Vec<Ask>,
    /// The tool calls and what sits among them since the last message, plan or prompt, until
    /// they form a group.
    run: Vec<Row>,
    /// The run's group, once it has one.
    group: Option<usize>,
    /// The tool groups, in transcript order.
    groups: Vec<Group>,
    /// The roots preamble a new session's first prompt carries, if there is anything to list.
    preamble: Option<String>,
    /// The buffer's tick when the prompt area's chips were last painted.
    painted: u64,
    /// The slash commands from the latest `available_commands_update`, hidden ones left out.
    commands: Vec<AvailableCommand>,
}

/// The project's roots with their role lines, the primary first; `None` with nothing to list.
fn preamble(project: &Project) -> Option<String> {
    if project.roots.is_empty() && project.primary.role.is_empty() {
        return None;
    }
    let mut text = "Project roots:".to_owned();
    let others = project.roots.iter().map(|root| (root, ""));
    for (root, primary) in std::iter::once((&project.primary, " (primary)")).chain(others) {
        text += &format!("\n- {}{primary}", root.path.display());
        if !root.role.is_empty() {
            text += &format!(": {}", root.role);
        }
    }
    Some(text)
}

fn session_title(title: Option<&str>) -> String {
    let title = title
        .unwrap_or_default()
        .lines()
        .next()
        .unwrap_or_default()
        .trim();
    let mut chars = title.chars();
    let mut title: String = chars.by_ref().take(40).collect();
    if chars.next().is_some() {
        title.push('…');
    }
    title
}

fn session_name(agent: &str, workspace: &str, title: Option<&str>) -> String {
    let title = session_title(title);
    if title.is_empty() {
        format!("*{agent}*<{workspace}>")
    } else {
        format!("*{agent}: {title}*<{workspace}>")
    }
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

fn options(select: &SessionConfigSelect) -> Vec<&SessionConfigSelectOption> {
    match &select.options {
        SessionConfigSelectOptions::Ungrouped(options) => options.iter().collect(),
        SessionConfigSelectOptions::Grouped(groups) => {
            groups.iter().flat_map(|g| &g.options).collect()
        }
        _ => Vec::new(),
    }
}

/// A select's values, its groups flattened.
fn values(select: &SessionConfigSelect) -> Vec<&SessionConfigValueId> {
    options(select).into_iter().map(|o| &o.value).collect()
}

impl Session {
    /// Whether updates are live, not `session/load` replaying history before its answer.
    fn live(&self) -> bool {
        self.load.is_none() || self.session.is_some()
    }

    /// Where the transcript's next block goes: before the working line and the prompt line.
    fn end(&self, buffer: &Buffer) -> usize {
        buffer.prompt_start() - PROMPT.len() - self.turn.as_ref().map_or(0, |t| t.len)
    }

    /// The buffer lines of the message at `point`, the last one starting at or before it.
    pub(crate) fn message_lines(
        &self,
        buffer: &Buffer,
        point: usize,
    ) -> Option<RangeInclusive<usize>> {
        let start = |m: &Message| buffer.marker(m.start).expect("live with its session");
        let after = self.messages.partition_point(|m| start(m) <= point);
        let message = &self.messages[after.checked_sub(1)?];
        let first = start(message);
        let last = first + message.len.saturating_sub(1);
        Some(buffer.text().char_to_line(first)..=buffer.text().char_to_line(last))
    }

    pub(crate) fn activity(&self) -> Option<Activity> {
        if self.ended {
            Some(Activity::Failed)
        } else if !self.asks.is_empty() {
            Some(Activity::Waiting)
        } else if self.turn.is_some() {
            Some(Activity::Working)
        } else {
            None
        }
    }

    pub(crate) fn mode_line(&self) -> ModeLine {
        let segment = |text: &str, face| vec![(text.replace(char::is_control, " "), face)];
        let options = self
            .session
            .as_ref()
            .and_then(|s| s.config_options.as_deref());
        let options = options.unwrap_or_default();
        let label = |category| {
            let option = options.iter().find(|o| {
                o.category.as_ref() == Some(&category)
                    && matches!(o.kind, SessionConfigKind::Select(_))
            })?;
            let SessionConfigKind::Select(select) = &option.kind else {
                return None;
            };
            let values = self::options(select);
            Some(
                values
                    .iter()
                    .find(|v| v.value == select.current_value)
                    .map_or_else(|| select.current_value.to_string(), |v| v.name.clone()),
            )
        };
        let labels = [
            label(SessionConfigOptionCategory::Model),
            label(SessionConfigOptionCategory::ThoughtLevel),
        ];
        let model = labels.into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let mut left = Vec::new();
        if !model.is_empty() {
            left.push(segment(&model, FaceName::ModeLineDim));
        }
        if let Some((_, mode)) = mode_option(options) {
            let face = match mode.current_value.0.as_ref() {
                "plan" | "read-only" => FaceName::StatusInfo,
                "acceptEdits" | "workspace-write" => FaceName::StatusWaiting,
                "auto" | "agent" => FaceName::StatusRunning,
                "bypassPermissions" | "agent-full-access" => FaceName::StatusFailed,
                _ => FaceName::ModeLine,
            };
            left.push(segment(&format!("[{}]", mode.current_value), face));
        }
        let title = if self.title.is_empty() {
            &self.agent
        } else {
            &self.title
        };
        left.push(segment(title, FaceName::ModeLineBufferId));
        let mut right = Vec::new();
        if let Some(activity) = self.activity() {
            let text = match activity {
                Activity::Working => "● working",
                Activity::Failed => "● exited",
                Activity::Waiting => "● awaiting permission",
            };
            right.push(segment(text, activity.face()));
        }
        if let Some((used, size)) = self.usage.filter(|(_, size)| *size > 0) {
            let percent = u128::from(used.min(size)) * 100 / u128::from(size);
            right.push(segment(&format!("ctx {percent}%"), FaceName::ModeLineDim));
        }
        ModeLine {
            left,
            right,
            path: None,
        }
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
    /// When it arrived; `None` when replayed.
    arrived: Option<Instant>,
    /// Its closed permission waits.
    waited: Duration,
    /// How long it ran, once it finished.
    ran: Option<Duration>,
    /// When it first finished.
    ended: Option<Instant>,
    /// The group it is a row of.
    group: Option<usize>,
    /// What its header shows at the right, the last part of the header.
    time: String,
    /// The terminal it announced, whose output it shows.
    term: Option<Term>,
    /// Its terminal changed since the block was last rendered.
    dirty: bool,
    /// The faces of the SGR escapes and diffs in its content, from the content's start.
    faces: Faces,
    /// Where a command's output lines start in its content.
    output: Option<usize>,
    /// The buffer showing its full output.
    output_buffer: Option<BufferId>,
    /// The indexed file its header links to, and the line, from 1.
    link: Option<(PathBuf, Option<usize>)>,
}

/// Faces on char ranges.
type Faces = Vec<(Range<usize>, Vec<FaceRef>)>;

/// A command's terminal, from the `_meta` extension (#100).
struct Term {
    id: String,
    sgr: Sgr,
    processor: Processor,
    rendered: Option<TermRender>,
    /// `exit <code>` or `signal <name>` once it failed.
    exit: Option<String>,
}

struct TermRender {
    bytes: usize,
    chars: usize,
    at: usize,
    faces: usize,
    fence: usize,
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
    /// The tool call it asks about, which shows the wait.
    tool: ToolCallId,
    since: Instant,
    /// The tool call's title, for the block.
    title: String,
    options: Vec<PermissionOption>,
    /// Where the block starts and its length in chars, the blank line after it not counted.
    start: MarkerId,
    len: usize,
    /// Its row's marker.
    row: MarkerId,
}

/// What a block above the prompt line holds. A chunk joins a streamed block of its own kind, so
/// a new `messageId` starts a block.
#[derive(Clone, Debug, PartialEq)]
enum Block {
    /// A line of the session's own, like a turn's end.
    Line,
    Message(Option<MessageId>),
    Thought(Option<MessageId>),
    User(Option<MessageId>),
}

/// An agent or user message's block. Prompts sent live have no `id`.
#[derive(Debug)]
struct Message {
    // Nothing reads it yet; timings and forking will.
    #[allow(dead_code)]
    id: Option<MessageId>,
    start: MarkerId,
    /// Its length in chars, the blank line after it not counted.
    len: usize,
}

/// A block being streamed.
struct Stream {
    /// Where the block starts.
    start: MarkerId,
    block: Block,
    /// Chunks arrived since it was last rendered.
    dirty: bool,
    /// When its first chunk arrived and the clock then; `None` when replayed.
    began: Option<(Instant, Option<String>)>,
}

/// A running turn.
struct Turn {
    start: Instant,
    /// The working line's length in chars, the blank line after it counted.
    len: usize,
}

impl Core {
    /// Puts `text` above session `id`'s prompt line: as a new block followed by a blank line, or,
    /// for a chunk of the kind streaming, at the end of the streamed block. A chunk's block is
    /// rendered as markdown by the next frame.
    fn output(&mut self, id: AgentId, text: &str, block: Block) {
        let now = self.now();
        let clock = self.clock.clone();
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let end = session.end(buffer);
        if let Some(stream) = &mut session.stream
            && stream.block == block
        {
            buffer.insert_output(end - "\n\n".len(), text);
            stream.dirty = true;
            if let Block::Thought(_) = block {
                session.thought_grew(text.chars().count());
                return;
            }
            if let (Block::Message(_), Some(message)) = (&block, session.messages.last_mut()) {
                message.len += text.chars().count();
            }
            return;
        }
        self.end_stream(id);
        let session = self.agents.get_mut(&id).expect("found above");
        let buffer = self.buffers.get_mut(session.buffer).expect("found above");
        let end = session.end(buffer);
        buffer.insert_output(end, &format!("{text}\n\n"));
        let mut marker = || (buffer.add_marker(end, InsertionType::Stay)).expect("in the text");
        if let Block::Message(message) | Block::User(message) = &block {
            session.messages.push(Message {
                id: message.clone(),
                start: marker(),
                len: text.chars().count(),
            });
        }
        let row = (matches!(block, Block::Thought(_))).then(&mut marker);
        if let Block::Message(_) | Block::Thought(_) = block {
            session.stream = Some(Stream {
                start: marker(),
                block,
                dirty: true,
                began: session.live().then_some((now, clock)),
            });
        }
        match row {
            Some(row) => {
                let row = Row::Thought(Thought::new(row, text.chars().count()));
                session.add_row(buffer, row);
            }
            None => session.end_run(buffer),
        }
    }

    /// Puts a replayed user message above session `id`'s prompt line as a sent prompt shows.
    fn show_user(&mut self, id: AgentId, text: &str, message: Option<MessageId>) {
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let index = self.index.get(&session.project);
        let file = |word: &str| file_link(index, word).map(|(path, _)| path);
        let linked = self.chips.replace_links(text);
        let text = &self.chips.replace_mentions(&linked, file);
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        // A chunk of the last block's user message joins it, as Claude's adapter replays each
        // of a message's blocks, a chip's link among them, as a chunk.
        let end = session.end(buffer).saturating_sub("\n\n".len());
        let last = (session.messages.last_mut()).filter(|m| m.id.is_some() && m.id == message);
        if let Some(last) = last
            && let Some(start) = buffer.marker(last.start)
            && start + last.len == end
            && session.stream.is_none()
        {
            buffer.insert_output(end, text);
            last.len += text.chars().count();
            let range = start + PROMPT.len()..start + last.len;
            render(buffer, range.clone(), false, None);
            self.chips.paint(buffer, range);
            return;
        }
        self.output(id, &format!("{PROMPT}{text}"), Block::User(message));
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let end = session.end(buffer) - "\n\n".len();
        render(buffer, end - text.chars().count()..end, false, None);
        self.chips.paint(buffer, end - text.chars().count()..end);
        if let (true, Some(clock), Some(message)) =
            (session.live(), &self.clock, session.messages.last_mut())
        {
            let at = line_end(buffer, end - text.chars().count());
            message.len += put_time(buffer, at, clock);
        }
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
        let new = !known;
        if new {
            self.end_stream(id);
        }
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let mut term = (session.tools.get_mut(&call.tool_call_id))
            .and_then(|t| t.term.take())
            .or_else(|| {
                terminal_id(&call).map(|id| Term {
                    id,
                    sgr: Sgr::default(),
                    processor: Processor::new(),
                    rendered: None,
                    exit: None,
                })
            });
        let now = self.now.expect("set by handle");
        let known = session.tools.get(&call.tool_call_id);
        let arrived = known.map_or(session.live().then_some(now), |t| t.arrived);
        let waited = known.map_or(Duration::ZERO, |t| t.waited);
        let asked = (session.asks.iter()).find(|a| a.tool == call.tool_call_id);
        let waiting = asked.map(|a| now - a.since);
        let done = finished(call.status);
        let ended = known.and_then(|t| t.ended).or(done.then_some(now));
        let group = known.map_or(session.group, |t| t.group);
        let ran = known.and_then(|t| t.ran).or_else(|| {
            let spent = (now - arrived?).checked_sub(waited + waiting.unwrap_or_default());
            done.then(|| spent.unwrap_or_default())
        });
        let exit = term.as_ref().and_then(|t| t.exit.clone());
        let time = match (waiting, exit, ran) {
            (Some(wait), ..) => waiting_text(wait),
            (None, Some(exit), _) => exit,
            (None, None, Some(ran)) => short_duration(ran),
            (None, None, None) => String::new(),
        };
        let open = known.is_some_and(|t| t.open);
        let mut header = tool_header(&call, term.as_ref(), open, &session.root);
        if group.is_some() {
            header.insert_str(0, "  ");
        }
        if !time.is_empty() {
            header += &format!(" {time}");
        }
        let content = tool_content(&call, term.as_ref());
        let (output, faces) = (content.output, content.faces);
        let text = match content.text.is_empty() {
            true => header.clone(),
            false => format!("{header}\n{}", content.text),
        };
        let (header, len) = (header.chars().count(), text.chars().count());
        if let Some(term) = &mut term {
            let output = term.sgr.text.strip_suffix('\n').unwrap_or(&term.sgr.text);
            let command = tool_command(&call, Some(&*term)).map_or(0, fence_len);
            let fence = (term.sgr.backticks.max(2) + 1).max(command);
            let chars = term.sgr.len - usize::from(term.sgr.text.ends_with('\n'));
            term.rendered = (!term.sgr.text.is_empty()).then(|| TermRender {
                bytes: output.len(),
                chars,
                at: len - header - 1 - fence - 1 - chars,
                faces: term.sgr.faces.len(),
                fence,
            });
        }
        let index = self.index.get(&session.project);
        let link = tool_path(&call, term.as_ref())
            .and_then(|(path, line)| Some((index?.file(path)?, line.map(|l| l as usize))));
        let call_id = call.tool_call_id.clone();
        match session.tools.get_mut(&call.tool_call_id) {
            Some(tool) => {
                let start = buffer.marker(tool.start).expect("live while shown");
                rewrite(buffer, start..start + tool.len, &text);
                (tool.call, tool.header, tool.len) = (call, header, len);
                (tool.term, tool.dirty, tool.faces, tool.output) = (term, false, faces, output);
                (tool.ran, tool.ended, tool.time, tool.link) = (ran, ended, time, link);
            }
            None => {
                let end = session.end(buffer);
                buffer.insert_output(end, &format!("{text}\n\n"));
                let tool = Tool {
                    start: (buffer.add_marker(end, InsertionType::Stay)).expect("in the text"),
                    call,
                    header,
                    len,
                    open: false,
                    arrived,
                    waited,
                    ran,
                    ended,
                    group,
                    time,
                    term,
                    dirty: false,
                    faces,
                    output,
                    output_buffer: None,
                    link,
                };
                session.tools.insert(call_id.clone(), tool);
                session.add_row(buffer, Row::Tool(call_id.clone()));
            }
        };
        if group.is_none_or(|g| session.group_open(g)) {
            render_tool(buffer, &session.tools[&call_id]);
        }
        if let Some(g) = group {
            session.show_group(buffer, g);
        }
        if new && session.forms_group() {
            session.form_group(buffer);
            let g = session.group.expect("just formed");
            for call in session.group_calls(g) {
                self.show_tool(id, self.agents[&id].tools[&call].call.clone());
            }
            let session = self.agents.get_mut(&id).expect("found above");
            let buffer = self.buffers.get_mut(session.buffer).expect("found above");
            session.render_group(buffer, g);
        }
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
                let term = tool.term.as_mut().expect("found by it");
                term.processor.advance(&mut term.sgr, data.as_bytes());
                tool.dirty = true;
            }
        }
        if let Some(value) = meta.get("terminal_exit")
            && let Some(tool) = terminal_tool(&mut session.tools, value)
        {
            let term = tool.term.as_mut().expect("found by it");
            term.rendered = None;
            term.exit = match (value["signal"].as_str(), value["exit_code"].as_i64()) {
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
        if session.plan.is_none() {
            session.end_run(buffer);
        }
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
                let end = session.end(buffer);
                buffer.insert_output(end, &format!("{text}\n\n"));
                let marker = buffer.add_marker(end, InsertionType::Stay);
                session.plan = Some((marker.expect("in the text"), len));
                end
            }
        };
        render_rows(buffer, start, &rows);
    }

    /// Puts permission request `ask` above session `id`'s prompt line as a block of its own,
    /// pending until answered.
    fn show_ask(&mut self, id: AgentId, request: RequestId, ask: RequestPermissionRequest) {
        self.end_stream(id);
        let now = self.now();
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
        let end = session.end(buffer);
        buffer.insert_output(end, &format!("{text}\n\n"));
        render_rows(buffer, end, &rows);
        let tool = ask.tool_call.tool_call_id;
        let shown = session.tools.get(&tool).map(|t| t.call.clone());
        let mut marker = || (buffer.add_marker(end, InsertionType::Stay)).expect("in the text");
        let (start, row) = (marker(), marker());
        let len = text.chars().count();
        session.asks.push(Ask {
            tool,
            since: now,
            request,
            title,
            options: ask.options,
            start,
            len,
            row,
        });
        session.add_row(buffer, Row::Ask { start: row, len });
        if let Some(g) = session.group {
            session.show_group(buffer, g);
        }
        if let Some(call) = shown {
            self.show_tool(id, call);
        }
    }

    /// Adds `ask`'s wait, just closed, to its tool call and shows the call without it, its block
    /// now the answer's `line`.
    // shortcut: overlapping asks on one call each add their wait, so the run time can read low;
    // both adapters ask once per call.
    fn close_wait(&mut self, id: AgentId, ask: &Ask, line: &str) {
        let now = self.now();
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        if let Some(buffer) = self.buffers.get_mut(session.buffer) {
            session.settle_row(buffer, ask.row, line.chars().count());
        }
        if let Some(tool) = session.tools.get_mut(&ask.tool) {
            tool.waited += now - ask.since;
            let call = tool.call.clone();
            self.show_tool(id, call);
        }
    }

    /// The time of the last `handle`.
    fn now(&self) -> Instant {
        self.now.expect("set by handle")
    }

    /// Renders session `id`'s streamed block if chunks arrived since it was last rendered.
    fn render_stream(&mut self, id: AgentId) {
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        if session.stream.is_none() {
            return;
        }
        let end = session.end(buffer) - "\n\n".len();
        let Some(stream) = &mut session.stream else {
            return;
        };
        if std::mem::take(&mut stream.dirty) {
            let start = buffer.marker(stream.start).expect("live while streaming");
            let thought = matches!(stream.block, Block::Thought(_));
            if !thought || !session.fold_stream(buffer) {
                // shortcut: links are found as the block renders; a file indexed later isn't
                // linked until it renders again, and RET on one since dropped sends the prompt.
                let index = (!thought)
                    .then(|| self.index.get(&session.project))
                    .flatten();
                render(buffer, start..end, thought, index);
            }
        }
    }

    /// Renders and ends session `id`'s stream, if any, writing its time: a message's clock time
    /// or a thought's header. They wait until now as its first line may still be growing.
    fn end_stream(&mut self, id: AgentId) {
        self.render_stream(id);
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let (Some(stream), Some(buffer)) =
            (session.stream.take(), self.buffers.get_mut(session.buffer))
        else {
            return;
        };
        let start = buffer.marker(stream.start).expect("live while streaming");
        buffer.remove_marker(stream.start);
        let thought = matches!(stream.block, Block::Thought(_));
        let mut header = None;
        if let Some((began, clock)) = stream.began {
            match (stream.block, clock, session.messages.last_mut()) {
                (Block::Thought(_), ..) => {
                    let elapsed = self.now.expect("set by handle") - began;
                    let text = format!("Thought for {}s", elapsed.as_secs());
                    buffer.insert_output(start, &format!("{text}\n"));
                    let end = start + text.chars().count();
                    (buffer.add_property(start..end, dim())).expect("just inserted");
                    header = Some(end - start);
                }
                (Block::Message(_), Some(clock), Some(message)) => {
                    message.len += put_time(buffer, line_end(buffer, start), &clock);
                }
                _ => {}
            }
        }
        if thought {
            session.end_thought(buffer, header);
        }
    }

    /// Ends session `id`'s turn, turning its working line into `Worked` in place.
    fn settle_turn(&mut self, id: AgentId) {
        self.end_stream(id);
        let now = self.now();
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let (Some(turn), Some(buffer)) =
            (session.turn.take(), self.buffers.get_mut(session.buffer))
        else {
            return;
        };
        let end = session.end(buffer) - "\n\n".len();
        let text = format!("Worked {}", long_duration(now - turn.start));
        let start = end + "\n\n".len() - turn.len;
        let len = retime(buffer, start..end, &text, FaceName::SessionThought);
        if let Some(clock) = &self.clock {
            put_time(buffer, start + len, clock);
        }
    }

    /// Brings session `id`'s working line and waiting counters up to the last `handle`.
    fn tick_session(&mut self, id: AgentId) {
        let now = self.now();
        let session = self.agents.get_mut(&id).expect("known session");
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        if let Some(turn) = &mut session.turn {
            let end = buffer.prompt_start() - PROMPT.len() - "\n\n".len();
            let text = format!("Working for {}", long_duration(now - turn.start));
            let start = end + "\n\n".len() - turn.len;
            let len = retime(buffer, start..end, &text, FaceName::SessionThought);
            turn.len = len + "\n\n".len();
        }
        for (at, ask) in session.asks.iter().enumerate() {
            // Its tool's oldest ask counts.
            let oldest = session.asks.iter().position(|a| a.tool == ask.tool);
            let Some(tool) = session
                .tools
                .get_mut(&ask.tool)
                .filter(|_| oldest == Some(at))
            else {
                continue;
            };
            let text = waiting_text(now - ask.since);
            let (old, start) = (tool.time.chars().count(), buffer.marker(tool.start));
            let end = start.expect("live while shown") + tool.header;
            let len = retime(buffer, end - old..end, &text, FaceName::Annotation);
            tool.header = tool.header - old + len;
            tool.len = tool.len - old + len;
            tool.time = text;
        }
    }

    /// Wakes the core each second while a shown session has something ticking, and not otherwise,
    /// so a hidden session costs no redraws.
    pub(crate) fn sync_session_tick(&mut self, now: Instant) {
        let shown = self.shown_buffers();
        let ticking = (self.agents.values())
            .any(|s| shown.contains(&s.buffer) && (s.turn.is_some() || !s.asks.is_empty()));
        match (ticking, self.session_tick) {
            (true, None) => {
                let tick = self.timers.start(now, TICK, Some(TICK), Msg::Tick);
                self.session_tick = Some(tick);
            }
            (false, Some(tick)) => {
                self.timers.cancel(tick);
                self.session_tick = None;
            }
            _ => {}
        }
    }

    /// Repaints the chips of session `id`'s prompt area if it changed since, as yank and undo
    /// bring chips back.
    fn paint_prompt(&mut self, id: AgentId) {
        let session = self.agents.get_mut(&id).expect("known session");
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        // The prompt area carries only chips' properties, so it is cleared whole.
        if std::mem::replace(&mut session.painted, buffer.tick()) != buffer.tick() {
            let range = buffer.prompt_start()..buffer.len_chars();
            buffer.remove_properties(range.clone());
            self.chips.paint(buffer, range);
        }
    }

    /// Repaints every session buffer's chips, after a chip's file went or came back.
    pub(crate) fn repaint_chips(&mut self) {
        for session in self.agents.values() {
            if let Some(buffer) = self.buffers.get_mut(session.buffer) {
                self.chips.paint(buffer, 0..buffer.len_chars());
            }
        }
    }

    /// The chip at point in a session buffer.
    fn chip_at_point(&self) -> Option<&chips::Chip> {
        let buffer = self.windows.window.buffer();
        self.agents.values().find(|s| s.buffer == buffer)?;
        self.chips
            .get(self.current().text().get_char(self.point())?)
    }

    /// Shows the path of the chip at point when the command left no message, as eldoc does.
    pub(crate) fn echo_chip(&mut self) {
        if self.message.is_none() {
            let path = self.chip_at_point().and_then(|c| c.path.as_ref());
            self.message = path.map(|p| p.display().to_string());
        }
    }

    /// Renders shown streamed blocks and dirty tool calls, at most once per frame.
    pub(crate) fn render_streams(&mut self, shown: &HashSet<BufferId>) {
        let ids: Vec<_> = self.agents.keys().copied().collect();
        for id in ids {
            if !shown.contains(&self.agents[&id].buffer) {
                continue;
            }
            self.render_stream(id);
            self.paint_prompt(id);
            let dirty: Vec<_> = (self.agents[&id].tools.values())
                .filter(|t| t.dirty)
                .map(|t| t.call.tool_call_id.clone())
                .collect();
            for call in dirty {
                if !self.append_tool_output(id, &call) {
                    self.show_tool(id, self.agents[&id].tools[&call].call.clone());
                }
            }
            self.tick_session(id);
        }
    }

    // shortcut: open calls still rebuild all markdown properties; make them incremental if long open output is slow.
    fn append_tool_output(&mut self, id: AgentId, call: &ToolCallId) -> bool {
        let session = self.agents.get_mut(&id).expect("known session");
        let tool = session.tools.get_mut(call).expect("dirty call");
        if tool.open {
            return false;
        }
        let Some(term) = &mut tool.term else {
            return false;
        };
        let Some(rendered) = &mut term.rendered else {
            return false;
        };
        if term.sgr.backticks >= rendered.fence {
            return false;
        }
        let output = term.sgr.text.strip_suffix('\n').unwrap_or(&term.sgr.text);
        let tail = &output[rendered.bytes..];
        let chars = term.sgr.len - usize::from(term.sgr.text.ends_with('\n'));
        let buffer = self
            .buffers
            .get_mut(session.buffer)
            .expect("session buffer");
        let start = buffer.marker(tool.start).expect("live while shown");
        let at = start + tool.len - rendered.fence - 1;
        buffer.replace_output(at..at, tail);
        tool.len += chars - rendered.chars;
        let keep = rendered.faces.saturating_sub(1);
        tool.faces
            .truncate(tool.faces.len() - rendered.faces + keep);
        tool.faces.extend(
            term.sgr.faces[keep..]
                .iter()
                .map(|(r, f)| (rendered.at + r.start..rendered.at + r.end, f.clone())),
        );
        (rendered.bytes, rendered.chars, rendered.faces) =
            (output.len(), chars, term.sgr.faces.len());
        tool.dirty = false;
        true
    }

    /// The session whose buffer the selected window shows.
    fn current_session(&mut self) -> Result<(AgentId, &mut Session), String> {
        let buffer = self.windows.window.buffer();
        (self.agents.iter_mut())
            .find(|(_, s)| s.buffer == buffer)
            .map(|(&id, s)| (id, s))
            .ok_or_else(|| "Not in an agent session buffer".into())
    }

    /// The current session's slash commands; none outside a session buffer.
    pub(crate) fn session_commands(&self) -> &[AvailableCommand] {
        let buffer = self.windows.window.buffer();
        (self.agents.values())
            .find(|s| s.buffer == buffer)
            .map_or(&[], |s| &s.commands)
    }

    /// The `Command:` annotation of the current session's command `name`: its argument hint and
    /// its description's first non-blank line, cut to 60 chars.
    pub(crate) fn command_note(&self, name: &str) -> Option<String> {
        let command = self.session_commands().iter().find(|c| c.name == name)?;
        let hint = match &command.input {
            Some(AvailableCommandInput::Unstructured(input)) => input.hint.as_str(),
            _ => "",
        };
        let mut lines = command.description.lines();
        let line = lines.find(|l| !l.trim().is_empty()).unwrap_or_default();
        let description = match line.chars().count() > 60 {
            true => format!("{}…", line.chars().take(59).collect::<String>()),
            false => line.to_string(),
        };
        let note = [hint, &description].into_iter().filter(|s| !s.is_empty());
        Some(note.collect::<Vec<_>>().join("  "))
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

    /// The session buffer of `agent`'s session `id`, if one is open or loading.
    pub(crate) fn open_session(&self, agent: &str, id: &SessionId) -> Option<BufferId> {
        (self.agents.values())
            .find(|s| {
                let answered = s.session.as_ref().map(|r| &r.session_id);
                let loading = s.load.as_ref().map(|(id, _)| id);
                s.agent == agent && answered.or(loading) == Some(id)
            })
            .map(|s| s.buffer)
            .filter(|&b| self.buffers.get(b).is_some())
    }

    /// The state label of the session `agent` started or answered as `id`, while it is open.
    pub(crate) fn session_state(&self, agent: &str, id: &SessionId) -> Option<&'static str> {
        let buffer = self.open_session(agent, id)?;
        let s = self.agents.values().find(|s| s.buffer == buffer)?;
        Some(if s.ended {
            "[ended]"
        } else if s.session.is_none() && s.load.is_some() {
            "[loading]"
        } else if s.session.is_none() {
            "[starting]"
        } else if !s.asks.is_empty() {
            "[waiting]"
        } else if s.turn.is_some() {
            "[working]"
        } else {
            "[idle]"
        })
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
        self.start_session(&args.strings[0], None)
    }

    /// Starts agent `name` as `agent-start` does; with `load`, loads that session, titled,
    /// instead of opening one, showing its buffer only once loaded.
    pub(crate) fn start_session(
        &mut self,
        name: &str,
        load: Option<(SessionId, Option<&str>)>,
    ) -> Result<(), String> {
        let core = &mut *self.core;
        let (load, title) = load.unzip();
        let session_title = session_title(title.flatten());
        let agent = (core.settings.agents.get(name))
            .ok_or_else(|| format!("{name:?} is not a registered agent"))?
            .clone();
        let workspace = core.current_workspace();
        let (project, log, title, workspace) = (
            workspace.project,
            format!("*{name} stderr*<{}>", workspace.name),
            session_name(name, &workspace.name, title.flatten()),
            workspace.name.clone(),
        );
        let loaded = &core.projects[core.project_at(project)?].project;
        // Under the captured environment, which `release_agents` puts first.
        let env = (agent.env.into_iter()).chain(loaded.env.clone()).collect();
        let cwd = loaded.primary.path.clone();
        let roots = loaded.roots.iter().map(|r| r.path.clone()).collect();
        let preamble = preamble(loaded).filter(|_| load.is_none());
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
        let from = core.windows.window.buffer();
        if load.is_none() {
            core.switch_to(buffer);
        }
        let spawn = AgentOp::Spawn {
            id,
            command: agent.command,
            args: agent.args,
            cwd: cwd.clone(),
            roots,
            env,
            session: load.clone(),
        };
        let echo = match load {
            Some(_) => format!("Loading {name} session..."),
            None => format!("Starting {name}..."),
        };
        let session = Session {
            root: cwd,
            agent: name.into(),
            workspace,
            title: session_title,
            usage: None,
            buffer,
            log,
            project,
            held: Some(spawn),
            session: None,
            load: load.map(|id| (id, from)),
            mode_sent: None,
            ended: false,
            turn: None,
            stream: None,
            messages: Vec::new(),
            tools: HashMap::new(),
            plan: None,
            asks: Vec::new(),
            run: Vec::new(),
            group: None,
            groups: Vec::new(),
            preamble,
            painted: 0,
            commands: Vec::new(),
        };
        core.agents.insert(id, session);
        core.echo(echo);
        self.release_agents(project);
        Ok(())
    }

    /// Sends the held spawns of `project`'s sessions and its sessions buffers' list requests,
    /// the captured environment first, unless a capture is still running.
    pub(crate) fn release_agents(&mut self, project: ProjectId) {
        let Some(vars) = self.core.captured(project) else {
            return;
        };
        let sessions = self.core.agents.values_mut();
        for session in sessions.filter(|s| s.project == project) {
            if let Some(mut spawn) = session.held.take() {
                if let AgentOp::Spawn { env, .. } = &mut spawn {
                    env.splice(0..0, vars.iter().cloned());
                }
                self.effects.push(Effect::Agent(spawn));
            }
        }
        let lists = self.core.release_lists(project, &vars);
        self.effects.extend(lists.into_iter().map(Effect::Agent));
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
                // A loaded session keeps its own mode.
                let default = (core.settings.agents.get(&agent))
                    .and_then(|a| a.default_mode.clone())
                    .filter(|_| session.load.is_none());
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
                let selected = core.windows.window.buffer();
                let show = (session.load.as_ref())
                    .filter(|(_, from)| *from == selected)
                    .map(|_| session.buffer);
                if let Some(buffer) = show {
                    core.switch_to(buffer);
                }
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

    pub(crate) fn agent_unscoped(&mut self, id: AgentId) {
        if let Some(session) = self.core.agents.get(&id) {
            let note = format!("{} can't take additional directories", session.agent);
            self.core.log(&note);
        }
    }

    pub(crate) fn agent_stderr(&mut self, id: AgentId, line: String) {
        let core = &mut *self.core;
        let buffer = (core.agents.get(&id)).and_then(|s| core.buffers.get_mut(s.log));
        if let Some(buffer) = buffer {
            buffer.append(&format!("{line}\n"));
        }
    }

    /// Puts updates above the prompt line and follows mode changes.
    pub(crate) fn agent_update(&mut self, id: AgentId, update: SessionNotification) {
        let core = &mut *self.core;
        let Some(session) = core.agents.get_mut(&id).filter(|s| !s.ended) else {
            return;
        };
        match update.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    core.output(id, &text.text, Block::Message(chunk.message_id));
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    core.output(id, &text.text, Block::Thought(chunk.message_id));
                }
            }
            SessionUpdate::UserMessageChunk(chunk) => match chunk.content {
                ContentBlock::Text(text) => core.show_user(id, &text.text, chunk.message_id),
                ContentBlock::Image(_) => {
                    let chip = core.chips.intern(None, false).to_string();
                    core.show_user(id, &chip, chunk.message_id);
                }
                _ => {}
            },
            SessionUpdate::ToolCall(call) => {
                let meta = call.meta.clone();
                core.show_tool(id, call);
                core.terminal_meta(id, meta.as_ref());
            }
            SessionUpdate::ToolCallUpdate(update) => {
                let meta = update.meta.clone();
                if update.fields != ToolCallUpdateFields::default() {
                    core.tool_update(id, update);
                }
                core.terminal_meta(id, meta.as_ref());
            }
            SessionUpdate::Plan(plan) => core.show_plan(id, &plan),
            SessionUpdate::ConfigOptionUpdate(options) => {
                if let Some(ready) = &mut session.session {
                    ready.config_options = Some(options.config_options);
                }
            }
            SessionUpdate::SessionInfoUpdate(info) => {
                if let Some(title) = info.title.as_opt_deref() {
                    session.title = session_title(title);
                    let name = session_name(&session.agent, &session.workspace, title);
                    core.buffers.rename(session.buffer, &name);
                }
            }
            SessionUpdate::UsageUpdate(usage) => session.usage = Some((usage.used, usage.size)),
            SessionUpdate::AvailableCommandsUpdate(update) => {
                session.commands = (update.available_commands.into_iter())
                    .filter(|c| !HIDDEN_COMMANDS.contains(&c.name.as_str()))
                    .collect();
            }
            _ => {}
        }
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
                answered(o.kind),
            ),
            None => (RequestPermissionOutcome::Cancelled, CANCELLED),
        };
        if let Some(b) = core.buffers.get_mut(session.buffer) {
            settle(b, &ask, line);
        }
        core.close_wait(id, &ask, line);
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

    /// The file and line, from 1, of the link point is on: a tool call header's, else the word's.
    fn link_at_point(&self) -> Option<(PathBuf, Option<usize>)> {
        let core = &*self.core;
        let (point, buffer) = (core.point(), core.windows.window.buffer());
        let b = core.buffers.get(buffer)?;
        let session = core.agents.values().find(|s| s.buffer == buffer)?;
        let link = FaceRef::Face(FaceName::Link);
        let linked = (b.properties_near(point..point).into_iter())
            .any(|(r, p)| r.contains(&point) && matches!(p, Prop::Named(f) if f.contains(&link)));
        if !linked || point >= b.prompt_start() {
            return None;
        }
        if let Some((_, call)) = self.tool_at_point() {
            return session.tools[&call].link.clone();
        }
        let text = b.text();
        let start = text.line_to_char(text.char_to_line(point));
        let line = text.slice(start..line_end(b, point)).to_string();
        let at = line
            .char_indices()
            .nth(point - start)
            .map_or(line.len(), |(i, _)| i);
        let word = markdown::words(&line)
            .into_iter()
            .find(|w| w.contains(&at))?;
        file_link(core.index.get(&session.project), &line[word])
    }

    /// The tool call whose header point is on.
    fn tool_at_point(&self) -> Option<(AgentId, ToolCallId)> {
        let core = &*self.core;
        let point = core.point();
        let buffer = core.windows.window.buffer();
        let b = core.buffers.get(buffer)?;
        let (&id, session) = core.agents.iter().find(|(_, s)| s.buffer == buffer)?;
        let tool = (session.tools.iter()).find(|(_, t)| {
            let shown = !session.hidden(t);
            shown && (b.marker(t.start)).is_some_and(|s| (s..=s + t.header).contains(&point))
        });
        tool.map(|(call, _)| (id, call.clone()))
    }

    /// Shows `call`'s full output in its own read-only buffer, refreshed from the call.
    // shortcut: a snapshot, not following a running command; RET refreshes it.
    fn show_tool_output(&mut self, id: AgentId, call: &ToolCallId) -> Result<(), String> {
        let core = &mut *self.core;
        let workspace = core.current_workspace().name.clone();
        let session = core.agents.get_mut(&id).expect("found by the caller");
        let tool = session.tools.get_mut(call).expect("found by the caller");
        let term = tool.term.as_ref();
        let (text, faces) = tool_output(&tool.call, term).ok_or("No output")?;
        let target = session_title(Some(&tool_target(&tool.call, term, &session.root)));
        let found = tool
            .output_buffer
            .filter(|b| core.buffers.get(*b).is_some());
        let buffer = found.unwrap_or_else(|| {
            let name = format!("*{} output: {target}*<{workspace}>", session.agent);
            let id = core.buffers.create(&name);
            core.buffers.get_mut(id).expect("just created").read_only = true;
            id
        });
        tool.output_buffer = Some(buffer);
        let b = core.buffers.get_mut(buffer).expect("live");
        b.set_text(&text);
        b.remove_properties(0..b.len_chars());
        for (r, face) in faces {
            (b.add_property(r, Prop::Named(face))).expect("inside the text");
        }
        self.display_buffer(buffer);
        Ok(())
    }

    /// `agent-send-input`: on a pending permission request's option line, answers it with that
    /// option; on a file link, visits the file in another window; on a tool call's header, shows
    /// its full output; elsewhere sends the prompt area's text as a `session/prompt`, leaving it above a
    /// new prompt line.
    pub(crate) fn agent_send_input(&mut self) -> Result<(), String> {
        if let Some((id, at, option)) = self.option_at_point() {
            self.answer_ask(id, at, Some(option));
            return Ok(());
        }
        if let Some(chip) = self.core.chip_at_point() {
            let (path, kind) = (chip.path.clone().ok_or("No file")?, chip.kind);
            if chip.gone {
                return Err(format!("{}: no such file", path.display()));
            }
            if kind == chips::Kind::Image {
                self.effects.push(Effect::OpenWithSystem(path));
                return Ok(());
            }
            if kind == chips::Kind::Directory {
                self.call("find-file", Args::default());
                return self.set_text(&path.join("").to_string_lossy());
            }
            if self.core.windows.tree.leaves().len() == 1 {
                self.split_window(false)?;
            }
            self.other_window(1)?;
            return self.find_file_at(&path, 0, 0);
        }
        if let Some((path, line)) = self.link_at_point() {
            if self.core.windows.tree.leaves().len() == 1 {
                self.split_window(false)?;
            }
            self.other_window(1)?;
            return self.find_file_at(&path, line.map_or(0, |l| l.saturating_sub(1)), 0);
        }
        if let Some((id, call)) = self.tool_at_point() {
            return self.show_tool_output(id, &call);
        }
        let core = &mut *self.core;
        let buffer = core.windows.window.buffer();
        let (id, session) = core.current_session()?;
        let agent = &session.agent;
        let ready = match (&session.session, session.ended, session.turn.is_some()) {
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
        let input = b.text().slice(b.prompt_start()..).to_string();
        if input.trim().is_empty() {
            return Ok(());
        }
        core.end_stream(id);
        let session = core.agents.get_mut(&id).expect("found above");
        let b = core
            .buffers
            .get_mut(buffer)
            .expect("current buffer is live");
        session.end_run(b);
        let start = b.prompt_start();
        let working = format!("Working for {}", long_duration(Duration::ZERO));
        b.append(&format!("\n\n{working}\n\n{PROMPT}"));
        render(b, start..start + input.chars().count(), false, None);
        core.chips.paint(b, start..start + input.chars().count());
        let end = b.len_chars();
        b.set_marker(b.prompt.expect("a session buffer has a prompt"), end)?;
        let line = start + input.chars().count() + "\n\n".len();
        (b.add_property(line..line + working.chars().count(), dim())).expect("just appended");
        let echo = start - PROMPT.len();
        let mut message = Message {
            id: None,
            start: b.add_marker(echo, InsertionType::Stay)?,
            len: PROMPT.len() + input.chars().count(),
        };
        // The old prompt area's edits would replay into read-only text.
        b.clear_undo();
        if let Some(clock) = &core.clock {
            message.len += put_time(b, line_end(b, start), clock);
        }
        let end = b.len_chars();
        let session = core.agents.get_mut(&id).expect("found above");
        session.messages.push(message);
        session.turn = Some(Turn {
            start: self.now,
            len: working.chars().count() + "\n\n".len(),
        });
        let preamble = session.preamble.take().into_iter().map(Into::into);
        let blocks = preamble.chain(core.chips.blocks(&input)).collect();
        let request = PromptRequest::new(session_id, blocks);
        self.effects
            .push(Effect::Agent(AgentOp::Prompt { id, request }));
        self.goto(end)
    }

    /// Moves point to the `n`th message's start after point, before it when `n` is negative, or
    /// the farthest one when fewer exist.
    pub(crate) fn agent_next_message(&mut self, n: i64) -> Result<(), String> {
        let core = &mut *self.core;
        let point = core.point();
        let (id, _) = core.current_session()?;
        let buffer = core.current();
        let starts: Vec<usize> = (core.agents[&id].messages.iter())
            .map(|m| buffer.marker(m.start).expect("live with its session"))
            .collect();
        let at = crate::motion::nth_mark(&starts, point, n).ok_or("No message found")?;
        self.goto(at)
    }

    /// `agent-toggle-tool-call`: shows or hides the content of the tool call or thought row whose
    /// header point is on, or hides that of the open one point is in, moving point to its header.
    /// Elsewhere in a group, it opens or folds the group.
    pub(crate) fn agent_toggle_tool_call(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let point = core.point();
        let buffer = core.windows.window.buffer();
        let session = (core.agents.values_mut()).find(|s| s.buffer == buffer);
        let b = core
            .buffers
            .get_mut(buffer)
            .expect("current buffer is live");
        let session = session.ok_or("No tool call here")?;
        let call = (session.tools.iter())
            .find(|(_, t)| {
                let end = if t.open { t.len } else { t.header };
                // A row with nothing to open leaves `TAB` to its group.
                (t.group.is_none() || t.len > t.header)
                    && !session.hidden(t)
                    && (b.marker(t.start)).is_some_and(|s| (s..=s + end).contains(&point))
            })
            .map(|(call, _)| call.clone());
        let Some(call) = call else {
            if let Some((start, first)) = session.toggle_thought(b, point) {
                return if point > first {
                    self.goto(start)
                } else {
                    Ok(())
                };
            }
            let (start, line, open) =
                (session.toggle_group(b, point)).ok_or("No tool call here")?;
            return if !open && point > line {
                self.goto(start)
            } else {
                Ok(())
            };
        };
        let tool = session.tools.get_mut(&call).expect("just found");
        // Without content there is nothing to show, so it stays folded for when content comes.
        tool.open = !tool.open && tool.len > tool.header;
        let start = b.marker(tool.start).expect("live while shown");
        b.remove_properties(start..start + tool.len);
        let mark = start + if tool.group.is_some() { 2 } else { 0 };
        b.replace_output(mark..mark + 1, if tool.open { "▾" } else { "▸" });
        render_tool(b, tool);
        if point > start + tool.header {
            return self.goto(start);
        }
        Ok(())
    }

    /// `agent-cancel`: sends `session/cancel` while a turn runs, then answers the pending
    /// permission requests `cancelled`, as ACP requires.
    pub(crate) fn agent_cancel(&mut self) -> Result<(), String> {
        let (id, session) = self.core.current_session()?;
        let ready = session.session.as_ref().filter(|_| session.turn.is_some());
        let asks = session.asks.len();
        let session = ready.ok_or("No turn is running")?.session_id.clone();
        (self.effects).push(Effect::Agent(AgentOp::Cancel { id, session }));
        for _ in 0..asks {
            self.answer_ask(id, 0, None);
        }
        Ok(())
    }

    /// `agent-command`: inserts `/name ` at the start of the prompt area, point after it.
    pub(crate) fn agent_command(&mut self, name: &str) -> Result<(), String> {
        self.core.current_session()?;
        let start = self.core.current().prompt_start();
        let text = format!("/{name} ");
        self.core.current_mut().insert(start, &text)?;
        self.goto(start + text.chars().count())
    }

    /// `/` in a session: `agent-command` at the start of the prompt area when the session has
    /// commands, else typed as itself.
    pub(crate) fn agent_slash(&mut self, args: Args) -> Result<(), String> {
        let core = &*self.core;
        let pick =
            !core.session_commands().is_empty() && core.point() == core.current().prompt_start();
        let command = if pick {
            "agent-command"
        } else {
            "self-insert-command"
        };
        self.call(command, args);
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

    /// Ends the turn, cancelling the requests it left pending, with a line saying why unless it
    /// ended normally.
    pub(crate) fn agent_prompted(&mut self, id: AgentId, result: Result<StopReason, String>) {
        let Some(session) = self.core.agents.get(&id).filter(|s| s.turn.is_some()) else {
            return;
        };
        for _ in 0..session.asks.len() {
            self.answer_ask(id, 0, None);
        }
        let core = &mut *self.core;
        let line = match result {
            Ok(StopReason::EndTurn) => None,
            Ok(StopReason::MaxTokens) => Some("Turn ended: max_tokens".into()),
            Ok(StopReason::MaxTurnRequests) => Some("Turn ended: max_turn_requests".into()),
            Ok(StopReason::Refusal) => Some("Turn ended: refusal".into()),
            Ok(StopReason::Cancelled) => Some("Turn ended: cancelled".into()),
            Ok(reason) => Some(format!("Turn ended: {reason:?}")),
            Err(err) => Some(format!("Error: {err}")),
        };
        if let Some(line) = line {
            core.output(id, &line, Block::Line);
        }
        core.settle_turn(id);
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
        core.settle_turn(id);
        let Some(session) = core.agents.get_mut(&id) else {
            return;
        };
        // The agent is gone with its requests, so only the blocks change.
        let asks = std::mem::take(&mut session.asks);
        if let Some(b) = core.buffers.get_mut(session.buffer) {
            asks.iter().for_each(|ask| settle(b, ask, CANCELLED));
        }
        (asks.iter()).for_each(|ask| core.close_wait(id, ask, CANCELLED));
        let session = core.agents.get_mut(&id).expect("found above");
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
        AvailableCommandsUpdate, ConfigOptionUpdate, ContentChunk, ImageContent, PlanEntryPriority,
        PlanEntryStatus, SessionConfigSelectOption, SessionUpdate, ToolCallLocation,
        ToolCallUpdateFields, ToolKind, UnstructuredCommandInput,
    };

    use super::*;
    use crate::buffer::{Face, Icon, Prop};
    use crate::harness::{Harness, listed, name};
    use crate::project::Root;
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
                    let texts: Vec<_> = (request.prompt.iter())
                        .map(|block| match block {
                            ContentBlock::Text(text) => format!("{:?}", text.text),
                            ContentBlock::ResourceLink(l) => format!("link {} {}", l.name, l.uri),
                            ContentBlock::Image(i) => {
                                format!("image {} {}", i.mime_type, i.uri.as_deref().unwrap())
                            }
                            _ => panic!("text, links and images only: {request:?}"),
                        })
                        .collect();
                    format!("prompt {} {} {}", id.0, request.session_id, texts.join(" "))
                }
                AgentOp::SetConfigOption { id, request } => {
                    let value = request.value.as_value_id().unwrap();
                    let (session, config) = (request.session_id, request.config_id);
                    format!("set {} {session} {config}={value}", id.0)
                }
                AgentOp::Cancel { id, session } => format!("cancel {} {session}", id.0),
                AgentOp::Kill { id } => format!("kill {}", id.0),
                AgentOp::List { request, .. } => format!("list {request}"),
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
    fn message_chunks_stream_above_the_prompt() {
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
        assert_eq!(h.core.text(), "Hi\n\nThought for 0s\nhmm\n\n there\n\n> ");
        assert_eq!(h.core.point(), h.core.text().chars().count());
        assert_eq!(log_text(&h, "*claude stderr*<work>"), before);
        exited(&mut h, 0, 0);
        let text = h.core.text().to_string();
        chunk(&mut h, "late");
        assert_eq!(h.core.text(), text);
    }

    #[test]
    fn an_unknown_agents_update_preserves_every_buffer() {
        let mut h = session();
        let before: Vec<_> = h
            .core
            .buffers
            .iter_mut()
            .map(|(id, b)| (id, b.text().to_string()))
            .collect();
        update(
            &mut h,
            9,
            SessionUpdate::AgentMessageChunk(ContentChunk::new("late".into())),
        );
        let after: Vec<_> = h
            .core
            .buffers
            .iter_mut()
            .map(|(id, b)| (id, b.text().to_string()))
            .collect();
        assert_eq!(after, before);
    }

    #[test]
    fn a_prompt_is_sent_and_its_reply_streams_above_the_prompt() {
        let mut h = session();
        h.keys("h e l l o RET");
        // No other roots and no role line: nothing to list, so no preamble.
        assert_eq!(ops(&mut h), [r#"prompt 0 s "hello""#]);
        assert_eq!(h.core.text(), "> hello\n\nWorking for 0s\n\n> ");
        assert_eq!(h.core.point(), 27);
        chunk(&mut h, "Hi");
        chunk(&mut h, " there");
        prompted(&mut h, Ok(StopReason::EndTurn));
        assert_eq!(h.core.text(), "> hello\n\nHi there\n\nWorked 0s\n\n> ");
        // The turn ended, so the next prompt goes out.
        h.keys("a g a i n RET");
        assert_eq!(ops(&mut h), [r#"prompt 0 s "again""#]);
        chunk(&mut h, "Again");
        assert_eq!(
            h.core.text(),
            "> hello\n\nHi there\n\nWorked 0s\n\n> again\n\nAgain\n\nWorking for 0s\n\n> "
        );
    }

    fn commands(h: &mut Harness, names: &[&str]) {
        let commands = (names.iter())
            .map(|&name| {
                let description = match name {
                    "review" => "Review changes".to_string(),
                    _ => format!("Run {name}"),
                };
                let command = AvailableCommand::new(name, description);
                match name {
                    "review" => command.input(AvailableCommandInput::Unstructured(
                        UnstructuredCommandInput::new("[path]"),
                    )),
                    _ => command,
                }
            })
            .collect();
        let update = AvailableCommandsUpdate::new(commands);
        self::update(h, 0, SessionUpdate::AvailableCommandsUpdate(update));
    }

    #[test]
    fn slash_at_the_prompt_start_picks_a_command_with_its_hint() {
        let mut h = session();
        commands(&mut h, &["review", "plan", "mcp"]);
        h.keys("/");
        let mb = h.core.minibuffer().unwrap();
        // `plan` is the editor's own mode switch.
        assert_eq!(mb.candidates, ["review", "mcp"]);
        assert_eq!(mb.annotations, ["[path]  Review changes", "Run mcp"]);
        h.keys("RET");
        assert_eq!(h.core.text(), "> /review ");
        assert_eq!(h.core.point(), h.core.text().chars().count());
        // Sent as text, as ACP expects.
        h.keys("x RET");
        assert_eq!(ops(&mut h), [r#"prompt 0 s "/review x""#]);
    }

    #[test]
    fn the_command_list_follows_updates() {
        let mut h = session();
        commands(&mut h, &["review"]);
        commands(&mut h, &["compact"]);
        h.keys("/");
        assert_eq!(listed(&mut h).0, ["compact"]);
    }

    #[test]
    fn long_descriptions_show_their_first_line_cut_to_sixty_chars() {
        let mut h = session();
        let long = format!("\n{}\nmore", "x".repeat(70));
        let list = AvailableCommandsUpdate::new(vec![AvailableCommand::new("skill", long)]);
        update(&mut h, 0, SessionUpdate::AvailableCommandsUpdate(list));
        let note = h.core.command_note("skill").unwrap();
        assert_eq!(note, format!("{}…", "x".repeat(59)));
    }

    #[test]
    fn slash_types_itself_elsewhere_and_without_commands() {
        let mut h = session();
        h.keys("/");
        assert!(h.core.minibuffer().is_none());
        assert_eq!(h.core.text(), "> /");
        commands(&mut h, &["review"]);
        h.keys("/");
        assert!(h.core.minibuffer().is_none());
        assert_eq!(h.core.text(), "> //");
        // At the start of a prompt that holds text, the command goes before it.
        h.ctx().goto(2).unwrap();
        h.keys("/ RET");
        assert_eq!(h.core.text(), "> /review //");
    }

    #[test]
    fn agent_command_from_the_transcript_inserts_at_the_prompt_start() {
        let mut h = session();
        commands(&mut h, &["compact"]);
        chunk(&mut h, "Hi");
        h.keys("w o r k M-<");
        h.keys("M-x agent-command RET RET");
        assert!(h.core.text().ends_with("> /compact work"));
        assert_eq!(h.core.point(), h.core.text().chars().count() - "work".len());
        // An answer that names no command goes in as typed.
        h.keys("M-x agent-command RET f o o RET");
        assert!(h.core.text().ends_with("> /foo /compact work"));
    }

    #[test]
    fn a_new_sessions_first_prompt_carries_the_roots_preamble() {
        let mut h = work();
        let project = &mut h.core.projects[0].project;
        project.primary.role = "The app".into();
        let lib = Root {
            path: h.path("lib"),
            role: String::new(),
        };
        let docs = Root {
            path: h.path("docs"),
            role: "Docs".into(),
        };
        h.core.projects[0].project.roots = vec![lib, docs];
        h.keys("M-x agent-start RET claude RET");
        let roots: Vec<_> = (h.agent_ops.iter())
            .filter_map(|op| match op {
                AgentOp::Spawn { roots, .. } => Some(roots.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(roots, [vec![h.path("lib"), h.path("docs")]]);
        ready(&mut h, 0);
        // Taken at start: a root added later waits for the next start.
        h.core.projects[0].project.roots.clear();
        ops(&mut h);
        h.keys("h i RET");
        let preamble = format!(
            "Project roots:\n- {} (primary): The app\n- {}\n- {}: Docs",
            h.path("app").display(),
            h.path("lib").display(),
            h.path("docs").display()
        );
        assert_eq!(ops(&mut h), [format!(r#"prompt 0 s {preamble:?} "hi""#)]);
        assert_eq!(h.core.text(), "> hi\n\nWorking for 0s\n\n> ");
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.keys("a g a i n RET");
        assert_eq!(ops(&mut h), [r#"prompt 0 s "again""#]);
    }

    #[test]
    fn an_agent_that_cant_take_the_roots_is_noted() {
        let mut h = session();
        h.send(Msg::AgentUnscoped { id: AgentId(0) });
        assert!(
            messages(&h).ends_with("claude can't take additional directories\n"),
            "{}",
            messages(&h)
        );
    }

    #[test]
    fn replayed_user_messages_show_as_sent_prompts() {
        let mut h = session();
        chunk(&mut h, "a");
        let user = |text: &str| SessionUpdate::UserMessageChunk(ContentChunk::new(text.into()));
        update(&mut h, 0, user("see `b`"));
        let image = ContentBlock::Image(ImageContent::new("", "image/png"));
        update(
            &mut h,
            0,
            SessionUpdate::UserMessageChunk(ContentChunk::new(image)),
        );
        chunk(&mut h, "c");
        let image = h.core.chips.intern(None, false);
        assert_eq!(
            h.core.text(),
            format!("a\n\n> see `b`\n\n> {image}\n\nc\n\n> ")
        );
        let b = props(&h).into_iter().find(|(t, _)| t == "b");
        let code = vec![FaceRef::Face(FaceName::SessionCode)];
        assert_eq!(b, Some(("b".into(), Prop::Named(code))));
    }

    /// Session 0's messages as their ids and text.
    fn message_list(h: &Harness) -> Vec<(Option<String>, String)> {
        let session = &h.core.agents[&AgentId(0)];
        let buffer = h.core.buffers.get(session.buffer).unwrap();
        (session.messages.iter())
            .map(|m| {
                let start = buffer.marker(m.start).unwrap();
                let text = buffer.text().slice(start..start + m.len).to_string();
                (m.id.as_ref().map(|id| id.0.to_string()), text)
            })
            .collect()
    }

    fn agent_chunk(h: &mut Harness, id: &str, text: &str) {
        let chunk = ContentChunk::new(text.into()).message_id(id);
        update(h, 0, SessionUpdate::AgentMessageChunk(chunk));
    }

    #[test]
    fn message_ids_are_kept_and_a_new_id_starts_a_block() {
        let mut h = session();
        agent_chunk(&mut h, "m1", "a");
        agent_chunk(&mut h, "m1", "b");
        agent_chunk(&mut h, "m2", "c");
        assert_eq!(h.core.text(), "ab\n\nc\n\n> ");
        assert_eq!(
            message_list(&h),
            [
                (Some("m1".into()), "ab".into()),
                (Some("m2".into()), "c".into())
            ]
        );
    }

    #[test]
    fn replayed_user_messages_keep_their_ids_in_order() {
        let mut h = session();
        let user = ContentChunk::new("hi".into()).message_id("u1");
        update(&mut h, 0, SessionUpdate::UserMessageChunk(user));
        agent_chunk(&mut h, "m1", "yo");
        assert_eq!(
            message_list(&h),
            [
                (Some("u1".into()), "> hi".into()),
                (Some("m1".into()), "yo".into())
            ]
        );
    }

    #[test]
    fn a_sent_prompt_is_a_message_after_the_last_agent_message() {
        let mut h = session();
        agent_chunk(&mut h, "m1", "yo");
        h.keys("h i RET");
        assert_eq!(
            message_list(&h),
            [(Some("m1".into()), "yo".into()), (None, "> hi".into())]
        );
    }

    #[test]
    fn message_keys_move_past_tool_calls_and_thoughts() {
        let mut h = session();
        agent_chunk(&mut h, "m1", "one\ntwo");
        let call = ToolCall::new("t1", "Read");
        update(&mut h, 0, SessionUpdate::ToolCall(call));
        thought(&mut h, "hmm");
        agent_chunk(&mut h, "m2", "three");
        let text = h.core.text();
        let second = text[..text.find("three").unwrap()].chars().count();
        h.keys("C-c C-p");
        assert_eq!(h.core.point(), second);
        h.keys("C-c C-p");
        assert_eq!(h.core.point(), 0);
        h.keys("C-c C-p");
        assert_eq!(h.core.message(), Some("No message found"));
        assert_eq!(h.core.point(), 0);
        h.keys("C-c C-n");
        assert_eq!(h.core.point(), second);
        h.keys("C-c C-n");
        assert_eq!(h.core.message(), Some("No message found"));
        assert_eq!(h.core.point(), second);
        // From inside a message it goes to that message's start.
        h.keys("C-f C-c C-p");
        assert_eq!(h.core.point(), second);
        // A count stops at the farthest, and a negative one turns back.
        h.keys("C-u 5 C-c C-p");
        assert_eq!(h.core.point(), 0);
        h.keys("C-u - C-c C-p");
        assert_eq!(h.core.point(), second);
    }

    #[test]
    fn the_bar_follows_point_and_each_window_has_its_own() {
        let bar = Decoration::Bar(FaceName::MessageAtPoint);
        let barred = |h: &mut Harness, window: usize| -> Vec<usize> {
            let frame = h.frame(800., 600.);
            let display = &frame.windows[window].display;
            (display.lines.iter())
                .filter(|l| l.decorations.contains(&bar))
                .map(|l| l.number)
                .collect()
        };
        let mut h = session();
        assert!(barred(&mut h, 0).is_empty());
        agent_chunk(&mut h, "m1", "one\ntwo");
        let call = ToolCall::new("t1", "Read");
        update(&mut h, 0, SessionUpdate::ToolCall(call.clone()));
        // Point in the prompt keeps it on the last message, past the tool call.
        assert_eq!(barred(&mut h, 0), [0, 1]);
        agent_chunk(&mut h, "m2", "three");
        assert_eq!(barred(&mut h, 0), [5]);
        h.keys("C-x 2 C-c C-p C-c C-p");
        assert_eq!(barred(&mut h, 0), [0, 1]);
        assert_eq!(barred(&mut h, 1), [5]);
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

    /// Reloads settings that switch to a theme giving `session-thought` and `session-block`
    /// these colours, as a user's `theme` change does.
    fn retheme(h: &mut Harness) -> (u32, u32) {
        let settings = "theme = \"other\"\n[themes.other.faces]\nsession-thought = { fg = 0x010203 }\nsession-block = { bg = 0x040506 }";
        h.send(Msg::SettingsLoaded(Ok(Some(settings.into()))));
        (0x010203, 0x040506)
    }

    /// The face the display gives `text`, a span of the frame.
    fn drawn(h: &mut Harness, text: &str) -> Face {
        h.keys("M-<");
        let frame = h.frame(800., 600.);
        let lines = &frame.windows[0].display.lines;
        let mut spans = lines.iter().flat_map(|l| &l.spans);
        spans.find(|s| s.text == text).expect("drawn").face
    }

    #[test]
    fn a_theme_change_redraws_completed_replies() {
        let mut h = session();
        h.keys("a RET");
        thought(&mut h, "hmm");
        chunk(&mut h, "see `x`\n\n```\nblock\n```");
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.core.frame();
        let before = props(&h);
        let (dim, code) = retheme(&mut h);
        assert_eq!(props(&h), before, "only the display changes");
        assert_eq!(drawn(&mut h, "hmm").fg, Some(dim));
        let frame = h.frame(800., 600.);
        let lines = &frame.windows[0].display.lines;
        let block = (lines.iter())
            .find(|l| l.spans.iter().any(|s| s.text == "block"))
            .expect("drawn");
        let face = Decoration::Background(FaceName::SessionBlock);
        assert_eq!(block.decorations, [face]);
        assert_eq!(
            h.core.settings().theme.face(FaceName::SessionBlock).bg,
            Some(code)
        );
    }

    #[test]
    fn a_tool_calls_rule_names_its_face() {
        let mut h = session();
        tool(&mut h, ls());
        let rules: Vec<_> = (props(&h).into_iter())
            .filter_map(|(_, p)| match p {
                Prop::Line(Decoration::Rule(name)) => Some(name),
                _ => None,
            })
            .collect();
        assert!(
            !rules.is_empty() && rules.iter().all(|&n| n == FaceName::SessionThought),
            "{rules:?}"
        );
    }

    #[test]
    fn faces_sharing_a_colour_part_on_a_theme_change() {
        let mut h = session();
        let same =
            "[themes.default.faces]\nsession-thought = { fg = 5 }\nsession-code = { fg = 5 }";
        h.send(Msg::SettingsLoaded(Ok(Some(same.into()))));
        thought(&mut h, "hmm");
        chunk(&mut h, "`x`");
        h.core.frame();
        let apart = format!(
            "{same}\n[themes.other.faces]\nsession-thought = {{ fg = 6 }}\nsession-code = {{ fg = 7 }}\n"
        );
        let apart = format!("theme = \"other\"\n{apart}");
        h.send(Msg::SettingsLoaded(Ok(Some(apart))));
        assert_eq!(drawn(&mut h, "hmm").fg, Some(6));
        assert_eq!(drawn(&mut h, "x").fg, Some(7));
    }

    #[test]
    fn a_streamed_block_renders_when_the_frame_is_built() {
        let rendered = |h: &Harness| {
            let own = |t: &String| t.starts_with("Work");
            (props(h).into_iter().filter(|(t, _)| !own(t))).collect::<Vec<_>>()
        };
        let mut h = session();
        h.keys("a RET");
        chunk(&mut h, "**a");
        chunk(&mut h, "b**");
        assert_eq!(h.core.text(), "> a\n\n**ab**\n\nWorking for 0s\n\n> ");
        assert!(rendered(&h).is_empty(), "nothing rendered before the frame");
        h.core.frame();
        let bold = Prop::Named(vec![FaceRef::Bold]);
        let hidden = Prop::Invisible(String::new());
        let first = vec![
            ("**".into(), hidden.clone()),
            ("ab".into(), bold),
            ("**".into(), hidden.clone()),
        ];
        assert_eq!(rendered(&h), first);
        // The next reply renders on its own; the first keeps its properties.
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.keys("b RET");
        chunk(&mut h, "*c*");
        assert_eq!(rendered(&h), first);
        h.core.frame();
        let italic = Prop::Named(vec![FaceRef::Italic]);
        let all = [
            first.clone(),
            vec![
                ("*".into(), hidden.clone()),
                ("c".into(), italic),
                ("*".into(), hidden.clone()),
            ],
        ]
        .concat();
        assert_eq!(rendered(&h), all);
        h.core.frame();
        assert_eq!(rendered(&h), all, "a frame without chunks changes nothing");
    }

    #[test]
    fn a_finished_block_renders_before_the_next_starts() {
        let mut h = session();
        chunk(&mut h, "`x`");
        thought(&mut h, "hmm");
        // The message rendered when the thought began; the thought waits for the frame.
        assert_eq!(props(&h).len(), 3);
        h.core.frame();
        let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
        assert_eq!(props(&h).last(), Some(&("hmm".into(), dim)));
    }

    #[test]
    fn hidden_streams_keep_pending_formatting_until_shown() {
        let mut h = session();
        let buffer = h.core.current().name.clone();
        let id = h.core.agents[&AgentId(0)].buffer;
        h.open("app/src/main.rs");
        chunk(&mut h, "**a");
        chunk(&mut h, "b**");
        h.frame(800., 600.);
        let b = h.core.buffers.get(id).unwrap();
        assert!(b.text().to_string().contains("**ab**"));
        assert!(h.core.agents[&AgentId(0)].stream.as_ref().unwrap().dirty);
        h.call(
            "switch-to-buffer",
            Args {
                strings: vec![buffer],
                ..Args::default()
            },
        );
        h.frame(800., 600.);
        assert!(props(&h).contains(&("ab".into(), Prop::Named(vec![FaceRef::Bold]))));
        assert!(!h.core.agents[&AgentId(0)].stream.as_ref().unwrap().dirty);
    }

    #[test]
    fn inactive_panes_duplicate_panes_and_prompt_origins_flush_streams() {
        let mut h = session();
        h.keys("C-x 3");
        chunk(&mut h, "**a**");
        h.frame(800., 600.);
        assert!(props(&h).contains(&("a".into(), Prop::Named(vec![FaceRef::Bold]))));
        h.open("app/src/main.rs");
        chunk(&mut h, " *b*");
        h.frame(800., 600.);
        let id = h.core.agents[&AgentId(0)].buffer;
        assert!(
            h.core
                .buffers
                .get(id)
                .unwrap()
                .properties()
                .any(|(_, p)| *p == Prop::Named(vec![FaceRef::Italic]))
        );
        h.keys("C-x o M-x");
        chunk(&mut h, " `c`");
        h.frame(800., 600.);
        assert!(!h.core.agents[&AgentId(0)].stream.as_ref().unwrap().dirty);
        assert!(
            h.core
                .buffers
                .get(id)
                .unwrap()
                .properties()
                .any(|(_, p)| *p == Prop::Named(vec![FaceRef::Face(FaceName::SessionCode)]))
        );
    }

    #[test]
    fn inactive_workspace_streams_finalize_hidden_blocks_and_kill_safely() {
        let mut h = session();
        let workspace = h.core.current_workspace().id;
        let project = h.core.current_workspace().project;
        h.core.new_workspace("other", project);
        chunk(&mut h, "**a**");
        h.frame(800., 600.);
        assert!(h.core.agents[&AgentId(0)].stream.as_ref().unwrap().dirty);
        thought(&mut h, "hmm");
        let id = h.core.agents[&AgentId(0)].buffer;
        assert!(
            h.core
                .buffers
                .get(id)
                .unwrap()
                .properties()
                .any(|(_, p)| *p == Prop::Named(vec![FaceRef::Bold]))
        );
        h.core.switch_workspace(workspace).unwrap();
        h.frame(800., 600.);
        assert!(props(&h).contains(&(
            "hmm".into(),
            Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)])
        )));
        h.keys("C-x k RET yes RET");
        h.frame(800., 600.);
        assert!(h.core.buffers.get(id).is_none());
    }

    #[test]
    fn the_echoed_prompt_renders_as_markdown() {
        let mut h = session();
        h.keys("* * h i * * RET");
        assert_eq!(h.core.text(), "> **hi**\n\nWorking for 0s\n\n> ");
        let bold = Prop::Named(vec![FaceRef::Bold]);
        assert!(props(&h).contains(&("hi".into(), bold)));
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
        let code = Decoration::Background(FaceName::SessionBlock);
        assert!((4..7).all(|i| lines[i].decorations.contains(&code)));
        assert!(!lines[3].decorations.contains(&code));
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
        let settings = h.core.settings();
        let height = rows as f32 * settings.line_height() + settings.mode_line_height();
        let frame = h.frame(800., height);
        (frame.windows[0].display.lines.iter())
            .map(|l| {
                // Right-aligning padding reads as the one space it is in the text.
                let padding = |s: &str| s.len() > 1 && s.trim().is_empty();
                let text = |(i, s): (usize, &crate::display::Span)| match i > 0 && padding(&s.text)
                {
                    true => " ".to_string(),
                    false => s.text.clone(),
                };
                l.spans.iter().enumerate().map(text).collect()
            })
            .collect()
    }

    /// An `execute` call `t` titled `` `ls` `` with the input `{"command":"ls"}`.
    fn ls() -> ToolCall {
        ToolCall::new("t", "`ls`")
            .kind(ToolKind::Execute)
            .raw_input(serde_json::json!({"command": "ls"}))
    }

    const LS_INPUT: &str = "```\n$ ls\n```";

    #[test]
    fn a_tool_call_starts_folded_and_updates_in_place() {
        let mut h = session();
        h.keys("a RET");
        chunk(&mut h, "Let me look.");
        tool(&mut h, ls());
        assert_eq!(
            h.core.text(),
            format!("> a\n\nLet me look.\n\n▸ ●    Running ls\n{LS_INPUT}\n\nWorking for 0s\n\n> ")
        );
        assert_eq!(
            shown(&mut h, 10)[..5],
            ["> a", "", "Let me look.", "", "▸ ●   Running ls"]
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
            "> a\n\nLet me look.\n\n▸ ✓    Ran ls 0.0s\n```\n$ ls\na.rs\nb.rs\n```\n\n\
             Done.\n\nWorking for 0s\n\n> "
        );
        assert_eq!(shown(&mut h, 12)[4..7], ["▸ ✓   Ran ls 0.0s", "", "Done."]);
        // Props stay in order, so the display can bisect them.
        let props: Vec<_> = h.core.current().properties().map(|(r, _)| r).collect();
        assert!(
            props
                .windows(2)
                .all(|w| w[0].start <= w[1].start && w[0].end <= w[1].end)
        );
        // The header carries the rule and its icon, the verb is dim and the target isn't.
        let frame = h.frame(800., 600.);
        let header = &frame.windows[0].display.lines[4];
        let dim = FaceName::SessionThought;
        assert_eq!(header.decorations, [Decoration::Rule(dim)]);
        assert_eq!(header.icons, [(4, Icon::Terminal)]);
        let span = |text: &str| header.spans.iter().find(|s| s.text.contains(text)).unwrap();
        let theme = &h.core.settings().theme;
        assert_eq!(span("Ran").face.fg, theme.face(dim).fg);
        assert_ne!(span("ls").face.fg, theme.face(dim).fg);
    }

    #[test]
    fn a_tool_call_without_content_is_its_header() {
        let mut h = session();
        tool(&mut h, ToolCall::new("t", "Think"));
        assert_eq!(h.core.text(), "▸ ●    Using Think\n\n> ");
        assert_eq!(shown(&mut h, 5)[0], "▸ ●   Using Think");
        // An update for an unknown call without a title shows nothing.
        let fields = ToolCallUpdateFields::new().status(ToolCallStatus::Failed);
        tool_update(&mut h, "u", fields.clone());
        assert_eq!(h.core.text(), "▸ ●    Using Think\n\n> ");
        // With a title it starts a block.
        tool_update(&mut h, "u", fields.title("Grep".to_string()));
        assert!(h.core.text().ends_with("▸ ✗    Used Grep 0.0s\n\n> "));
    }

    #[test]
    fn tab_on_a_header_shows_and_hides_its_content() {
        let mut h = session();
        tool(&mut h, ls());
        h.keys("TAB");
        assert_eq!(h.core.message(), Some("No tool call here"));
        h.keys("M-< C-e TAB");
        let open = ["▾ ●   Running ls", "", "$ ls", "", "", "> "];
        assert_eq!(shown(&mut h, 10)[..6], open);
        let frame = h.frame(800., 600.);
        let lines = &frame.windows[0].display.lines;
        let rule = Decoration::Rule(FaceName::SessionThought);
        assert!(
            (0..4).all(|i| lines[i].decorations.contains(&rule)),
            "{lines:?}"
        );
        assert!(!lines[4].decorations.contains(&rule));
        // An update keeps it open.
        let output = ToolCallUpdateFields::new().raw_output(serde_json::json!("a.rs"));
        tool_update(&mut h, "t", output);
        assert_eq!(shown(&mut h, 12)[1..5], ["", "$ ls", "a.rs", ""]);
        // TAB inside the open content folds it, point going to the header.
        h.keys("C-n C-n C-n TAB");
        assert_eq!(shown(&mut h, 5)[..3], ["▸ ●   Running ls", "", "> "]);
        assert_eq!(h.core.point(), 0);
    }

    #[test]
    fn line_motion_and_scrolling_pass_a_folded_call() {
        let mut h = session();
        let output: String = (0..50).map(|i| format!("line {i}\n")).collect();
        tool(&mut h, ls().raw_output(serde_json::json!(output)));
        // The window keeps the header in view above the prompt line.
        assert_eq!(shown(&mut h, 5)[..3], ["▸ ●   Running ls", "", "> "]);
        h.keys("M-< C-n");
        assert_eq!(h.core.current().line_col(h.core.point()), Ok((54, 0)));
        h.keys("C-p");
        assert_eq!(h.core.point(), 0);
        h.keys("TAB C-n");
        assert_eq!(h.core.current().line_col(h.core.point()), Ok((1, 0)));
        // Open, its first lines show and the rest is counted.
        let lines = shown(&mut h, 70);
        assert!((0..20).all(|i| lines.contains(&format!("line {i}"))));
        assert!(lines.contains(&"… 30 more lines".to_string()));
    }

    #[test]
    fn a_repeated_tool_call_replaces_it_and_point_stays_on_its_header() {
        let mut h = session();
        tool(&mut h, ls());
        h.keys("M-< C-e TAB");
        tool(&mut h, ls().status(ToolCallStatus::Completed));
        assert_eq!(h.core.text().matches("Ran ls").count(), 1);
        assert_eq!(shown(&mut h, 3)[..2], ["▾ ✓   Ran ls 0.0s", ""]);
        assert_eq!(h.core.current().line_col(h.core.point()).unwrap().0, 0);
        // TAB on a call without content leaves it folded.
        chunk(&mut h, "Next.");
        tool(&mut h, ToolCall::new("e", "Empty"));
        h.keys("M-> C-p C-p TAB");
        let content = ToolCallUpdateFields::new().raw_output(serde_json::json!("x"));
        tool_update(&mut h, "e", content);
        assert!(shown(&mut h, 20).contains(&"▸ ●   Using Empty".to_string()));
    }

    /// `ls` finished with `lines` lines of output, opened, point on its header.
    fn long_ls(lines: usize) -> Harness {
        let mut h = session();
        let output: String = (1..=lines).map(|i| format!("line {i}\n")).collect();
        let call = ls().status(ToolCallStatus::Completed);
        tool(&mut h, call.raw_output(serde_json::json!(output)));
        h.keys("M-< C-e TAB");
        h
    }

    #[test]
    fn headers_name_the_kind_and_its_target() {
        let header = |call: ToolCall| {
            let mut h = session();
            tool(&mut h, call);
            let lines = h.core.text();
            lines.lines().next().unwrap().to_string()
        };
        let at = |path: &str| vec![ToolCallLocation::new(path)];
        let read = ToolCall::new("t", "Read File").kind(ToolKind::Read);
        let path = read.clone().locations(at("/aiditor-test/app/src/a.rs"));
        assert_eq!(header(path.clone()), "▸ ●    Reading src/a.rs");
        let done = path.status(ToolCallStatus::Completed);
        assert_eq!(header(done), "▸ ✓    Read src/a.rs 0.0s");
        // Outside the primary root the path stays whole, and without one the title shows.
        assert_eq!(
            header(read.clone().locations(at("/etc/hosts"))),
            "▸ ●    Reading /etc/hosts"
        );
        assert_eq!(header(read), "▸ ●    Reading Read File");
        // A diff's path stands in for a location.
        let diff = agent_client_protocol_schema::v1::Diff::new("/aiditor-test/app/a.rs", "b\n");
        let edit = ToolCall::new("t", "Editing files").kind(ToolKind::Edit);
        let edit = edit.content(vec![ToolCallContent::Diff(diff)]);
        assert_eq!(header(edit), "▸ ●    Editing a.rs");
        // A search keeps its pattern, whatever it found.
        let search = ToolCall::new("t", "grep x").kind(ToolKind::Search);
        assert_eq!(
            header(search.locations(at("/x"))),
            "▸ ●    Searching grep x"
        );
        let fetch = ToolCall::new("t", "Fetch").kind(ToolKind::Fetch);
        let fetch = fetch.raw_input(serde_json::json!({"url": "https://x.dev"}));
        assert_eq!(header(fetch), "▸ ●    Fetching https://x.dev");
        let two = ToolCall::new("t", "`a`").kind(ToolKind::Execute);
        let two = two.raw_input(serde_json::json!({"command": "a\nb"}));
        assert_eq!(header(two.clone()), "▸ ●    Running a …");
        // Backticks in a title go; a pending delete still has its verb.
        assert_eq!(header(ToolCall::new("t", "`rm`")), "▸ ●    Using rm");
        let gone = ToolCall::new("t", "x").kind(ToolKind::Delete);
        assert_eq!(header(gone.locations(at("/y"))), "▸ ●    Deleting /y");
        let moved = ToolCall::new("t", "x").kind(ToolKind::Move);
        let moved = moved.status(ToolCallStatus::Failed).locations(at("/y"));
        assert_eq!(header(moved), "▸ ✗    Moved /y 0.0s");
    }

    #[test]
    fn each_kind_has_its_icon() {
        for (kind, icon) in [
            (ToolKind::Read, Icon::Book),
            (ToolKind::Search, Icon::Magnifier),
            (ToolKind::Edit, Icon::Pencil),
            (ToolKind::Delete, Icon::Pencil),
            (ToolKind::Move, Icon::Pencil),
            (ToolKind::Execute, Icon::Terminal),
            (ToolKind::Fetch, Icon::Globe),
            (ToolKind::Other, Icon::Wrench),
        ] {
            let mut h = session();
            tool(&mut h, ToolCall::new("t", "x").kind(kind));
            let frame = h.frame(800., 600.);
            assert_eq!(frame.windows[0].display.lines[0].icons, [(4, icon)]);
        }
    }

    #[test]
    fn a_failed_header_marks_its_status_and_exit() {
        let mut h = session();
        tool(&mut h, command());
        tool_update(
            &mut h,
            "t",
            ToolCallUpdateFields::new().status(ToolCallStatus::Failed),
        );
        let exit = serde_json::json!({
            "terminal_exit": {"terminal_id": "t", "exit_code": 2, "signal": null}
        });
        meta_update(&mut h, ToolCallUpdateFields::new(), exit);
        let frame = h.frame(800., 600.);
        let line = &frame.windows[0].display.lines[0];
        let span = |text: &str| line.spans.iter().find(|s| s.text.contains(text)).unwrap();
        let failed = h.core.settings().theme.face(FaceName::StatusFailed).fg;
        assert_eq!(span("✗").face.fg, failed);
        assert_eq!(span("exit 2").face.fg, failed);
        // Running and finished calls share the green status face.
        let mut h = session();
        tool(&mut h, ls());
        let face = |h: &mut Harness, mark: &str| {
            let frame = h.frame(800., 600.);
            let line = &frame.windows[0].display.lines[0];
            (line.spans.iter())
                .find(|s| s.text.contains(mark))
                .unwrap()
                .face
                .fg
        };
        let theme = h.core.settings().theme.clone();
        assert_eq!(face(&mut h, "●"), theme.face(FaceName::StatusRunning).fg);
        tool(&mut h, ls().status(ToolCallStatus::Completed));
        assert_eq!(face(&mut h, "✓"), theme.face(FaceName::StatusRunning).fg);
    }

    #[test]
    fn tab_changes_the_fold_marker_and_point_stays() {
        let mut h = session();
        tool(&mut h, ls());
        h.keys("M-<");
        h.keys("TAB");
        assert!(h.core.text().starts_with("▾ ●"));
        assert_eq!(h.core.point(), 0);
        h.keys("C-e TAB");
        assert!(h.core.text().starts_with("▸ ●"));
        assert_eq!(h.core.current().line_col(h.core.point()).unwrap().0, 0);
        assert!(h.core.point() > 0);
    }

    #[test]
    fn an_open_command_shows_twenty_lines_and_counts_the_rest() {
        let mut h = long_ls(50);
        let lines = shown(&mut h, 30);
        assert_eq!(
            lines[..3],
            ["▾ ✓   Ran ls 0.0s".to_string(), "".into(), "$ ls".into()]
        );
        assert_eq!(
            (lines[3].as_str(), lines[22].as_str()),
            ("line 1", "line 20")
        );
        assert_eq!(lines[23], "… 30 more lines");
        assert_eq!(lines[24..27], ["", "", "> "]);
        // The rest stays in the text, and props stay in order for bisecting.
        assert!(h.core.text().contains("line 50\n"));
        let props: Vec<_> = h.core.current().properties().map(|(r, _)| r).collect();
        assert!(
            props
                .windows(2)
                .all(|w| w[0].start <= w[1].start && w[0].end <= w[1].end)
        );
        // The count's line keeps the rule and the block's background.
        let frame = h.frame(800., 700.);
        let more = &frame.windows[0].display.lines[23];
        assert!(
            more.decorations
                .contains(&Decoration::Rule(FaceName::SessionThought))
        );
        assert!(
            more.decorations
                .contains(&Decoration::Background(FaceName::SessionBlock))
        );
        // Motion passes the count, and one line more reads singular.
        let mut h = long_ls(21);
        assert_eq!(shown(&mut h, 30)[23], "… 1 more line");
        h.keys("M-> C-p C-p C-p C-p");
        assert_eq!(h.core.current().line_col(h.core.point()).unwrap().0, 22);
    }

    #[test]
    fn twenty_lines_show_whole() {
        let mut h = long_ls(20);
        let lines = shown(&mut h, 30);
        assert_eq!(lines[22], "line 20");
        assert!(!lines.iter().any(|l| l.contains("more line")));
    }

    #[test]
    fn output_streams_past_the_cut_of_an_open_call() {
        let mut h = session();
        tool(
            &mut h,
            command().raw_input(serde_json::json!({"command": "make"})),
        );
        output(&mut h, "terminal_output", "t", "first\n");
        h.frame(800., 600.);
        h.keys("M-< TAB");
        let more: String = (2..=25)
            .map(|i| format!("\u{1b}[31mline {i}\u{1b}[0m\n"))
            .collect();
        output(&mut h, "terminal_output", "t", &more);
        let lines = shown(&mut h, 30);
        assert_eq!(
            (lines[3].as_str(), lines[22].as_str()),
            ("first", "line 20")
        );
        assert_eq!(lines[23], "… 5 more lines");
        output(&mut h, "terminal_output", "t", "line 26\n");
        assert_eq!(shown(&mut h, 30)[23], "… 6 more lines");
        // A folded call's tail goes the short way and opens the same.
        h.keys("TAB");
        output(&mut h, "terminal_output", "t", "line 27\n");
        h.frame(800., 600.);
        h.keys("TAB");
        assert_eq!(shown(&mut h, 30)[23], "… 7 more lines");
    }

    #[test]
    fn backticks_in_a_command_and_its_output_stay_fenced() {
        let mut h = session();
        let call = command().raw_input(serde_json::json!({"command": "echo ````"}));
        tool(&mut h, call);
        output(&mut h, "terminal_output", "t", "ok\n");
        h.frame(800., 600.);
        output(&mut h, "terminal_output", "t", "more\n");
        h.frame(800., 600.);
        let text = h.core.text();
        assert!(
            text.contains("\n`````\n$ echo ````\nok\nmore\n`````\n"),
            "{text}"
        );
    }

    #[test]
    fn other_kinds_stay_uncut_and_edits_show_their_diff() {
        let mut h = session();
        let output: String = (1..=50).map(|i| format!("line {i}\n")).collect();
        let read = ToolCall::new("t", "x").kind(ToolKind::Read);
        tool(&mut h, read.raw_output(serde_json::json!(output)));
        h.keys("M-< TAB");
        let lines = shown(&mut h, 60);
        assert!(lines.contains(&"Output:".to_string()) && lines.contains(&"line 50".to_string()));
        let mut h = session();
        let diff = agent_client_protocol_schema::v1::Diff::new("a.rs", "b\nc\n");
        let edit = ToolCall::new("t", "Edit")
            .kind(ToolKind::Edit)
            .content(vec![ToolCallContent::Diff(
                diff.old_text("b\n".to_string()),
            )]);
        tool(
            &mut h,
            edit.raw_input(serde_json::json!({"file_path": "a.rs"})),
        );
        h.keys("M-< TAB");
        assert!(!h.core.text().contains("Input:"));
        let added = drawn(&mut h, "+c").fg;
        let theme = &h.core.settings().theme;
        assert_eq!(added, theme.face(FaceName::DiffAdded).fg);
        assert_eq!(
            drawn(&mut h, "@@ -1 +1,2 @@").fg,
            h.core.settings().theme.face(FaceName::DiffHunk).fg
        );
    }

    /// A session whose project indexes `src/a.rs` and `src/main.rs`, `a.rs` three lines long, and
    /// `lib/b.rs` in a second root.
    fn indexed_session() -> Harness {
        let mut h = session();
        h.index_reply("app", &["src/a.rs", "src/main.rs"], &["app", "app/src"]);
        let other = crate::project::Root {
            path: h.path("other"),
            role: String::new(),
        };
        h.core.projects[0].project.roots.push(other);
        h.index_reply("other", &["lib/b.rs"], &["other", "other/lib"]);
        h.read_reply("app/src/a.rs", Some(b"one\ntwo\nthree\n"));
        h.keys("M-x project-index RET home RET");
        h
    }

    /// Whether `text` draws as a link.
    fn linked(h: &mut Harness, text: &str) -> bool {
        let link = h.core.settings().theme.face(FaceName::Link).fg;
        let face = drawn(h, text);
        face.underline && face.fg == link
    }

    /// Puts point `n` chars into the first `text` in the buffer.
    fn point_in(h: &mut Harness, text: &str, n: usize) {
        let all = h.core.text();
        let at = all[..all.find(text).expect("in the buffer")]
            .chars()
            .count();
        h.ctx().goto(at + n).unwrap();
    }

    #[test]
    fn existing_paths_in_messages_are_links() {
        let mut h = indexed_session();
        h.keys("a RET");
        chunk(&mut h, "see `src/a.rs:2`, `lib/b.rs`, not `src/gone.rs`");
        prompted(&mut h, Ok(StopReason::EndTurn));
        assert!(linked(&mut h, "src/a.rs:2"));
        assert!(linked(&mut h, "lib/b.rs"), "a second root's file");
        assert!(!linked(&mut h, "src/gone.rs"));
    }

    /// A chip for `path`, a directory when `dir`, typed at point.
    fn put_chip(h: &mut Harness, path: &str, dir: bool) -> char {
        let chip = h.core.chips.intern(Some(path.into()), dir);
        h.send(Msg::Insert {
            text: chip.to_string(),
            replace: None,
        });
        chip
    }

    /// The display line holding `text`.
    fn drawn_line(h: &mut Harness, text: &str) -> crate::display::Line {
        let mut frame = h.frame(800., 600.);
        let lines = std::mem::take(&mut frame.windows[0].display.lines);
        let line = lines.into_iter().find(|l| {
            let drawn: String = l.spans.iter().map(|s| s.text.as_str()).collect();
            drawn.contains(text)
        });
        line.unwrap_or_else(|| panic!("{text:?} drawn"))
    }

    #[test]
    fn a_chip_draws_as_its_icon_and_name_and_edits_as_one_char() {
        let mut h = session();
        h.keys("x");
        let chip = put_chip(&mut h, "/p/src/main.rs", false);
        h.keys("y");
        let line = drawn_line(&mut h, "x ▤ main.rs y");
        let attachment = h.core.settings().theme.face(FaceName::Attachment);
        let name = line.spans.iter().find(|s| s.text == " main.rs ").unwrap();
        assert_eq!(name.face.boxed, attachment.boxed);
        h.keys("C-b C-b C-f");
        assert_eq!(h.core.point(), 4, "past the chip");
        h.keys("DEL");
        assert_eq!(h.core.text(), "> xy");
        h.keys("C-/");
        assert_eq!(h.core.text(), format!("> x{chip}y"));
        drawn_line(&mut h, "x ▤ main.rs y");
        h.keys("C-e C-b C-b C-k C-y C-y");
        drawn_line(&mut h, "x ▤ main.rs y ▤ main.rs y");
    }

    #[test]
    fn chips_go_as_resource_links_and_images_and_show_in_the_echo() {
        let mut h = session();
        h.keys("l o o k SPC");
        put_chip(&mut h, "/p/a.rs", false);
        put_chip(&mut h, "/p/src", true);
        put_chip(&mut h, "/p/Cat.JPG", false);
        h.keys("SPC o k RET");
        assert_eq!(
            ops(&mut h),
            [concat!(
                r#"prompt 0 s "look " link a.rs file:///p/a.rs link src file:///p/src/ "#,
                r#"image image/jpeg file:///p/Cat.JPG " ok""#
            )]
        );
        drawn_line(&mut h, "> look  ▤ a.rs  ▦ src  ▣ Cat.JPG  ok");
        // The agent service answers an unreadable image as the prompt's error.
        prompted(&mut h, Err("can't read /p/Cat.JPG: not found"));
        assert!(
            h.core
                .text()
                .contains("Error: can't read /p/Cat.JPG: not found\n\nWorked")
        );
    }

    #[test]
    fn replayed_links_and_images_are_chips_in_one_message() {
        let mut h = session();
        let user = |block: ContentBlock| {
            let chunk = ContentChunk::new(block).message_id(Some("u1".into()));
            SessionUpdate::UserMessageChunk(chunk)
        };
        for text in ["look at ", "[@a.rs](file:///p/a.rs)", " please"] {
            update(&mut h, 0, user(text.into()));
        }
        update(
            &mut h,
            0,
            user(ContentBlock::Image(ImageContent::new("Zm9v", "image/png"))),
        );
        chunk(&mut h, "ok");
        let a = h.core.chips.intern(Some("/p/a.rs".into()), false);
        let image = h.core.chips.intern(None, false);
        assert_eq!(
            h.core.text(),
            format!("> look at {a} please{image}\n\nok\n\n> ")
        );
        assert_eq!(message_list(&h).len(), 2);
        drawn_line(&mut h, "> look at  ▤ a.rs  please ▣ image ");
    }

    #[test]
    fn replayed_cli_mentions_of_indexed_files_are_chips() {
        let mut h = indexed_session();
        let user = |text: &str| SessionUpdate::UserMessageChunk(ContentChunk::new(text.into()));
        update(&mut h, 0, user("@src/a.rs vs @src/gone.rs"));
        let a = h.core.chips.intern(Some(h.path("app/src/a.rs")), false);
        assert!(h.core.text().starts_with(&format!("> {a} vs @src/gone.rs")));
        drawn_line(&mut h, ">  ▤ a.rs  vs @src/gone.rs");
    }

    #[test]
    fn word_motion_takes_a_chip_as_a_word_of_its_own() {
        let mut h = session();
        h.keys("a b SPC");
        let chip = put_chip(&mut h, "/p/a.rs", false);
        h.keys("c d");
        // `> ab <chip>cd`: the chip is at 5.
        h.keys("C-a M-f M-f");
        assert_eq!(h.core.point(), 6, "after the chip");
        h.keys("M-f");
        assert_eq!(h.core.point(), 8, "the word after it");
        h.keys("M-b M-b");
        assert_eq!(h.core.point(), 5, "back before the chip");
        h.keys("C-f M-DEL");
        assert_eq!(h.core.text(), "> ab cd", "it kills alone");
        h.keys("C-y");
        assert_eq!(h.core.text(), format!("> ab {chip}cd"), "and yanks back");
    }

    #[test]
    fn ret_on_an_image_chip_opens_it_with_the_system_and_a_gone_file_echoes() {
        let mut h = session();
        put_chip(&mut h, "/p/cat.png", false);
        h.keys("C-b RET");
        let opened = (h.effects.iter()).any(
            |e| matches!(e, Effect::OpenWithSystem(p) if p == std::path::Path::new("/p/cat.png")),
        );
        assert!(opened);
        assert_eq!(h.core.windows.tree.leaves().len(), 1, "no split");
        h.checks.insert("/p/gone.rs".into(), None);
        h.keys("C-e");
        put_chip(&mut h, "/p/gone.rs", false);
        h.keys("C-b RET");
        assert_eq!(h.core.message.as_deref(), Some("/p/gone.rs: no such file"));
        assert_eq!(h.core.windows.tree.leaves().len(), 1, "no split");
        assert!(ops(&mut h).is_empty(), "nothing is sent");
    }

    #[test]
    fn ret_on_a_chip_visits_it_and_the_echo_shows_its_path() {
        let mut h = indexed_session();
        let a = h.path("app/src/a.rs");
        put_chip(&mut h, &a.to_string_lossy(), false);
        h.keys("C-b");
        assert_eq!(h.core.message.as_deref(), Some(&*a.to_string_lossy()));
        h.keys("RET");
        assert!(ops(&mut h).is_empty(), "nothing is sent");
        assert_eq!(h.core.windows.tree.leaves().len(), 2);
        assert_eq!(h.core.current().file, Some(a));
    }

    #[test]
    fn ret_on_a_directory_chip_opens_find_file_there_and_a_pathless_one_echoes() {
        let mut h = session();
        put_chip(&mut h, "/p/src", true);
        h.keys("C-b RET");
        assert_eq!(h.core.current().text().to_string(), "/p/src/");
        h.keys("C-g");
        let image = h.core.chips.intern(None, false);
        h.send(Msg::Insert {
            text: image.to_string(),
            replace: None,
        });
        h.keys("C-b RET");
        assert_eq!(h.core.message.as_deref(), Some("No file"));
        assert!(ops(&mut h).is_empty(), "nothing is sent");
    }

    #[test]
    fn a_gone_file_is_struck_through_wherever_its_chip_shows() {
        let mut h = session();
        h.checks.insert("/p/gone.rs".into(), None);
        put_chip(&mut h, "/p/gone.rs", false);
        h.keys("RET");
        let theme = h.core.settings().theme.clone();
        let line = drawn_line(&mut h, ">  ▤ gone.rs");
        let icon = line.spans.iter().find(|s| s.text == "▤").unwrap();
        assert_eq!(icon.face.fg, theme.face(FaceName::StatusFailed).fg);
        assert!(
            line.spans
                .iter()
                .find(|s| s.text == " gone.rs ")
                .unwrap()
                .face
                .strike
        );
        // Back on disk, as the watcher reports.
        h.checks.insert("/p/gone.rs".into(), Some(1));
        h.send(Msg::FilesChanged {
            paths: vec![("/p".into(), crate::Disk::Dir)],
            rescan: false,
        });
        let line = drawn_line(&mut h, ">  ▤ gone.rs");
        assert!(
            !line
                .spans
                .iter()
                .find(|s| s.text == " gone.rs ")
                .unwrap()
                .face
                .strike
        );
    }

    #[test]
    fn ret_on_a_message_link_visits_its_line_in_another_window() {
        let mut h = indexed_session();
        h.keys("a RET");
        chunk(&mut h, "see src/a.rs:3 here");
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.frame(800., 600.);
        ops(&mut h);
        point_in(&mut h, "src/a.rs:3", 2);
        h.keys("RET");
        assert!(ops(&mut h).is_empty(), "nothing is sent");
        assert_eq!(h.core.windows.tree.leaves().len(), 2);
        assert_eq!(h.core.current().file, Some(h.path("app/src/a.rs")));
        assert_eq!(h.core.current().line_col(h.point()).unwrap(), (2, 0));
    }

    #[test]
    fn tool_headers_link_indexed_files_and_ret_visits_them() {
        let mut h = indexed_session();
        let read = |id: &str, path: PathBuf| {
            let location = ToolCallLocation::new(path).line(2);
            ToolCall::new(id.to_string(), "Read")
                .kind(ToolKind::Read)
                .locations(vec![location])
        };
        let a = h.path("app/src/a.rs");
        // Two calls would fold into a group.
        let mut outside = indexed_session();
        tool(&mut outside, read("u", "/etc/hosts".into()));
        // Plain, it runs on from the space before it.
        assert!(!linked(&mut outside, " /etc/hosts"));
        tool(&mut h, read("t", a));
        assert!(linked(&mut h, "src/a.rs"));
        // Off the link, the header still opens the output.
        point_in(&mut h, "Reading src/a.rs", 1);
        h.keys("RET");
        assert_eq!(h.core.message.as_deref(), Some("No output"));
        point_in(&mut h, "src/a.rs", 1);
        h.keys("RET");
        assert_eq!(h.core.current().file, Some(h.path("app/src/a.rs")));
        assert_eq!(h.core.current().line_col(h.point()).unwrap(), (1, 0));
    }

    #[test]
    fn ret_on_a_header_opens_the_full_output() {
        let other = |h: &Harness| h.core.windows.others.values().next().map(|w| w.buffer());
        let mut h = session();
        let call = command().raw_input(serde_json::json!({"command": "make"}));
        tool(&mut h, call);
        output(&mut h, "terminal_output", "t", "\u{1b}[31mred\u{1b}[0m\n");
        h.frame(800., 600.);
        ops(&mut h);
        h.keys("x M-< RET");
        assert!(ops(&mut h).is_empty(), "nothing is sent");
        let id = other(&h).expect("shown in another window");
        let buffer = h.core.buffers.get(id).unwrap();
        assert!(buffer.read_only);
        assert_eq!(buffer.name, "*claude output: make*<work>");
        assert_eq!(buffer.text().to_string(), "red\n");
        assert_eq!(buffer.properties().count(), 1);
        // RET again refreshes that buffer.
        output(&mut h, "terminal_output", "t", "more\n");
        h.frame(800., 600.);
        h.keys("RET");
        let text = h.core.buffers.get(id).unwrap().text().to_string();
        assert_eq!((other(&h), text.as_str()), (Some(id), "red\nmore\n"));
        // A killed one comes back.
        h.keys("C-x o C-x k RET C-x o");
        assert!(h.core.buffers.get(id).is_none());
        h.keys("M-< RET");
        let again = other(&h).unwrap();
        assert_ne!(again, id);
        assert_eq!(
            h.core.buffers.get(again).unwrap().text().to_string(),
            "red\nmore\n"
        );
    }

    #[test]
    fn ret_on_a_header_works_during_a_turn_and_after_the_session_ended() {
        let mut h = session();
        h.keys("a RET");
        tool(&mut h, ls().raw_output(serde_json::json!("x")));
        h.keys("M-< C-n C-n RET");
        assert_eq!(h.core.message(), None);
        let other = |h: &Harness| h.core.windows.others.values().next().map(|w| w.buffer());
        let id = other(&h).expect("shown during the turn");
        h.keys("C-x 1");
        exited(&mut h, 0, 0);
        h.keys("RET");
        assert_eq!(other(&h), Some(id));
    }

    #[test]
    fn ret_on_a_call_without_output_echoes() {
        let mut h = session();
        tool(&mut h, ToolCall::new("t", "Think"));
        h.keys("M-< RET");
        assert_eq!(h.core.message(), Some("No output"));
    }

    #[test]
    fn full_output_prefers_diffs_then_raw_output_then_text() {
        let diff = agent_client_protocol_schema::v1::Diff::new("a", "b\n");
        let edit = ToolCall::new("t", "Edit").content(vec![ToolCallContent::Diff(diff)]);
        let (text, faces) = tool_output(&edit, None).unwrap();
        assert!(text.contains("+b"));
        assert!(!faces.is_empty());
        let raw = ToolCall::new("t", "x")
            .content(vec!["seen".into()])
            .raw_output(serde_json::json!("all"));
        assert_eq!(tool_output(&raw, None).unwrap().0, "all");
        let item = ToolCall::new("t", "x").content(vec!["seen".into()]);
        assert_eq!(tool_output(&item, None).unwrap().0, "seen\n");
        assert!(tool_output(&ToolCall::new("t", "x"), None).is_none());
    }

    #[test]
    fn tool_content_is_uncut_in_long_enough_fences() {
        let mut call = ToolCall::new("t", "Run\nmore");
        assert_eq!(
            tool_header(&call, None, false, "/x".as_ref()),
            "▸ ●    Using Run …"
        );
        // A list of text blocks shows as their text; other output as JSON.
        let content = |call: &ToolCall| tool_content(call, None).text;
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
        let blue = vec![FaceRef::Fg(4)];
        let content = tool_content(&call, None);
        assert_eq!(content.text, "Input:\n```json\n1\n```\n\nfile");
        assert_eq!(content.faces, vec![(22..26, blue)]);
    }

    #[test]
    fn sgr_escapes_become_faces() {
        use FaceRef::{Bg, Bold, Fg, Underline};
        assert_eq!(
            coloured("\u{1b}[1;4;92mok\u{1b}[22mx\u{1b}[0my"),
            (
                "okxy".into(),
                vec![
                    (0..2, vec![Bold, Underline, Fg(10)]),
                    (2..3, vec![Underline, Fg(10)])
                ]
            )
        );
        // Colours beyond the 16 show in the default colour.
        let beyond = "\u{1b}[38;5;196ma\u{1b}[38;2;1;2;3mb\u{1b}[38;5;4mc";
        assert_eq!(coloured(beyond), ("abc".into(), vec![(2..3, vec![Fg(4)])]));
        // A face stops at each line's end; `39` and `49` reset the colours.
        assert_eq!(
            coloured("\u{1b}[48;5;4ma\u{1b}[101mb"),
            ("ab".into(), vec![(0..1, vec![Bg(4)]), (1..2, vec![Bg(9)])])
        );
        assert_eq!(
            coloured("\u{1b}[32;41ma\r\nb\u{1b}[39;49mc\u{1b}]0;title\u{7}"),
            (
                "a\nbc".into(),
                vec![(0..1, vec![Fg(2), Bg(1)]), (2..3, vec![Fg(2), Bg(1)])]
            )
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
        assert_eq!(h.core.text(), "▸ ●    Running make\n\n> ");
        output(&mut h, "terminal_output_delta", "t", "\u{1b}[31mre");
        output(&mut h, "terminal_output", "t", "d\u{1b}[0m\r\n");
        output(&mut h, "terminal_output", "other", "lost");
        assert_eq!(h.core.text(), "▸ ●    Running make\n\n> ");
        h.frame(800., 600.);
        assert_eq!(
            h.core.text(),
            "▸ ●    Running make\nOutput:\n```\nred\n```\n\n> "
        );
        // Its colour shows once the call is open.
        h.keys("M-< C-e TAB");
        let red = h.core.settings().theme.ansi[1];
        assert_eq!(drawn(&mut h, "red").fg, Some(red));
    }

    #[test]
    fn folded_and_open_output_agree_across_chunk_boundaries() {
        let (mut folded, mut open) = (session(), session());
        for h in [&mut folded, &mut open] {
            tool(h, command());
            output(h, "terminal_output", "t", "initial\n");
            h.frame(800., 600.);
        }
        open.keys("M-< C-e TAB");
        for text in [
            "\u{1b}[3",
            "1mred",
            "der\u{1b}[0m\n",
            "```",
            "`été\n",
            "tail\n",
        ] {
            for h in [&mut folded, &mut open] {
                output(h, "terminal_output_delta", "t", text);
                h.frame(800., 600.);
            }
            // Past the fold marker, the one difference.
            let text = |h: &Harness| h.core.text().chars().skip(1).collect::<String>();
            assert_eq!(text(&folded), text(&open));
            let faces = |h: &Harness| {
                h.core.agents[&AgentId(0)]
                    .tools
                    .values()
                    .next()
                    .unwrap()
                    .faces
                    .clone()
            };
            assert_eq!(faces(&folded), faces(&open));
        }
        folded.keys("M-< C-e TAB");
        assert_eq!(shown(&mut folded, 20), shown(&mut open, 20));
        let red = folded.core.settings.theme.ansi[1];
        assert_eq!(drawn(&mut folded, "redder").fg, Some(red));
    }

    #[test]
    fn hidden_tool_output_waits_for_its_session_to_be_shown() {
        for workspace in [false, true] {
            let mut h = session();
            let work = h.core.workspace();
            let buffer = h.core.windows.window.buffer();
            tool(&mut h, command());
            output(&mut h, "terminal_output", "t", "initial\n");
            h.frame(800., 600.);
            let before = h.core.buffers.get(buffer).unwrap().text().to_string();
            if workspace {
                h.new_workspace("other", "home");
            } else {
                h.open("other.txt");
            }
            output(&mut h, "terminal_output_delta", "t", "later\n");
            h.frame(800., 600.);
            assert_eq!(
                h.core.buffers.get(buffer).unwrap().text().to_string(),
                before
            );
            h.core.switch_workspace(work).unwrap();
            h.core.switch_to(buffer);
            h.frame(800., 600.);
            assert!(h.core.text().contains("initial\nlater\n```"));
        }
    }

    #[test]
    fn a_coloured_text_item_shows_its_colour_when_open() {
        let mut h = session();
        let call = ToolCall::new("x", "ls").content(vec!["\u{1b}[34mfile\u{1b}[0m".into()]);
        tool(&mut h, call);
        h.keys("M-< C-e TAB");
        assert_eq!(h.core.text(), "▾ ●    Using ls\nfile\n\n> ");
        let blue = h.core.settings().theme.ansi[4];
        assert_eq!(drawn(&mut h, "file").fg, Some(blue));
    }

    #[test]
    fn a_theme_change_redraws_command_output() {
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
            "[themes.default]\nansi = [{ansi}]"
        )))));
        assert_eq!(drawn(&mut h, "red").fg, Some(2));
        // Folding and opening the call again keeps the new colour.
        h.keys("TAB M-< C-e TAB");
        assert_eq!(drawn(&mut h, "red").fg, Some(2));
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
        assert_eq!(header(&mut h), "▸ ✗    Ran make exit 2");
        meta_update(
            &mut h,
            ToolCallUpdateFields::new(),
            exit(Value::Null, "SIGTERM".into()),
        );
        assert_eq!(header(&mut h), "▸ ✗    Ran make signal SIGTERM");
        let completed = ToolCallUpdateFields::new().status(ToolCallStatus::Completed);
        meta_update(&mut h, completed, exit(0.into(), Value::Null));
        assert_eq!(header(&mut h), "▸ ✓    Ran make 0.0s");
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
            "▸ ●    Using make\nOutput:\n```\nok\n```\n\n> "
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
            "▸ ●    Running make\nOutput:\n```\nok\n```\n\n> "
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
        let rule = Prop::Line(Decoration::Rule(FaceName::SessionThought));
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
    fn session_model_effort_and_title_follow_configuration() {
        use agent_client_protocol_schema::v1::SessionConfigSelectGroup;
        let mut h = session();
        let model = SessionConfigOption::select(
            "model",
            "Model",
            "opus",
            vec![SessionConfigSelectOption::new("opus", "Opus 5.5")],
        )
        .category(SessionConfigOptionCategory::Model);
        let effort = SessionConfigOption::select(
            "effort",
            "Effort",
            "high",
            vec![SessionConfigSelectGroup::new(
                "levels",
                "Levels",
                vec![SessionConfigSelectOption::new("high", "High")],
            )],
        )
        .category(SessionConfigOptionCategory::ThoughtLevel);
        let mut config = options("plan");
        config.push(
            SessionConfigOption::boolean("model-flag", "Flag", true)
                .category(SessionConfigOptionCategory::Model),
        );
        config.extend([model.clone(), effort.clone()]);
        update(
            &mut h,
            0,
            SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(config)),
        );
        info(&mut h, 0, serde_json::json!({"title":" Fix it \nmore"}));
        assert_eq!(
            h.display(5).mode_line.text(),
            "Opus 5.5 · High │ [plan] │ Fix it  "
        );
        let unknown = SessionConfigOption::select(
            "model",
            "Model",
            "new-model",
            Vec::<SessionConfigSelectOption>::new(),
        )
        .category(SessionConfigOptionCategory::Model);
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(vec![unknown, effort.clone()]),
        });
        assert_eq!(h.display(5).mode_line.text(), "new-model · High │ Fix it  ");
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(vec![effort]),
        });
        assert_eq!(h.display(5).mode_line.text(), "High │ Fix it  ");
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(vec![model]),
        });
        assert_eq!(h.display(5).mode_line.text(), "Opus 5.5 │ Fix it  ");
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(vec![]),
        });
        info(&mut h, 0, serde_json::json!({"title":null}));
        assert_eq!(h.display(5).mode_line.text(), "claude  ");
        info(&mut h, 0, serde_json::json!({"title":"é".repeat(41)}));
        assert_eq!(
            h.display(5).mode_line.text(),
            format!("{}…  ", "é".repeat(40))
        );
    }

    #[test]
    fn every_adapter_mode_has_its_risk_face() {
        let mut h = session();
        for (mode, face) in [
            ("plan", FaceName::StatusInfo),
            ("read-only", FaceName::StatusInfo),
            ("default", FaceName::ModeLine),
            ("acceptEdits", FaceName::StatusWaiting),
            ("workspace-write", FaceName::StatusWaiting),
            ("auto", FaceName::StatusRunning),
            ("agent", FaceName::StatusRunning),
            ("bypassPermissions", FaceName::StatusFailed),
            ("agent-full-access", FaceName::StatusFailed),
            ("future-mode", FaceName::ModeLine),
        ] {
            h.send(Msg::AgentConfigured {
                id: AgentId(0),
                result: Ok(options(mode)),
            });
            assert_eq!(
                h.display(5).mode_line.left[0],
                vec![(format!("[{mode}]"), face)]
            );
        }
    }

    #[test]
    fn workspace_dots_follow_owned_sessions_even_while_another_workspace_is_current() {
        let mut h = session();
        let work = h.core.workspace();
        let working = h.core.windows.window.buffer();
        let dots = |h: &mut Harness| {
            h.frame(1600., 600.)
                .workspaces
                .into_iter()
                .filter(|s| s.text == " ●")
                .map(|s| s.face.fg)
                .collect::<Vec<_>>()
        };
        let face = |h: &Harness, name| h.core.settings.theme.face(name).fg;
        assert_eq!(dots(&mut h), vec![]);
        h.keys("a RET");
        assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusRunning)]);
        h.keys("M-x agent-start RET claude RET");
        h.send(Msg::AgentStarted {
            id: AgentId(1),
            result: Err("failed".into()),
        });
        let failed = h.core.windows.window.buffer();
        assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusFailed)]);
        ask(&mut h, 1, "one", Some("First"));
        h.core.rename_workspace("renamed").unwrap();
        let other = h.new_workspace("other", "home");
        assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusWaiting)]);
        h.core.switch_workspace(work).unwrap();
        h.core.switch_to(working);
        h.keys("C-c C-y");
        assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusFailed)]);
        h.core.kill_buffer(failed);
        assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusRunning)]);
        h.core.switch_workspace(other).unwrap();
        prompted(&mut h, Ok(StopReason::EndTurn));
        assert_eq!(dots(&mut h), vec![]);
        exited(&mut h, 0, 0);
        assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusFailed)]);
        h.core.kill_workspace(work).unwrap();
        assert_eq!(dots(&mut h), vec![]);
    }

    #[test]
    fn session_status_follows_turns_permissions_and_exit() {
        let mut h = session();
        assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ");
        h.keys("a RET");
        assert_eq!(
            h.display(5).mode_line.right[0],
            vec![("● working".into(), FaceName::StatusRunning)]
        );
        ask(&mut h, 1, "one", Some("First"));
        ask(&mut h, 2, "two", Some("Second"));
        let waiting = vec![("● awaiting permission".into(), FaceName::StatusWaiting)];
        assert_eq!(h.display(5).mode_line.right[0], waiting);
        h.keys("C-c C-y");
        assert_eq!(h.display(5).mode_line.right[0], waiting);
        h.keys("C-c C-k");
        assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ● working");
        prompted(&mut h, Ok(StopReason::EndTurn));
        assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ");
        h.keys("b RET");
        ask(&mut h, 3, "three", Some("Third"));
        h.keys("C-c C-c");
        assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ● working");
        prompted(&mut h, Ok(StopReason::Cancelled));
        assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ");
        h.keys("c RET");
        ask(&mut h, 4, "four", Some("Fourth"));
        exited(&mut h, 0, 1);
        assert_eq!(
            h.display(5).mode_line.right[0],
            vec![("● exited".into(), FaceName::StatusFailed)]
        );
        h.keys("M-x agent-start RET claude RET");
        assert_eq!(h.display(5).mode_line.text(), "claude  ");
        h.send(Msg::AgentStarted {
            id: AgentId(1),
            result: Err("failed".into()),
        });
        assert_eq!(h.display(5).mode_line.text(), "claude  ● exited");
    }

    #[test]
    fn context_usage_is_bounded_and_keeps_the_latest_update() {
        use agent_client_protocol_schema::v1::UsageUpdate;
        let mut h = session();
        for (used, size, expected) in [
            (23613, 475000, "ctx 4%"),
            (25499, 475000, "ctx 5%"),
            (0, 100, "ctx 0%"),
            (u64::MAX, u64::MAX, "ctx 100%"),
            (u64::MAX, 1, "ctx 100%"),
            (1, 0, ""),
            (1, 100, "ctx 1%"),
        ] {
            update(
                &mut h,
                0,
                SessionUpdate::UsageUpdate(UsageUpdate::new(used, size)),
            );
            assert_eq!(
                h.display(5).mode_line.text(),
                format!("[auto] │ claude  {expected}")
            );
        }
        exited(&mut h, 0, 0);
        update(
            &mut h,
            0,
            SessionUpdate::UsageUpdate(UsageUpdate::new(99, 100)),
        );
        assert_eq!(
            h.display(5).mode_line.text(),
            "[auto] │ claude  ● exited │ ctx 1%"
        );
    }

    #[test]
    fn session_frame_spans_follow_active_inactive_and_narrow_windows() {
        let mut h = session();
        info(&mut h, 0, serde_json::json!({"title":"A session title"}));
        h.keys("a RET C-x 3");
        let frame = h.frame(1200., 300.);
        let theme = &h.core.settings.theme;
        for window in &frame.windows {
            let bar = &window.mode_line;
            let text = |spans: &[crate::display::Span]| {
                spans.iter().map(|s| s.text.as_str()).collect::<String>()
            };
            assert_eq!(text(&bar.left), "[auto] │ A session title");
            assert_eq!(text(&bar.right), "● working");
            if window.selected {
                let base = theme.face(FaceName::ModeLine);
                assert_eq!(
                    bar.left[0].face,
                    theme.face(FaceName::StatusRunning).over(base)
                );
                assert_eq!(bar.left[1].face, theme.face(FaceName::Separator).over(base));
                assert_eq!(
                    bar.left[2].face,
                    theme.face(FaceName::ModeLineBufferId).over(base)
                );
            } else {
                assert!(
                    bar.left
                        .iter()
                        .chain(&bar.right)
                        .all(|s| s.face == theme.face(FaceName::ModeLineInactive))
                );
            }
        }
        let frame = h.frame(400., 300.);
        for window in frame.windows {
            let left = window
                .mode_line
                .left
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>();
            assert!(left.ends_with('…'), "{left}");
            assert_eq!(window.mode_line.right[0].text, "● working");
        }
    }

    #[test]
    fn the_mode_line_shows_the_current_mode() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        assert!(!h.display(5).mode_line.text().contains('['));
        ready(&mut h, 0);
        assert!(h.display(5).mode_line.text().contains("[auto]"));
        let options = ConfigOptionUpdate::new(options("plan"));
        update(&mut h, 0, SessionUpdate::ConfigOptionUpdate(options));
        let line = h.display(5).mode_line.text();
        assert!(line.contains("[plan]") && !line.contains("auto"), "{line}");
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
        assert!(h.display(5).mode_line.text().contains("[auto]"));
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(options("plan")),
        });
        assert!(h.display(5).mode_line.text().contains("[plan]"));
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Err("no".into()),
        });
        assert!(messages(&h).contains("claude could not set mode plan: no\n"));
        assert!(h.display(5).mode_line.text().contains("[plan]"));
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
        assert!(!h.display(5).mode_line.text().contains('['));
        // An answer or update without a mode option clears the mode.
        ready(&mut h, 0);
        assert!(h.display(5).mode_line.text().contains("[auto]"));
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(options),
        });
        assert!(!h.display(5).mode_line.text().contains('['));
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
        assert!(h.display(5).mode_line.text().contains("[auto]"));
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
        assert!(h.display(5).mode_line.text().contains("[plan]"));
        h.keys("S-TAB");
        assert_eq!(ops(&mut h), ["set 0 s mode=auto"]);
        configured(&mut h, Ok(options("auto")));
        assert!(h.display(5).mode_line.text().contains("[auto]"));
        // The agent's own change is where the next cycle starts.
        let plan = ConfigOptionUpdate::new(options("plan"));
        update(&mut h, 0, SessionUpdate::ConfigOptionUpdate(plan));
        h.keys("S-TAB");
        assert_eq!(ops(&mut h), ["set 0 s mode=auto"]);
        configured(&mut h, Err("no".into()));
        assert!(messages(&h).contains("claude could not set mode auto: no\n"));
        assert!(h.display(5).mode_line.text().contains("[plan]"));
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

    fn working(h: &mut Harness) -> String {
        h.core.frame();
        let text = h.core.text();
        let mut lines = text.lines().filter(|l| l.starts_with("Work"));
        lines.next_back().unwrap_or_default().to_owned()
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn messages_show_their_clock_times_and_the_turn_its_length() {
        let mut h = session();
        h.send(Msg::Clock("14:01".into()));
        h.keys("h i RET");
        h.send(Msg::Clock("14:03".into()));
        h.advance(secs(5));
        agent_chunk(&mut h, "m", "Hi\nthere");
        h.send(Msg::Clock("14:04".into()));
        h.advance(secs(120));
        // Stamped when it arrived; the line goes in when the block ends.
        agent_chunk(&mut h, "m", "!");
        prompted(&mut h, Ok(StopReason::EndTurn));
        assert_eq!(
            h.core.text(),
            "> hi 14:01\n\nHi 14:03\nthere!\n\nWorked 2m 05s 14:04\n\n> "
        );
        assert_eq!(
            props(&h)
                .iter()
                .filter(|(_, p)| *p == Prop::AlignRight)
                .count(),
            3
        );
        let annotation = Prop::Named(vec![FaceRef::Face(FaceName::Annotation)]);
        assert!(props(&h).contains(&("14:03".into(), annotation)));
        // The message's lines still cover its whole block.
        assert_eq!(message_list(&h)[1].1, "Hi 14:03\nthere!");
    }

    #[test]
    fn a_thought_gets_a_header_when_it_ends() {
        let mut h = session();
        h.keys("a RET");
        thought(&mut h, "hmm");
        h.advance(Duration::from_millis(4200));
        chunk(&mut h, "So.");
        assert_eq!(
            h.core.text(),
            "> a\n\nThought for 4s\nhmm\n\nSo.\n\nWorking for 0s\n\n> "
        );
        let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
        assert!(props(&h).contains(&("Thought for 4s".into(), dim)));
    }

    #[test]
    fn a_tool_call_shows_how_long_it_ran_without_its_permission_wait() {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        tool(&mut h, ls());
        h.advance(secs(1));
        ask(&mut h, 1, "t", None);
        h.advance(secs(12));
        h.core.frame();
        assert!(h.core.text().contains("Running ls waiting 0:12\n"));
        h.keys("C-c C-y");
        assert!(h.core.text().contains("Running ls\n"));
        h.advance(Duration::from_millis(1300));
        let done = ToolCallUpdateFields::new().status(ToolCallStatus::Completed);
        tool_update(&mut h, "t", done);
        assert!(h.core.text().contains("Ran ls 2.3s\n"));
        // Later updates keep it, and a long one reads as minutes.
        h.advance(secs(9));
        tool_update(
            &mut h,
            "t",
            ToolCallUpdateFields::new().title("ls".to_string()),
        );
        assert!(h.core.text().contains("Ran ls 2.3s\n"));
        assert_eq!(short_duration(secs(125)), "2m 05s");
    }

    #[test]
    fn the_waiting_counter_keeps_folds_and_an_unshown_call_has_none() {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        tool(&mut h, ls());
        ask(&mut h, 1, "t", None);
        ask(&mut h, 2, "unknown", Some("U"));
        for n in 1..=3 {
            h.advance(secs(1));
            h.core.frame();
            let want = format!("Running ls waiting 0:0{n}\n");
            assert!(h.core.text().contains(&want));
        }
        h.keys("M-<");
        assert_eq!(shown(&mut h, 20)[2], "▸ ●   Running ls waiting 0:03");
        h.keys("C-c C-c");
        assert!(h.core.text().contains("Running ls\n"));
    }

    #[test]
    fn the_working_line_ticks_below_output_and_keeps_the_prompt() {
        let mut h = session();
        h.keys("a RET d r a f t");
        let point = h.core.point();
        h.advance(secs(1));
        assert_eq!(working(&mut h), "Working for 1s");
        assert_eq!(h.core.point(), point, "same length");
        h.advance(secs(135));
        assert_eq!(working(&mut h), "Working for 2m 16s");
        assert!(h.core.text().ends_with("\n\n> draft"));
        assert_eq!(h.core.point(), h.core.text().chars().count());
        chunk(&mut h, "out");
        h.keys("X");
        assert!(
            h.core
                .text()
                .ends_with("out\n\nWorking for 2m 16s\n\n> draftX")
        );
        // Ticks leave the prompt area's undo alone.
        h.advance(secs(1));
        h.core.frame();
        h.keys("C-/");
        assert!(h.core.text().ends_with("Working for 2m 17s\n\n> "));
        assert_eq!(long_duration(secs(3725)), "1h 02m 05s");
    }

    #[test]
    fn the_line_settles_however_the_turn_ends() {
        for (end, line) in [
            (Some(Ok(StopReason::EndTurn)), ""),
            (Some(Ok(StopReason::Cancelled)), "Turn ended: cancelled\n\n"),
            (Some(Err("boom")), "Error: boom\n\n"),
            (None, ""),
        ] {
            let mut h = session();
            h.keys("a RET");
            h.advance(secs(3));
            match end {
                Some(result) => prompted(&mut h, result),
                None => exited(&mut h, 0, 1),
            }
            assert_eq!(h.core.text(), format!("> a\n\n{line}Worked 3s\n\n> "));
            assert_eq!(h.core.next_deadline(), None);
        }
    }

    #[test]
    fn a_hidden_session_wakes_nothing_and_is_current_when_shown() {
        let mut h = session();
        h.keys("a RET");
        assert!(h.core.next_deadline().is_some());
        h.keys("C-x b * M e s s a g e s * RET");
        assert_eq!(h.core.next_deadline(), None);
        h.advance(secs(60));
        let session = h.core.agents[&AgentId(0)].buffer;
        let text = |h: &Harness| h.core.buffers.get(session).unwrap().text().to_string();
        assert!(
            text(&h).contains("Working for 0s"),
            "untouched while hidden"
        );
        h.keys("C-x b RET");
        assert!(h.core.next_deadline().is_some());
        assert_eq!(working(&mut h), "Working for 1m 00s");
    }

    #[test]
    fn a_hidden_turn_end_settles_at_once() {
        let mut h = session();
        h.keys("a RET C-x b * M e s s a g e s * RET");
        h.advance(secs(7));
        prompted(&mut h, Ok(StopReason::EndTurn));
        let session = h.core.agents[&AgentId(0)].buffer;
        let text = h.core.buffers.get(session).unwrap().text().to_string();
        assert_eq!(text, "> a\n\nWorked 7s\n\n> ");
    }

    /// A finished `read` call `id` of `path`.
    fn read(id: &'static str, path: &'static str) -> ToolCall {
        ToolCall::new(id, format!("Read {path}"))
            .kind(ToolKind::Read)
            .status(ToolCallStatus::Completed)
            .raw_output(serde_json::json!("text"))
            .locations(vec![ToolCallLocation::new(path)])
    }

    /// An unfinished call `id` of `kind` without content.
    fn call(id: &'static str, kind: ToolKind) -> ToolCall {
        ToolCall::new(id, id.to_string()).kind(kind)
    }

    fn visible(h: &mut Harness) -> Vec<String> {
        let lines = shown(h, 30);
        (lines.into_iter()).filter(|l| !l.is_empty()).collect()
    }

    /// A session with a turn running and `read` calls `a` and `b` folded into a group.
    fn grouped() -> Harness {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        tool(&mut h, read("a", "a.rs"));
        tool(&mut h, read("b", "b.rs"));
        h
    }

    /// Moves point to the line starting with `line` after any indent, from the top.
    fn on_line(h: &mut Harness, line: &str) {
        let text = h.core.text();
        let at = (text.char_indices())
            .scan(0, |chars, (byte, c)| {
                let at = *chars;
                *chars += 1;
                Some((byte, at, c))
            })
            .find(|&(byte, _, c)| {
                let start = byte == 0 || text[..byte].ends_with('\n');
                start && c != '\n' && text[byte..].trim_start_matches(' ').starts_with(line)
            })
            .map(|(_, at, _)| at)
            .expect("the line");
        let core = &mut h.core;
        let window = &mut core.windows.window;
        window.set_point(&mut core.buffers, at).unwrap();
    }

    #[test]
    fn two_calls_fold_into_one_line() {
        let mut h = grouped();
        assert_eq!(
            h.core.text(),
            "> a\n\n▸    Read files 0.0s\n  ▸ ✓    Read a.rs 0.0s\nOutput:\n```\ntext\n```\n  \
             ▸ ✓    Read b.rs 0.0s\nOutput:\n```\ntext\n```\n\nWorking for 0s\n\n> "
        );
        assert_eq!(
            visible(&mut h),
            ["> a", "▸    Read files 0.0s", "Working for 0s", "> "]
        );
        let rule = |(_, p): &(String, Prop)| matches!(p, Prop::Line(Decoration::Rule(_)));
        assert!(!props(&h).iter().any(rule), "the group line has no rule");
        // A third call and updates keep it folded.
        tool(&mut h, read("c", "c.rs"));
        tool_update(
            &mut h,
            "a",
            ToolCallUpdateFields::new().title("x".to_string()),
        );
        assert_eq!(visible(&mut h)[1], "▸    Read files 0.0s");
        assert_eq!(visible(&mut h).len(), 4);
    }

    #[test]
    fn a_single_call_has_no_group_line_and_messages_split_groups() {
        let mut h = session();
        h.keys("a RET");
        tool(&mut h, read("a", "a.rs"));
        chunk(&mut h, "one");
        assert_eq!(visible(&mut h)[1], "▸ ✓   Read a.rs 0.0s");
        tool(&mut h, read("b", "b.rs"));
        tool(&mut h, read("c", "c.rs"));
        chunk(&mut h, "two");
        tool(&mut h, read("d", "d.rs"));
        tool(&mut h, read("e", "e.rs"));
        let lines = visible(&mut h);
        let groups = lines.iter().filter(|l| l.contains("Read files")).count();
        assert_eq!(groups, 2, "{lines:?}");
        assert!(lines.contains(&"two".to_string()));
    }

    #[test]
    fn a_prompt_and_a_session_line_end_a_run() {
        let groups = |h: &mut Harness| {
            let lines = visible(h);
            lines.iter().filter(|l| l.contains("Read files")).count()
        };
        let mut h = grouped();
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.keys("b RET");
        tool(&mut h, read("c", "c.rs"));
        tool(&mut h, read("d", "d.rs"));
        assert_eq!(groups(&mut h), 2);
        let mut h = grouped();
        prompted(&mut h, Ok(StopReason::MaxTokens));
        tool(&mut h, read("c", "c.rs"));
        tool(&mut h, read("d", "d.rs"));
        assert_eq!(groups(&mut h), 2);
    }

    #[test]
    fn a_shown_plan_doesnt_split_a_group_and_a_first_plan_does() {
        let mut h = session();
        h.keys("a RET");
        let entry = [("x", PlanEntryStatus::Pending, PlanEntryPriority::Low)];
        tool(&mut h, read("a", "a.rs"));
        plan(&mut h, &entry);
        tool(&mut h, read("b", "b.rs"));
        assert!(
            !h.core.text().contains("Read files"),
            "the first plan ends the run"
        );
        tool(&mut h, read("c", "c.rs"));
        plan(&mut h, &entry);
        tool(&mut h, read("d", "d.rs"));
        assert!(h.core.text().contains("Read files"));
        assert_eq!(
            visible(&mut h)
                .iter()
                .filter(|l| l.contains("Read"))
                .count(),
            2
        );
    }

    #[test]
    fn verbs_follow_first_appearance_and_the_running_one_is_present() {
        let line = |h: &mut Harness| visible(h)[1].clone();
        let mut h = session();
        h.keys("a RET");
        tool(&mut h, read("a", "a.rs"));
        tool(&mut h, call("b", ToolKind::Execute));
        tool(&mut h, read("c", "c.rs"));
        assert_eq!(line(&mut h), "▸    Read files, running commands");
        let mut h = session();
        h.keys("a RET");
        tool(
            &mut h,
            call("a", ToolKind::Edit).status(ToolCallStatus::Completed),
        );
        tool(
            &mut h,
            call("b", ToolKind::Search).status(ToolCallStatus::Completed),
        );
        tool(
            &mut h,
            call("c", ToolKind::Delete).status(ToolCallStatus::Completed),
        );
        tool(
            &mut h,
            call("d", ToolKind::Fetch).status(ToolCallStatus::Completed),
        );
        tool(
            &mut h,
            call("e", ToolKind::Other).status(ToolCallStatus::Completed),
        );
        assert_eq!(
            line(&mut h),
            "▸    Edited files, searched code, fetched pages, used tools 0.0s"
        );
        let icons: Vec<_> = (props(&h).into_iter())
            .filter_map(|(_, p)| match p {
                Prop::Icon(icon) => Some(icon),
                _ => None,
            })
            .collect();
        assert_eq!(icons, [Icon::Pencil]);
    }

    #[test]
    fn a_failed_call_shows_on_the_folded_line_and_the_duration_spans_the_group() {
        let mut h = session();
        h.keys("a RET");
        tool(&mut h, call("a", ToolKind::Read));
        h.advance(secs(1));
        tool(&mut h, call("b", ToolKind::Execute));
        let finish = |s| ToolCallUpdateFields::new().status(s);
        tool_update(&mut h, "a", finish(ToolCallStatus::Completed));
        h.advance(secs(2));
        assert_eq!(visible(&mut h)[1], "▸    Read files, running commands");
        tool_update(&mut h, "b", finish(ToolCallStatus::Failed));
        assert_eq!(
            visible(&mut h)[1],
            "▸    Read files, ran commands 1 failed 3.0s"
        );
        assert!(h.core.text().contains("ran commands 1 failed  3.0s\n"));
        let failed = (props(&h).into_iter()).find(|(t, _)| t == "1 failed");
        let want = Prop::Named(vec![FaceRef::Face(FaceName::StatusFailed)]);
        assert_eq!(failed.map(|(_, p)| p), Some(want));
    }

    #[test]
    fn tab_opens_a_group_then_a_row_and_folds_back_a_level_at_a_time() {
        let mut h = grouped();
        on_line(&mut h, "▸    Read");
        h.keys("TAB");
        assert_eq!(
            visible(&mut h)[1..4],
            [
                "▾    Read files 0.0s",
                "  ▸ ✓   Read a.rs 0.0s",
                "  ▸ ✓   Read b.rs 0.0s"
            ]
        );
        on_line(&mut h, "▸ ✓    Read a.rs");
        h.keys("TAB");
        assert_eq!(visible(&mut h)[2], "  ▾ ✓   Read a.rs 0.0s");
        assert!(visible(&mut h).iter().any(|l| l.contains("text")));
        // Inside the call, back to its row, the group staying open.
        h.keys("C-n C-n TAB");
        assert_eq!(visible(&mut h)[2], "  ▸ ✓   Read a.rs 0.0s");
        assert_eq!(visible(&mut h).len(), 6);
        on_line(&mut h, "▾    Read");
        h.keys("TAB");
        assert_eq!(visible(&mut h).len(), 4);
        // What was open stays open across a fold.
        h.keys("TAB");
        on_line(&mut h, "▸ ✓    Read b.rs");
        h.keys("TAB");
        on_line(&mut h, "▾    Read");
        h.keys("TAB TAB");
        assert_eq!(visible(&mut h)[3], "  ▾ ✓   Read b.rs 0.0s");
    }

    #[test]
    fn tab_on_a_row_with_nothing_to_open_folds_the_group() {
        let mut h = session();
        h.keys("a RET");
        tool(&mut h, call("a", ToolKind::Read));
        tool(&mut h, call("b", ToolKind::Read));
        on_line(&mut h, "▸    Reading");
        h.keys("TAB");
        on_line(&mut h, "▸ ●    Reading a");
        h.keys("TAB");
        assert_eq!(visible(&mut h)[1], "▸    Reading files");
        assert_eq!(visible(&mut h).len(), 4);
        let at = h.core.current().text().line_to_char(2);
        assert_eq!(h.core.point(), at);
    }

    #[test]
    fn an_open_call_keeps_its_run_open_as_a_group_forms() {
        let mut h = session();
        h.keys("a RET");
        tool(&mut h, read("a", "a.rs"));
        on_line(&mut h, "▸ ✓    Read a.rs");
        h.keys("TAB");
        tool(&mut h, read("b", "b.rs"));
        let lines = visible(&mut h);
        assert_eq!(lines[1], "▾    Read files 0.0s");
        assert_eq!(lines[2], "  ▾ ✓   Read a.rs 0.0s");
    }

    #[test]
    fn thoughts_are_rows_of_the_group() {
        let mut h = session();
        h.keys("a RET");
        thought(&mut h, "first\nthought");
        tool(&mut h, read("a", "a.rs"));
        thought(&mut h, "second");
        tool(&mut h, read("b", "b.rs"));
        thought(&mut h, "last");
        tool(&mut h, read("c", "c.rs"));
        assert_eq!(visible(&mut h).len(), 4);
        on_line(&mut h, "▸    Read");
        h.keys("TAB");
        assert_eq!(
            visible(&mut h)[1..8],
            [
                "▾    Read files 0.0s",
                "  ▸ Thought for 0s",
                "  ▸ ✓   Read a.rs 0.0s",
                "  ▸ Thought for 0s",
                "  ▸ ✓   Read b.rs 0.0s",
                "  ▸ Thought for 0s",
                "  ▸ ✓   Read c.rs 0.0s"
            ]
        );
        on_line(&mut h, "▸ Thought");
        h.keys("TAB");
        assert_eq!(visible(&mut h)[2], "  ▾ Thought for 0s");
        assert_eq!(visible(&mut h)[3..5], ["first", "thought"]);
        h.keys("C-n TAB");
        assert_eq!(visible(&mut h)[2], "  ▸ Thought for 0s");
        assert_eq!(visible(&mut h)[3], "  ▸ ✓   Read a.rs 0.0s");
    }

    #[test]
    fn a_thought_streaming_into_a_folded_group_stays_hidden() {
        let mut h = grouped();
        thought(&mut h, "hmm");
        assert_eq!(visible(&mut h).len(), 4);
        thought(&mut h, " more");
        assert_eq!(visible(&mut h).len(), 4);
        chunk(&mut h, "done");
        assert_eq!(visible(&mut h).len(), 5);
        assert!(
            h.core
                .text()
                .contains("  ▸ Thought for 0s\nhmm more\n\ndone")
        );
    }

    #[test]
    fn a_waiting_call_shows_under_the_fold_until_answered() {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        tool(&mut h, read("a", "a.rs"));
        tool(&mut h, call("t", ToolKind::Execute));
        ask(&mut h, 1, "t", Some("t"));
        h.advance(secs(2));
        let lines = visible(&mut h);
        assert_eq!(
            lines[1..6],
            [
                "▸    Read files, running commands",
                "  ▸ ●   Running t waiting 0:02",
                "Permission: t",
                "1. Yes (allow_once)",
                "2. Always (allow_always)"
            ],
            "{lines:?}"
        );
        h.keys("C-c C-y");
        assert_eq!(visible(&mut h).len(), 4);
        on_line(&mut h, "▸    Read");
        h.keys("TAB");
        assert_eq!(
            visible(&mut h)[2..5],
            [
                "  ▸ ✓   Read a.rs 0.0s",
                "  ▸ ●   Running t",
                "✓ allowed once"
            ]
        );
    }

    #[test]
    fn a_permission_block_before_the_group_joins_it() {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        ask(&mut h, 1, "u", Some("U"));
        tool(&mut h, read("a", "a.rs"));
        tool(&mut h, read("b", "b.rs"));
        let lines = visible(&mut h);
        assert_eq!(lines[1], "▸    Read files 0.0s", "{lines:?}");
        assert_eq!(lines[2], "Permission: U");
        h.keys("C-c C-y");
        assert_eq!(visible(&mut h).len(), 4);
    }

    #[test]
    fn line_motion_passes_a_folded_group_and_search_and_copy_reach_it() {
        let mut h = grouped();
        h.keys("M-< C-n C-n");
        assert_eq!(h.core.current().text().line_to_char(2), h.core.point());
        h.keys("C-n");
        assert!(visible(&mut h)[1].starts_with('▸'));
        let working = h.core.text().find("Working").unwrap();
        let chars = h.core.text()[..working].chars().count();
        h.keys("C-n");
        assert_eq!(
            h.core.point(),
            chars,
            "the working line, past the blank one"
        );
        h.keys("C-p C-p");
        assert_eq!(h.core.current().text().line_to_char(2), h.core.point());
    }

    #[test]
    fn search_lands_in_a_folded_group_and_tab_opens_it() {
        let mut h = grouped();
        h.keys("C-s b . r s RET");
        let line = h.core.current().line_col(h.core.point()).unwrap().0;
        let text = h.core.text();
        assert!(text.lines().nth(line).unwrap().contains("Read b.rs"));
        h.keys("TAB");
        assert!(h.core.text().contains("▾    Read files 0.0s"));
        h.keys("M-<");
        assert_eq!(visible(&mut h)[3], "  ▸ ✓   Read b.rs 0.0s");
    }

    #[test]
    fn a_region_over_a_folded_group_copies_its_rows() {
        let mut h = grouped();
        h.keys("M-< C-SPC C-n C-n C-n C-n M-w M-> C-y");
        let prompt = h.core.text();
        let prompt = prompt.rsplit("\n> ").next().unwrap();
        assert!(prompt.contains("Read a.rs") && prompt.contains("Read b.rs"));
    }

    #[test]
    fn terminal_output_reaches_a_hidden_call() {
        let mut h = session();
        h.keys("a RET");
        tool(&mut h, read("a", "a.rs"));
        tool(&mut h, command());
        output(&mut h, "terminal_output", "t", "one\n");
        assert_eq!(visible(&mut h).len(), 4);
        output(&mut h, "terminal_output", "t", "two\n");
        assert_eq!(visible(&mut h).len(), 4);
        on_line(&mut h, "▸    Read");
        h.keys("TAB");
        on_line(&mut h, "▸ ●    Running");
        h.keys("TAB");
        let lines = visible(&mut h);
        assert!(lines.contains(&"two".to_string()), "{lines:?}");
    }

    #[test]
    fn ret_on_hidden_text_shows_no_output() {
        let mut h = grouped();
        on_line(&mut h, "▸ ✓    Read a.rs");
        h.keys("RET");
        assert_eq!(h.core.message(), Some("claude is busy"));
    }

    #[test]
    fn replayed_calls_group_without_a_duration() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        let loading = h.core.agents.get_mut(&AgentId(0)).unwrap();
        loading.load = Some(("old".into(), loading.buffer));
        update(
            &mut h,
            0,
            SessionUpdate::UserMessageChunk(ContentChunk::new("old".into())),
        );
        thought(&mut h, "hmm\nmore");
        tool(&mut h, read("a", "a.rs"));
        tool(&mut h, read("b", "b.rs"));
        assert_eq!(visible(&mut h)[1], "▸    Read files");
        on_line(&mut h, "▸    Read");
        h.keys("TAB");
        assert_eq!(
            visible(&mut h)[2..5],
            ["  ▸ hmm", "  ▸ ✓   Read a.rs", "  ▸ ✓   Read b.rs"]
        );
    }

    #[test]
    fn replayed_history_shows_no_times_and_later_updates_do() {
        let mut h = work();
        h.keys("M-x agent-start RET claude RET");
        ops(&mut h);
        h.send(Msg::Clock("14:01".into()));
        let loading = h.core.agents.get_mut(&AgentId(0)).unwrap();
        loading.load = Some(("old".into(), loading.buffer));
        let user = ContentChunk::new("old".into());
        update(&mut h, 0, SessionUpdate::UserMessageChunk(user));
        thought(&mut h, "hmm");
        h.advance(secs(5));
        chunk(&mut h, "reply");
        tool(&mut h, ls().status(ToolCallStatus::Completed));
        assert_eq!(
            h.core.text(),
            format!("> old\n\nhmm\n\nreply\n\n▸ ✓    Ran ls\n{LS_INPUT}\n\n> ")
        );
        ready(&mut h, 0);
        h.advance(secs(1));
        let later = ToolCallUpdateFields::new().title("ls".to_string());
        tool_update(&mut h, "t", later);
        assert!(h.core.text().contains("Ran ls\n"), "no arrival, no time");
        chunk(&mut h, "live");
        h.keys("a RET");
        assert!(h.core.text().contains("live 14:01\n"));
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
        assert_eq!(h.core.text(), "> x\n\nWorking for 0s\n\n> y");
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
        assert_eq!(
            h.core.text(),
            "> a\n\nTurn ended: refusal\n\nWorked 0s\n\n> "
        );
        h.keys("b RET");
        chunk(&mut h, "B");
        prompted(&mut h, Err("Internal error"));
        assert!(
            h.core
                .text()
                .ends_with("> b\n\nB\n\nError: Internal error\n\nWorked 0s\n\n> ")
        );
        // A late answer, with no turn running, adds nothing.
        prompted(&mut h, Ok(StopReason::MaxTokens));
        assert!(
            h.core
                .text()
                .ends_with("Error: Internal error\n\nWorked 0s\n\n> ")
        );
    }

    #[test]
    fn c_c_c_c_cancels_a_running_turn() {
        let mut h = session();
        h.keys("C-c C-c");
        assert_eq!(h.core.message(), Some("No turn is running"));
        h.keys("a RET C-c C-c");
        assert_eq!(ops(&mut h), [r#"prompt 0 s "a""#, "cancel 0 s"]);
        prompted(&mut h, Ok(StopReason::Cancelled));
        assert_eq!(
            h.core.text(),
            "> a\n\nTurn ended: cancelled\n\nWorked 0s\n\n> "
        );
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
        assert_eq!(h.core.text(), "> a\n\nHi there\n\nWorking for 0s\n\n> draf");
        assert_eq!(h.core.point(), h.core.text().chars().count());
        h.keys("C-/");
        assert_eq!(
            h.core.text(),
            "> a\n\nHi there\n\nWorking for 0s\n\n> draft"
        );
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
        assert_eq!(
            h.core.text(),
            format!("> a\n\nEditing.\n\n{block}\n\nWorking for 0s\n\n> ")
        );
        let rule = Prop::Line(Decoration::Rule(FaceName::SessionThought));
        let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
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
                " (reject_once)",
                "Working for 0s"
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
        h.keys("C-c C-k");
        assert_eq!(ops(&mut h), ["reply 0 2 reject"]);
        let c = asked("C");
        assert_eq!(
            h.core.text(),
            format!("> a\n\n✓ allowed once\n\n✗ rejected\n\n{c}\n\nWorking for 0s\n\n> ")
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
        h.keys("C-p C-p C-p C-p C-p RET");
        assert_eq!(ops(&mut h), ["reply 0 2 always"]);
        assert!(h.core.text().contains("✓ allowed always\n"));
        h.keys("M-< C-n C-n RET");
        assert_eq!(h.core.message(), Some("claude is busy"));
        h.keys("C-n C-n C-n RET");
        assert_eq!(ops(&mut h), ["reply 0 1 reject"]);
        assert_eq!(
            h.core.text(),
            "> a\n\n✗ rejected\n\n✓ allowed always\n\nWorking for 0s\n\n> "
        );
    }

    #[test]
    fn an_answered_line_is_dim_and_has_no_rule() {
        use PermissionOptionKind::*;
        for (kind, line) in [
            (AllowOnce, "✓ allowed once"),
            (AllowAlways, "✓ allowed always"),
            (RejectOnce, "✗ rejected"),
            (RejectAlways, "✗ rejected always"),
        ] {
            let mut h = session();
            h.keys("a RET");
            ops(&mut h);
            let ask = RequestPermissionRequest::new(
                "s",
                ToolCallUpdate::new("c", ToolCallUpdateFields::new()),
                vec![PermissionOption::new("o", "Option", kind)],
            );
            h.send(Msg::AgentRequest {
                id: AgentId(0),
                request: RequestId::Number(1),
                body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
            });
            h.keys("C-p C-p C-p C-p RET");
            assert_eq!(ops(&mut h), ["reply 0 1 o"]);
            assert_eq!(
                h.core.text(),
                format!("> a\n\n{line}\n\nWorking for 0s\n\n> ")
            );
            let rule = Prop::Line(Decoration::Rule(FaceName::SessionThought));
            let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
            let props = props(&h);
            assert!(props.contains(&(line.to_string(), dim)), "{props:?}");
            assert!(!props.iter().any(|(_, p)| *p == rule), "{props:?}");
        }
    }

    #[test]
    fn a_turn_ending_cancels_its_pending_requests() {
        let ends = [
            (Ok(StopReason::EndTurn), ""),
            (Ok(StopReason::Refusal), "Turn ended: refusal\n\n"),
            (Err("boom"), "Error: boom\n\n"),
        ];
        for (result, end) in ends {
            let mut h = session();
            h.keys("a RET");
            ops(&mut h);
            ask(&mut h, 1, "a", Some("A"));
            ask(&mut h, 2, "b", Some("B"));
            prompted(&mut h, result);
            assert_eq!(ops(&mut h), ["reply 0 1 cancelled", "reply 0 2 cancelled"]);
            assert_eq!(
                h.core.text(),
                format!("> a\n\n✗ cancelled\n\n✗ cancelled\n\n{end}Worked 0s\n\n> ")
            );
        }
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
        h.keys("C-c C-k");
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
        let cancelled = "✗ cancelled\n\n✗ cancelled";
        assert_eq!(
            h.core.text(),
            format!("> a\n\n{cancelled}\n\nWorking for 0s\n\n> ")
        );
        prompted(&mut h, Ok(StopReason::Cancelled));
        let ended = format!("> a\n\n{cancelled}\n\nTurn ended: cancelled\n\nWorked 0s\n\n> ");
        assert_eq!(h.core.text(), ended);
        h.keys("b RET");
        ops(&mut h);
        ask(&mut h, 3, "c", Some("C"));
        exited(&mut h, 0, 1);
        assert_eq!(ops(&mut h), ["kill 0"]);
        assert!(h.core.text().ends_with("✗ cancelled\n\nWorked 0s\n\n> "));
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
