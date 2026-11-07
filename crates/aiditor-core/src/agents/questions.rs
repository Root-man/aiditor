//! Questions from agents (`elicitation/create`): which forms are questions of one or several
//! single-choice or multi-select fields, the rows of a question's block, its options' previews
//! and the content of its answers.

use std::collections::BTreeMap;
use std::ops::Range;

use agent_client_protocol_schema::v1::{
    CreateElicitationRequest, ElicitationContentValue, ElicitationMode, ElicitationPropertySchema,
    ElicitationScope, EnumOption, MultiSelectItems, StringPropertySchema,
};

use super::ToolCallId;
use super::render::{dim, first_line};
use crate::buffer::{Buffer, Decoration, FaceRef, Prop};
use crate::settings::FaceName;

/// A line's faces, by char range.
pub(super) type Faces = Vec<(Range<usize>, Prop)>;

/// codex-acp's own-answer option, which `o` stands in for.
const NONE_OF_THE_ABOVE: &str = "None of the above";

/// The most lines a preview shows; a longer one shows one less and how many more it has.
const PREVIEW_LINES: usize = 8;

/// How much further in an open field's rows sit in a block of several questions.
const NESTED: &str = "   ";

/// A preview as drawn: the column it starts in, and its lines, each padded to the widest and
/// whether it is the dim count of the lines left out.
pub(crate) type Preview = (usize, Vec<(String, bool)>);

/// An option's value, title, description and preview.
pub(super) type Choice = (String, String, Option<String>, Option<String>);

/// A question: one or several choice fields, one of them open.
#[derive(Debug, PartialEq)]
pub(super) struct Question {
    pub(super) message: String,
    pub(super) fields: Vec<Field>,
    /// The field whose options show.
    pub(super) open: usize,
}

/// A single-choice or multi-select field.
#[derive(Debug, PartialEq)]
pub(super) struct Field {
    title: Option<String>,
    description: Option<String>,
    key: String,
    pub(super) options: Vec<Choice>,
    pub(super) own: Option<Own>,
    /// A multi-select field's ticks and bounds.
    pub(super) multi: Option<Multi>,
    /// A single-choice field's answer, kept while a block of several waits to be sent.
    pub(super) chosen: Option<Answer>,
}

/// What a multi-select field has ticked, and how many it takes.
#[derive(Debug, PartialEq)]
pub(super) struct Multi {
    pub(super) ticked: Vec<bool>,
    /// The own answer's text, once ticked.
    pub(super) own: Option<String>,
    min: Option<u64>,
    max: Option<u64>,
}

/// Where an own answer goes, in the adapter's shape.
#[derive(Debug, PartialEq)]
pub(super) enum Own {
    /// claude-agent-acp's `<choice>_custom` field, sent alone.
    Custom(String),
    /// codex-acp's note field, sent with the choice set to `None of the above`.
    Note(String),
}

/// What a field was answered with.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Answer {
    Option(usize),
    Own(String),
    /// A multi-select field's ticks, or a block of several's answers.
    Ticked,
}

/// Whether `field` is a string field without choices.
fn plain(field: &ElicitationPropertySchema) -> Option<&StringPropertySchema> {
    match field {
        ElicitationPropertySchema::String(s) if s.one_of.is_none() && s.enum_values.is_none() => {
            Some(s)
        }
        _ => None,
    }
}

/// A choice field without its companion, or `None` for any other field.
fn choices(key: &str, field: &ElicitationPropertySchema) -> Option<Field> {
    let titled = |options: &[EnumOption]| -> Vec<Choice> {
        let preview = |o: &EnumOption| {
            let meta = o.meta.as_ref()?.get("_claude/askUserQuestionOption")?;
            Some(
                meta["preview"]
                    .as_str()
                    .filter(|p| !p.is_empty())?
                    .to_owned(),
            )
        };
        (options.iter())
            .map(|o| {
                (
                    o.value.clone(),
                    o.title.clone(),
                    o.description.clone(),
                    preview(o),
                )
            })
            .collect()
    };
    let untitled = |values: &[String]| -> Vec<Choice> {
        (values.iter())
            .map(|v| (v.clone(), v.clone(), None, None))
            .collect()
    };
    let (options, title, description, multi) = match field {
        ElicitationPropertySchema::String(s) => match (&s.one_of, &s.enum_values) {
            (Some(one_of), _) => (titled(one_of), &s.title, &s.description, None),
            (None, Some(values)) => (untitled(values), &s.title, &s.description, None),
            (None, None) => return None,
        },
        ElicitationPropertySchema::Array(a) => {
            let options = match &a.items {
                MultiSelectItems::Titled(t) => titled(&t.options),
                MultiSelectItems::String(s) => untitled(&s.values),
                _ => return None,
            };
            let multi = Multi {
                ticked: Vec::new(),
                own: None,
                min: a.min_items,
                max: a.max_items,
            };
            (options, &a.title, &a.description, Some(multi))
        }
        _ => return None,
    };
    let line = |t: &Option<String>| t.as_deref().filter(|t| !t.is_empty()).map(first_line);
    let field = Field {
        title: line(title),
        description: line(description),
        key: key.into(),
        options,
        own: None,
        multi,
        chosen: None,
    };
    (!field.options.is_empty()).then_some(field)
}

/// `key` split for sorting, its trailing number compared as a number.
fn natural(key: &str) -> (&str, Option<u64>) {
    let prefix = key.trim_end_matches(|c: char| c.is_ascii_digit());
    (prefix, key[prefix.len()..].parse().ok())
}

/// `request` as a question with the tool call it names: choice fields, each with at most one
/// companion own-answer field, and nothing else; `None` for any other request.
pub(super) fn parse(request: &CreateElicitationRequest) -> Option<(Question, Option<ToolCallId>)> {
    let ElicitationMode::Form(form) = &request.mode else {
        return None;
    };
    let props = &form.requested_schema.properties;
    let mut fields: Vec<_> = props.iter().filter_map(|(k, f)| choices(k, f)).collect();
    fields.sort_by(|a, b| natural(&a.key).cmp(&natural(&b.key)));
    let single = fields.len() == 1;
    let mut companions = 0;
    for field in &mut fields {
        let key = &field.key;
        let mut own = props.iter().filter_map(|(k, f)| {
            let codex = plain(f)?.meta.as_ref().and_then(|m| m.get("codex"));
            let named = codex.map(|c| &c["questionId"]);
            if *k == format!("{key}_custom") {
                Some(Own::Custom(k.clone()))
            } else if codex.is_some_and(|c| c["role"] == "user_note")
                && named.is_some_and(|q| *q == **key || (single && q.is_null()))
            {
                Some(Own::Note(k.clone()))
            } else {
                None
            }
        });
        field.own = own.next();
        if own.next().is_some() {
            return None;
        }
        if let Some(Own::Note(_)) = field.own {
            field
                .options
                .retain(|(value, ..)| value != NONE_OF_THE_ABOVE);
            if field.options.is_empty() {
                return None;
            }
        }
        if let Some(m) = &mut field.multi {
            m.ticked = vec![false; field.options.len()];
        }
        companions += usize::from(field.own.is_some());
    }
    if fields.is_empty() || fields.len() + companions != props.len() {
        return None;
    }
    let message = first_line(&request.message);
    let tool = match request.scope() {
        ElicitationScope::Session(scope) => scope.tool_call_id.clone(),
        _ => None,
    };
    let question = Question {
        message,
        fields,
        open: 0,
    };
    Some((question, tool))
}

/// `label` in its face: `status-waiting`, bold.
fn label() -> Prop {
    Prop::Named(vec![FaceRef::Face(FaceName::StatusWaiting), FaceRef::Bold])
}

/// A face of its own.
fn face(name: FaceName) -> Prop {
    Prop::Named(vec![FaceRef::Face(name)])
}

/// The dim hint line: `lead`, then each key in `label` and what it does.
fn hint(lead: &str, hints: &[(&str, &str)]) -> (String, Faces) {
    let mut text = String::from(lead);
    let mut props = Vec::new();
    for &(key, what) in hints {
        let start = text.chars().count();
        text.push_str(key);
        props.push((start..start + key.chars().count(), label()));
        text.push_str(what);
    }
    props.insert(0, (0..text.chars().count(), dim()));
    (text, props)
}

impl Field {
    /// Its option and own-answer rows, `indent` further in than a single question's.
    fn rows(&self, indent: &str) -> Vec<(String, Faces)> {
        let titles: Vec<_> = self
            .options
            .iter()
            .map(|(_, t, ..)| first_line(t))
            .collect();
        let width = titles.iter().map(|t| t.chars().count()).max().unwrap_or(0);
        let tick = |on: bool| match (&self.multi, on) {
            (None, _) => "",
            (Some(_), false) => "[ ] ",
            (Some(_), true) => "[x] ",
        };
        // `  [ 1 ]  <title>` with the chip 3 columns wide from column 2; digits reach nine.
        let at = 2 + indent.len();
        let chip = |n: &str, on: bool| {
            let props = match n.is_empty() {
                true => Vec::new(),
                false => vec![(at..at + 3, face(FaceName::KeyHint))],
            };
            (format!("{indent}  {n:^3}  {}", tick(on)), props)
        };
        let mut lines = Vec::new();
        for (i, (title, (_, _, description, _))) in titles.iter().zip(&self.options).enumerate() {
            let n = match i < 9 {
                true => (i + 1).to_string(),
                false => String::new(),
            };
            let on = self.multi.as_ref().is_some_and(|m| m.ticked[i]);
            let (mut text, mut props) = chip(&n, on);
            text.push_str(title);
            if let Some(d) = description {
                let pad = width - title.chars().count();
                text.push_str(&" ".repeat(pad + 2));
                let start = text.chars().count();
                text.push_str(&first_line(d));
                props.push((start..text.chars().count(), dim()));
            }
            lines.push((text, props));
        }
        if self.own.is_some() {
            let on = self.multi.as_ref().is_some_and(|m| m.own.is_some());
            let (mut text, props) = chip("o", on);
            text.push_str("Own answer");
            lines.push((text, props));
        }
        lines
    }

    /// The keys that pick its options: `1`, or `1-<n>` up to nine.
    fn keys(&self) -> String {
        match self.options.len() {
            1 => "1".into(),
            n => format!("1-{}", n.min(9)),
        }
    }

    /// Its name in a block of several: its title, else its key.
    fn name(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.key)
    }

    /// What row `i` of its rows answers with, if anything.
    fn answer_at(&self, i: usize) -> Option<Answer> {
        match i.cmp(&self.options.len()) {
            std::cmp::Ordering::Less => Some(Answer::Option(i)),
            std::cmp::Ordering::Equal if self.own.is_some() => Some(Answer::Own(String::new())),
            _ => None,
        }
    }

    /// Why a multi-select field can't be sent with what it has ticked, if it can't.
    fn refusal(&self) -> Option<String> {
        let m = self.multi.as_ref()?;
        let n = (m.ticked.iter().filter(|t| **t).count() + usize::from(m.own.is_some())) as u64;
        match (m.min, m.max) {
            (Some(min), _) if n < min => Some(format!("Pick at least {min}")),
            (_, Some(max)) if n > max => Some(format!("Pick at most {max}")),
            _ => None,
        }
    }

    /// Its answer in a block of several: what it picked or ticked, if anything.
    fn answered(&self) -> Option<Answer> {
        match &self.multi {
            Some(m) if m.ticked.contains(&true) || m.own.is_some() => Some(Answer::Ticked),
            Some(_) => None,
            None => self.chosen.clone(),
        }
    }

    /// Adds `answer`'s content to `content`, returning the text its answered line shows.
    fn content(
        &self,
        answer: &Answer,
        content: &mut BTreeMap<String, ElicitationContentValue>,
    ) -> String {
        match (answer, &self.own) {
            (Answer::Ticked, own) => {
                let multi = self.multi.as_ref().expect("only a multi-select is ticked");
                let ticked = (self.options.iter().zip(&multi.ticked)).filter(|(_, on)| **on);
                let (mut values, mut shown): (Vec<_>, Vec<_>) = ticked
                    .map(|((v, t, ..), _)| (v.clone(), first_line(t)))
                    .unzip();
                if let (Some(text), Some(own)) = (&multi.own, own) {
                    let key = match own {
                        Own::Custom(key) => key,
                        Own::Note(key) => {
                            values.push(NONE_OF_THE_ABOVE.into());
                            key
                        }
                    };
                    content.insert(key.clone(), text.as_str().into());
                    shown.push(first_line(text));
                }
                content.insert(
                    self.key.clone(),
                    ElicitationContentValue::StringArray(values),
                );
                match shown.is_empty() {
                    true => "Nothing picked".into(),
                    false => shown.join(", "),
                }
            }
            (Answer::Option(i), _) => {
                let (value, title, ..) = &self.options[*i];
                content.insert(self.key.clone(), value.as_str().into());
                first_line(title)
            }
            (Answer::Own(text), Some(Own::Custom(key))) => {
                content.insert(key.clone(), text.as_str().into());
                first_line(text)
            }
            (Answer::Own(text), Some(Own::Note(key))) => {
                content.insert(self.key.clone(), NONE_OF_THE_ABOVE.into());
                content.insert(key.clone(), text.as_str().into());
                first_line(text)
            }
            (Answer::Own(_), None) => unreachable!("only a question with a companion takes one"),
        }
    }
}

impl Question {
    /// Whether it asks several fields in one block.
    pub(super) fn several(&self) -> bool {
        self.fields.len() > 1
    }

    /// The open field.
    pub(super) fn field(&self) -> &Field {
        &self.fields[self.open]
    }

    pub(super) fn field_mut(&mut self) -> &mut Field {
        &mut self.fields[self.open]
    }

    /// The block's lines, frame 6d's for one field and 6e's for several: each with its char
    /// ranges' faces, the box drawn behind them by `render`.
    pub(super) fn lines(&self) -> Vec<(String, Faces)> {
        let field = self.field();
        if !self.several() {
            let header = format!(" Question  {}", self.message);
            let mut lines = vec![(header, vec![(1..9, label())])];
            let title = field.title.as_ref().filter(|t| **t != self.message);
            lines.extend(title.map(|t| (format!(" {t}"), Vec::new())));
            lines.extend(field.rows(""));
            let previews = field.options.iter().any(|(.., p)| p.is_some());
            let keys = field.keys();
            let hints: &[_] = match (&field.multi, previews) {
                (Some(_), _) => &[
                    (&*keys, " tick   "),
                    ("RET", " tick at point   "),
                    ("C-c C-y", " send   "),
                    ("C-c C-k", " skip"),
                ],
                (None, true) => &[
                    (&*keys, " answer   "),
                    ("C-n C-p", " preview another   "),
                    ("C-c C-k", " skip"),
                ],
                (None, false) => &[
                    (&*keys, " answer   "),
                    ("RET", " answer with the row at point   "),
                    ("C-c C-k", " skip"),
                ],
            };
            lines.push(hint("  ", hints));
            return lines;
        }
        let header = format!(" Questions  {} of {}", self.open + 1, self.fields.len());
        let mut lines = vec![(header, vec![(1..10, label())])];
        let width = (self.fields.iter())
            .map(|f| f.name().chars().count())
            .max()
            .unwrap_or(0);
        for (i, f) in self.fields.iter().enumerate() {
            let name = f.name();
            let pad = " ".repeat(width - name.chars().count());
            let answer = (i != self.open)
                .then(|| f.answered())
                .flatten()
                .map(|a| f.content(&a, &mut BTreeMap::new()));
            let (marks, mut props) = match (i == self.open, &answer) {
                (true, _) => ("▾ ●", vec![(1..2, dim()), (3..4, label())]),
                (false, Some(_)) => (
                    "▸ ✓",
                    vec![(1..2, dim()), (3..4, face(FaceName::StatusRunning))],
                ),
                (false, None) => ("▸ ○", vec![(1..4, dim())]),
            };
            let mut text = format!(" {marks} {name}");
            let rest = answer.as_ref().or(f.description.as_ref());
            if let Some(rest) = rest {
                text.push_str(&format!("{pad}  "));
                let start = text.chars().count();
                text.push_str(rest);
                if i != self.open {
                    props.push((start..text.chars().count(), dim()));
                }
            }
            lines.push((text, props));
            if i == self.open {
                lines.extend(f.rows(NESTED));
            }
        }
        let verb = match field.multi {
            Some(_) => " tick  ",
            None => " answer  ",
        };
        let hints = [
            (&*field.keys(), verb),
            ("TAB", " next  "),
            ("C-c C-y", " send all  "),
            ("C-c C-k", " skip all"),
        ];
        lines.push(hint(" ", &hints));
        lines
    }

    /// Draws the block of `lines()` at `start`, whose properties were removed: each line in the
    /// `permission` box, one column in, with its faces.
    pub(super) fn render(&self, buffer: &mut Buffer, start: usize) {
        let mut at = start;
        for (text, props) in self.lines() {
            let decorations = [
                Decoration::Indent(1),
                Decoration::Background(FaceName::Permission),
            ];
            for decoration in decorations {
                (buffer.add_property(at..at + 1, Prop::Line(decoration)))
                    .expect("inside the block");
            }
            for (r, prop) in props {
                (buffer.add_property(at + r.start..at + r.end, prop)).expect("inside the block");
            }
            at += text.chars().count() + 1;
        }
    }

    /// The line of the block the open field's first option is on.
    pub(super) fn first(&self) -> usize {
        match self.several() {
            true => 2 + self.open,
            false => {
                let title = self.field().title.as_ref();
                1 + usize::from(title.is_some_and(|t| *t != self.message))
            }
        }
    }

    /// What line `line` of the block answers with, if anything.
    pub(super) fn answer_at(&self, line: usize) -> Option<Answer> {
        self.field().answer_at(line.checked_sub(self.first())?)
    }

    /// Why it can't be sent with what it has ticked, if it can't: a block of several checks
    /// each field that has an answer.
    pub(super) fn refusal(&self) -> Option<String> {
        if !self.several() {
            return self.field().refusal();
        }
        let answered = self.fields.iter().filter(|f| f.answered().is_some());
        answered
            .filter_map(|f| Some(format!("{}: {}", f.name(), f.refusal()?)))
            .next()
    }

    /// Option `i` of the open field's preview, the lines drawn under its row.
    pub(super) fn preview(&self, i: usize) -> Option<Preview> {
        let field = self.field();
        let text = field.options.get(i)?.3.as_ref()?.replace('\t', "    ");
        let all: Vec<_> = text.lines().collect();
        let shown = match all.len() > PREVIEW_LINES {
            true => PREVIEW_LINES - 1,
            false => all.len(),
        };
        let mut lines: Vec<_> = all[..shown]
            .iter()
            .map(|l| (l.to_string(), false))
            .collect();
        if shown < all.len() {
            lines.push((format!("… {} more lines", all.len() - shown), true));
        }
        let width = lines
            .iter()
            .map(|(l, _)| l.chars().count())
            .max()
            .unwrap_or(0);
        for (line, _) in &mut lines {
            let pad = width - line.chars().count();
            *line = format!(" {line}{} ", " ".repeat(pad));
        }
        // Under the title: past two spaces, the chip, two more and a multi-select's tick.
        let nested = if self.several() { NESTED.len() } else { 0 };
        let column = 7 + nested + if field.multi.is_some() { 4 } else { 0 };
        Some((column, lines))
    }

    /// `answer`'s content and the text its answered line shows; a block of several sends each
    /// field's answer, whatever `answer` is.
    pub(super) fn content(
        &self,
        answer: &Answer,
    ) -> (BTreeMap<String, ElicitationContentValue>, String) {
        let mut content = BTreeMap::new();
        if !self.several() {
            let shown = self.field().content(answer, &mut content);
            return (content, shown);
        }
        let shown: Vec<_> = (self.fields.iter())
            .filter_map(|f| {
                let answer = f.answered()?;
                Some(format!(
                    "{}: {}",
                    f.name(),
                    f.content(&answer, &mut content)
                ))
            })
            .collect();
        let shown = match shown.is_empty() {
            true => "Nothing answered".into(),
            false => shown.join("; "),
        };
        (content, shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(fields: serde_json::Value) -> CreateElicitationRequest {
        serde_json::from_value(json!({
            "sessionId": "s", "toolCallId": "t", "mode": "form", "message": "Pick one?",
            "requestedSchema": {"type": "object", "properties": fields},
        }))
        .unwrap()
    }

    fn one_of() -> serde_json::Value {
        json!([{"const": "a", "title": "A", "description": "First"}, {"const": "b", "title": "B"}])
    }

    #[test]
    fn claude_questions_take_their_custom_field() {
        let r = request(json!({
            "question_0": {"type": "string", "title": "Pick", "oneOf": one_of()},
            "question_0_custom": {"type": "string", "title": "Other"},
        }));
        let (q, tool) = parse(&r).unwrap();
        assert_eq!(tool, Some("t".into()));
        assert_eq!(q.field().own, Some(Own::Custom("question_0_custom".into())));
        let lines = q.lines();
        let texts: Vec<_> = lines.iter().map(|l| l.0.as_str()).collect();
        assert_eq!(
            texts,
            [
                " Question  Pick one?",
                " Pick",
                "   1   A  First",
                "   2   B",
                "   o   Own answer",
                "  1-2 answer   RET answer with the row at point   C-c C-k skip"
            ]
        );
        // The label, each chip and each hint key in their faces, descriptions and the hint dim.
        let faced = |(text, props): &(String, Faces)| -> Vec<String> {
            let chars: Vec<_> = text.chars().collect();
            (props.iter())
                .map(|(r, _)| chars[r.clone()].iter().collect())
                .collect()
        };
        assert_eq!(faced(&lines[0]), ["Question"]);
        assert_eq!(faced(&lines[2]), [" 1 ", "First"]);
        assert_eq!(faced(&lines[4]), [" o "]);
        assert_eq!(faced(&lines[5])[1..], ["1-2", "RET", "C-c C-k"]);
        assert!(matches!(q.answer_at(2), Some(Answer::Option(0))));
        assert!(matches!(q.answer_at(4), Some(Answer::Own(_))));
        assert!(q.answer_at(1).is_none() && q.answer_at(5).is_none());
        let (content, shown) = q.content(&Answer::Own("mine".into()));
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            json!({"question_0_custom": "mine"})
        );
        assert_eq!(shown, "mine");
    }

    #[test]
    fn codex_questions_hide_none_of_the_above_and_send_it_with_the_note() {
        let mut options = one_of();
        options
            .as_array_mut()
            .unwrap()
            .push(json!({"const": NONE_OF_THE_ABOVE, "title": NONE_OF_THE_ABOVE}));
        let r = request(json!({
            "env": {"type": "string", "title": "Pick one?", "oneOf": options},
            "env_note": {"type": "string", "_meta": {"codex": {"role": "user_note"}}},
        }));
        let (q, _) = parse(&r).unwrap();
        assert_eq!(q.field().options.len(), 2);
        assert_eq!(q.first(), 1, "a title equal to the message isn't repeated");
        let (content, _) = q.content(&Answer::Own("qa".into()));
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            json!({"env": NONE_OF_THE_ABOVE, "env_note": "qa"})
        );
        let (content, shown) = q.content(&Answer::Option(1));
        assert_eq!(serde_json::to_value(content).unwrap(), json!({"env": "b"}));
        assert_eq!(shown, "B");
    }

    #[test]
    fn a_bare_enum_is_a_question_without_an_own_answer() {
        let (q, _) = parse(&request(json!({"c": {"type": "string", "enum": ["x"]}}))).unwrap();
        assert_eq!(q.field().own, None);
        assert_eq!(
            q.lines().last().unwrap().0,
            "  1 answer   RET answer with the row at point   C-c C-k skip"
        );
    }

    #[test]
    fn multi_selects_send_their_ticks_and_read_previews() {
        let mut options = one_of();
        options[0]["_meta"] = json!({"_claude/askUserQuestionOption": {"preview": "x\ty"}});
        let r = request(json!({
            "question_0": {"type": "array", "minItems": 1, "items": {"anyOf": options}},
            "question_0_custom": {"type": "string"},
        }));
        let (mut q, _) = parse(&r).unwrap();
        assert_eq!(q.preview(0), Some((11, vec![(" x    y ".into(), false)])));
        assert_eq!(q.preview(1), None);
        let mut empty = one_of();
        empty[0]["_meta"] = json!({"_claude/askUserQuestionOption": {"preview": ""}});
        let (e, _) = parse(&request(json!({"c": {"type": "string", "oneOf": empty}}))).unwrap();
        assert!(e.preview(0).is_none() && e.lines().last().unwrap().0.contains("RET answer"));
        assert_eq!(q.refusal().as_deref(), Some("Pick at least 1"));
        let (content, shown) = q.content(&Answer::Ticked);
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            json!({"question_0": []})
        );
        assert_eq!(shown, "Nothing picked");
        let multi = q.field_mut().multi.as_mut().unwrap();
        (multi.ticked[1], multi.own) = (true, Some("mine".into()));
        assert_eq!(q.refusal(), None);
        assert_eq!(q.lines()[2].0, "   2   [x] B");
        let (content, shown) = q.content(&Answer::Ticked);
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            json!({"question_0": ["b"], "question_0_custom": "mine"})
        );
        assert_eq!(shown, "B, mine");
        // A codex note joins as `None of the above`; bare enum items are their own titles.
        let r = request(json!({
            "env": {"type": "array", "items": {"type": "string", "enum": ["x", NONE_OF_THE_ABOVE]}},
            "env_note": {"type": "string", "_meta": {"codex": {"role": "user_note"}}},
        }));
        let (mut q, _) = parse(&r).unwrap();
        assert_eq!(q.field().options.len(), 1);
        q.field_mut().multi.as_mut().unwrap().own = Some("qa".into());
        let (content, _) = q.content(&Answer::Ticked);
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            json!({"env": [NONE_OF_THE_ABOVE], "env_note": "qa"})
        );
    }

    #[test]
    fn a_long_preview_folds() {
        let mut options = one_of();
        let long: Vec<_> = (0..12).map(|n| n.to_string()).collect();
        options[0]["_meta"] =
            json!({"_claude/askUserQuestionOption": {"preview": long.join("\n")}});
        let (q, _) = parse(&request(json!({"c": {"type": "string", "oneOf": options}}))).unwrap();
        let (column, lines) = q.preview(0).unwrap();
        assert_eq!(column, 7);
        assert_eq!(lines.len(), PREVIEW_LINES);
        assert_eq!(lines[6], (" 6              ".into(), false));
        assert_eq!(lines[7], (" … 5 more lines ".into(), true));
    }

    #[test]
    fn several_questions_fold_all_but_the_open_one() {
        let one = |title: &str, d: &str, options| json!({"type": "string", "title": title, "description": d, "oneOf": options});
        let r = request(json!({
            "question_10": one("Commit", "One or many?", one_of()),
            "question_2": one("Status", "Which status?", json!([{"const": "p", "title": "Prompt"}, {"const": "b", "title": "Both"}])),
            "question_2_custom": {"type": "string"},
            "question_3": {"type": "array", "title": "Tests", "description": "Which cases?", "minItems": 2, "items": {"anyOf": one_of()}},
        }));
        let (mut q, _) = parse(&r).unwrap();
        let texts = |q: &Question| q.lines().into_iter().map(|l| l.0).collect::<Vec<_>>();
        assert_eq!(
            texts(&q),
            [
                " Questions  1 of 3",
                " ▾ ● Status  Which status?",
                "      1   Prompt",
                "      2   Both",
                "      o   Own answer",
                " ▸ ○ Tests   Which cases?",
                " ▸ ○ Commit  One or many?",
                " 1-2 answer  TAB next  C-c C-y send all  C-c C-k skip all",
            ]
        );
        assert_eq!(q.first(), 2);
        assert!(matches!(q.answer_at(4), Some(Answer::Own(_))));
        let (content, shown) = q.content(&Answer::Ticked);
        assert!(content.is_empty());
        assert_eq!(shown, "Nothing answered");
        (q.fields[0].chosen, q.open) = (Some(Answer::Option(1)), 1);
        assert_eq!(
            texts(&q)[..5],
            [
                " Questions  2 of 3",
                " ▸ ✓ Status  Both",
                " ▾ ● Tests   Which cases?",
                "      1   [ ] A  First",
                "      2   [ ] B",
            ]
        );
        assert!(texts(&q)[6].starts_with(" 1-2 tick  TAB next"));
        assert_eq!(q.first(), 3);
        assert!(matches!(q.answer_at(3), Some(Answer::Option(0))));
        // The ✓ in its face, the answer dim.
        let faced: Vec<String> = (q.lines()[1].1.iter())
            .map(|(r, _)| q.lines()[1].0.chars().skip(r.start).take(r.len()).collect())
            .collect();
        assert_eq!(faced, ["▸", "✓", "Both"]);
        q.field_mut().multi.as_mut().unwrap().ticked[0] = true;
        assert_eq!(q.refusal().as_deref(), Some("Tests: Pick at least 2"));
        q.field_mut().multi.as_mut().unwrap().ticked[1] = true;
        assert_eq!(q.refusal(), None);
        let (content, shown) = q.content(&Answer::Ticked);
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            json!({"question_2": "b", "question_3": ["a", "b"]})
        );
        assert_eq!(shown, "Status: Both; Tests: A, B");
    }

    #[test]
    fn codex_notes_pair_by_question_id() {
        let note = |id: &str| json!({"type": "string", "_meta": {"codex": {"role": "user_note", "questionId": id}}});
        let choice = json!({"type": "string", "oneOf": one_of()});
        let r = request(json!({
            "env": choice, "env_note": note("env"), "branch": choice, "branch_note": note("branch"),
        }));
        let (q, _) = parse(&r).unwrap();
        let owns: Vec<_> = q
            .fields
            .iter()
            .map(|f| (f.key.as_str(), f.own.as_ref()))
            .collect();
        assert_eq!(
            owns,
            [
                ("branch", Some(&Own::Note("branch_note".into()))),
                ("env", Some(&Own::Note("env_note".into())))
            ]
        );
        let unnamed = json!({"type": "string", "_meta": {"codex": {"role": "user_note"}}});
        let r = request(json!({"env": choice, "branch": choice, "n": unnamed}));
        assert!(
            parse(&r).is_none(),
            "a note naming no question pairs only with a single one"
        );
    }

    #[test]
    fn other_forms_are_not_questions() {
        for fields in [
            json!({}),
            json!({"a": {"type": "string", "oneOf": one_of()}, "b": {"type": "boolean"}}),
            json!({"a": {"type": "boolean"}}),
            json!({"a": {"type": "string"}}),
            json!({"a": {"type": "string", "oneOf": one_of()}, "note": {"type": "string"}}),
            json!({"a": {"type": "string", "oneOf": one_of()}, "a_custom": {"type": "number"}}),
            json!({"a": {"type": "string", "enum": [NONE_OF_THE_ABOVE]},
                   "n": {"type": "string", "_meta": {"codex": {"role": "user_note"}}}}),
            json!({"a": {"type": "array", "items": {"type": "number"}}}),
        ] {
            assert!(parse(&request(fields.clone())).is_none(), "{fields}");
        }
        let url: CreateElicitationRequest = serde_json::from_value(json!({
            "sessionId": "s", "mode": "url", "message": "m", "url": "https://x", "elicitationId": "e",
        }))
        .unwrap();
        assert!(parse(&url).is_none());
    }
}
