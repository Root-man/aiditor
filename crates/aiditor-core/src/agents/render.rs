use super::*;

/// Renders `range` of `buffer` as markdown, replacing its properties.
pub(super) fn render(buffer: &mut Buffer, range: Range<usize>, thought: bool) {
    buffer.remove_properties(range.clone());
    let text = buffer.text().slice(range.clone()).to_string();
    for (r, prop) in markdown::render(&text, thought) {
        (buffer.add_property(range.start + r.start..range.start + r.end, prop))
            .expect("inside the rendered range");
    }
}

/// Replaces `range` of `buffer`'s output with `text`, removing its properties, line by line so
/// point on an unchanged line stays.
pub(super) fn rewrite(buffer: &mut Buffer, range: Range<usize>, text: &str) {
    let old = buffer.text().slice(range.clone()).to_string();
    buffer.remove_properties(range.clone());
    for (r, new) in buffer::diff(&old, text).into_iter().rev() {
        buffer.replace_output(range.start + r.start..range.start + r.end, &new);
    }
}

/// The ACP name of an enum value, as `in_progress`.
pub(super) fn acp_name(value: impl serde::Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        _ => "?".into(),
    }
}

/// `text`'s first line, followed by ` …` when it has more.
pub(super) fn first_line(text: &str) -> String {
    let mut line = text.lines().next().unwrap_or_default().to_string();
    if text.trim_end().contains('\n') {
        line.push_str(" …");
    }
    line
}

/// A tool call's header line, `<title> (<kind>, <status>)` with the title's first line and its
/// terminal's exit after the status, and the title part's length in chars.
pub(super) fn tool_header(call: &ToolCall, term: Option<&Term>) -> (String, usize) {
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
pub(super) fn plan_line(entry: &PlanEntry) -> (String, usize) {
    let content = first_line(&entry.content);
    let suffix = format!(
        " ({}, {})",
        acp_name(&entry.status),
        acp_name(&entry.priority)
    );
    (format!("{content}{suffix}"), content.chars().count())
}

/// The plan block's rows: `Plan`, then a line per entry.
pub(super) fn plan_rows(plan: &Plan) -> Vec<(String, usize)> {
    let rows = plan.entries.iter().map(plan_line);
    std::iter::once(("Plan".to_string(), 4))
        .chain(rows)
        .collect()
}

/// A permission request's rows: `Permission: <title>`, then a numbered line per option,
/// `<n>. <name> (<kind>)`.
pub(super) fn ask_rows(title: &str, options: &[PermissionOption]) -> Vec<(String, usize)> {
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
pub(super) fn rows_text(rows: &[(String, usize)]) -> String {
    let lines: Vec<_> = rows.iter().map(|(text, _)| text.as_str()).collect();
    lines.join("\n")
}

/// Renders the block of `rows` at `start`, whose properties were removed: a rule beside each
/// line and each row's text past its length before the suffix dim.
pub(super) fn render_rows(buffer: &mut Buffer, start: usize, rows: &[(String, usize)]) {
    let rule = Prop::Line(Decoration::Rule(FaceName::SessionThought));
    let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
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
pub(super) fn cancelled(ask: &Ask) -> String {
    format!("Cancelled permission for {}", ask.title)
}

/// Replaces `ask`'s block with `line`, which stays plain.
pub(super) fn settle(buffer: &mut Buffer, ask: &Ask, line: &str) {
    let start = buffer.marker(ask.start).expect("live while pending");
    rewrite(buffer, start..start + ask.len, line);
    buffer.remove_marker(ask.start);
}

/// Everything a tool call carries, uncut, as markdown: its raw input, its content items without
/// ANSI escapes, and its terminal's output, or its raw output when no item carries the result;
/// and the faces of the escapes.
pub(super) fn tool_content(call: &ToolCall, term: Option<&Term>) -> (String, Faces) {
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
                ContentBlock::Text(text) => coloured(&text.text),
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
        if !term.sgr.text.is_empty() {
            let block = fenced("", &term.sgr.text);
            // The faces start after `Output:` and the fence line.
            let at = "Output:\n".len() + block.find('\n').expect("fenced") + 1;
            push(format!("Output:\n{block}"), term.sgr.faces.clone(), at);
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

#[derive(Default)]
pub(super) struct Sgr {
    pub(super) text: String,
    pub(super) len: usize,
    face: Vec<FaceRef>,
    pub(super) faces: Faces,
    pub(super) backticks: usize,
    run: usize,
}
impl Handler for Sgr {
    fn input(&mut self, c: char) {
        self.run = if c == '`' { self.run + 1 } else { 0 };
        self.backticks = self.backticks.max(self.run);
        self.text.push(c);
        if !self.face.is_empty() {
            match self.faces.last_mut() {
                Some((r, f)) if r.end == self.len && *f == self.face => r.end += 1,
                _ => self.faces.push((self.len..self.len + 1, self.face.clone())),
            }
        }
        self.len += 1;
    }
    fn put_tab(&mut self, count: u16) {
        (0..count).for_each(|_| self.input('\t'));
    }
    // A face never runs across a line's start, where a code line's decoration begins.
    fn linefeed(&mut self) {
        self.run = 0;
        self.text.push('\n');
        self.len += 1;
    }
    fn terminal_attribute(&mut self, attr: Attr) {
        let index = |c: Color| match c {
            Color::Named(n) if (n as usize) < 16 => Some(n as u8),
            Color::Indexed(i) if i < 16 => Some(i),
            _ => None,
        };
        let mut set = |of: fn(&FaceRef) -> bool, new: Option<FaceRef>| {
            self.face.retain(|f| !of(f));
            self.face.extend(new);
        };
        let fg = |f: &FaceRef| matches!(f, FaceRef::Fg(_));
        let bg = |f: &FaceRef| matches!(f, FaceRef::Bg(_));
        match attr {
            Attr::Reset => self.face.clear(),
            Attr::Foreground(c) => set(fg, index(c).map(FaceRef::Fg)),
            Attr::Background(c) => set(bg, index(c).map(FaceRef::Bg)),
            Attr::Bold => set(|f| *f == FaceRef::Bold, Some(FaceRef::Bold)),
            Attr::CancelBold | Attr::CancelBoldDim => set(|f| *f == FaceRef::Bold, None),
            Attr::Underline => set(|f| *f == FaceRef::Underline, Some(FaceRef::Underline)),
            Attr::CancelUnderline => set(|f| *f == FaceRef::Underline, None),
            _ => {}
        }
    }
}

/// `text` without ANSI escape sequences or carriage returns, and the faces its SGR escapes give
/// its chars: the theme's 16 colours, bold and underline.
// shortcut: `\r` is dropped, not taken as a return to the line's start; overwrite the line if progress output shows garbled.
pub(super) fn coloured(text: &str) -> (String, Faces) {
    let mut sgr = Sgr::default();
    <Processor>::new().advance(&mut sgr, text.as_bytes());
    (sgr.text, sgr.faces)
}

/// `text` in a fenced code block whose fence is longer than any run of backticks in it.
pub(super) fn fenced(lang: &str, text: &str) -> String {
    let run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(run.max(2) + 1);
    let text = text.strip_suffix('\n').unwrap_or(text);
    format!("{fence}{lang}\n{text}\n{fence}")
}

/// Renders a tool call's block, whose properties were removed: its header with a rule beside it,
/// then its content as markdown with a rule beside each line, or folded behind ` …`. No property
/// contains another's end, so starts and ends ascend.
pub(super) fn render_tool(buffer: &mut Buffer, tool: &Tool) {
    let start = buffer.marker(tool.start).expect("live while shown");
    let (header, title) = tool_header(&tool.call, tool.term.as_ref());
    let rule = Prop::Line(Decoration::Rule(FaceName::SessionThought));
    let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
    // A rule on one char touches only its line and sorts before the faces on it.
    let mut props = vec![(0..1, rule.clone())];
    let title: String = header.chars().take(title).collect();
    props.extend(markdown::render(&title, false));
    props.push((title.chars().count()..tool.header, dim));
    let body = tool.header + 1..tool.len;
    if tool.open && !body.is_empty() {
        let content = buffer
            .text()
            .slice(start + body.start..start + body.end)
            .to_string();
        let shifted = |(r, p): (Range<usize>, Prop)| (body.start + r.start..body.start + r.end, p);
        props.extend(markdown::render(&content, false).into_iter().map(shifted));
        // Sorted with the markdown faces by range; where both apply, the later sorted one wins.
        let faces = (tool.faces.iter()).map(|(r, f)| (r.clone(), Prop::Named(f.clone())));
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
