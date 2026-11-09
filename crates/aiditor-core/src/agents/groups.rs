//! Tool groups: a run of consecutive tool calls folded into one line. The rows sit one newline
//! apart in the rope, and a folded group's hidden rows carry no properties, only the `Invisible`
//! runs over them, so properties never nest.

use super::render::{
    META_INDENT, dim, group_line, group_meta, put_meta, render, render_tool, take_tool_meta,
};
use super::*;

/// A block `TAB` acts on, by its index.
pub(super) enum At {
    Ask(usize),
    Paste(usize),
    Answer(usize),
    Tool(ToolCallId),
    /// A thought row, as its group and row.
    Thought(usize, usize),
    Group(usize),
}

/// A row of a run: a tool call, or a thought or permission block among its calls.
pub(super) enum Row {
    Tool(ToolCallId),
    Thought(Thought),
    /// `start` is the row's own, as `Ask::start` goes with the answer and the line stays.
    Ask {
        start: MarkerId,
        len: usize,
    },
}

/// A thought's block in a run.
pub(super) struct Thought {
    pub(super) start: MarkerId,
    /// Its first line's length, the row a group shows; 0 while it streams.
    first: usize,
    pub(super) end: MarkerId,
    open: bool,
}

impl Thought {
    pub(super) fn new(start: MarkerId, end: MarkerId) -> Self {
        Self {
            start,
            first: 0,
            end,
            open: false,
        }
    }

    fn range(&self, buffer: &Buffer) -> Range<usize> {
        extent(buffer, self.start, self.end)
    }

    /// Makes it a row: the indent and fold mark go before its first line.
    fn indent(&mut self, buffer: &mut Buffer) {
        let start = buffer.marker(self.start).expect("live with its run");
        buffer.insert_output(start, THOUGHT_ROW);
        self.first += THOUGHT_ROW.chars().count();
    }
}

const THOUGHT_ROW: &str = "  ▸ ";
/// Where a thought row's fold mark is.
const THOUGHT_FOLD: usize = 2;

/// A group: its line, then its rows.
pub(super) struct Group {
    start: MarkerId,
    /// Its line's length in chars, the newline not counted.
    line: usize,
    open: bool,
    rows: Vec<Row>,
    /// The length of its failed count and time on the line under its last row, 0 for none.
    meta: usize,
}

fn render_thought(buffer: &mut Buffer, thought: &Thought) {
    let Range { start, end } = thought.range(buffer);
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
    /// Each block's text, in transcript order, as its markers bound it.
    #[cfg(test)]
    pub(super) fn block_texts(&self, buffer: &Buffer) -> Vec<String> {
        let rows = (self.run.iter()).chain(self.groups.iter().flat_map(|g| &g.rows));
        let thoughts = rows.filter_map(|r| match r {
            Row::Thought(t) => Some(t.range(buffer)),
            _ => None,
        });
        let working = (self.turn.as_ref()).map(|t| {
            buffer.marker(t.line).unwrap()..buffer.prompt_start() - PROMPT.len() - "\n\n".len()
        });
        let mut ranges: Vec<_> = (self.messages.iter().map(|m| m.range(buffer)))
            .chain(self.tools.values().map(|t| t.range(buffer)))
            .chain(self.asks.iter().map(|a| a.range(buffer)))
            .chain((self.answers.iter()).map(|a| extent(buffer, a.start, a.end)))
            .chain(thoughts)
            .chain(self.plan.map(|(start, end)| extent(buffer, start, end)))
            .chain(working)
            .collect();
        ranges.sort_by_key(|r| r.start);
        (ranges.into_iter())
            .map(|r| buffer.text().slice(r).to_string())
            .collect()
    }

    fn row_span(&self, buffer: &Buffer, row: &Row) -> Range<usize> {
        let (start, len) = match row {
            Row::Tool(call) => return self.tools[call].range(buffer),
            Row::Thought(thought) => return thought.range(buffer),
            Row::Ask { start, len } => (*start, *len),
        };
        let start = buffer.marker(start).expect("live with its run");
        start..start + len
    }

    /// Where group `g`'s line starts and its last row ends.
    fn group_span(&self, buffer: &Buffer, g: usize) -> Range<usize> {
        let group = &self.groups[g];
        let last = group.rows.last().expect("a group has rows");
        let start = buffer.marker(group.start).expect("live with its session");
        start..self.row_span(buffer, last).end
    }

    /// Whether `tool` is out of sight: in a folded group, or replaced by its question's line.
    pub(super) fn hidden(&self, tool: &Tool) -> bool {
        let waiting = (self.asks.iter()).any(|a| a.holds(&tool.call.tool_call_id));
        tool.replaced || !waiting && tool.group.is_some_and(|g| !self.groups[g].open)
    }

    /// The group with a row starting at `at`, an answered line's.
    fn group_at(&self, buffer: &Buffer, at: usize) -> Option<usize> {
        let starts =
            |r: &Row| matches!(r, Row::Ask { start, .. } if buffer.marker(*start) == Some(at));
        self.groups.iter().position(|g| g.rows.iter().any(starts))
    }

    /// Replaces `ask`'s block with `line`, returning its length: a question's answered line, `clock`
    /// at its right, or a permission request's dim line.
    pub(super) fn settle(
        &mut self,
        buffer: &mut Buffer,
        ask: &Ask,
        line: &str,
        clock: Option<&str>,
    ) -> usize {
        if ask.question().is_none() {
            settle(buffer, ask, line);
            return line.chars().count();
        }
        let answered = settle_question(buffer, ask, line, clock);
        let start = buffer.marker(answered.start).expect("just settled");
        render_answered(buffer, &answered, self.group_at(buffer, start).is_some());
        let len = extent(buffer, answered.start, answered.end).len();
        self.answers.push(answered);
        len
    }

    /// The answered line `point` is on or in, if it sent anything and its group shows it.
    fn answer_at(&self, buffer: &Buffer, point: usize) -> Option<usize> {
        let (i, start) = self.answers.iter().enumerate().find_map(|(i, a)| {
            let Range { start, end: all } = extent(buffer, a.start, a.end);
            let end = if a.open { all } else { start + a.line };
            (all - start > a.line && (start..=end).contains(&point)).then_some((i, start))
        })?;
        let group = self.group_at(buffer, start);
        (!group.is_some_and(|g| !self.groups[g].open)).then_some(i)
    }

    /// Opens or folds answered line `i`, returning where it starts and where its line ends.
    pub(super) fn toggle_answer(&mut self, buffer: &mut Buffer, i: usize) -> (usize, usize) {
        let start = buffer
            .marker(self.answers[i].start)
            .expect("live with its session");
        let group = self.group_at(buffer, start);
        let a = &mut self.answers[i];
        a.open = !a.open;
        buffer.remove_properties(extent(buffer, a.start, a.end));
        buffer.replace_output(start..start + 1, if a.open { "▾" } else { "▸" });
        render_answered(buffer, a, group.is_some());
        (start, start + a.line)
    }

    /// Whether `row` shows under its folded group.
    fn waiting(&self, row: &Row) -> bool {
        match row {
            Row::Tool(call) => self.asks.iter().any(|a| a.holds(call)),
            Row::Thought(_) => false,
            Row::Ask { start, .. } => self.asks.iter().any(|a| a.row == *start),
        }
    }

    fn render_row(&self, buffer: &mut Buffer, row: &Row) {
        match row {
            Row::Tool(call) => render_tool(buffer, &self.tools[call]),
            Row::Thought(thought) => render_thought(buffer, thought),
            Row::Ask { start, len } => {
                let at = buffer.marker(*start).expect("live with its run");
                let answered = (self.answers.iter()).find(|a| buffer.marker(a.start) == Some(at));
                match (self.asks.iter().find(|a| a.row == *start), answered) {
                    (Some(ask), _) => ask.body.render(buffer, at, &ask.title),
                    (None, Some(a)) => render_answered(buffer, a, true),
                    (None, None) => {
                        (buffer.add_property(at..at + len, dim())).expect("inside the block");
                    }
                }
            }
        }
    }

    /// Lays folded group `g`'s properties below its line: its waiting rows, and over the rest
    /// `Invisible` runs, each from the end of what shows above it.
    fn fold_rows(&self, buffer: &mut Buffer, g: usize) {
        let span = self.group_span(buffer, g);
        let group = &self.groups[g];
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
            let row_span = self.row_span(buffer, row);
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
        let group = &self.groups[g];
        let calls: Vec<_> = (group.rows.iter())
            .filter_map(|row| match row {
                Row::Tool(call) => Some(&self.tools[call]),
                _ => None,
            })
            .collect();
        let start = buffer.marker(group.start).expect("live with its session");
        let meta = group_meta(&calls);
        let line = group_line(buffer, start..start + group.line, &calls, group.open);
        self.groups[g].line = line;
        self.take_meta(buffer, g);
        // A thought streaming as its last row grows into the line under it.
        if !(self.group == Some(g) && self.stream.is_some()) {
            let end = self.group_span(buffer, g).end;
            let parts: Vec<_> = meta.iter().map(|(t, f)| (t.as_str(), *f)).collect();
            self.groups[g].meta = put_meta(buffer, end, META_INDENT, &parts);
        }
        if !self.groups[g].open {
            self.fold_rows(buffer, g);
        }
    }

    /// Takes group `g`'s failed count and time off the line under it.
    fn take_meta(&mut self, buffer: &mut Buffer, g: usize) {
        let at = self.group_span(buffer, g).end + 1;
        let meta = std::mem::take(&mut self.groups[g].meta);
        if meta > 0 {
            buffer.remove_properties(at..at + meta);
            buffer.replace_output(at..at + meta, "");
        }
    }

    /// Renders all of group `g`, after it opened or folded.
    pub(super) fn render_group(&mut self, buffer: &mut Buffer, g: usize) {
        self.show_group(buffer, g);
        if self.groups[g].open {
            let span = self.group_span(buffer, g);
            buffer.remove_properties(span.start + self.groups[g].line..span.end);
            for row in &self.groups[g].rows {
                self.render_row(buffer, row);
            }
        }
    }

    /// Adds `row`, whose block was just put above the prompt line, to the run.
    pub(super) fn add_row(&mut self, buffer: &mut Buffer, row: Row) {
        let Some(g) = self.group else {
            return self.run.push(row);
        };
        self.take_meta(buffer, g);
        let start = self.row_span(buffer, &row).start;
        buffer.replace_output(start - 1..start, "");
        self.groups[g].rows.push(row);
    }

    /// Ends the run, freeing the markers of rows no group kept.
    pub(super) fn end_run(&mut self, buffer: &mut Buffer) {
        self.group = None;
        for row in std::mem::take(&mut self.run) {
            match row {
                Row::Tool(_) => {}
                Row::Thought(Thought { start, end, .. }) => {
                    buffer.remove_marker(start);
                    buffer.remove_marker(end);
                }
                Row::Ask { start, .. } => buffer.remove_marker(start),
            }
        }
    }

    /// Whether the run has just got its second call.
    pub(super) fn forms_group(&self) -> bool {
        self.group.is_none()
            && (self.run.iter())
                .filter(|r| matches!(r, Row::Tool(_)))
                .count()
                == 2
    }

    /// Turns the run into a group: joins its rows, puts an empty line above them and marks them
    /// as rows. Its calls still need showing indented.
    pub(super) fn form_group(&mut self, buffer: &mut Buffer) {
        let mut rows = std::mem::take(&mut self.run);
        // Its calls' times go back into their headers when they are shown as rows.
        for row in &rows {
            if let Row::Tool(call) = row {
                take_tool_meta(buffer, self.tools.get_mut(call).expect("shown"));
            }
        }
        for row in &rows[1..] {
            let start = self.row_span(buffer, row).start;
            buffer.replace_output(start - 1..start, "");
        }
        let at = self.row_span(buffer, &rows[0]).start;
        let start = (buffer.add_marker(at, InsertionType::Stay)).expect("in the text");
        // The markers at `at` stay before the line, as blocks' do when text goes in at them.
        buffer.insert_output(at, "\n");
        let first = match &rows[0] {
            Row::Tool(call) => self.tools[call].start,
            Row::Thought(thought) => thought.start,
            Row::Ask { start, .. } => *start,
        };
        let asks = self.asks.iter().map(|a| a.start);
        let answers = self.answers.iter().map(|a| a.start);
        for marker in asks.chain(answers).chain([first]) {
            if buffer.marker(marker) == Some(at) {
                buffer.set_marker(marker, at + 1).expect("in the text");
            }
        }
        let g = self.groups.len();
        for row in &mut rows {
            match row {
                Row::Tool(call) => {
                    self.tools.get_mut(call).expect("shown").group = Some(g);
                }
                Row::Thought(thought) => thought.indent(buffer),
                Row::Ask { .. } => {}
            }
        }
        let open = (rows.iter()).any(|r| matches!(r, Row::Tool(call) if self.tools[call].open));
        self.groups.push(Group {
            start,
            line: 0,
            open,
            rows,
            meta: 0,
        });
        self.group = Some(g);
    }

    /// The ids of group `g`'s calls.
    pub(super) fn group_calls(&self, g: usize) -> Vec<ToolCallId> {
        (self.groups[g].rows.iter())
            .filter_map(|row| match row {
                Row::Tool(call) => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    /// The thought being streamed, the run's last.
    fn last_thought(&mut self) -> Option<&mut Thought> {
        let rows = match self.group {
            Some(g) => &mut self.groups[g].rows,
            None => &mut self.run,
        };
        rows.iter_mut().rev().find_map(|row| match row {
            Row::Thought(thought) => Some(thought),
            _ => None,
        })
    }

    /// Counts `chars` more of the thought being streamed, added at its end.
    pub(super) fn thought_grew(&mut self, buffer: &mut Buffer, chars: usize) {
        if let Some(thought) = self.last_thought() {
            let end = thought.range(buffer).end + chars;
            buffer.set_marker(thought.end, end).expect("just inserted");
        }
    }

    /// The thought being streamed ended, with a header of `header` chars if it got one; in a
    /// group it becomes a row.
    pub(super) fn end_thought(&mut self, buffer: &mut Buffer, header: Option<usize>) {
        let group = self.group;
        let open = group.is_some_and(|g| self.groups[g].open);
        let Some(thought) = self.last_thought() else {
            return;
        };
        let range = thought.range(buffer);
        thought.first = header.unwrap_or_else(|| {
            let line = buffer.text().slice(range);
            line.chars().take_while(|&c| c != '\n').count()
        });
        let Some(g) = group else {
            return;
        };
        thought.indent(buffer);
        if open {
            buffer.remove_properties(thought.range(buffer));
            render_thought(buffer, thought);
        }
        self.show_group(buffer, g);
    }

    /// Whether the thought being streamed is hidden in a folded group; it is rendered with it.
    pub(super) fn fold_stream(&mut self, buffer: &mut Buffer) -> bool {
        match self.group.filter(|&g| !self.groups[g].open) {
            Some(g) => {
                self.fold_rows(buffer, g);
                true
            }
            None => false,
        }
    }

    /// The permission block of `row` became an answer of `len` chars.
    pub(super) fn settle_row(&mut self, buffer: &mut Buffer, row: MarkerId, len: usize) {
        let answer = |rows: &mut [Row]| {
            let new = rows.iter_mut().find_map(|r| match r {
                Row::Ask { start, len } if *start == row => Some(len),
                _ => None,
            });
            new.map(|old| *old = len).is_some()
        };
        if answer(&mut self.run) {
            return;
        }
        let g = (self.groups.iter_mut()).rposition(|group| answer(&mut group.rows));
        if let Some(g) = g {
            self.show_group(buffer, g);
        }
    }

    /// Whether group `g` shows its rows.
    pub(super) fn group_open(&self, g: usize) -> bool {
        self.groups[g].open
    }

    /// The thought row `point` is on or in, in an open group, as its group and row.
    fn thought_at(&self, buffer: &Buffer, point: usize) -> Option<(usize, usize)> {
        let groups = self.groups.iter().enumerate().filter(|(_, g)| g.open);
        groups
            .flat_map(|(g, group)| (group.rows.iter().enumerate()).map(move |(r, row)| (g, r, row)))
            .find_map(|(g, r, row)| {
                let Row::Thought(thought) = row else {
                    return None;
                };
                let range = thought.range(buffer);
                let start = range.start;
                let end = if thought.open {
                    range.end
                } else {
                    start + thought.first
                };
                (range.len() != thought.first && (start..=end).contains(&point)).then_some((g, r))
            })
    }

    /// Opens or folds row `r` of group `g`, a thought, returning where it starts and where its
    /// first line ends.
    pub(super) fn toggle_thought(
        &mut self,
        buffer: &mut Buffer,
        g: usize,
        r: usize,
    ) -> (usize, usize) {
        let Row::Thought(thought) = &mut self.groups[g].rows[r] else {
            unreachable!("found as a thought");
        };
        let range = thought.range(buffer);
        let start = range.start;
        thought.open = !thought.open;
        buffer.remove_properties(range);
        let mark = start + THOUGHT_FOLD;
        let fold = if thought.open { "▾" } else { "▸" };
        buffer.replace_output(mark..mark + 1, fold);
        render_thought(buffer, thought);
        (start, start + thought.first)
    }

    /// Opens or folds group `g`, returning where its line starts and ends and whether it is
    /// open now.
    pub(super) fn toggle_group(&mut self, buffer: &mut Buffer, g: usize) -> (usize, usize, bool) {
        self.groups[g].open = !self.groups[g].open;
        self.render_group(buffer, g);
        let span = self.group_span(buffer, g);
        let group = &self.groups[g];
        (span.start, span.start + group.line, group.open)
    }

    /// The block `point` is in that `TAB` acts on, in its precedence: a question with several
    /// fields, a paste, an answered line, a call, a thought row, then a group.
    pub(super) fn block_at(&self, buffer: &Buffer, point: usize) -> Option<At> {
        let ask = self.asks.iter().position(|ask| {
            ask.question().is_some_and(|q| q.several())
                && (ask.range(buffer).start..=ask.range(buffer).end).contains(&point)
        });
        let tool = || {
            (self.tools.iter()).find(|(_, t)| {
                let Range { start, end: all } = t.range(buffer);
                let end = if t.open { all } else { start + t.header };
                // A row with nothing to open leaves `TAB` to its group.
                (t.group.is_none() || all - start > t.header)
                    && !self.hidden(t)
                    && (start..=end).contains(&point)
            })
        };
        let group = || {
            (0..self.groups.len()).find(|&g| {
                let span = self.group_span(buffer, g);
                (span.start..=span.end).contains(&point)
            })
        };
        (ask.map(At::Ask))
            .or_else(|| self.paste_at(buffer, point).map(At::Paste))
            .or_else(|| self.answer_at(buffer, point).map(At::Answer))
            .or_else(|| tool().map(|(call, _)| At::Tool(call.clone())))
            .or_else(|| {
                self.thought_at(buffer, point)
                    .map(|(g, r)| At::Thought(g, r))
            })
            .or_else(|| group().map(At::Group))
    }
}
