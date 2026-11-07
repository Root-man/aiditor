//! Tool groups: a run of consecutive tool calls folded into one line. The rows sit one newline
//! apart in the rope, and a folded group's hidden rows carry no properties, only the `Invisible`
//! runs over them, so properties never nest.

use super::render::{dim, group_line, render, render_rows, render_tool};
use super::*;

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
    pub(super) len: usize,
    open: bool,
}

impl Thought {
    pub(super) fn new(start: MarkerId, len: usize) -> Self {
        Self {
            start,
            first: 0,
            len,
            open: false,
        }
    }

    /// Makes it a row: the indent and fold mark go before its first line.
    fn indent(&mut self, buffer: &mut Buffer) {
        let start = buffer.marker(self.start).expect("live with its run");
        buffer.insert_output(start, THOUGHT_ROW);
        let added = THOUGHT_ROW.chars().count();
        (self.first, self.len) = (self.first + added, self.len + added);
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
}

fn render_thought(buffer: &mut Buffer, thought: &Thought) {
    let start = buffer.marker(thought.start).expect("live with its run");
    let end = start + thought.len;
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
    fn row_span(&self, buffer: &Buffer, row: &Row) -> Range<usize> {
        let (start, len) = match row {
            Row::Tool(call) => (self.tools[call].start, self.tools[call].len),
            Row::Thought(thought) => (thought.start, thought.len),
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

    /// Whether `tool` is out of sight in a folded group.
    pub(super) fn hidden(&self, tool: &Tool) -> bool {
        let waiting = (self.asks.iter()).any(|a| a.holds(&tool.call.tool_call_id));
        !waiting && tool.group.is_some_and(|g| !self.groups[g].open)
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
                match self.asks.iter().find(|a| a.row == *start) {
                    Some(ask) => render_rows(buffer, at, &ask.rows()),
                    None => {
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
        let line = group_line(buffer, start..start + group.line, &calls, group.open);
        self.groups[g].line = line;
        if !self.groups[g].open {
            self.fold_rows(buffer, g);
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
                Row::Thought(Thought { start, .. }) | Row::Ask { start, .. } => {
                    buffer.remove_marker(start)
                }
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
        for marker in asks.chain([first]) {
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

    /// Counts `chars` more of the thought being streamed.
    pub(super) fn thought_grew(&mut self, chars: usize) {
        if let Some(thought) = self.last_thought() {
            thought.len += chars;
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
        let start = buffer.marker(thought.start).expect("live with its run");
        thought.first = header.unwrap_or_else(|| {
            let line = buffer.text().slice(start..start + thought.len);
            line.chars().take_while(|&c| c != '\n').count()
        });
        thought.len += header.map_or(0, |n| n + 1);
        let Some(g) = group else {
            return;
        };
        thought.indent(buffer);
        if open {
            buffer.remove_properties(start..start + thought.len);
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

    /// Opens or folds the thought row `point` is on or in, returning where it starts and where
    /// its first line ends.
    pub(super) fn toggle_thought(
        &mut self,
        buffer: &mut Buffer,
        point: usize,
    ) -> Option<(usize, usize)> {
        for group in self.groups.iter_mut().filter(|g| g.open) {
            for row in &mut group.rows {
                let Row::Thought(thought) = row else {
                    continue;
                };
                let start = buffer.marker(thought.start).expect("live with its session");
                let end = start
                    + if thought.open {
                        thought.len
                    } else {
                        thought.first
                    };
                if thought.len == thought.first || !(start..=end).contains(&point) {
                    continue;
                }
                thought.open = !thought.open;
                buffer.remove_properties(start..start + thought.len);
                let mark = start + THOUGHT_FOLD;
                let fold = if thought.open { "▾" } else { "▸" };
                buffer.replace_output(mark..mark + 1, fold);
                render_thought(buffer, thought);
                return Some((start, start + thought.first));
            }
        }
        None
    }

    /// Opens or folds the group `point` is in, returning where its line starts and ends and
    /// whether it is open now.
    pub(super) fn toggle_group(
        &mut self,
        buffer: &mut Buffer,
        point: usize,
    ) -> Option<(usize, usize, bool)> {
        let inside = |g: &usize| {
            let span = self.group_span(buffer, *g);
            (span.start..=span.end).contains(&point)
        };
        let g = (0..self.groups.len()).find(inside)?;
        self.groups[g].open = !self.groups[g].open;
        self.render_group(buffer, g);
        let span = self.group_span(buffer, g);
        Some((
            span.start,
            span.start + self.groups[g].line,
            self.groups[g].open,
        ))
    }
}
