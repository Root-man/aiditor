//! The transcript as one ordered list of blocks, each with a start and an end marker, and the
//! one function that writes a changed block into the buffer and repairs its markers.

use super::groups::{Group, Thought};
use super::render::{META_INDENT, Props, answered_props, paint, plan_props, put_meta, tool_props};
use super::*;
use crate::file_index::FileIndex;

/// A transcript block: where it starts and ends, the blank line after it not counted, the length
/// of the line under it that holds its time, 0 for none, and what it is.
pub(super) struct Block {
    pub(super) start: MarkerId,
    pub(super) end: MarkerId,
    pub(super) meta: usize,
    pub(super) kind: BlockKind,
}

impl Block {
    /// Its range in `buffer`.
    pub(super) fn range(&self, buffer: &Buffer) -> Range<usize> {
        let at = |m| buffer.marker(m).expect("live while shown");
        at(self.start)..at(self.end)
    }
}

pub(super) enum BlockKind {
    /// A line of the session's own: a turn's end, or a settled permission request's, dim.
    Line {
        text: String,
        dim: bool,
    },
    Message(Message),
    Thought(Thought),
    Tool(ToolCallId),
    Plan(Plan),
    Ask(Ask),
    Answered(Answered),
    /// A run of tool calls folded into one line; its end is its last row's.
    Group(Group),
    /// A running turn's working line, the last block, its start `Advance` so the blocks put
    /// before it move it on.
    Working,
}

/// A block's place: its index in `Session::blocks`, and its row's in that group.
pub(super) type BlockPath = (usize, Option<usize>);

/// A block as drawn: its text and properties from `from` chars into it, which are kept, and the
/// parts of the line under it, `None` leaving that line as it is.
pub(super) struct Rendered {
    pub(super) from: usize,
    pub(super) text: String,
    pub(super) props: Props,
    pub(super) meta: Option<Vec<(String, FaceName)>>,
}

/// What rendering reads besides the session: the files a message links to, the table width
/// and the time. Only a message reads the first two.
pub(super) struct Look<'a> {
    pub(super) index: Option<&'a FileIndex>,
    pub(super) table_width: usize,
    pub(super) now: Instant,
}

impl Look<'_> {
    /// For any block but a message.
    pub(super) fn at(now: Instant) -> Self {
        Look {
            index: None,
            table_width: 0,
            now,
        }
    }
}

/// Replaces `range` of `buffer`'s output with `text`, changing only what differs: char runs
/// when the length stays, else changed lines without their common ends, so point and markers
/// on unchanged text stay.
fn edit(buffer: &mut Buffer, range: Range<usize>, text: &str) {
    let old = buffer.text().slice(range.clone()).to_string();
    if old == text {
        return;
    }
    if old.chars().count() == text.chars().count() {
        let new: Vec<char> = text.chars().collect();
        let mut runs: Vec<Range<usize>> = Vec::new();
        for (i, (o, n)) in old.chars().zip(&new).enumerate() {
            match runs.last_mut() {
                _ if o == *n => {}
                Some(run) if run.end == i => run.end += 1,
                _ => runs.push(i..i + 1),
            }
        }
        for run in runs {
            let new: String = new[run.clone()].iter().collect();
            buffer.replace_output(range.start + run.start..range.start + run.end, &new);
        }
        return;
    }
    // Output put before the closing lines past a cut goes in at their start, which they follow.
    if let Some(head) = text.strip_suffix(old.as_str()) {
        return buffer.insert_output(range.start, head);
    }
    let chars: Vec<char> = old.chars().collect();
    for (r, new) in buffer::diff(&old, text).into_iter().rev() {
        let gone = &chars[r.clone()];
        let new: Vec<char> = new.chars().collect();
        let head = gone.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let ends = gone[head..].iter().rev().zip(new[head..].iter().rev());
        let tail = ends.take_while(|(a, b)| a == b).count();
        let new: String = new[head..new.len() - tail].iter().collect();
        let at = range.start + r.start;
        buffer.replace_output(at + head..at + r.len() - tail, &new);
    }
}

/// Whether the properties inside `range` of `buffer` are `props`, from its start.
fn shows(buffer: &Buffer, range: Range<usize>, props: &Props) -> bool {
    let key = |r: &Range<usize>| (r.start, r.end);
    let mut have: Vec<_> = (buffer.properties_near(range.clone()).into_iter())
        .filter(|(r, _)| r.start >= range.start && r.end <= range.end)
        .map(|(r, p)| (r.start - range.start..r.end - range.start, p))
        .collect();
    let mut want: Vec<_> = props.iter().map(|(r, p)| (r.clone(), p)).collect();
    if have.len() != want.len() {
        return false;
    }
    have.sort_by_key(|(r, _)| key(r));
    want.sort_by_key(|(r, _)| key(r));
    // Properties on one range compare as a set.
    let mut i = 0;
    while i < want.len() {
        let j = i + want[i..].iter().take_while(|w| w.0 == want[i].0).count();
        let (have, want) = (&have[i..j], &want[i..j]);
        if have.iter().any(|h| h.0 != want[0].0)
            || !want.iter().all(|w| have.iter().any(|h| h.1 == w.1))
        {
            return false;
        }
        i = j;
    }
    true
}

impl Session {
    /// The block at `path`.
    pub(super) fn block(&self, (at, row): BlockPath) -> &Block {
        let block = &self.blocks[at];
        match (row, &block.kind) {
            (None, _) => block,
            (Some(r), BlockKind::Group(group)) => &group.rows[r],
            _ => unreachable!("a row is in a group"),
        }
    }

    pub(super) fn block_mut(&mut self, (at, row): BlockPath) -> &mut Block {
        let block = &mut self.blocks[at];
        let Some(r) = row else {
            return block;
        };
        match &mut block.kind {
            BlockKind::Group(group) => &mut group.rows[r],
            _ => unreachable!("a row is in a group"),
        }
    }

    /// The range of the block at `path`.
    pub(super) fn range(&self, buffer: &Buffer, path: BlockPath) -> Range<usize> {
        self.block(path).range(buffer)
    }

    /// Every block with its place, a group's rows after it.
    pub(super) fn all(&self) -> impl Iterator<Item = (BlockPath, &Block)> {
        self.blocks.iter().enumerate().flat_map(|(at, block)| {
            let rows = match &block.kind {
                BlockKind::Group(group) => &group.rows[..],
                _ => &[],
            };
            let rows = (rows.iter().enumerate()).map(move |(r, row)| ((at, Some(r)), row));
            std::iter::once(((at, None), block)).chain(rows)
        })
    }

    /// The pending permission requests and questions, oldest first.
    pub(super) fn asks(&self) -> impl Iterator<Item = (BlockPath, &Ask)> {
        self.all().filter_map(|(path, block)| match &block.kind {
            BlockKind::Ask(ask) => Some((path, ask)),
            _ => None,
        })
    }

    pub(super) fn ask(&self, path: BlockPath) -> &Ask {
        match &self.block(path).kind {
            BlockKind::Ask(ask) => ask,
            _ => unreachable!("a pending request"),
        }
    }

    pub(super) fn ask_mut(&mut self, path: BlockPath) -> &mut Ask {
        match &mut self.block_mut(path).kind {
            BlockKind::Ask(ask) => ask,
            _ => unreachable!("a pending request"),
        }
    }

    /// The agent and user messages, in transcript order.
    pub(super) fn messages(&self) -> impl Iterator<Item = (&Block, &Message)> {
        self.blocks.iter().filter_map(|block| match &block.kind {
            BlockKind::Message(message) => Some((block, message)),
            _ => None,
        })
    }

    /// The last message starting at or before `point`.
    pub(super) fn message_at(&self, buffer: &Buffer, point: usize) -> Option<(&Block, &Message)> {
        let start = |b: &Block| buffer.marker(b.start).expect("live with its session");
        let after = self.blocks.partition_point(|b| start(b) <= point);
        self.blocks[..after]
            .iter()
            .rev()
            .find_map(|block| match &block.kind {
                BlockKind::Message(message) => Some((block, message)),
                _ => None,
            })
    }

    /// The last message's place.
    pub(super) fn last_message(&self) -> Option<BlockPath> {
        let at = (self.blocks.iter()).rposition(|b| matches!(b.kind, BlockKind::Message(_)))?;
        Some((at, None))
    }

    /// The place of the block of shown call `call`.
    pub(super) fn tool_path(&self, call: &ToolCallId) -> BlockPath {
        let is = |b: &Block| matches!(&b.kind, BlockKind::Tool(c) if c == call);
        match self.tools[call].group {
            Some(g) => match &self.blocks[g].kind {
                BlockKind::Group(group) => (g, group.rows.iter().position(is)),
                _ => unreachable!("a group"),
            },
            None => (self.blocks.iter().rposition(is).expect("shown"), None),
        }
    }

    /// The plan's place, once it has one.
    pub(super) fn plan_path(&self) -> Option<BlockPath> {
        let at = (self.blocks.iter()).position(|b| matches!(b.kind, BlockKind::Plan(_)))?;
        Some((at, None))
    }

    /// Where the transcript's next block goes in `blocks`: before the working line.
    pub(super) fn tail(&self) -> usize {
        self.blocks.len() - usize::from(self.turn.is_some())
    }

    /// Puts `text` above the prompt line as a new block of `kind` followed by a blank line,
    /// returning its place. A row joins the run; anything else ends it and starts it afresh.
    pub(super) fn push(
        &mut self,
        buffer: &mut Buffer,
        text: &str,
        kind: BlockKind,
        row: bool,
    ) -> BlockPath {
        let end = self.end(buffer);
        buffer.insert_output(end, &format!("{text}\n\n"));
        let mut marker = |at| (buffer.add_marker(at, InsertionType::Stay)).expect("in the text");
        let block = Block {
            start: marker(end),
            end: marker(end + text.chars().count()),
            meta: 0,
            kind,
        };
        if row {
            return self.add_row(buffer, block);
        }
        let at = self.tail();
        debug_assert_eq!(self.run, at, "the run ended first");
        self.blocks.insert(at, block);
        self.run = at + 1;
        (at, None)
    }

    /// Block `path` as drawn whole, from its own state.
    pub(super) fn render(&self, path: BlockPath, look: &Look) -> Rendered {
        let (text, props, meta) = match &self.block(path).kind {
            BlockKind::Line { text, dim } => {
                let len = text.chars().count();
                let props = if *dim {
                    vec![(0..len, render::dim())]
                } else {
                    vec![]
                };
                (text.clone(), props, None)
            }
            BlockKind::Message(_) => {
                let source = &self
                    .stream
                    .as_ref()
                    .expect("written while streaming")
                    .source;
                let is_file = |word: &str| file_link(look.index, word).is_some();
                let tables = Some(look.table_width);
                let (text, props, _) = markdown::render(source, false, false, tables, is_file);
                (text, props, None)
            }
            BlockKind::Tool(call) => {
                let tool = &self.tools[call];
                let content = tool_content(&tool.call, tool.term.as_ref());
                return self.render_tool(tool, &content.text);
            }
            BlockKind::Plan(plan) => (plan_text(plan), plan_props(plan), None),
            BlockKind::Ask(ask) => {
                let text = rows_text(&ask.body.rows(&ask.title));
                (text, ask.body.props(&ask.title), None)
            }
            BlockKind::Answered(a) => (a.text.clone(), answered_props(a, path.1.is_some()), None),
            BlockKind::Working => {
                let turn = self.turn.as_ref().expect("a running turn");
                let text = format!("Working for {}", long_duration(look.now - turn.start));
                let len = text.chars().count();
                (text, vec![(0..len, render::dim())], None)
            }
            BlockKind::Thought(_) | BlockKind::Group(_) => {
                unreachable!("thoughts and groups are drawn in place")
            }
        };
        Rendered {
            from: 0,
            text,
            props,
            meta,
        }
    }

    /// Tool call `tool`'s block whole, its `content` given: its header, then its content, and
    /// its time under it unless it is a group's row, whose header holds it. A call out of sight
    /// has no properties.
    pub(super) fn render_tool(&self, tool: &Tool, content: &str) -> Rendered {
        let header = self.tool_header(tool);
        let text = match content.is_empty() {
            true => header,
            false => format!("{header}\n{content}"),
        };
        let props = match self.hidden(tool) {
            true => vec![],
            false => tool_props(tool, &text),
        };
        let meta = match tool.group {
            Some(_) => vec![],
            None => vec![(tool.time.clone(), time_face(tool))],
        };
        Rendered {
            from: 0,
            text,
            props,
            meta: Some(meta),
        }
    }

    /// Tool call `tool`'s header line: in a group two columns in, with its time at its right.
    pub(super) fn tool_header(&self, tool: &Tool) -> String {
        let mut header = tool_header(&tool.call, tool.term.as_ref(), tool.open, &self.root);
        if tool.group.is_some() {
            header.insert_str(0, "  ");
            if !tool.time.is_empty() {
                header += &format!(" {}", tool.time);
            }
        }
        header
    }

    /// Writes `rendered` over block `path` and repairs its markers, then, in debug builds, checks
    /// that the buffer shows the block as drawn whole.
    pub(super) fn write(
        &mut self,
        buffer: &mut Buffer,
        path: BlockPath,
        rendered: Rendered,
        look: &Look,
    ) {
        self.put(buffer, path, rendered);
        #[cfg(debug_assertions)]
        self.check(buffer, path, look);
        #[cfg(not(debug_assertions))]
        let _ = look;
    }

    /// Writes `rendered` over block `path`: replaces its text from `from` on, removing the
    /// properties there and adding its own, sets its markers and rewrites the line under it.
    pub(super) fn put(&mut self, buffer: &mut Buffer, path: BlockPath, rendered: Rendered) {
        let Range { start, end } = self.range(buffer, path);
        let at = start + rendered.from;
        edit(buffer, at..end, &rendered.text);
        let end = at + rendered.text.chars().count();
        let block = self.block(path);
        // Text put at its start moves on an `Advance` start, and a `Stay` end stays.
        (buffer.set_marker(block.start, start)).expect("just written");
        (buffer.set_marker(block.end, end)).expect("just written");
        if let Some(meta) = rendered.meta {
            self.set_meta(buffer, path, &meta);
        }
        // Removing walks every property in the buffer, so properties already as drawn, as the
        // closing lines' past appended output, stay.
        if !shows(buffer, at..end, &rendered.props) {
            buffer.remove_properties(at..end);
            paint(buffer, at, rendered.props);
        }
    }

    /// Checks that block `path` shows as drawn whole: its text, and the properties inside it.
    #[cfg(debug_assertions)]
    pub(super) fn check(&self, buffer: &Buffer, path: BlockPath, look: &Look) {
        let whole = self.render(path, look);
        let range = self.range(buffer, path);
        assert_eq!(buffer.text().slice(range.clone()).to_string(), whole.text);
        let mut want: Vec<_> = (whole.props.into_iter())
            .filter(|(r, _)| !r.is_empty())
            .map(|(r, p)| format!("{r:?} {p:?}"))
            .collect();
        let mut have: Vec<_> = (buffer.properties_near(range.clone()).into_iter())
            .filter(|(r, _)| r.start >= range.start && r.end <= range.end)
            .map(|(r, p)| format!("{:?} {p:?}", r.start - range.start..r.end - range.start))
            .collect();
        want.sort();
        have.sort();
        assert_eq!(have, want, "a written block's properties");
    }

    /// Replaces the line under block `path` with `parts`, each in its face; none takes it off.
    pub(super) fn set_meta(
        &mut self,
        buffer: &mut Buffer,
        path: BlockPath,
        parts: &[(String, FaceName)],
    ) {
        let block = self.block_mut(path);
        let at = buffer.marker(block.end).expect("live with its session") + 1;
        let meta = std::mem::take(&mut block.meta);
        if meta > 0 {
            buffer.remove_properties(at..at + meta);
            buffer.replace_output(at..at + meta, "");
        }
        let parts: Vec<_> = parts.iter().map(|(t, f)| (t.as_str(), *f)).collect();
        block.meta = put_meta(buffer, at - 1, META_INDENT, &parts);
    }

    /// The block `point` is in, a group's row before the group.
    pub(super) fn path_at(&self, buffer: &Buffer, point: usize) -> Option<BlockPath> {
        let at = |blocks: &[Block]| {
            let start = |b: &Block| buffer.marker(b.start).expect("live with its session");
            let i = blocks
                .partition_point(|b| start(b) <= point)
                .checked_sub(1)?;
            (point <= buffer.marker(blocks[i].end).expect("live with its session")).then_some(i)
        };
        let i = at(&self.blocks)?;
        Some(match &self.blocks[i].kind {
            BlockKind::Group(group) => (i, at(&group.rows)),
            _ => (i, None),
        })
    }

    /// The block `point` is in that `TAB` acts on: a paste, a question with several fields, an
    /// answered line, a call, a thought row, then the group around them.
    pub(super) fn block_at(&self, buffer: &Buffer, point: usize) -> Option<At> {
        if let Some(i) = self.paste_at(buffer, point) {
            return Some(At::Paste(i));
        }
        let path = self.path_at(buffer, point)?;
        let target = self.target(buffer, path, point);
        match path {
            (g, None) if matches!(self.blocks[g].kind, BlockKind::Group(_)) => Some(At::Group(g)),
            (g, Some(_)) => target.or(Some(At::Group(g))),
            _ => target,
        }
    }

    /// What `TAB` acts on at `point` in block `path`, if it acts there.
    pub(super) fn target(&self, buffer: &Buffer, path: BlockPath, point: usize) -> Option<At> {
        let Range { start, end: all } = self.range(buffer, path);
        let open = path.1.is_none_or(|_| self.group(path.0).open);
        let on = |end: usize| (start..=end).contains(&point);
        match &self.block(path).kind {
            BlockKind::Ask(ask) if ask.question().is_some_and(|q| q.several()) && on(all) => {
                Some(At::Ask(path))
            }
            BlockKind::Answered(a) => {
                let end = if a.open { all } else { start + a.line };
                (all - start > a.line && open && on(end)).then_some(At::Answer(path))
            }
            BlockKind::Tool(call) => {
                let t = &self.tools[call];
                let end = if t.open { all } else { start + t.header };
                // A row with nothing to open leaves `TAB` to its group.
                let opens = t.group.is_none() || all - start > t.header;
                (opens && !self.hidden(t) && on(end)).then(|| At::Tool(call.clone()))
            }
            BlockKind::Thought(thought) if path.1.is_some() && open => {
                let end = if thought.open {
                    all
                } else {
                    start + thought.first
                };
                (all - start != thought.first && on(end)).then_some(At::Thought(path))
            }
            _ => None,
        }
    }

    /// The shown call whose header `point` is on.
    pub(super) fn tool_at(&self, buffer: &Buffer, point: usize) -> Option<&ToolCallId> {
        let path = self.path_at(buffer, point)?;
        let BlockKind::Tool(call) = &self.block(path).kind else {
            return None;
        };
        let start = self.range(buffer, path).start;
        (!self.hidden(&self.tools[call]) && point <= start + self.tools[call].header)
            .then_some(call)
    }
}
