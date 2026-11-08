use std::path::{Path, PathBuf};

use agent_client_protocol_schema::v1::{Diff, ToolKind};

use crate::buffer::Icon;
use crate::file_index::FileIndex;

use super::*;

/// Renders `range` of `buffer` as markdown, replacing its properties, with links to the files
/// of `index`.
pub(super) fn render(
    buffer: &mut Buffer,
    range: Range<usize>,
    thought: bool,
    index: Option<&FileIndex>,
) {
    buffer.remove_properties(range.clone());
    let text = buffer.text().slice(range.clone()).to_string();
    let is_file = |word: &str| file_link(index, word).is_some();
    for (r, prop) in markdown::render(&text, thought, false, false, is_file).1 {
        (buffer.add_property(range.start + r.start..range.start + r.end, prop))
            .expect("inside the rendered range");
    }
}

/// Replaces `range` of `buffer`, a streamed message, with `source` rendered as markdown with its
/// tables drawn, linking the files of `index`; returns the rendered length in chars.
pub(super) fn render_message(
    buffer: &mut Buffer,
    range: Range<usize>,
    source: &str,
    index: Option<&FileIndex>,
) -> usize {
    let is_file = |word: &str| file_link(index, word).is_some();
    let (text, props) = markdown::render(source, false, false, true, is_file);
    rewrite(buffer, range.clone(), &text);
    for (r, prop) in props {
        (buffer.add_property(range.start + r.start..range.start + r.end, prop))
            .expect("inside the rendered range");
    }
    text.chars().count()
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

/// The path and line of a `path`, `path:line` or `path:line:col` word.
fn link_parts(word: &str) -> Option<(&str, Option<usize>)> {
    let Some((path, rest)) = word.split_once(':') else {
        return Some((word, None));
    };
    let numbers: Vec<_> = rest.split(':').collect();
    let digits = |n: &&str| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
    (numbers.len() <= 2 && numbers.iter().all(digits)).then(|| (path, numbers[0].parse().ok()))
}

/// The indexed file and line, from 1, a `path`, `path:line` or `path:line:col` word names.
pub(super) fn file_link(index: Option<&FileIndex>, word: &str) -> Option<(PathBuf, Option<usize>)> {
    let (path, line) = link_parts(word)?;
    Some((index?.file(Path::new(path))?, line))
}

/// The dim face of a session's own lines.
pub(super) fn dim() -> Prop {
    Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)])
}

/// Draws the prompt's `>` at `at` as frame 6's `❯`; the text keeps `>`.
pub(super) fn prompt_glyph(buffer: &mut Buffer, at: usize) {
    let glyph = vec![FaceRef::Face(FaceName::StatusInfo), FaceRef::Bold];
    let prop = Prop::Display(vec![("❯".into(), glyph)]);
    (buffer.add_property(at..at + 1, prop)).expect("the prompt's `>`");
}

/// Puts the `session-user` background on each line of the echo at `range`, a top border on its
/// first and a bottom one on its last. A one-char range per line keeps the properties sorted.
pub(super) fn user_lines(buffer: &mut Buffer, range: Range<usize>) {
    let text = buffer.text().slice(range.clone()).to_string();
    let after_newlines = text.chars().enumerate().filter(|(_, c)| *c == '\n');
    let starts: Vec<_> = (std::iter::once(0).chain(after_newlines.map(|(i, _)| i + 1)))
        .filter(|&i| i < text.chars().count())
        .map(|i| range.start + i)
        .collect();
    let border = |top| {
        let face = FaceName::SessionUserRule;
        Prop::Line(Decoration::Border { top, face })
    };
    let background = Prop::Line(Decoration::Background(FaceName::SessionUser));
    for &at in &starts {
        (buffer.add_property(at..at + 1, background.clone())).expect("inside the echo");
    }
    let (Some(&first), Some(&last)) = (starts.first(), starts.last()) else {
        return;
    };
    (buffer.add_property(first..first + 1, border(true))).expect("inside the echo");
    (buffer.add_property(last..last + 1, border(false))).expect("inside the echo");
}

/// `12s`, `2m 05s` or `1h 02m 05s`.
pub(super) fn long_duration(d: Duration) -> String {
    let s = d.as_secs();
    match (s / 3600, s / 60 % 60, s % 60) {
        (0, 0, s) => format!("{s}s"),
        (0, m, s) => format!("{m}m {s:02}s"),
        (h, m, s) => format!("{h}h {m:02}m {s:02}s"),
    }
}

/// A tool call's duration: tenths under a minute, as `0.3s`.
pub(super) fn short_duration(d: Duration) -> String {
    match d.as_secs() {
        0..60 => format!("{}.{}s", d.as_secs(), d.subsec_millis() / 100),
        _ => long_duration(d),
    }
}

/// A permission wait, as `waiting 0:12`.
pub(super) fn waiting_text(d: Duration) -> String {
    let s = d.as_secs();
    format!("waiting {}:{:02}", s / 60, s % 60)
}

/// Where the line holding `at` ends.
pub(super) fn line_end(buffer: &Buffer, at: usize) -> usize {
    let rest = buffer.text().slice(at..).chars().position(|c| c == '\n');
    at + rest.unwrap_or(buffer.len_chars() - at)
}

/// Puts a right-aligned `time` in `annotation` at `at`, a line's end, returning the chars added.
pub(super) fn put_time(buffer: &mut Buffer, at: usize, time: &str) -> usize {
    buffer.insert_output(at, &format!(" {time}"));
    let end = at + 1 + time.chars().count();
    let face = Prop::Named(vec![FaceRef::Face(FaceName::Annotation)]);
    (buffer.add_property(at..at + 1, Prop::AlignRight)).expect("just inserted");
    (buffer.add_property(at + 1..end, face)).expect("just inserted");
    end - at
}

/// How far in the line under a block puts its timings: two columns past where its text starts,
/// after a prompt's `> `, at a message's start, after a group's icon and after a call's verb.
pub(super) const PROMPT_META: usize = 2 + 2;
pub(super) const MESSAGE_META: usize = 2;
pub(super) const GROUP_META: usize = GROUP_VERBS + 2;
pub(super) const TOOL_META: usize = VERB + 2;

/// Puts `parts`, each in its face, on the blank line after the block ending at `end`, small and
/// `indent` columns in, as frame 6's timings, returning the chars added.
pub(super) fn put_meta(
    buffer: &mut Buffer,
    end: usize,
    indent: usize,
    parts: &[(&str, FaceName)],
) -> usize {
    let at = end + 1;
    let text = parts.iter().map(|(t, _)| *t).collect::<Vec<_>>().join("  ");
    if text.is_empty() {
        return 0;
    }
    buffer.insert_output(at, &text);
    let mut props = vec![
        (at..at + 1, Prop::Line(Decoration::Indent(indent))),
        (at..at + 1, Prop::Line(Decoration::Small)),
    ];
    let mut from = at;
    for (part, face) in parts {
        let len = part.chars().count();
        props.push((from..from + len, Prop::Named(vec![FaceRef::Face(*face)])));
        from += len + "  ".len();
    }
    props.sort_by_key(|(r, _)| r.start);
    for (range, prop) in props {
        (buffer.add_property(range, prop)).expect("just inserted");
    }
    text.chars().count()
}

/// Replaces the time at `range` with `text`, in `face`, returning its length in chars. Only the
/// chars that differ change, so point on the line stays.
pub(super) fn retime(
    buffer: &mut Buffer,
    range: Range<usize>,
    text: &str,
    face: FaceName,
) -> usize {
    let len = text.chars().count();
    let old = buffer.text().slice(range.clone()).to_string();
    if old == text {
        return len;
    }
    let same = (old.chars().zip(text.chars()))
        .take_while(|(a, b)| a == b)
        .count();
    buffer.remove_properties(range.clone());
    let tail: String = text.chars().skip(same).collect();
    buffer.replace_output(range.start + same..range.end, &tail);
    let prop = Prop::Named(vec![FaceRef::Face(face)]);
    (buffer.add_property(range.start..range.start + len, prop)).expect("just replaced");
    len
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

/// The output lines an open command shows.
const SHOWN_LINES: usize = 20;

/// Where a header's fixed columns end: the fold and status marks with their spaces, and the icon
/// with the space after it.
const MARKS: Range<usize> = 0..2;
const STATUS: Range<usize> = 2..3;
const ICON: Range<usize> = 4..6;
const VERB: usize = 7;
/// Where a group line's verbs start.
const GROUP_VERBS: usize = 5;

/// Whether a call with `status` has finished.
pub(super) fn finished(status: ToolCallStatus) -> bool {
    matches!(status, ToolCallStatus::Completed | ToolCallStatus::Failed)
}

/// A kind's icon and its verb, past and present.
fn kind_parts(kind: ToolKind) -> (Icon, &'static str, &'static str) {
    match kind {
        ToolKind::Read => (Icon::Book, "Read", "Reading"),
        ToolKind::Search => (Icon::Magnifier, "Searched", "Searching"),
        ToolKind::Edit => (Icon::Pencil, "Edited", "Editing"),
        ToolKind::Delete => (Icon::Pencil, "Deleted", "Deleting"),
        ToolKind::Move => (Icon::Pencil, "Moved", "Moving"),
        ToolKind::Execute => (Icon::Terminal, "Ran", "Running"),
        ToolKind::Fetch => (Icon::Globe, "Fetched", "Fetching"),
        _ => (Icon::Wrench, "Used", "Using"),
    }
}

/// A call's phrase on its group's line, past and present. Edits, deletes and moves share one.
fn group_phrase(kind: ToolKind) -> (&'static str, &'static str) {
    match kind {
        ToolKind::Read => ("read files", "reading files"),
        ToolKind::Search => ("searched code", "searching code"),
        ToolKind::Edit | ToolKind::Delete | ToolKind::Move => ("edited files", "editing files"),
        ToolKind::Execute => ("ran commands", "running commands"),
        ToolKind::Fetch => ("fetched pages", "fetching pages"),
        _ => ("used tools", "using tools"),
    }
}

/// `calls`' phrases in order of first appearance, comma separated, a phrase with an unfinished
/// call in the present.
fn group_verbs(calls: &[&Tool]) -> String {
    let mut phrases: Vec<(&str, &str, bool)> = Vec::new();
    for tool in calls {
        let (past, present) = group_phrase(tool.call.kind);
        let running = !finished(tool.call.status);
        match phrases.iter_mut().find(|p| p.0 == past) {
            Some(phrase) => phrase.2 |= running,
            None => phrases.push((past, present, running)),
        }
    }
    let phrases: Vec<_> = (phrases.iter())
        .map(|&(past, present, running)| if running { present } else { past })
        .collect();
    let verbs = phrases.join(", ");
    let mut chars = verbs.chars();
    (chars.next()).map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
}

/// How long `calls` took, first start to last end, once all finished live.
fn group_duration(calls: &[&Tool]) -> Option<Duration> {
    let arrived = (calls.iter())
        .map(|t| t.arrived)
        .collect::<Option<Vec<_>>>()?;
    let ended = (calls.iter())
        .map(|t| t.ended)
        .collect::<Option<Vec<_>>>()?;
    Some(*ended.iter().max()? - *arrived.iter().min()?)
}

/// A group's failed count and, once all its calls finished live, how long they took, for the line
/// under it.
pub(super) fn group_meta(calls: &[&Tool]) -> Vec<(String, FaceName)> {
    let failed = (calls.iter())
        .filter(|t| t.call.status == ToolCallStatus::Failed)
        .count();
    let failed = (failed > 0).then(|| (format!("{failed} failed"), FaceName::StatusFailed));
    let duration = group_duration(calls).map(|d| (short_duration(d), FaceName::Annotation));
    failed.into_iter().chain(duration).collect()
}

/// Writes a group's line over `range`, with its properties, returning its length in chars.
pub(super) fn group_line(
    buffer: &mut Buffer,
    range: Range<usize>,
    calls: &[&Tool],
    open: bool,
) -> usize {
    const ICON: Range<usize> = 2..4;
    let fold = if open { '▾' } else { '▸' };
    let verbs = group_verbs(calls);
    let verbs_end = GROUP_VERBS + verbs.chars().count();
    let text = format!("{fold}    {verbs}");
    let start = range.start;
    rewrite(buffer, range, &text);
    let icon = calls
        .first()
        .map_or(Icon::Wrench, |t| kind_parts(t.call.kind).0);
    let props = [
        (MARKS, dim()),
        (ICON, Prop::Icon(icon)),
        (ICON, dim()),
        (GROUP_VERBS..verbs_end, dim()),
    ];
    for (r, prop) in props {
        (buffer.add_property(start + r.start..start + r.end, prop)).expect("inside the line");
    }
    text.chars().count()
}

/// The verb a call's header shows.
fn verb(call: &ToolCall) -> &'static str {
    let (_, past, present) = kind_parts(call.kind);
    if finished(call.status) { past } else { present }
}

/// The command a call runs, if its input names one.
pub(super) fn tool_command<'a>(call: &'a ToolCall, term: Option<&Term>) -> Option<&'a str> {
    let command = (call.raw_input.as_ref()?.get("command")?.as_str())?;
    (call.kind == ToolKind::Execute || term.is_some()).then_some(command)
}

/// The file a call's header shows, with its location's line: a read's, edit's, delete's or
/// move's first location, else its diff's path, unless it runs a command.
pub(super) fn tool_path<'a>(
    call: &'a ToolCall,
    term: Option<&Term>,
) -> Option<(&'a Path, Option<u32>)> {
    let file = matches!(
        call.kind,
        ToolKind::Read | ToolKind::Edit | ToolKind::Delete | ToolKind::Move
    );
    if !file || tool_command(call, term).is_some() {
        return None;
    }
    let diff = call.content.iter().find_map(|item| match item {
        ToolCallContent::Diff(d) => Some((d.path.as_path(), None)),
        _ => None,
    });
    let location = (call.locations.first()).map(|l| (l.path.as_path(), l.line));
    location.or(diff)
}

/// What a call acts on: its command, URL or path, else its title. A path under `root` is relative
/// to it. A location would hide a search's pattern, so a search shows its title.
pub(super) fn tool_target(call: &ToolCall, term: Option<&Term>, root: &Path) -> String {
    let input = |key| call.raw_input.as_ref()?.get(key)?.as_str();
    let path = tool_path(call, term).map(|(path, _)| {
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
    });
    let fetch = (call.kind == ToolKind::Fetch).then(|| input("url").map(first_line));
    let command = tool_command(call, term).map(first_line);
    command
        .or(fetch.flatten())
        .or(path)
        .unwrap_or_else(|| first_line(&call.title).trim_matches('`').to_string())
}

/// The lines `call`'s diffs add and remove, if it has any.
fn diff_counts(call: &ToolCall) -> Option<(usize, usize)> {
    let mut diffs = (call.content.iter()).filter_map(|item| match item {
        ToolCallContent::Diff(d) => Some(d),
        _ => None,
    });
    let first = diffs.next()?;
    let changes = |d: &Diff| {
        let old = d.old_text.as_deref().unwrap_or_default();
        let diff = similar::TextDiff::from_lines(old, &d.new_text);
        let tags: Vec<_> = diff.iter_all_changes().map(|c| c.tag()).collect();
        let count = |tag| tags.iter().filter(|t| **t == tag).count();
        (
            count(similar::ChangeTag::Insert),
            count(similar::ChangeTag::Delete),
        )
    };
    Some(
        std::iter::once(first)
            .chain(diffs)
            .map(changes)
            .fold((0, 0), |(a, r), (x, y)| (a + x, r + y)),
    )
}

/// ` +<added> −<removed>` after a call's target, empty without diffs, and where `−` starts.
fn counts_text(call: &ToolCall) -> (String, usize) {
    diff_counts(call).map_or((String::new(), 0), |(added, removed)| {
        let added = format!(" +{added}");
        let at = added.chars().count();
        (format!("{added} −{removed}"), at + 1)
    })
}

/// A tool call's header line, the time left out: its fold and status marks, spaces for its icon,
/// its verb, what it acts on and its diffs' counts.
pub(super) fn tool_header(call: &ToolCall, term: Option<&Term>, open: bool, root: &Path) -> String {
    let fold = if open { '▾' } else { '▸' };
    let status = match call.status {
        ToolCallStatus::Completed => '✓',
        ToolCallStatus::Failed => '✗',
        _ => '●',
    };
    let target = tool_target(call, term, root);
    let (counts, _) = counts_text(call);
    format!("{fold} {status}    {} {target}{counts}", verb(call))
}

/// A plan entry's line, `  <mark> <content>` with the content's first line.
fn plan_line(entry: &PlanEntry) -> String {
    let mark = match entry.status {
        PlanEntryStatus::Completed => '✓',
        PlanEntryStatus::InProgress => '●',
        _ => '○',
    };
    format!("  {mark} {}", first_line(&entry.content))
}

/// The plan block's text: `Plan`, then a line per entry.
pub(super) fn plan_text(plan: &Plan) -> String {
    let lines: Vec<_> = std::iter::once("Plan".to_string())
        .chain(plan.entries.iter().map(plan_line))
        .collect();
    lines.join("\n")
}

/// Renders the plan block at `start`, whose properties were removed: `Plan` and each finished or
/// pending entry dim, an entry in progress with its mark in `status-waiting`.
pub(super) fn render_plan(buffer: &mut Buffer, start: usize, plan: &Plan) {
    let waiting = Prop::Named(vec![FaceRef::Face(FaceName::StatusWaiting)]);
    let mut props = vec![(0.."Plan".len(), dim())];
    let mut line = "Plan\n".len();
    for entry in &plan.entries {
        let len = plan_line(entry).chars().count();
        props.push(match entry.status {
            PlanEntryStatus::InProgress => (line + 2..line + 3, waiting.clone()),
            _ => (line + 2..line + len, dim()),
        });
        line += len + 1;
    }
    for (r, prop) in props {
        (buffer.add_property(start + r.start..start + r.end, prop)).expect("inside the block");
    }
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
    let dim = dim();
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
pub(super) const CANCELLED: &str = "✗ cancelled";

/// The line a permission request's block becomes once an option of `kind` is picked.
pub(super) fn answered(kind: PermissionOptionKind) -> &'static str {
    match kind {
        PermissionOptionKind::AllowOnce => "✓ allowed once",
        PermissionOptionKind::AllowAlways => "✓ allowed always",
        PermissionOptionKind::RejectOnce => "✗ rejected",
        PermissionOptionKind::RejectAlways => "✗ rejected always",
        // The enum is non-exhaustive.
        _ => "answered",
    }
}

/// Replaces permission request `ask`'s block with the dim `line`.
pub(super) fn settle(buffer: &mut Buffer, ask: &Ask, line: &str) {
    let start = buffer.marker(ask.start).expect("live while pending");
    rewrite(buffer, start..start + ask.len, line);
    (buffer.add_property(start..start + line.chars().count(), dim())).expect("inside the block");
    buffer.remove_marker(ask.start);
}

/// Replaces question `ask`'s block with its answered line, from `line`'s `<mark> <outcome>`,
/// `clock` at its right, and the rows of what it sent, folded.
pub(super) fn settle_question(
    buffer: &mut Buffer,
    ask: &Ask,
    line: &str,
    clock: Option<&str>,
) -> Answered {
    let question = ask.question().expect("settled as a question");
    let start = buffer.marker(ask.start).expect("live while pending");
    let mut chars = line.chars();
    let mark = chars.next().expect("a mark");
    let label = question.label();
    let head = format!("▸ {mark} {label}  {}", chars.as_str().trim_start());
    let name = (question.sent.iter()).map(|(n, _)| n.chars().count()).max();
    let name = name.unwrap_or_default();
    let rows: String = (question.sent.iter())
        .map(|(n, a)| format!("\n {n:<name$}  {a}"))
        .collect();
    rewrite(buffer, start..start + ask.len, &format!("{head}{rows}"));
    let outcome = head.chars().count();
    let time = clock.map_or(String::new(), |c| format!(" {c}"));
    buffer.insert_output(start + outcome, &time);
    let line = outcome + time.chars().count();
    Answered {
        start: ask.start,
        label: 4 + label.chars().count(),
        outcome,
        line,
        len: line + rows.chars().count(),
        name,
        mark: match mark {
            '✓' => FaceName::StatusRunning,
            '✗' => FaceName::StatusFailed,
            _ => FaceName::SessionThought,
        },
        open: false,
    }
}

/// Renders answered line `a`, two columns in when `grouped`: its marks, its dim outcome and
/// time, and its rows on `session-block` when open, else hidden.
pub(super) fn render_answered(buffer: &mut Buffer, a: &Answered, grouped: bool) {
    let start = buffer.marker(a.start).expect("live with its session");
    let face = |name| Prop::Named(vec![FaceRef::Face(name)]);
    let indent = if grouped { 2 } else { 0 };
    let mut props = vec![
        (0..1, dim()),
        (2..3, face(a.mark)),
        (a.label + 2..a.outcome, dim()),
    ];
    if grouped {
        props.push((0..1, Prop::Line(Decoration::Indent(indent))));
    }
    if a.line > a.outcome {
        props.push((a.outcome..a.outcome + 1, Prop::AlignRight));
        props.push((a.outcome + 1..a.line, face(FaceName::Annotation)));
    }
    if a.open {
        let rows = buffer
            .text()
            .slice(start + a.line..start + a.len)
            .to_string();
        let mut at = a.line + 1;
        for row in rows.split('\n').skip(1) {
            // Under the label, the row's leading space its padding.
            props.push((at..at + 1, Prop::Line(Decoration::Indent(indent + 3))));
            props.push((
                at..at + 1,
                Prop::Line(Decoration::Background(FaceName::SessionBlock)),
            ));
            props.push((at + 1..at + 1 + a.name, dim()));
            at += row.chars().count() + 1;
        }
    } else if a.len > a.line {
        props.push((a.line..a.len, Prop::Invisible(String::new())));
    }
    for (r, prop) in props {
        (buffer.add_property(start + r.start..start + r.end, prop)).expect("inside the line");
    }
}

/// A tool call's raw output as text.
fn raw_text(output: &Value) -> String {
    match output {
        Value::String(text) => text.clone(),
        // Claude's adapter passes a tool result's text blocks through.
        Value::Array(items) if !items.is_empty() && items.iter().all(|i| i["text"].is_string()) => {
            let texts: Vec<_> = items.iter().filter_map(|i| i["text"].as_str()).collect();
            texts.join("\n")
        }
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// `d` as a unified diff, with its `---`/`+++` header when `header`, else from its first hunk.
fn unified(d: &Diff, header: bool) -> String {
    let (old, path) = (d.old_text.as_deref().unwrap_or_default(), d.path.display());
    let path = path.to_string();
    let diff = similar::TextDiff::from_lines(old, &d.new_text);
    let mut unified = diff.unified_diff();
    if header {
        unified.header(&path, &path);
    }
    unified.to_string()
}

/// The `diff-*` faces of a unified diff's lines, its first two the header when `header`, which
/// never run across a line's start.
fn diff_faces(text: &str, header: bool) -> Faces {
    let mut at = 0;
    let lines = text.split('\n').enumerate().map(|(i, line)| {
        let face = match line.as_bytes().first() {
            _ if (header && i < 2) || line.starts_with("@@") => Some(FaceName::DiffHunk),
            Some(b'+') => Some(FaceName::DiffAdded),
            Some(b'-') => Some(FaceName::DiffRemoved),
            _ => None,
        };
        let len = line.chars().count();
        at += len + 1;
        face.map(|f| (at - len - 1..at - 1, vec![FaceRef::Face(f)]))
    });
    lines.flatten().collect()
}

/// A tool call's content as markdown: what it ran or its raw input, its content items without
/// ANSI escapes, and its terminal's output, or its raw output when no item carries the result.
pub(super) struct ToolContent {
    pub(super) text: String,
    /// The faces of the escapes and diffs.
    pub(super) faces: Faces,
    /// Where a command's output lines start.
    pub(super) output: Option<usize>,
}

pub(super) fn tool_content(call: &ToolCall, term: Option<&Term>) -> ToolContent {
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
    let command = tool_command(call, term);
    let diffs = (call.content.iter()).any(|item| matches!(item, ToolCallContent::Diff(_)));
    if let (None, false, Some(input)) = (command, diffs, &call.raw_input) {
        let input = serde_json::to_string_pretty(input).unwrap_or_default();
        push(format!("Input:\n{}", fenced("json", &input)), vec![], 0);
    }
    let execute = call.kind == ToolKind::Execute;
    // A command's text items repeat its raw output.
    let raw_only = term.is_none() && execute && call.raw_output.is_some();
    for item in &call.content {
        if let (ToolCallContent::Terminal(t), Some(term)) = (item, term)
            && *t.terminal_id.0 == *term.id
        {
            continue;
        }
        if raw_only && matches!(item, ToolCallContent::Content(_)) {
            continue;
        }
        let (part, part_faces, at) = match item {
            ToolCallContent::Content(c) => match &c.content {
                // Codex sends a read or list command's output here, colored.
                ContentBlock::Text(text) => {
                    let (text, faces) = coloured(&text.text);
                    (text, faces, 0)
                }
                ContentBlock::ResourceLink(link) => (link.uri.clone(), vec![], 0),
                other => match serde_json::to_value(other) {
                    Ok(v) => (
                        format!("[{}]", v["type"].as_str().unwrap_or("?")),
                        vec![],
                        0,
                    ),
                    Err(_) => ("[?]".into(), vec![], 0),
                },
            },
            ToolCallContent::Diff(d) => {
                let diff = unified(d, false);
                let block = fenced("diff", &diff);
                let at = block.find('\n').expect("fenced") + 1;
                (block, diff_faces(&diff, false), at)
            }
            ToolCallContent::Terminal(t) => (format!("Terminal {}", t.terminal_id), vec![], 0),
            _ => ("[?]".into(), vec![], 0),
        };
        push(part, part_faces, at);
    }
    let output = match (term, &call.raw_output) {
        (Some(term), _) => {
            (!term.sgr.text.is_empty()).then(|| (term.sgr.text.clone(), term.sgr.faces.clone()))
        }
        (None, Some(output)) if call.content.is_empty() || raw_only => {
            Some((raw_text(output), vec![]))
        }
        _ => None,
    };
    let mut from_end = None;
    if output.is_some() || command.is_some() {
        let (output, output_faces) = output.unwrap_or_default();
        let above = command.map_or(String::new(), |c| format!("$ {}\n", c.trim_end()));
        let label = if command.is_some() { "" } else { "Output:\n" };
        let block = fenced("", &format!("{above}{output}"));
        // The faces start after the label, the fence line and the command.
        let at = label.len() + block.find('\n').expect("fenced") + 1 + above.chars().count();
        let part = format!("{label}{block}");
        from_end = Some(part.chars().count() - at);
        push(part, output_faces, at);
    }
    let output = from_end.filter(|_| execute || term.is_some());
    ToolContent {
        output: output.map(|from_end| content.chars().count() - from_end),
        text: content,
        faces,
    }
}

/// A tool call's full output, for its own buffer: its terminal's, else its diffs, else its raw
/// output, else its text items.
pub(super) fn tool_output(call: &ToolCall, term: Option<&Term>) -> Option<(String, Faces)> {
    if let Some(term) = term.filter(|t| !t.sgr.text.is_empty()) {
        return Some((term.sgr.text.clone(), term.sgr.faces.clone()));
    }
    let diffs: Vec<_> = (call.content.iter())
        .filter_map(|item| match item {
            ToolCallContent::Diff(d) => Some(unified(d, true)),
            _ => None,
        })
        .collect();
    if !diffs.is_empty() {
        let diff = diffs.concat();
        let faces = diffs.iter().fold((0, Vec::new()), |(at, mut faces), diff| {
            let shift = |(r, f): (Range<usize>, _)| (at + r.start..at + r.end, f);
            faces.extend(diff_faces(diff, true).into_iter().map(shift));
            (at + diff.chars().count(), faces)
        });
        return Some((diff, faces.1));
    }
    if let Some(output) = &call.raw_output {
        return Some((raw_text(output), vec![]));
    }
    let (mut text, mut faces) = (String::new(), Vec::new());
    for item in &call.content {
        if let ToolCallContent::Content(c) = item
            && let ContentBlock::Text(t) = &c.content
        {
            let (item, item_faces) = coloured(&t.text);
            let at = text.chars().count();
            faces.extend((item_faces.into_iter()).map(|(r, f)| (at + r.start..at + r.end, f)));
            text += &item;
            text.push('\n');
        }
    }
    (!text.is_empty()).then_some((text, faces))
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

/// The length of a fence around `text`, longer than any run of backticks in it.
pub(super) fn fence_len(text: &str) -> usize {
    let run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    run.max(2) + 1
}

/// `text` in a fenced code block whose fence is longer than any run of backticks in it.
pub(super) fn fenced(lang: &str, text: &str) -> String {
    let fence = "`".repeat(fence_len(text));
    let text = text.strip_suffix('\n').unwrap_or(text);
    format!("{fence}{lang}\n{text}\n{fence}")
}

/// The output lines of `content` past the shown ones when they start at `at`, its closing fence
/// the last line, and how many there are.
fn hidden_output(content: &str, at: usize) -> Option<(Range<usize>, usize)> {
    let ends: Vec<_> = (content.chars().enumerate().skip(at))
        .filter_map(|(i, c)| (c == '\n').then_some(i))
        .collect();
    let lines = ends.len().checked_sub(SHOWN_LINES).filter(|n| *n > 0)?;
    Some((ends[SHOWN_LINES - 1] + 1..ends[ends.len() - 1], lines))
}

/// The face of a call's time: `status-failed` for an exit code.
pub(super) fn time_face(tool: &Tool) -> FaceName {
    let exit = tool.term.as_ref().and_then(|t| t.exit.as_ref());
    match exit == Some(&tool.time) {
        true => FaceName::StatusFailed,
        false => FaceName::Annotation,
    }
}

/// Takes the line under a call that is not in a group off, returning where the block ends.
pub(super) fn take_tool_meta(buffer: &mut Buffer, tool: &mut Tool) -> usize {
    let end = buffer.marker(tool.start).expect("live while shown") + tool.len;
    let meta = std::mem::take(&mut tool.meta);
    if meta > 0 {
        buffer.remove_properties(end + 1..end + 1 + meta);
        buffer.replace_output(end + 1..end + 1 + meta, "");
    }
    end
}

/// Puts the time of a call that is not in a group on the line under its block.
pub(super) fn put_tool_meta(buffer: &mut Buffer, tool: &mut Tool) {
    let end = take_tool_meta(buffer, tool);
    if tool.group.is_none() {
        tool.meta = put_meta(buffer, end, TOOL_META, &[(&tool.time, time_face(tool))]);
    }
}

/// Renders a tool call's block, whose properties were removed: its header with a rule beside it,
/// then its content as markdown with a rule beside each line, or folded. An open command's
/// hidden output gets no properties of its own, so no property contains another's end and starts
/// and ends ascend.
pub(super) fn render_tool(buffer: &mut Buffer, tool: &Tool) {
    let start = buffer.marker(tool.start).expect("live while shown");
    if tool.replaced {
        // With what parts it from the row before in a group, else from the next block.
        let span = match tool.group {
            Some(_) => start - 1..start + tool.len,
            None => start..start + tool.len + "\n\n".len() + tool.meta,
        };
        buffer.remove_properties(span.clone());
        let hide = Prop::Invisible(String::new());
        return (buffer.add_property(span, hide)).expect("inside the text");
    }
    let rule = Prop::Line(Decoration::Rule(FaceName::SessionThought));
    let status = match tool.call.status {
        ToolCallStatus::Failed => FaceName::StatusFailed,
        _ => FaceName::StatusRunning,
    };
    let face = |name| Prop::Named(vec![FaceRef::Face(name)]);
    // A row of a group sits two columns in, its rule still on the line's first char.
    let indent = if tool.group.is_some() { 2 } else { 0 };
    let head = |r: Range<usize>| r.start + indent..r.end + indent;
    let verb_end = VERB + verb(&tool.call).chars().count();
    let (icon, ..) = kind_parts(tool.call.kind);
    // A rule on one char touches only its line and sorts before the faces on it.
    let mut props = vec![
        (0..1, rule.clone()),
        (head(MARKS), dim()),
        (head(STATUS), face(status)),
        (head(ICON), Prop::Icon(icon)),
        (head(ICON), dim()),
        (head(VERB..verb_end), dim()),
    ];
    // Where the target ends, before the counts and the time.
    let (counts, removed) = counts_text(&tool.call);
    let inline = !tool.time.is_empty() && tool.group.is_some();
    let mut end = match inline {
        false => tool.header,
        true => tool.header - tool.time.chars().count() - 1,
    };
    if !counts.is_empty() {
        end -= counts.chars().count();
        // The status marks' faces, as frame 6 draws them: the diff faces carry a background.
        props.push((end + 1..end + removed - 1, face(FaceName::StatusRunning)));
        props.push((
            end + removed..end + counts.chars().count(),
            face(FaceName::StatusFailed),
        ));
    }
    if tool.link.is_some() {
        let link = vec![FaceRef::Face(FaceName::Link), FaceRef::Underline];
        props.push((verb_end + indent + 1..end, Prop::Named(link)));
    }
    if tool_command(&tool.call, tool.term.as_ref()).is_some() {
        props.push((verb_end + indent + 1..end, face(FaceName::SessionCode)));
    }
    if inline {
        let suffix = tool.header - tool.time.chars().count() - 1;
        props.push((suffix..suffix + 1, Prop::AlignRight));
        props.push((suffix + 1..tool.header, face(time_face(tool))));
    }
    let body = tool.header + 1..tool.len;
    if tool.open && !body.is_empty() {
        let content = buffer
            .text()
            .slice(start + body.start..start + body.end)
            .to_string();
        let shifted = |(r, p): (Range<usize>, Prop)| (body.start + r.start..body.start + r.end, p);
        let cut = tool.output.and_then(|at| hidden_output(&content, at));
        let hidden = cut.as_ref().map(|(r, _)| r.clone());
        let visible = hidden.as_ref().map_or(content.clone(), |h| {
            let tail = content.chars().skip(h.end);
            content.chars().take(h.start).chain(tail).collect()
        });
        // Past the cut, as the closing lines follow the hidden ones in the text.
        let past = |r: Range<usize>| match &hidden {
            Some(h) if r.start >= h.start => r.start + h.len()..r.end + h.len(),
            _ => r,
        };
        let markdown = markdown::render(&visible, false, true, false, |_| false)
            .1
            .into_iter();
        props.extend(markdown.map(|(r, p)| (past(r), p)).map(shifted));
        // Sorted with the markdown faces by range; where both apply, the later sorted one wins.
        let faces = tool.faces.iter().filter_map(|(r, f)| {
            let end = hidden.as_ref().map_or(r.end, |h| r.end.min(h.start));
            (r.start < end).then(|| (r.start..end, Prop::Named(f.clone())))
        });
        props.extend(faces.map(shifted));
        if let Some((hidden, lines)) = cut {
            let plural = if lines == 1 { "" } else { "s" };
            let more = format!("… {lines} more line{plural}");
            props.push(shifted((hidden, Prop::Invisible(more))));
        }
        // Two columns past the status mark.
        let body_indent = Prop::Line(Decoration::Indent(STATUS.end + indent + 1));
        let mut line = 0;
        for l in content.split('\n') {
            if hidden
                .as_ref()
                .is_none_or(|h| line <= h.start || line > h.end)
            {
                props.push(shifted((line..line + 1, rule.clone())));
                props.push(shifted((line..line + 1, body_indent.clone())));
            }
            line += l.chars().count() + 1;
        }
    } else if !body.is_empty() {
        props.push((tool.header..tool.len, Prop::Invisible(String::new())));
    }
    props.sort_by_key(|(r, _)| (r.start, r.end));
    for (r, prop) in props {
        (buffer.add_property(start + r.start..start + r.end, prop)).expect("inside the block");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_words_take_a_line_and_column() {
        assert_eq!(link_parts("a.rs"), Some(("a.rs", None)));
        assert_eq!(link_parts("a.rs:12"), Some(("a.rs", Some(12))));
        assert_eq!(link_parts("a.rs:12:5"), Some(("a.rs", Some(12))));
        assert_eq!(link_parts("a.rs:x"), None);
        assert_eq!(link_parts("a.rs:12-20"), None);
        assert_eq!(link_parts("a.rs:1:2:3"), None);
        assert_eq!(file_link(None, "a.rs"), None);
    }
}
