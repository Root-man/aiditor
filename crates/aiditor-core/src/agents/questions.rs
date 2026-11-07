//! Questions from agents (`elicitation/create`): which forms are single-choice or multi-select
//! questions, the rows of a question's block, its options' previews and the content of its
//! answers.

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

/// A preview as drawn: the column it starts in, and its lines, each padded to the widest and
/// whether it is the dim count of the lines left out.
pub(crate) type Preview = (usize, Vec<(String, bool)>);

/// An option's value, title, description and preview.
pub(super) type Choice = (String, String, Option<String>, Option<String>);

/// A single-choice or multi-select question.
#[derive(Debug, PartialEq)]
pub(super) struct Question {
    pub(super) message: String,
    /// The choice field's title, shown when it differs from the message.
    title: Option<String>,
    key: String,
    pub(super) options: Vec<Choice>,
    pub(super) own: Option<Own>,
    /// A multi-select question's ticks and bounds.
    pub(super) multi: Option<Multi>,
}

/// What a multi-select question has ticked, and how many it takes.
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

/// What a question was answered with.
pub(super) enum Answer {
    Option(usize),
    Own(String),
    /// A multi-select question's ticks.
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

/// A choice field's options, title and, for a multi-select, bounds; `None` for any other field.
fn choices(
    field: &ElicitationPropertySchema,
) -> Option<(Vec<Choice>, Option<&String>, Option<Multi>)> {
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
    let (options, title, multi) = match field {
        ElicitationPropertySchema::String(s) => match (&s.one_of, &s.enum_values) {
            (Some(one_of), _) => (titled(one_of), s.title.as_ref(), None),
            (None, Some(values)) => (untitled(values), s.title.as_ref(), None),
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
            (options, a.title.as_ref(), Some(multi))
        }
        _ => return None,
    };
    (!options.is_empty()).then_some((options, title, multi))
}

/// `request` as a question with the tool call it names: one choice field and at most one
/// companion own-answer field; `None` for any other request.
pub(super) fn parse(request: &CreateElicitationRequest) -> Option<(Question, Option<ToolCallId>)> {
    let ElicitationMode::Form(form) = &request.mode else {
        return None;
    };
    let fields = &form.requested_schema.properties;
    let mut choice = fields.iter().filter_map(|(k, f)| Some((k, choices(f)?)));
    let (key, (mut options, title, mut multi)) = choice.next()?;
    if choice.next().is_some() || fields.len() > 2 {
        return None;
    }
    let own = match fields.iter().find(|(k, _)| *k != key) {
        None => None,
        Some((k, f)) => {
            let note = plain(f)?.meta.as_ref().and_then(|m| m.get("codex"));
            if *k == format!("{key}_custom") {
                Some(Own::Custom(k.clone()))
            } else if note.is_some_and(|c| c["role"] == "user_note") {
                options.retain(|(value, ..)| value != NONE_OF_THE_ABOVE);
                if options.is_empty() {
                    return None;
                }
                Some(Own::Note(k.clone()))
            } else {
                return None;
            }
        }
    };
    if let Some(m) = &mut multi {
        m.ticked = vec![false; options.len()];
    }
    let message = first_line(&request.message);
    let title = (title.map(String::as_str)).filter(|t| !t.is_empty() && first_line(t) != message);
    let tool = match request.scope() {
        ElicitationScope::Session(scope) => scope.tool_call_id.clone(),
        _ => None,
    };
    let question = Question {
        message,
        title: title.map(first_line),
        key: key.clone(),
        options,
        own,
        multi,
    };
    Some((question, tool))
}

impl Question {
    /// The block's lines, frame 6d's: each with its char ranges' faces, the box drawn behind
    /// them by `render`.
    pub(super) fn lines(&self) -> Vec<(String, Faces)> {
        let named = |faces: &[FaceRef]| Prop::Named(faces.to_vec());
        let label = named(&[FaceRef::Face(FaceName::StatusWaiting), FaceRef::Bold]);
        let (key, dim) = (named(&[FaceRef::Face(FaceName::KeyHint)]), dim());
        let header = format!(" Question  {}", self.message);
        let mut lines = vec![(header, vec![(1..9, label.clone())])];
        lines.extend(self.title.as_ref().map(|t| (format!(" {t}"), Vec::new())));
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
        let chip = |n: &str, on: bool| {
            let props = match n.is_empty() {
                true => Vec::new(),
                false => vec![(2..5, key.clone())],
            };
            (format!("  {n:^3}  {}", tick(on)), props)
        };
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
                props.push((start..text.chars().count(), dim.clone()));
            }
            lines.push((text, props));
        }
        if self.own.is_some() {
            let on = self.multi.as_ref().is_some_and(|m| m.own.is_some());
            let (mut text, props) = chip("o", on);
            text.push_str("Own answer");
            lines.push((text, props));
        }
        let keys = match self.options.len() {
            1 => "1".into(),
            n => format!("1-{}", n.min(9)),
        };
        let mut text = String::from("  ");
        let mut props = Vec::new();
        let previews = self.options.iter().any(|(.., p)| p.is_some());
        let hints: &[_] = match (&self.multi, previews) {
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
        for &(key, what) in hints {
            let start = text.chars().count();
            text.push_str(key);
            props.push((start..start + key.chars().count(), label.clone()));
            text.push_str(what);
        }
        props.insert(0, (0..text.chars().count(), dim));
        lines.push((text, props));
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

    /// The line of the block its first option is on.
    pub(super) fn first(&self) -> usize {
        1 + usize::from(self.title.is_some())
    }

    /// What line `line` of the block answers with, if anything.
    pub(super) fn answer_at(&self, line: usize) -> Option<Answer> {
        let i = line.checked_sub(self.first())?;
        match i.cmp(&self.options.len()) {
            std::cmp::Ordering::Less => Some(Answer::Option(i)),
            std::cmp::Ordering::Equal if self.own.is_some() => Some(Answer::Own(String::new())),
            _ => None,
        }
    }

    /// Why a multi-select question can't be sent with what it has ticked, if it can't.
    pub(super) fn refusal(&self) -> Option<String> {
        let m = self.multi.as_ref()?;
        let n = (m.ticked.iter().filter(|t| **t).count() + usize::from(m.own.is_some())) as u64;
        match (m.min, m.max) {
            (Some(min), _) if n < min => Some(format!("Pick at least {min}")),
            (_, Some(max)) if n > max => Some(format!("Pick at most {max}")),
            _ => None,
        }
    }

    /// Option `i`'s preview, the lines drawn under its row.
    pub(super) fn preview(&self, i: usize) -> Option<Preview> {
        let text = self.options.get(i)?.3.as_ref()?.replace('\t', "    ");
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
        let column = 7 + if self.multi.is_some() { 4 } else { 0 };
        Some((column, lines))
    }

    /// `answer`'s content and the text its answered line shows.
    pub(super) fn content(
        &self,
        answer: &Answer,
    ) -> (BTreeMap<String, ElicitationContentValue>, String) {
        let mut content = BTreeMap::new();
        let shown = match (answer, &self.own) {
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
        assert_eq!(q.own, Some(Own::Custom("question_0_custom".into())));
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
        assert_eq!(q.options.len(), 2);
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
        assert_eq!(q.own, None);
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
        let multi = q.multi.as_mut().unwrap();
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
        assert_eq!(q.options.len(), 1);
        q.multi.as_mut().unwrap().own = Some("qa".into());
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
    fn other_forms_are_not_questions() {
        for fields in [
            json!({}),
            json!({"a": {"type": "string", "oneOf": one_of()}, "b": {"type": "string", "oneOf": one_of()}}),
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
