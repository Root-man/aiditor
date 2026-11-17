//! Agent sessions: each one an ACP agent process started in a workspace, with a session buffer,
//! comint-style, and a read-only buffer for its stderr.

use agent_client_protocol_schema::v1::{
    AgentRequest, AvailableCommand, AvailableCommandInput, ClientResponse, ContentBlock,
    CreateElicitationResponse, ElicitationAcceptAction, ElicitationAction, Error, MessageId,
    NewSessionResponse, PermissionOption, PermissionOptionKind, Plan, PlanEntry, PlanEntryStatus,
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

use crate::buffer::{
    self, Buffer, BufferId, Decoration, FaceRef, InsertionType, Kind, MarkerId, Prop,
};
use crate::command::Args;
use crate::display::ModeLine;
use crate::markdown;
use crate::project::ProjectId;
use crate::settings::FaceName;
use crate::{AgentId, AgentOp, Core, Ctx, Effect, Msg, Open, Request};

mod asks;
mod blocks;
mod chips;
mod groups;
mod pastes;
mod questions;
mod render;
use asks::{Answered, Ask};
use blocks::{Block, BlockKind, BlockPath, Look, Rendered};
pub(crate) use chips::{Chips, is_chip};
use groups::*;
pub(crate) use questions::Preview;
use render::*;

/// The prompt line's text, just before the prompt area.
pub(crate) const PROMPT: &str = "> ";
/// The session buffer's major mode.
pub(crate) const MODE: &str = "agent-session-mode";
/// How often a shown session's working line and waiting counters tick.
const TICK: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Activity {
    Working,
    Failed,
    Asking,
    Waiting,
}

impl Activity {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::Working => "● working",
            Self::Failed => "● exited",
            Self::Asking => "● awaiting answer",
            Self::Waiting => "● awaiting permission",
        }
    }

    pub(crate) fn face(self) -> FaceName {
        match self {
            Self::Working => FaceName::StatusRunning,
            Self::Failed => FaceName::StatusFailed,
            Self::Asking | Self::Waiting => FaceName::StatusWaiting,
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
    /// The session it loads instead of opening one, none for a fork until it answers, and the
    /// buffer it was loaded from, which shows it once loaded if still selected.
    load: Option<(Option<SessionId>, BufferId)>,
    /// Whether it is a rollback of `load`'s buffer, taking that buffer's place once loaded.
    rollback: bool,
    /// The agent that deletes its session once it stopped, a rolled back source's fork.
    delete: Option<AgentId>,
    /// What was last sent with `session/set_config_option`, as `mode <value>`, `model
    /// <value>` or `effort <value>`, for a refusal's note.
    sent: Option<String>,
    /// Whether the agent failed to start or exited; its buffers stay until killed.
    ended: bool,
    /// A `session/prompt` waits for its answer; its working line is the last block.
    turn: Option<Turn>,
    /// The last block above the prompt line, while an agent message or thought streams into it.
    stream: Option<Stream>,
    /// The transcript, in order; their starts ascend.
    blocks: Vec<Block>,
    /// The tool calls shown, by id.
    tools: HashMap<ToolCallId, Tool>,
    /// Where the run starts in `blocks`: the tool calls and what sits among them since the last
    /// message, plan or prompt, until they form a group.
    run: usize,
    /// The run's group, by its index in `blocks`, once it has one.
    group: Option<usize>,
    /// The buffer's tick when the prompt area's chips were last painted.
    painted: u64,
    /// The slash commands from the latest `available_commands_update`.
    commands: Vec<AvailableCommand>,
    pastes: Vec<pastes::Paste>,
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

/// The first select config option of `category`: the session's mode or model.
fn select_option(
    options: &[SessionConfigOption],
    category: SessionConfigOptionCategory,
) -> Option<(&SessionConfigId, &SessionConfigSelect)> {
    options.iter().find_map(|o| match &o.kind {
        SessionConfigKind::Select(select) if o.category.as_ref() == Some(&category) => {
            Some((&o.id, select))
        }
        _ => None,
    })
}

/// The session's mode: the first select config option of category `mode`.
fn mode_option(
    options: &[SessionConfigOption],
) -> Option<(&SessionConfigId, &SessionConfigSelect)> {
    select_option(options, SessionConfigOptionCategory::Mode)
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
        match (&self.turn, self.blocks.last()) {
            (Some(_), Some(working)) => buffer.marker(working.start).expect("live while working"),
            _ => buffer.prompt_start() - PROMPT.len(),
        }
    }

    /// The buffer lines of the message at `point`, the last one starting at or before it.
    pub(crate) fn message_lines(
        &self,
        buffer: &Buffer,
        point: usize,
    ) -> Option<RangeInclusive<usize>> {
        let range = self.message_at(buffer, point)?.0.range(buffer);
        let (first, last) = (range.start, range.end.saturating_sub(1).max(range.start));
        Some(buffer.text().char_to_line(first)..=buffer.text().char_to_line(last))
    }

    /// The buffer line of the question row `point` is on, which frame 6d highlights, and the
    /// preview of its option, if it has one.
    pub(crate) fn question_row(
        &self,
        buffer: &Buffer,
        point: usize,
    ) -> Option<(usize, Option<Preview>)> {
        self.asks().find_map(|(path, ask)| {
            let Range { start, end } = self.range(buffer, path);
            let range = start..=end;
            let first = buffer.text().char_to_line(start);
            let line = buffer.text().char_to_line(point);
            let question = ask.question().filter(|_| range.contains(&point))?;
            let preview = match question.answer_at(line - first)? {
                questions::Answer::Option(i) => question.preview(i),
                _ => None,
            };
            Some((line, preview))
        })
    }

    pub(crate) fn activity(&self) -> Option<Activity> {
        if self.ended {
            Some(Activity::Failed)
        } else if self.asks().any(|(_, a)| a.question().is_none()) {
            Some(Activity::Waiting)
        } else if self.asks().any(|(_, a)| a.blocks()) {
            Some(Activity::Asking)
        } else if self.turn.is_some() {
            Some(Activity::Working)
        } else {
            None
        }
    }

    pub(crate) fn model_effort(&self) -> Vec<(String, FaceName)> {
        let options = self
            .session
            .as_ref()
            .and_then(|s| s.config_options.as_deref());
        let options = options.unwrap_or_default();
        // The current value's display name, falling back to the value, and the value.
        let label = |category| {
            let (_, select) = select_option(options, category)?;
            let value = select.current_value.to_string();
            let values = self::options(select);
            let name = values.iter().find(|v| v.value == select.current_value);
            Some((
                name.map_or_else(|| value.clone(), |v| v.name.clone()),
                value,
            ))
        };
        let model = label(SessionConfigOptionCategory::Model).map(|(name, value)| {
            let face = model_quality(&name).or_else(|| model_quality(&value));
            (
                name.replace(char::is_control, " "),
                face.unwrap_or(FaceName::ModeLineDim),
            )
        });
        let effort = label(SessionConfigOptionCategory::ThoughtLevel).map(|(name, value)| {
            let face = effort_quality(&value).unwrap_or(FaceName::ModeLineDim);
            (name.replace(char::is_control, " "), face)
        });
        let mut model: Vec<_> = model.into_iter().chain(effort).collect();
        if model.len() == 2 {
            model.insert(1, (" · ".into(), FaceName::ModeLineDim));
        }
        model
    }

    pub(crate) fn mode_line(&self) -> ModeLine {
        let segment = |text: &str, face| vec![(text.replace(char::is_control, " "), face)];
        let options = self
            .session
            .as_ref()
            .and_then(|s| s.config_options.as_deref())
            .unwrap_or_default();
        let model = self.model_effort();
        let mut left = Vec::new();
        if !model.is_empty() {
            left.push(model);
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
            right.push(segment(activity.text(), activity.face()));
        }
        match self.asks().filter(|(_, a)| !a.blocks()).count() {
            0 => {}
            1 => right.push(segment("1 question", FaceName::StatusWaiting)),
            n => right.push(segment(&format!("{n} questions"), FaceName::StatusWaiting)),
        }
        if let Some((used, size)) = self.usage.filter(|(_, size)| *size > 0) {
            let percent = u128::from(used.min(size)) * 100 / u128::from(size);
            let face = match percent {
                0..50 => FaceName::StatusRunning,
                50..80 => FaceName::StatusWaiting,
                _ => FaceName::StatusFailed,
            };
            right.push(segment(&format!("ctx {percent}%"), face));
        }
        ModeLine {
            left,
            right,
            path: None,
        }
    }
}

/// A model's item quality, from a family word in `name` such as `Opus 5.5` or `gpt-6-astra`.
fn model_quality(name: &str) -> Option<FaceName> {
    let words = name.split(|c: char| !c.is_alphanumeric());
    words.map(str::to_lowercase).find_map(|w| match w.as_str() {
        "fable" | "astra" => Some(FaceName::QualityLegendary),
        "opus" | "sol" => Some(FaceName::QualityEpic),
        "sonnet" | "terra" => Some(FaceName::QualityRare),
        "haiku" | "luna" => Some(FaceName::QualityUncommon),
        _ => None,
    })
}

/// An effort's item quality, from its value.
fn effort_quality(value: &str) -> Option<FaceName> {
    Some(match value {
        "default" => FaceName::QualityPoor,
        "low" => FaceName::QualityCommon,
        "medium" => FaceName::QualityUncommon,
        "high" => FaceName::QualityRare,
        "xhigh" => FaceName::QualityEpic,
        "max" => FaceName::QualityLegendary,
        "ultra" => FaceName::QualityArtifact,
        _ => return None,
    })
}

/// A tool call's block: a header line, then its content when it has any.
struct Tool {
    /// The call, with every update merged in.
    call: ToolCall,
    /// The header's length in chars.
    header: usize,
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
    /// Its time: at the right of its header in a group, else on the line under its block.
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
    /// A closed question's line stands for it, so it shows no row.
    replaced: bool,
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

/// What text put above the prompt line is. A chunk joins a streamed block of its own kind, so a
/// new `messageId` starts a block.
#[derive(Clone, Debug, PartialEq)]
enum Chunk {
    /// A line of the session's own, like a turn's end.
    Line,
    Message(Option<MessageId>),
    Thought(Option<MessageId>),
    User(Option<MessageId>),
}

/// An agent or user message. Prompts sent live have no `id`.
#[derive(Debug)]
struct Message {
    id: Option<MessageId>,
    /// Whether it is the user's, which can't be a fork point.
    user: bool,
}

/// A block being streamed.
struct Stream {
    /// Where the block starts.
    start: MarkerId,
    chunk: Chunk,
    /// Its markdown, which a message's drawn tables replace in the text.
    source: String,
    /// Chunks arrived since it was last rendered.
    dirty: bool,
    /// Where a message's last rendered top-level block starts, in `source` bytes and chars from
    /// `start`; earlier blocks keep their text and properties.
    stable: markdown::Stable,
    /// When its first chunk arrived and the clock then; `None` when replayed.
    began: Option<(Instant, Option<String>)>,
}

/// A running turn, its working line the last block.
struct Turn {
    start: Instant,
}

impl Core {
    /// Session `id` and its buffer, borrowed apart.
    fn session_buffer(&mut self, id: AgentId) -> Option<(&mut Session, &mut Buffer)> {
        let session = self.agents.get_mut(&id)?;
        let buffer = self.buffers.get_mut(session.buffer)?;
        Some((session, buffer))
    }

    /// Puts `text` above session `id`'s prompt line: as a new block followed by a blank line, or,
    /// for a chunk of the kind streaming, at the end of the streamed block. A chunk's block is
    /// rendered as markdown by the next frame.
    fn output(&mut self, id: AgentId, text: &str, chunk: Chunk) {
        let now = self.now();
        let clock = self.clock();
        let Some((session, buffer)) = self.session_buffer(id) else {
            return;
        };
        let end = session.end(buffer);
        if let Some(stream) = &mut session.stream
            && stream.chunk == chunk
        {
            buffer.insert_output(end - "\n\n".len(), text);
            stream.source.push_str(text);
            stream.dirty = true;
            if let Chunk::Thought(_) = chunk {
                session.thought_grew(buffer, text.chars().count());
                return;
            }
            if let (Chunk::Message(_), Some(message)) = (&chunk, session.last_message()) {
                let to = end - "\n\n".len() + text.chars().count();
                (buffer.set_marker(session.block(message).end, to)).expect("in the text");
            }
            return;
        }
        // codex-acp starts each reasoning summary part, the first too, with a blank line.
        let text = match chunk {
            Chunk::Thought(_) => text.trim_start_matches('\n'),
            _ => text,
        };
        if text.is_empty() && matches!(chunk, Chunk::Thought(_)) {
            return;
        }
        self.end_stream(id);
        let Some((session, buffer)) = self.session_buffer(id) else {
            return;
        };
        let row = matches!(chunk, Chunk::Thought(_));
        let kind = match &chunk {
            Chunk::Line => BlockKind::Line {
                text: text.into(),
                dim: false,
            },
            Chunk::Message(message) | Chunk::User(message) => BlockKind::Message(Message {
                id: message.clone(),
                user: matches!(chunk, Chunk::User(_)),
            }),
            Chunk::Thought(_) => BlockKind::Thought(Thought::default()),
        };
        if !row {
            session.end_run(buffer);
        }
        let path = session.push(buffer, text, kind, row);
        if let Chunk::Message(_) | Chunk::Thought(_) = chunk {
            let start = session.range(buffer, path).start;
            session.stream = Some(Stream {
                start: (buffer.add_marker(start, InsertionType::Stay)).expect("in the text"),
                chunk,
                source: text.into(),
                dirty: true,
                stable: (0, 0),
                began: session.live().then_some((now, clock)),
            });
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
        let mentioned = self.chips.replace_mentions(&linked, file);
        let known = |name: &str| session.commands.iter().any(|c| c.name == name);
        let text = &self.chips.replace_commands(&mentioned, known);
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        // A chunk of the last block's user message joins it, as Claude's adapter replays each
        // of a message's blocks, a chip's link among them, as a chunk.
        let end = session.end(buffer).saturating_sub("\n\n".len());
        let last = session.last_message().map(|path| session.block(path));
        let last = last.filter(
            |b| matches!(&b.kind, BlockKind::Message(m) if m.id.is_some() && m.id == message),
        );
        if let Some(last) = last
            && let Some(start) = buffer.marker(last.start)
            && buffer.marker(last.end) == Some(end)
            && session.stream.is_none()
        {
            buffer.insert_output(end, text);
            let to = end + text.chars().count();
            buffer.set_marker(last.end, to).expect("just inserted");
            let range = start + PROMPT.len()..to;
            render(buffer, range.clone(), false, None);
            self.chips.paint(buffer, range);
            // Its first line's band and borders go on again, the bottom one moving down.
            buffer.remove_properties(start..start + 1);
            prompt_glyph(buffer, start);
            user_lines(buffer, start..to);
            return;
        }
        self.output(id, &format!("{PROMPT}{text}"), Chunk::User(message));
        let clock = self.clock();
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let end = session.end(buffer) - "\n\n".len();
        render(buffer, end - text.chars().count()..end, false, None);
        self.chips.paint(buffer, end - text.chars().count()..end);
        let echo = end - text.chars().count() - PROMPT.len();
        prompt_glyph(buffer, echo);
        let end = session.end(buffer) - "\n\n".len();
        user_lines(buffer, echo..end);
        if let (true, Some(clock)) = (session.live(), clock) {
            put_meta(buffer, end, META_INDENT, &[(&clock, FaceName::Annotation)]);
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
        let term = (session.tools.get_mut(&call.tool_call_id))
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
        let asked = (session.asks()).find(|(_, a)| a.tool.as_ref() == Some(&call.tool_call_id));
        let waiting = asked.map(|(_, a)| now - a.since);
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
        let content = tool_content(&call, term.as_ref());
        let index = self.index.get(&session.project);
        let link = tool_path(&call, term.as_ref())
            .and_then(|(path, line)| Some((index?.file(path)?, line.map(|l| l as usize))));
        let call_id = call.tool_call_id.clone();
        let tool = Tool {
            call,
            header: 0,
            open: false,
            arrived,
            waited,
            ran,
            ended,
            group,
            time,
            term,
            dirty: false,
            faces: content.faces,
            output: content.output,
            output_buffer: None,
            link,
            replaced: false,
        };
        let mut tool = match session.tools.remove(&call_id) {
            Some(old) => Tool {
                open: old.open,
                output_buffer: old.output_buffer,
                replaced: old.replaced,
                ..tool
            },
            None => tool,
        };
        let header = session.tool_header(&tool).chars().count();
        let len = header + content.text.chars().count() + usize::from(!content.text.is_empty());
        tool.header = header;
        let call = &tool.call;
        if let Some(term) = &mut tool.term {
            let output = term.sgr.text.strip_suffix('\n').unwrap_or(&term.sgr.text);
            let command = tool_command(call, Some(&*term)).map_or(0, fence_len);
            let fence = (term.sgr.backticks.max(2) + 1).max(command);
            let chars = term.sgr.chars - usize::from(term.sgr.text.ends_with('\n'));
            term.rendered = (!term.sgr.text.is_empty()).then(|| TermRender {
                bytes: output.len(),
                chars,
                at: len - header - 1 - fence - 1 - chars,
                faces: term.sgr.faces.len(),
                fence,
            });
        }
        let rendered = session.render_tool(&tool, &content.text);
        let replaced = tool.replaced;
        session.tools.insert(call_id.clone(), tool);
        let path = match new {
            false => session.tool_path(&call_id),
            true => session.push(buffer, "", BlockKind::Tool(call_id.clone()), true),
        };
        session.write(buffer, path, rendered, &Look::at(now));
        if replaced && group.is_none_or(|g| session.group_open(g)) {
            let block = session.block(path);
            let range = block.range(buffer);
            hide_replaced(buffer, &session.tools[&call_id], range, block.meta);
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
            if let Some((session, buffer)) = self.session_buffer(id) {
                session.render_group(buffer, g);
            }
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
        if self
            .agents
            .get(&id)
            .is_some_and(|s| s.plan_path().is_none())
        {
            self.end_stream(id);
        }
        let now = self.now();
        let Some((session, buffer)) = self.session_buffer(id) else {
            return;
        };
        let path = match session.plan_path() {
            Some(path) => {
                session.block_mut(path).kind = BlockKind::Plan(plan.clone());
                path
            }
            None => {
                session.end_run(buffer);
                session.push(buffer, "", BlockKind::Plan(plan.clone()), false)
            }
        };
        let rendered = session.render(path, &Look::at(now));
        session.write(buffer, path, rendered, &Look::at(now));
    }

    /// The time of the last `handle`.
    fn now(&self) -> Instant {
        self.now.expect("set by handle")
    }

    /// Renders session `id`'s streamed block if chunks arrived since it was last rendered.
    fn render_stream(&mut self, id: AgentId) {
        let table_width = self.settings.table_width;
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
            let thought = matches!(stream.chunk, Chunk::Thought(_));
            if !thought {
                // shortcut: links are found as the block renders; a file indexed later isn't
                // linked until it renders again, and RET on one since dropped sends the prompt.
                let look = Look {
                    index: self.index.get(&session.project),
                    table_width,
                    now: self.now.expect("set by handle"),
                };
                let is_file = |word: &str| file_link(look.index, word).is_some();
                let (byte, at) = stream.stable;
                let tail = &stream.source[byte..];
                let (text, props, (b, c)) =
                    markdown::render(tail, false, false, Some(table_width), is_file);
                stream.stable = (byte + b, at + c);
                let path = session.last_message().expect("a message streams");
                let rendered = Rendered {
                    from: at,
                    text,
                    props,
                    meta: None,
                };
                session.write(buffer, path, rendered, &look);
            } else if !session.fold_stream(buffer) {
                render(buffer, start..end, thought, None);
            }
        }
    }

    /// Renders and ends session `id`'s stream, if any, writing its time: a message's clock time
    /// or a thought's header. They wait until now as its first line may still be growing.
    fn end_stream(&mut self, id: AgentId) {
        self.trim_message(id);
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
        let thought = matches!(stream.chunk, Chunk::Thought(_));
        let mut header = None;
        if let Some((began, clock)) = stream.began {
            match (stream.chunk, clock, session.last_message()) {
                (Chunk::Thought(_), ..) => {
                    let elapsed = self.now.expect("set by handle") - began;
                    let text = format!("Thought for {}s", elapsed.as_secs());
                    buffer.insert_output(start, &format!("{text}\n"));
                    let end = start + text.chars().count();
                    (buffer.add_property(start..end, dim())).expect("just inserted");
                    header = Some(end - start);
                }
                (Chunk::Message(_), Some(clock), Some(message)) => {
                    session.set_meta(buffer, message, &[(clock, FaceName::Annotation)]);
                }
                _ => {}
            }
        }
        if thought {
            session.end_thought(buffer, header);
        }
    }

    /// Drops the newlines a streamed message or thought ends with, as Claude's message does before
    /// a tool call and Codex's thought at each summary part, so one blank line still separates it
    /// from the next block.
    fn trim_message(&mut self, id: AgentId) {
        let Some((session, buffer)) = self.session_buffer(id) else {
            return;
        };
        let Some(Stream {
            start,
            chunk: Chunk::Message(_) | Chunk::Thought(_),
            source,
            ..
        }) = &mut session.stream
        else {
            return;
        };
        source.truncate(source.trim_end_matches('\n').len());
        let start = buffer.marker(*start).expect("live while streaming");
        let end = session.end(buffer) - "\n\n".len();
        let text = buffer.text().slice(start..end);
        let n = text
            .chars_at(end - start)
            .reversed()
            .take_while(|&c| c == '\n')
            .count();
        if n > 0 {
            buffer.replace_output(end - n..end, "");
            // The message's end marker moves back with the deleted newlines.
            session.stream.as_mut().expect("matched above").dirty = true;
        }
    }

    /// Ends session `id`'s turn, turning its working line into `Worked` in place.
    fn settle_turn(&mut self, id: AgentId) {
        self.end_stream(id);
        let now = self.now();
        let clock = self.clock();
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let (Some(turn), Some(buffer)) = (&session.turn, self.buffers.get_mut(session.buffer))
        else {
            return;
        };
        let worked = format!("Worked {}", long_duration(now - turn.start));
        let len = worked.chars().count();
        let time = clock.map_or(String::new(), |c| format!(" {c}"));
        let annotation = || Prop::Named(vec![FaceRef::Face(FaceName::Annotation)]);
        // Frame 6's summary rule: two columns in, a line through the rest of the row.
        let mut props = vec![
            (0..len, annotation()),
            (0..1, Prop::Line(Decoration::Indent(2))),
            (0..1, Prop::Line(Decoration::HRule(FaceName::Separator))),
        ];
        if !time.is_empty() {
            props.push((len..len + 1, Prop::AlignRight));
            props.push((len + 1..len + time.chars().count(), annotation()));
        }
        let rendered = Rendered {
            from: 0,
            text: worked + &time,
            props,
            meta: None,
        };
        // Its line stays as written; the block goes with the turn.
        let path = (session.blocks.len() - 1, None);
        session.put(buffer, path, rendered);
        let working = session.blocks.pop().expect("the working line");
        buffer.remove_marker(working.start);
        buffer.remove_marker(working.end);
        session.turn = None;
    }

    /// Brings session `id`'s working line and waiting counters up to the last `handle`.
    fn tick_session(&mut self, id: AgentId) {
        let look = Look::at(self.now());
        let session = self.agents.get_mut(&id).expect("known session");
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        if session.turn.is_some() {
            let path = (session.blocks.len() - 1, None);
            let rendered = session.render(path, &look);
            let shown = buffer.text().slice(session.range(buffer, path)) == rendered.text.as_str();
            if !shown {
                session.write(buffer, path, rendered, &look);
            }
        }
        let asks: Vec<_> = (session.asks())
            .map(|(_, a)| (a.tool.clone(), a.since))
            .collect();
        for (at, (call, since)) in asks.iter().enumerate() {
            // Its tool's oldest ask counts.
            let oldest = asks.iter().position(|(c, _)| c == call);
            let Some(call) = call.as_ref().filter(|_| oldest == Some(at)) else {
                continue;
            };
            let Some(tool) = session.tools.get_mut(call) else {
                continue;
            };
            let text = waiting_text(look.now - *since);
            if tool.time == text {
                continue;
            }
            tool.time = text;
            let path = session.tool_path(call);
            let tool = &session.tools[call];
            if tool.group.is_none() {
                let meta = [(tool.time.clone(), time_face(tool))];
                session.set_meta(buffer, path, &meta);
                continue;
            }
            let header = session.tool_header(tool).chars().count();
            session.tools.get_mut(call).expect("shown").header = header;
            let tool = &session.tools[call];
            let content = tool_content(&tool.call, tool.term.as_ref());
            let rendered = session.render_tool(tool, &content.text);
            session.write(buffer, path, rendered, &look);
        }
    }

    /// Wakes the core each second while a shown session has something ticking, and not otherwise,
    /// so a hidden session costs no redraws.
    pub(crate) fn sync_session_tick(&mut self, now: Instant) {
        let shown = self.shown_buffers();
        let ticking = (self.agents.values())
            .any(|s| shown.contains(&s.buffer) && (s.turn.is_some() || s.asks().next().is_some()));
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
        // The prompt area carries only chips and pastes, so it is cleared whole.
        if std::mem::replace(&mut session.painted, buffer.tick()) != buffer.tick() {
            let range = buffer.prompt_start()..buffer.len_chars();
            // `remove_properties` walks the whole transcript's; a bisect finds none to remove.
            let inside = |r: &Range<usize>| r.start >= range.start && r.end <= range.end;
            if buffer
                .properties_near(range.clone())
                .iter()
                .any(|(r, _)| inside(r))
            {
                buffer.remove_properties(range.clone());
            }
            self.chips.paint(buffer, range);
            session.paint_pastes(buffer, buffer.prompt_start());
        }
    }

    /// Repaints every session buffer's chips, after a chip's file went or came back.
    pub(crate) fn repaint_chips(&mut self) {
        for session in self.agents.values_mut() {
            if let Some(buffer) = self.buffers.get_mut(session.buffer) {
                self.chips.paint(buffer, 0..buffer.len_chars());
                session.paint_pastes(buffer, 0);
            }
        }
    }

    /// The chip at point in a session buffer.
    fn chip_at_point(&self) -> Option<&chips::Chip> {
        let buffer = self.windows.window.buffer();
        self.session_of(buffer)?;
        // A command chip has nothing to open, so `RET` on it sends the prompt.
        (self
            .chips
            .get(self.current().text().get_char(self.point())?))
        .filter(|c| c.kind != chips::Kind::Command)
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

    /// Appends a command's new output to its block: a folded call's, or an open call's past the
    /// cut, into its hidden lines; false for the rest, which `show_tool` rebuilds.
    fn append_tool_output(&mut self, id: AgentId, call: &ToolCallId) -> bool {
        let now = self.now();
        let session = self.agents.get_mut(&id).expect("known session");
        let path = session.tool_path(call);
        let buffer = self
            .buffers
            .get_mut(session.buffer)
            .expect("session buffer");
        let Range { start, end } = session.range(buffer, path);
        let tool = session.tools.get_mut(call).expect("dirty call");
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
        let chars = term.sgr.chars - usize::from(term.sgr.text.ends_with('\n'));
        let at = end - rendered.fence - 1;
        // An open call's hidden lines end where its output does, under the `… N more lines` fold.
        let fold = match tool.open {
            true => match buffer.fold_ending_at(at) {
                Some(fold) => Some(fold),
                None => return false,
            },
            false => None,
        };
        // The closing lines keep their text and properties, past the new output.
        let shift = tail.chars().count();
        let closing = (buffer.properties_near(at..end).into_iter())
            .filter(|(r, _)| r.start >= at && r.end <= end)
            .map(|(r, p)| (r.start - at + shift..r.end - at + shift, p.clone()))
            .collect();
        let text = format!("{tail}{}", buffer.text().slice(at..end));
        let grew = chars - rendered.chars;
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
        let tail = Rendered {
            from: at - start,
            text,
            props: closing,
            meta: None,
        };
        session.put(buffer, path, tail);
        if let Some((fold, from)) = fold {
            let text = buffer.text();
            let lines = text.char_to_line(at + grew) - text.char_to_line(from) + 1;
            buffer.extend_fold(fold, at + grew, more_lines(lines));
        }
        #[cfg(debug_assertions)]
        session.check(buffer, path, &Look::at(now));
        #[cfg(not(debug_assertions))]
        let _ = now;
        true
    }

    /// The agent and session whose session buffer is `buffer`.
    pub(crate) fn session_of(&self, buffer: BufferId) -> Option<(AgentId, &Session)> {
        let id = self.buffers.get(buffer)?.agent()?;
        Some((id, self.agents.get(&id)?))
    }

    /// The session whose buffer the selected window shows.
    fn current_session(&mut self) -> Result<(AgentId, &mut Session), String> {
        let id = self
            .current()
            .agent()
            .ok_or("Not in an agent session buffer")?;
        let session = self
            .agents
            .get_mut(&id)
            .ok_or("Not in an agent session buffer")?;
        Ok((id, session))
    }

    /// The current session, once its `session/new` is answered and while its agent runs.
    fn ready_session(&mut self) -> Result<(AgentId, &mut Session), String> {
        let (id, session) = self.current_session()?;
        let agent = &session.agent;
        match (&session.session, session.ended) {
            (_, true) => Err(format!("{agent} session has ended")),
            (None, _) => Err(format!("{agent} is not ready")),
            _ => Ok((id, session)),
        }
    }

    /// The values of the current session's `category` option (model or effort), groups
    /// flattened; none outside a session buffer.
    fn session_values(
        &self,
        category: SessionConfigOptionCategory,
    ) -> (
        Vec<&SessionConfigSelectOption>,
        Option<&SessionConfigValueId>,
    ) {
        let buffer = self.windows.window.buffer();
        let session = self
            .session_of(buffer)
            .and_then(|(_, s)| s.session.as_ref());
        let options = (session.and_then(|s| s.config_options.as_deref())).unwrap_or_default();
        match select_option(options, category) {
            Some((_, select)) => (self::options(select), Some(&select.current_value)),
            None => (Vec::new(), None),
        }
    }

    /// The `Model:` or `Effort:` annotation of the value named `name`: its description's first
    /// line.
    pub(crate) fn value_note(
        &self,
        category: SessionConfigOptionCategory,
        name: &str,
    ) -> Option<String> {
        let (values, _) = self.session_values(category);
        let value = values.into_iter().find(|v| v.name == name)?;
        Some(value.description.as_deref()?.lines().next()?.to_owned())
    }

    /// Where the current value is among the `Model:` or `Effort:` candidates, else 0.
    pub(crate) fn value_at(&self, category: SessionConfigOptionCategory) -> usize {
        let (values, current) = self.session_values(category);
        (values.iter().position(|v| Some(&v.value) == current)).unwrap_or(0)
    }

    /// The current session's slash commands; none outside a session buffer.
    pub(crate) fn session_commands(&self) -> &[AvailableCommand] {
        let buffer = self.windows.window.buffer();
        self.session_of(buffer).map_or(&[], |(_, s)| &s.commands)
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

    /// The registered agents `project` enables, in registry order: its own list, else the
    /// settings', else every one.
    pub(crate) fn enabled_agents(&self, project: &crate::project::Project) -> Vec<String> {
        let list = (project.enabled_agents.as_ref()).or(self.settings.enabled_agents.as_ref());
        (self.settings.agents.keys())
            .filter(|name| list.is_none_or(|list| list.contains(name)))
            .cloned()
            .collect()
    }

    fn current_project(&self) -> Option<&crate::project::Project> {
        let workspace = self.current_workspace().ok()?;
        Some(&self.projects[self.project_at(workspace.project).ok()?].project)
    }

    /// The agent names `agent-start` completes over: the current project's enabled agents.
    pub(crate) fn agent_names(&self) -> Vec<String> {
        (self.current_project()).map_or_else(Vec::new, |p| self.enabled_agents(p))
    }

    /// The current project's `default_agent` among `agent_names`, else the first.
    pub(crate) fn default_agent_index(&self) -> usize {
        let default = self
            .current_project()
            .and_then(|p| p.default_agent.as_ref());
        default
            .and_then(|name| self.agent_names().iter().position(|n| n == name))
            .unwrap_or(0)
    }

    /// The session buffer of `agent`'s session `id`, if one is open or loading.
    pub(crate) fn open_session(&self, agent: &str, id: &SessionId) -> Option<BufferId> {
        let session = &self.agents[&self.open_agent(agent, id)?];
        Some(session.buffer).filter(|&b| self.buffers.get(b).is_some())
    }

    /// The agent running `agent`'s session `id`, if it is open or loading.
    pub(crate) fn open_agent(&self, agent: &str, id: &SessionId) -> Option<AgentId> {
        (self.agents.iter())
            .find(|(_, s)| {
                let answered = s.session.as_ref().map(|r| &r.session_id);
                let loading = s.load.as_ref().and_then(|(id, _)| id.as_ref());
                s.agent == agent && answered.or(loading) == Some(id)
            })
            .map(|(&id, _)| id)
    }

    /// The state of the session `agent` started or answered as `id`, while it is open, in the
    /// workspace list's words and faces.
    pub(crate) fn session_state(
        &self,
        agent: &str,
        id: &SessionId,
    ) -> Option<(&'static str, FaceName)> {
        let buffer = self.open_session(agent, id)?;
        let (_, s) = self.session_of(buffer)?;
        let quiet = |text| (text, FaceName::ModeLineDim);
        Some(if s.session.is_none() && !s.ended {
            quiet(if s.load.is_some() {
                "○ loading"
            } else {
                "○ starting"
            })
        } else if let Some(activity) = s.activity() {
            (activity.text(), activity.face())
        } else {
            quiet("○ idle")
        })
    }

    /// The selected window's session, ready, its session id and, unless point is on the prompt
    /// line, the message the bar marks, if any.
    fn session_at_point(&self) -> Result<(&Session, SessionId, Option<Option<&Message>>), String> {
        let (point, buffer) = (self.point(), self.current());
        let shown = self.windows.window.buffer();
        let (_, session) = (self.session_of(shown)).ok_or("Not in an agent session buffer")?;
        let source = (session.session.as_ref())
            .map(|r| r.session_id.clone())
            .ok_or_else(|| format!("{} is not ready", session.agent))?;
        if point >= buffer.prompt_start() - PROMPT.len() {
            return Ok((session, source, None));
        }
        let message = session.message_at(buffer, point).map(|(_, m)| m);
        Ok((session, source, Some(message)))
    }

    /// Whether any agent session hasn't ended: waiting, starting or ready.
    pub(crate) fn live_agents(&self) -> bool {
        self.agents.values().any(|s| !s.ended)
    }

    /// Whether buffer `id` is the session buffer of a session that hasn't ended.
    pub(crate) fn live_agent(&self, id: BufferId) -> bool {
        self.session_of(id).is_some_and(|(_, s)| !s.ended)
    }

    /// Puts rolled back session buffer `source`'s fork, `buffer` with its `log`, in its place:
    /// in every window showing it and under its and its log's names. Killing the source stops
    /// its agent, then the fork's agent `id` deletes its session. Nothing happens once the
    /// source is gone. A pin of the source moves to the fork.
    fn take_place(
        &mut self,
        id: AgentId,
        (source, buffer, log): (BufferId, BufferId, BufferId),
        effects: &mut Vec<Effect>,
    ) {
        let Some(old) = (self.buffers.get(source).and_then(Buffer::agent))
            .and_then(|old| self.agents.get_mut(&old))
        else {
            return;
        };
        old.delete = Some(id);
        let listed = (
            old.agent.clone(),
            old.session.as_ref().map(|r| r.session_id.clone()),
        );
        let (old, root) = (old.log, old.root.clone());
        if let (agent, Some(from)) = listed {
            self.forget_listed(&agent, &from);
            let fork = self.agents[&id]
                .session
                .as_ref()
                .map(|r| r.session_id.clone());
            if let Some(to) = fork {
                self.move_pin(&agent, &root, (&from, &to), effects);
            }
        }
        let name = |id| self.buffers.get(id).map(|b| b.name.clone());
        let names = (name(source), name(old));
        let inactive = (self.workspaces.iter_mut()).filter_map(|w| w.windows.as_mut());
        for windows in std::iter::once(&mut self.windows).chain(inactive) {
            for window in windows.windows_mut().filter(|w| w.buffer() == source) {
                window.show(&mut self.buffers, buffer);
            }
        }
        self.buffers.raise(buffer);
        self.kill_buffer(source);
        self.kill_buffer(old);
        for (id, name) in [(buffer, names.0), (log, names.1)] {
            if let Some(name) = name {
                self.buffers.rename(id, &name);
            }
        }
    }

    /// Stops the agent of session `id`, whose session buffer was killed, and forgets it,
    /// cancelling its pending requests first.
    pub(crate) fn release_session(&mut self, id: AgentId) {
        let Some(session) = self.agents.remove(&id) else {
            return;
        };
        for (_, ask) in session.asks() {
            let (response, _) = ask.cancelled();
            self.released.push(Effect::Agent(AgentOp::Reply {
                id,
                request: ask.request.clone(),
                result: Ok(response),
            }));
        }
        let opened = session.session.as_ref().map(|r| r.session_id.clone());
        self.released
            .push(Effect::Agent(match session.delete.zip(opened) {
                Some((fork, session)) => AgentOp::Delete {
                    id: fork,
                    source: id,
                    session,
                },
                None => AgentOp::Kill { id },
            }));
    }
}

impl Ctx<'_> {
    /// `agent-start`: starts the named agent in the current workspace's project, showing a new
    /// session buffer, with a new log buffer for its stderr.
    pub(crate) fn agent_start(&mut self, args: Args) -> Result<(), String> {
        self.start_session(&args.strings[0], Open::New, None)
    }

    /// Starts agent `name` as `agent-start` does; one that loads or forks a session is titled
    /// `title` and shows its buffer only once loaded.
    pub(crate) fn start_session(
        &mut self,
        name: &str,
        open: Open,
        title: Option<&str>,
    ) -> Result<(), String> {
        let core = &mut *self.core;
        let session_title = session_title(title);
        let agent = (core.settings.agents.get(name))
            .ok_or_else(|| format!("{name:?} is not a registered agent"))?
            .clone();
        let workspace = core.current_workspace()?;
        let (project, log, title, workspace) = (
            workspace.project,
            format!("*{name} stderr*<{}>", workspace.name),
            session_name(name, &workspace.name, title),
            workspace.name.clone(),
        );
        let loaded = &core.projects[core.project_at(project)?].project;
        if matches!(open, Open::New) && !core.enabled_agents(loaded).iter().any(|n| n == name) {
            return Err(format!("{name:?} is not enabled in {}", loaded.name));
        }
        // Under the captured environment, which `release_agents` puts first.
        let env = (agent.env.into_iter()).chain(loaded.env.clone()).collect();
        let cwd = loaded.primary.path.clone();
        let roots = loaded.roots.iter().map(|r| r.path.clone()).collect();
        let id = AgentId(core.next_agent);
        core.next_agent += 1;
        let log = core.buffers.create(&log);
        core.buffers.get_mut(log).expect("just created").read_only = true;
        let buffer = core.buffers.create(&title);
        let b = core.buffers.get_mut(buffer).expect("just created");
        b.append(PROMPT);
        prompt_glyph(b, 0);
        let start = PROMPT.len();
        let prompt = (b.add_marker(start, InsertionType::Stay)).expect("after the prompt");
        b.kind = Kind::Session { agent: id, prompt };
        b.set_marker(b.point, start).expect("after the prompt");
        b.major_mode = MODE;
        let from = core.windows.window.buffer();
        let (echo, load) = match &open {
            Open::New => (format!("Starting {name}..."), None),
            Open::Load(session) => (format!("Loading {name} session..."), Some(session.clone())),
            Open::Fork(..) => (format!("Forking {name} session..."), None),
            Open::Rollback(..) => (format!("Rolling back {name} session..."), None),
        };
        let rollback = matches!(open, Open::Rollback(..));
        let load = (open != Open::New).then_some((load, from));
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
            session: open,
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
            load,
            rollback,
            delete: None,
            sent: None,
            ended: false,
            turn: None,
            stream: None,
            blocks: Vec::new(),
            tools: HashMap::new(),
            run: 0,
            group: None,
            painted: 0,
            commands: Vec::new(),
            pastes: Vec::new(),
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
                            session.sent = Some(format!("mode {default}"));
                        }
                    }
                    (Some(default), _) => note = Some(format!("{agent} has no mode {default}")),
                    (None, _) => {}
                }
                session.session = Some(response);
                let selected = core.windows.window.buffer();
                let (buffer, log) = (session.buffer, session.log);
                match session.load {
                    Some((_, source)) if session.rollback => {
                        core.take_place(id, (source, buffer, log), &mut self.effects)
                    }
                    Some((_, from)) if from == selected => core.switch_to(buffer),
                    _ => {}
                }
                core.echo(format!("{agent} session ready (modes: {modes})"));
                if let Some(note) = note {
                    core.log(&note);
                }
                self.effects.extend(set.map(Effect::Agent));
            }
            // A failed fork leaves nothing: killing its buffers forgets it and stops its agent.
            Err(err) if matches!(session.load, Some((None, _))) => {
                let (buffer, log) = (session.buffer, session.log);
                let (what, at) = match session.rollback {
                    true => ("rollback", "roll back to"),
                    false => ("fork", "fork at"),
                };
                core.kill_buffer(buffer);
                core.kill_buffer(log);
                // claude-agent-acp's answer for a message not in the transcript, like a local
                // command's reply.
                if err.contains("Fork point message") {
                    core.log(&format!("{agent} {what} failed: {err}"));
                    core.echo(format!("Can't {at} this message"));
                } else {
                    core.echo(format!("{agent} {what} failed: {err}"));
                }
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
                    core.output(id, &text.text, Chunk::Message(chunk.message_id));
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    core.output(id, &text.text, Chunk::Thought(chunk.message_id));
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
                session.commands = update.available_commands;
            }
            _ => {}
        }
    }

    /// Shows a permission request or a question from a ready session, declining a form it can't
    /// draw, and cancels one from an ended or killed session; refuses either before the session
    /// is ready, and every other request.
    pub(crate) fn agent_request(&mut self, id: AgentId, request: RequestId, body: Request) {
        let core = &mut *self.core;
        let session = core.agents.get(&id);
        let ready = session.is_some_and(|s| !s.ended && s.session.is_some());
        // An unknown id is a session forgotten when its buffer was killed.
        let gone = session.is_none_or(|s| s.ended);
        let elicited = |action| {
            Ok(ClientResponse::CreateElicitationResponse(
                CreateElicitationResponse::new(action),
            ))
        };
        let result = match *body.0 {
            AgentRequest::RequestPermissionRequest(ask) if ready => {
                return core.show_permission(id, request, ask);
            }
            AgentRequest::RequestPermissionRequest(_) if gone => {
                Ok(ClientResponse::RequestPermissionResponse(
                    RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled),
                ))
            }
            AgentRequest::CreateElicitationRequest(ask) if ready => match questions::parse(&ask) {
                Some(question) => return core.show_question(id, request, question),
                None => {
                    let agent = &core.agents[&id].agent;
                    core.echo(format!("Can't show {agent}'s question yet; declined"));
                    elicited(ElicitationAction::Decline)
                }
            },
            AgentRequest::CreateElicitationRequest(_) if gone => {
                elicited(ElicitationAction::Cancel)
            }
            _ => Err(Error::method_not_found()),
        };
        (self.effects).push(Effect::Agent(AgentOp::Reply {
            id,
            request,
            result,
        }));
    }

    /// `agent-toggle-prompt`: moves point from the prompt area to the transcript point, put in
    /// the rows shown, and from the transcript to where point was left in the prompt area.
    pub(crate) fn agent_toggle_prompt(&mut self) -> Result<(), String> {
        self.core.current_session()?;
        let at = match self.prompt_line() {
            Some(prompt) => self.reading_point(prompt),
            None => self.drafting_point(),
        };
        self.goto(at)
    }

    /// Where point was left in the current session's prompt area, else its end.
    pub(crate) fn drafting_point(&self) -> usize {
        let core = &*self.core;
        (core.windows.window.remembered(&core.buffers, true))
            .filter(|&p| p >= core.current().prompt_start())
            .unwrap_or(core.current().len_chars())
    }

    /// Point's line and its byte offset in it, when point is on a link above the prompt.
    fn linked_line(&self) -> Option<(String, usize)> {
        let core = &*self.core;
        let (point, buffer) = (core.point(), core.windows.window.buffer());
        let b = core.buffers.get(buffer)?;
        let link = FaceRef::Face(FaceName::Link);
        let linked = (b.properties_near(point..point).into_iter())
            .any(|(r, p)| r.contains(&point) && matches!(p, Prop::Named(f) if f.contains(&link)));
        if !linked || point >= b.prompt_start() {
            return None;
        }
        let text = b.text();
        let start = text.line_to_char(text.char_to_line(point));
        let line = text.slice(start..line_end(b, point)).to_string();
        let at = line
            .char_indices()
            .nth(point - start)
            .map_or(line.len(), |(i, _)| i);
        Some((line, at))
    }

    /// The destination of the link point is on, outside tool call headers.
    fn destination_at_point(&self) -> Option<String> {
        if self.tool_at_point().is_some() {
            return None;
        }
        let (line, at) = self.linked_line()?;
        // A drawn table keeps a link's destination on its label, as the text has none.
        let (core, point) = (&*self.core, self.core.point());
        let b = core.buffers.get(core.windows.window.buffer())?;
        let kept = (b.properties_near(point..point).into_iter()).find_map(|(r, p)| match p {
            Prop::Url(url) if r.contains(&point) => Some(url.clone()),
            _ => None,
        });
        kept.or_else(|| markdown::url_at(&line, at))
    }

    /// The URL of the link point is on, when it has a scheme; a relative destination is left
    /// to file links.
    fn url_at_point(&self) -> Option<String> {
        let url = self.destination_at_point()?;
        // A scheme is a letter, then letters, digits, `+`, `-` or `.`, before a `:` that isn't
        // a `path:line`'s.
        let (scheme, rest) = url.split_once(':')?;
        let scheme_chars = |c: char| c.is_ascii_alphanumeric() || "+-.".contains(c);
        let is_scheme = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme.chars().all(scheme_chars)
            && !rest.starts_with(|c: char| c.is_ascii_digit());
        is_scheme.then_some(url)
    }

    /// The file and line, from 1, of the link point is on: a tool call header's, else a Markdown
    /// link's destination, else the word's.
    fn link_at_point(&self) -> Option<(PathBuf, Option<usize>)> {
        let (line, at) = self.linked_line()?;
        let core = &*self.core;
        let buffer = core.windows.window.buffer();
        let (_, session) = core.session_of(buffer)?;
        if let Some((_, call)) = self.tool_at_point() {
            return session.tools[&call].link.clone();
        }
        let index = core.index.get(&session.project);
        if let Some(to) = self.destination_at_point() {
            return file_link(index, &to);
        }
        let word = markdown::words(&line)
            .into_iter()
            .find(|w| w.contains(&at))?;
        file_link(index, &line[word])
    }

    /// The tool call whose header point is on.
    fn tool_at_point(&self) -> Option<(AgentId, ToolCallId)> {
        let core = &*self.core;
        let point = core.point();
        let buffer = core.windows.window.buffer();
        let b = core.buffers.get(buffer)?;
        let (id, session) = core.session_of(buffer)?;
        Some((id, session.tool_at(b, point)?.clone()))
    }

    /// Shows `call`'s full output in its own read-only buffer, refreshed from the call.
    // shortcut: a snapshot, not following a running command; RET refreshes it.
    fn show_tool_output(&mut self, id: AgentId, call: &ToolCallId) -> Result<(), String> {
        let core = &mut *self.core;
        let workspace = core.current_workspace()?.name.clone();
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
    /// option; on a web link, opens it in the browser; on a file link, visits the file in another window; on a tool call's header, shows
    /// its full output; in the prompt area sends its text as a `session/prompt`, leaving it above a
    /// new prompt line; elsewhere does nothing.
    pub(crate) fn agent_send_input(&mut self) -> Result<(), String> {
        if let Some((id, at, line)) = self.ask_at_point() {
            let ask = &self.core.agents[&id].ask(at);
            match ask.question().map(|q| q.answer_at(line)) {
                None if line > 0 => {
                    self.answer_ask(id, at, Some(line - 1));
                    return Ok(());
                }
                Some(Some(questions::Answer::Option(option))) => {
                    self.pick(id, at, option);
                    return Ok(());
                }
                Some(Some(questions::Answer::Own(_))) => {
                    self.own_answer(id, at);
                    return Ok(());
                }
                _ => {}
            }
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
        if let Some(url) = self.url_at_point() {
            if !markdown::is_web(&url) {
                return Err(format!("Not a web link: {url}"));
            }
            self.effects.push(Effect::OpenUrl(url));
            return Ok(());
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
        if self.core.point() < self.core.current().prompt_start() {
            return Ok(());
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
        let clock = core.clock();
        let session = core.agents.get_mut(&id).ok_or("the session is gone")?;
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
        prompt_glyph(b, end - PROMPT.len());
        b.set_marker(b.prompt().expect("a session buffer has a prompt"), end)?;
        let line = start + input.chars().count() + "\n\n".len();
        let echo = start - PROMPT.len();
        let to = start + input.chars().count();
        session.blocks.push(Block {
            start: b.add_marker(echo, InsertionType::Stay)?,
            end: b.add_marker(to, InsertionType::Stay)?,
            meta: 0,
            kind: BlockKind::Message(Message {
                id: None,
                user: true,
            }),
        });
        session.run = session.blocks.len();
        session.blocks.push(Block {
            start: b.add_marker(line, InsertionType::Advance)?,
            end: b.add_marker(line + working.chars().count(), InsertionType::Stay)?,
            meta: 0,
            kind: BlockKind::Working,
        });
        session.turn = Some(Turn { start: self.now });
        let (path, look) = ((session.blocks.len() - 1, None), Look::at(self.now));
        let rendered = session.render(path, &look);
        session.write(b, path, rendered, &look);
        // The old prompt area's edits would replay into read-only text.
        b.clear_undo();
        user_lines(b, echo..to);
        if let Some(clock) = clock {
            put_meta(b, to, META_INDENT, &[(&clock, FaceName::Annotation)]);
        }
        let end = b.len_chars();
        core.windows.window.follow = true;
        session.fold_pastes(b, start);
        let request = PromptRequest::new(session_id, core.chips.blocks(&input));
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
        let starts: Vec<usize> = (core.agents[&id].messages())
            .map(|(b, _)| buffer.marker(b.start).expect("live with its session"))
            .collect();
        let at = crate::motion::nth_mark(&starts, point, n).ok_or("No message found")?;
        self.goto(at)
    }

    /// `agent-fork-session`: forks the session into a new session buffer, whole with point on the
    /// prompt line, else at the message the bar marks.
    pub(crate) fn agent_fork_session(&mut self) -> Result<(), String> {
        let (session, source, message) = self.core.session_at_point()?;
        let at = match message {
            None => None,
            Some(Some(message)) if message.user => return Err("Fork at an agent message".into()),
            Some(message) => {
                Some((message.and_then(|m| m.id.clone())).ok_or("No message id to fork at")?)
            }
        };
        let (agent, title) = (session.agent.clone(), session.title.clone());
        self.start_session(&agent, Open::Fork(source, at), Some(&title))
    }

    /// `agent-rollback-session`: after asking, forks the session at the message the bar marks
    /// into a session that takes its buffer's place once loaded.
    pub(crate) fn agent_rollback_session(&mut self, args: Args) -> Result<(), String> {
        let (session, source, message) = self.core.session_at_point()?;
        let message = message.flatten().ok_or("No message at point")?;
        if message.user {
            return Err("Roll back to an agent message".into());
        }
        let at = message.id.clone().ok_or("No message id to roll back to")?;
        let (agent, title) = (session.agent.clone(), session.title.clone());
        match args.strings.first().map(String::as_str) {
            None => {
                let command = self.core.commands["agent-rollback-session"];
                let prompt = "Roll back to this message? Files stay as they are ".into();
                self.read_string(command, args, prompt, Some(&["no", "yes"]))
            }
            Some("yes") => self.start_session(&agent, Open::Rollback(source, at), Some(&title)),
            Some(_) => Ok(()),
        }
    }

    /// `agent-toggle-tool-call`: shows or hides the content of the tool call or thought row whose
    /// header point is on, or hides that of the open one point is in, moving point to its header.
    /// Elsewhere in a group, it opens or folds the group. In a block of several questions, it
    /// opens the next one.
    pub(crate) fn agent_toggle_tool_call(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let (point, look) = (core.point(), Look::at(core.now()));
        let buffer = core.windows.window.buffer();
        let agent = core.buffers.get(buffer).and_then(Buffer::agent);
        let session = agent.and_then(|id| Some((id, core.agents.get_mut(&id)?)));
        let b = core
            .buffers
            .get_mut(buffer)
            .expect("current buffer is live");
        let (id, session) = session.ok_or("No tool call here")?;
        // Point past the line it ends on goes back to its start.
        let back = |(start, line): (usize, usize)| (point > line).then_some(start);
        let to = match session.block_at(b, point).ok_or("No tool call here")? {
            At::Ask(at) => {
                self.open_question(id, at, |q| {
                    q.open = Some(q.open.map_or(0, |i| (i + 1) % q.fields.len()))
                });
                return Ok(());
            }
            At::Paste(i) => Some(session.toggle_paste(b, i)),
            At::Answer(at) => back(session.toggle_answer(b, at, &look)),
            At::Thought(at) => back(session.toggle_thought(b, at)),
            At::Group(g) => match session.toggle_group(b, g) {
                (start, line, false) => back((start, line)),
                _ => None,
            },
            At::Tool(call) => {
                let path = session.tool_path(&call);
                let range = session.range(b, path);
                let tool = session.tools.get_mut(&call).expect("just found");
                // Without content there is nothing to show, so it stays folded for when content
                // comes.
                tool.open = !tool.open && range.len() > tool.header;
                let mut rendered = session.render(path, &look);
                rendered.meta = None;
                session.write(b, path, rendered, &look);
                back((range.start, range.start + session.tools[&call].header))
            }
        };
        match to {
            Some(start) => self.goto(start),
            None => Ok(()),
        }
    }

    /// `agent-cancel`: sends `session/cancel` while a turn runs, then answers the pending
    /// permission requests `cancelled`, as ACP requires.
    pub(crate) fn agent_cancel(&mut self) -> Result<(), String> {
        let (id, session) = self.core.current_session()?;
        let ready = session.session.as_ref().filter(|_| session.turn.is_some());
        // Settling a request leaves the others' places as they are.
        let asks: Vec<_> = session.asks().map(|(at, _)| at).collect();
        let session = ready.ok_or("No turn is running")?.session_id.clone();
        (self.effects).push(Effect::Agent(AgentOp::Cancel { id, session }));
        for at in asks {
            self.cancel_ask(id, at);
        }
        Ok(())
    }

    pub(crate) fn agent_attach_file(&mut self, args: Args) -> Result<(), String> {
        self.core.current_session()?;
        let Some(name) = args.strings.first() else {
            let command = self.core.commands["agent-attach-file"];
            return self.read_string(command, args, "Attach: ".into(), None);
        };
        let directory = name.ends_with('/');
        let path = self.core.directory().join(name).components().collect();
        let chip = self.core.chips.intern(Some(path), directory);
        let at = if self.core.point() < self.core.current().prompt_start() {
            self.core.current().len_chars()
        } else {
            self.core.point()
        };
        self.core.current_mut().insert(at, &chip.to_string())?;
        self.goto(at + 1)
    }

    pub(crate) fn agent_reference(&mut self, args: Args) -> Result<(), String> {
        self.core.current_session()?;
        if let Some(path) = args.strings.first() {
            let point = self.core.point();
            let at = point.checked_sub(1).ok_or("No reference here")?;
            if at < self.core.current().prompt_start() || self.core.current().text().char(at) != '@'
            {
                return Err("No reference here".into());
            }
            let directory = path.ends_with('/');
            let path = PathBuf::from(path);
            let chip = self
                .core
                .chips
                .intern(Some(path.components().collect()), directory);
            self.delete_to(at)?;
            return self.insert_n(&chip.to_string(), 1, false);
        }
        self.insert_n("@", args.count(), false)?;
        if args.count() == 1 {
            self.project_file_prompt(args, true)?;
        }
        Ok(())
    }

    /// `agent-command`: inserts the command's chip and a space at point in the prompt area, or
    /// at its end from the transcript; an answer naming no command goes in as `/answer `.
    pub(crate) fn agent_command(&mut self, name: &str) -> Result<(), String> {
        self.core.current_session()?;
        let core = &mut *self.core;
        let known = core.session_commands().iter().any(|c| c.name == name);
        let text = match known {
            true => format!("{} ", core.chips.command(name)),
            false => format!("/{name} "),
        };
        let (point, start, end) = (
            core.point(),
            core.current().prompt_start(),
            core.current().len_chars(),
        );
        let at = if point >= start { point } else { end };
        // Appended after the prompt's text from the transcript, it starts a word of its own.
        let joins =
            point < start && end > start && !core.current().text().char(end - 1).is_whitespace();
        let text = if joins { format!(" {text}") } else { text };
        core.current_mut().insert(at, &text)?;
        self.goto(at + text.chars().count())
    }

    /// `/` in a session: `agent-command` at a word's start in the prompt area when the session
    /// has commands, else typed as itself.
    pub(crate) fn agent_slash(&mut self, args: Args) -> Result<(), String> {
        let core = &*self.core;
        let (point, start) = (core.point(), core.current().prompt_start());
        let before = point.checked_sub(1).map(|p| core.current().text().char(p));
        let word_start = point == start || before.is_some_and(char::is_whitespace);
        let pick = !core.session_commands().is_empty() && point >= start && word_start;
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
        let (id, session) = self.core.ready_session()?;
        let agent = &session.agent;
        let ready = session.session.as_ref().expect("ready");
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
        session.sent = Some(format!("mode {next}"));
        (self.effects).push(Effect::Agent(AgentOp::SetConfigOption { id, request }));
        Ok(())
    }

    /// `agent-select-model` and `agent-select-effort`: asks for a value of the session's
    /// `category` option, called `word`, by name, then sets the option to it.
    pub(crate) fn agent_select(
        &mut self,
        args: Args,
        category: SessionConfigOptionCategory,
        word: &str,
    ) -> Result<(), String> {
        let (id, session) = self.core.ready_session()?;
        let agent = &session.agent;
        let ready = session.session.as_ref().expect("ready");
        let options = ready.config_options.as_deref().unwrap_or_default();
        let none = || format!("{agent} has no {word} choice");
        let (config, select) = select_option(options, category).ok_or_else(none)?;
        let values = self::options(select);
        let Some(name) = args.strings.first() else {
            let names: Vec<String> = values.iter().map(|v| v.name.clone()).collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            let command = self.core.commands[format!("agent-select-{word}").as_str()];
            let prompt = format!("{}{}: ", word[..1].to_uppercase(), &word[1..]);
            return self.read_string(command, args, prompt, Some(&names));
        };
        let value = (values.iter().find(|v| v.name == *name))
            .ok_or_else(|| format!("{agent} has no {word} {name}"))?;
        let sent = format!("{word} {}", value.value);
        let request = SetSessionConfigOptionRequest::new(
            ready.session_id.clone(),
            config.clone(),
            value.value.clone(),
        );
        session.sent = Some(sent);
        (self.effects).push(Effect::Agent(AgentOp::SetConfigOption { id, request }));
        Ok(())
    }

    /// Ends the turn, cancelling the requests it left pending, with a line saying why unless it
    /// ended normally.
    pub(crate) fn agent_prompted(&mut self, id: AgentId, result: Result<StopReason, String>) {
        let Some(session) = self.core.agents.get(&id).filter(|s| s.turn.is_some()) else {
            return;
        };
        let asks: Vec<_> = session.asks().map(|(at, _)| at).collect();
        for at in asks {
            // codex-acp withdraws a non-blocking question at its turn's end, maybe after this.
            if self.core.agents[&id].ask(at).blocks() {
                self.cancel_ask(id, at);
            } else {
                let (response, _) = self.core.agents[&id].ask(at).cancelled();
                self.close_ask(id, at, "○ Closed, the turn ended first", Some(response));
            }
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
            core.output(id, &line, Chunk::Line);
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
                let (agent, sent) = (&session.agent, session.sent.as_deref());
                let note = format!("{agent} could not set {}: {err}", sent.unwrap_or(""));
                core.log(&note);
            }
        }
    }

    /// Ends the session after its agent exited, noting a crash, and releases the agent's handle.
    pub(crate) fn agent_exited(&mut self, id: AgentId, code: u32, signal: Option<String>) {
        let core = &mut *self.core;
        core.settle_turn(id);
        let (clock, look) = (core.clock(), Look::at(core.now()));
        let Some(session) = core.agents.get_mut(&id) else {
            return;
        };
        // The agent is gone with its requests, so only the blocks change.
        let b = (core.buffers.get_mut(session.buffer)).expect("live with its session");
        let paths: Vec<_> = session.asks().map(|(at, _)| at).collect();
        let asks: Vec<_> = (paths.into_iter())
            .map(|at| {
                let line = session.ask(at).cancelled().1;
                (at, session.settle(b, at, &line, clock.as_deref(), &look))
            })
            .collect();
        for (at, ask) in &asks {
            core.close_wait(id, ask, Some(*at));
        }
        if let Some(session) = core.agents.get_mut(&id)
            && !std::mem::replace(&mut session.ended, true)
        {
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
mod tests;
