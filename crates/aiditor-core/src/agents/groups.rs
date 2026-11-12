//! Tool groups: a run of consecutive tool calls folded into one line. The rows sit one newline
//! apart in the rope, and a folded group's hidden rows carry no properties, only the `Invisible`
//! runs over them, so properties never nest.

use super::blocks::{Block, BlockKind, BlockPath, Look};
use super::render::{answered_props, dim, group_line, group_meta, hide_replaced, paint, render};
use super::render::{settled_question, tool_props};
use super::*;

/// A block `TAB` acts on.
#[derive(Debug, PartialEq)]
pub(super) enum At {
    Ask(BlockPath),
    Paste(usize),
    Answer(BlockPath),
    Tool(ToolCallId),
    /// A thought row of a group.
    Thought(BlockPath),
    Group(usize),
}

/// A thought's block, a row of a run.
#[derive(Default)]
pub(super) struct Thought {
    /// Its first line's length, the row a group shows; 0 while it streams.
    pub(super) first: usize,
    pub(super) open: bool,
}

const THOUGHT_ROW: &str = "  ▸ ";
/// Where a thought row's fold mark is.
const THOUGHT_FOLD: usize = 2;

/// A group's line, its rows under it.
pub(super) struct Group {
    /// Its line's length in chars, the newline not counted.
    pub(super) line: usize,
    pub(super) open: bool,
    pub(super) rows: Vec<Block>,
}

fn thought_mut(block: &mut Block) -> &mut Thought {
    match &mut block.kind {
        BlockKind::Thought(thought) => thought,
        _ => unreachable!("a thought"),
    }
}

fn render_thought(buffer: &mut Buffer, block: &Block, thought: &Thought) {
    let Range { start, end } = block.range(buffer);
    if thought.first == 0 {
        return render(buffer, start..end, true, None);
    }
    let first = start + thought.first;
    (buffer.add_property(start..first, dim())).expect("inside the block");
    if thought.open {
        render(buffer, first + 1..end, true, None);
    } else if first < end {
        (buffer.add_property(first..end, Prop::Invisible(String::new())))
            .expect("inside the block");
    }
}

impl Session {
    /// Each block's text, in transcript order, as its markers bound it; a group's line and the
    /// session's own lines left out.
    #[cfg(test)]
    pub(super) fn block_texts(&self, buffer: &Buffer) -> Vec<String> {
        let blocks = self
            .all()
            .filter(|(_, b)| !matches!(b.kind, BlockKind::Line { .. } | BlockKind::Group(_)));
        let mut ranges: Vec<_> = blocks.map(|(_, b)| b.range(buffer)).collect();
        ranges.sort_by_key(|r| r.start);
        (ranges.into_iter())
            .map(|r| buffer.text().slice(r).to_string())
            .collect()
    }

    pub(super) fn group(&self, g: usize) -> &Group {
        match &self.blocks[g].kind {
            BlockKind::Group(group) => group,
            _ => unreachable!("a group"),
        }
    }

    fn group_mut(&mut self, g: usize) -> &mut Group {
        match &mut self.blocks[g].kind {
            BlockKind::Group(group) => group,
            _ => unreachable!("a group"),
        }
    }

    /// Whether `tool` is out of sight: in a folded group, or replaced by its question's line.
    pub(super) fn hidden(&self, tool: &Tool) -> bool {
        let waiting = (self.asks()).any(|(_, a)| a.holds(&tool.call.tool_call_id));
        tool.replaced || !waiting && tool.group.is_some_and(|g| !self.group(g).open)
    }

    /// Replaces pending request `path`'s block with `line`, returning the request: a question's
    /// answered line, `clock` at its right, or a permission request's dim line.
    pub(super) fn settle(
        &mut self,
        buffer: &mut Buffer,
        path: BlockPath,
        line: &str,
        clock: Option<&str>,
        look: &Look,
    ) -> Ask {
        let kind = match self.ask(path).question() {
            Some(question) => BlockKind::Answered(settled_question(question, line, clock)),
            None => BlockKind::Line {
                text: line.into(),
                dim: true,
            },
        };
        let BlockKind::Ask(ask) = std::mem::replace(&mut self.block_mut(path).kind, kind) else {
            unreachable!("a pending request");
        };
        let rendered = self.render(path, look);
        self.write(buffer, path, rendered, look);
        ask
    }

    /// Opens or folds answered line `path`, returning where it starts and where its line ends.
    pub(super) fn toggle_answer(
        &mut self,
        buffer: &mut Buffer,
        path: BlockPath,
        look: &Look,
    ) -> (usize, usize) {
        let BlockKind::Answered(a) = &mut self.block_mut(path).kind else {
            unreachable!("found as an answer");
        };
        a.open = !a.open;
        let mark = if a.open { "▾" } else { "▸" };
        a.text.replace_range(..'▸'.len_utf8(), mark);
        let line = a.line;
        let rendered = self.render(path, look);
        self.write(buffer, path, rendered, look);
        let start = self.range(buffer, path).start;
        (start, start + line)
    }

    /// Whether `row` shows under its folded group.
    fn waiting(&self, row: &Block) -> bool {
        match &row.kind {
            BlockKind::Tool(call) => self.asks().any(|(_, a)| a.holds(call)),
            BlockKind::Ask(_) => true,
            _ => false,
        }
    }

    fn render_row(&self, buffer: &mut Buffer, row: &Block) {
        let range = row.range(buffer);
        match &row.kind {
            BlockKind::Tool(call) => {
                let tool = &self.tools[call];
                if tool.replaced {
                    return hide_replaced(buffer, tool, range, row.meta);
                }
                let text = buffer.text().slice(range.clone()).to_string();
                paint(buffer, range.start, tool_props(tool, &text));
            }
            BlockKind::Thought(thought) => render_thought(buffer, row, thought),
            BlockKind::Ask(ask) => paint(buffer, range.start, ask.body.props(&ask.title)),
            BlockKind::Answered(a) => paint(buffer, range.start, answered_props(a, true)),
            BlockKind::Line { dim: true, .. } => {
                (buffer.add_property(range, dim())).expect("inside the block");
            }
            _ => {}
        }
    }

    /// Lays folded group `g`'s properties below its line: its waiting rows, and over the rest
    /// `Invisible` runs, each from the end of what shows above it.
    fn fold_rows(&self, buffer: &mut Buffer, g: usize) {
        let span = self.range(buffer, (g, None));
        let group = self.group(g);
        let fold = |buffer: &mut Buffer, run: Range<usize>| {
            if !run.is_empty() {
                let hide = Prop::Invisible(String::new());
                (buffer.add_property(run, hide)).expect("inside the group");
            }
        };
        let shown = span.start + group.line;
        buffer.remove_properties(shown..span.end);
        let (mut from, mut to) = (shown, shown);
        for row in &group.rows {
            let row_span = row.range(buffer);
            if self.waiting(row) {
                fold(buffer, from..to);
                self.render_row(buffer, row);
                from = row_span.end;
            }
            to = row_span.end;
        }
        fold(buffer, from..to);
    }

    /// Brings group `g`'s line up to date, and its fold when folded. An open group's rows
    /// render on their own.
    pub(super) fn show_group(&mut self, buffer: &mut Buffer, g: usize) {
        let group = self.group(g);
        let calls: Vec<_> = (group.rows.iter())
            .filter_map(|row| match &row.kind {
                BlockKind::Tool(call) => Some(&self.tools[call]),
                _ => None,
            })
            .collect();
        let start = buffer
            .marker(self.blocks[g].start)
            .expect("live with its session");
        let meta = group_meta(&calls);
        let line = group_line(buffer, start..start + group.line, &calls, group.open);
        self.group_mut(g).line = line;
        // A thought streaming as its last row grows into the line under it.
        let streaming = self.group == Some(g) && self.stream.is_some();
        let meta = if streaming { vec![] } else { meta };
        self.set_meta(buffer, (g, None), &meta);
        if !self.group(g).open {
            self.fold_rows(buffer, g);
        }
    }

    /// Renders all of group `g`, after it opened or folded.
    pub(super) fn render_group(&mut self, buffer: &mut Buffer, g: usize) {
        self.show_group(buffer, g);
        let group = self.group(g);
        if group.open {
            let span = self.range(buffer, (g, None));
            buffer.remove_properties(span.start + group.line..span.end);
            for row in &group.rows {
                self.render_row(buffer, row);
            }
        }
    }

    /// Adds `row`, whose block was just put above the prompt line, to the run, returning its
    /// place.
    pub(super) fn add_row(&mut self, buffer: &mut Buffer, row: Block) -> BlockPath {
        let Some(g) = self.group else {
            let at = self.tail();
            self.blocks.insert(at, row);
            return (at, None);
        };
        self.set_meta(buffer, (g, None), &[]);
        let start = buffer.marker(row.start).expect("just put");
        buffer.replace_output(start - 1..start, "");
        self.blocks[g].end = row.end;
        let rows = &mut self.group_mut(g).rows;
        rows.push(row);
        (g, Some(rows.len() - 1))
    }

    /// Ends the run, dropping the rows no group kept that nothing acts on: its thoughts and
    /// settled permission requests' lines.
    pub(super) fn end_run(&mut self, buffer: &mut Buffer) {
        self.group = None;
        let mut at = self.run;
        while at < self.tail() {
            match self.blocks[at].kind {
                BlockKind::Thought(_) | BlockKind::Line { dim: true, .. } => {
                    let block = self.blocks.remove(at);
                    buffer.remove_marker(block.start);
                    buffer.remove_marker(block.end);
                }
                _ => at += 1,
            }
        }
        self.run = self.tail();
    }

    /// Whether the run has just got its second call.
    pub(super) fn forms_group(&self) -> bool {
        self.group.is_none()
            && (self.blocks[self.run..self.tail()].iter())
                .filter(|b| matches!(b.kind, BlockKind::Tool(_)))
                .count()
                == 2
    }

    /// Turns the run into a group: joins its rows, puts an empty line above them and marks them
    /// as rows. Its calls still need showing indented.
    pub(super) fn form_group(&mut self, buffer: &mut Buffer) {
        let g = self.run;
        // Its calls' times go back into their headers when they are shown as rows.
        for at in g..self.tail() {
            if let BlockKind::Tool(_) = self.blocks[at].kind {
                self.set_meta(buffer, (at, None), &[]);
            }
        }
        let tail = self.tail();
        let mut rows: Vec<Block> = self.blocks.drain(g..tail).collect();
        for row in &rows[1..] {
            let start = buffer.marker(row.start).expect("live with its run");
            buffer.replace_output(start - 1..start, "");
        }
        let at = buffer.marker(rows[0].start).expect("live with its run");
        let start = (buffer.add_marker(at, InsertionType::Stay)).expect("in the text");
        // The first row's start stays before the line, as blocks' do when text goes in at them.
        buffer.insert_output(at, "\n");
        buffer
            .set_marker(rows[0].start, at + 1)
            .expect("in the text");
        for row in &mut rows {
            if let BlockKind::Tool(call) = &row.kind {
                self.tools.get_mut(call).expect("shown").group = Some(g);
            }
            if let BlockKind::Thought(_) = row.kind {
                buffer.insert_output(at_start(buffer, row), THOUGHT_ROW);
                thought_mut(row).first += THOUGHT_ROW.chars().count();
            }
        }
        let open = (rows.iter())
            .any(|r| matches!(&r.kind, BlockKind::Tool(call) if self.tools[call].open));
        let end = rows.last().expect("a group has rows").end;
        let group = Group {
            line: 0,
            open,
            rows,
        };
        self.blocks.insert(
            g,
            Block {
                start,
                end,
                meta: 0,
                kind: BlockKind::Group(group),
            },
        );
        self.group = Some(g);
    }

    /// The ids of group `g`'s calls.
    pub(super) fn group_calls(&self, g: usize) -> Vec<ToolCallId> {
        (self.group(g).rows.iter())
            .filter_map(|row| match &row.kind {
                BlockKind::Tool(call) => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    /// The place of the thought being streamed, the run's last.
    fn last_thought(&self) -> Option<BlockPath> {
        let is = |b: &Block| matches!(b.kind, BlockKind::Thought(_));
        match self.group {
            Some(g) => Some((g, Some(self.group(g).rows.iter().rposition(is)?))),
            None => {
                let run = &self.blocks[self.run..self.tail()];
                Some((self.run + run.iter().rposition(is)?, None))
            }
        }
    }

    /// Counts `chars` more of the thought being streamed, added at its end.
    pub(super) fn thought_grew(&mut self, buffer: &mut Buffer, chars: usize) {
        if let Some(path) = self.last_thought() {
            let block = self.block(path);
            let end = block.range(buffer).end + chars;
            buffer.set_marker(block.end, end).expect("just inserted");
        }
    }

    /// The thought being streamed ended, with a header of `header` chars if it got one; in a
    /// group it becomes a row.
    pub(super) fn end_thought(&mut self, buffer: &mut Buffer, header: Option<usize>) {
        let group = self.group;
        let open = group.is_some_and(|g| self.group(g).open);
        let Some(path) = self.last_thought() else {
            return;
        };
        let block = self.block_mut(path);
        let range = block.range(buffer);
        thought_mut(block).first = header.unwrap_or_else(|| {
            let line = buffer.text().slice(range);
            line.chars().take_while(|&c| c != '\n').count()
        });
        let Some(g) = group else {
            return;
        };
        buffer.insert_output(at_start(buffer, block), THOUGHT_ROW);
        thought_mut(block).first += THOUGHT_ROW.chars().count();
        if open {
            buffer.remove_properties(block.range(buffer));
            let block = self.block(path);
            let BlockKind::Thought(thought) = &block.kind else {
                unreachable!("a thought");
            };
            render_thought(buffer, block, thought);
        }
        self.show_group(buffer, g);
    }

    /// Whether the thought being streamed is hidden in a folded group; it is rendered with it.
    pub(super) fn fold_stream(&mut self, buffer: &mut Buffer) -> bool {
        match self.group.filter(|&g| !self.group(g).open) {
            Some(g) => {
                self.fold_rows(buffer, g);
                true
            }
            None => false,
        }
    }

    /// Whether group `g` shows its rows.
    pub(super) fn group_open(&self, g: usize) -> bool {
        self.group(g).open
    }

    /// Opens or folds thought row `path`, returning where it starts and where its first line
    /// ends.
    pub(super) fn toggle_thought(
        &mut self,
        buffer: &mut Buffer,
        path: BlockPath,
    ) -> (usize, usize) {
        let block = self.block_mut(path);
        let range = block.range(buffer);
        let start = range.start;
        let thought = thought_mut(block);
        thought.open = !thought.open;
        let fold = if thought.open { "▾" } else { "▸" };
        buffer.remove_properties(range);
        let mark = start + THOUGHT_FOLD;
        buffer.replace_output(mark..mark + 1, fold);
        let block = self.block(path);
        let BlockKind::Thought(thought) = &block.kind else {
            unreachable!("a thought");
        };
        render_thought(buffer, block, thought);
        (start, start + thought.first)
    }

    /// Opens or folds group `g`, returning where its line starts and ends and whether it is
    /// open now.
    pub(super) fn toggle_group(&mut self, buffer: &mut Buffer, g: usize) -> (usize, usize, bool) {
        let group = self.group_mut(g);
        group.open = !group.open;
        self.render_group(buffer, g);
        let start = self.range(buffer, (g, None)).start;
        let group = self.group(g);
        (start, start + group.line, group.open)
    }
}

/// Where `block` starts.
fn at_start(buffer: &Buffer, block: &Block) -> usize {
    buffer.marker(block.start).expect("live with its session")
}
