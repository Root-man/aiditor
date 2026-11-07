//! Questions from agents (`elicitation/create`): which forms are single-choice questions, the
//! rows of a question's block and the content of its answers.

use std::collections::BTreeMap;

use agent_client_protocol_schema::v1::{
    CreateElicitationRequest, ElicitationContentValue, ElicitationMode, ElicitationPropertySchema,
    ElicitationScope, StringPropertySchema,
};

use super::ToolCallId;
use super::render::first_line;

/// codex-acp's own-answer option, which `o` stands in for.
const NONE_OF_THE_ABOVE: &str = "None of the above";

/// A single-choice question.
#[derive(Debug, PartialEq)]
pub(super) struct Question {
    pub(super) message: String,
    /// The choice field's title, shown when it differs from the message.
    title: Option<String>,
    key: String,
    /// Each option's value, title and description.
    pub(super) options: Vec<(String, String, Option<String>)>,
    pub(super) own: Option<Own>,
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

/// A choice field's options, `None` for any other field.
fn choices(field: &ElicitationPropertySchema) -> Option<Vec<(String, String, Option<String>)>> {
    let ElicitationPropertySchema::String(s) = field else {
        return None;
    };
    let options: Vec<_> = match (&s.one_of, &s.enum_values) {
        (Some(one_of), _) => (one_of.iter())
            .map(|o| (o.value.clone(), o.title.clone(), o.description.clone()))
            .collect(),
        (None, Some(values)) => values
            .iter()
            .map(|v| (v.clone(), v.clone(), None))
            .collect(),
        (None, None) => return None,
    };
    (!options.is_empty()).then_some(options)
}

/// `request` as a single-choice question with the tool call it names: one choice field and at
/// most one companion own-answer field; `None` for any other request.
pub(super) fn parse(request: &CreateElicitationRequest) -> Option<(Question, Option<ToolCallId>)> {
    let ElicitationMode::Form(form) = &request.mode else {
        return None;
    };
    let fields = &form.requested_schema.properties;
    let mut choice = fields.iter().filter_map(|(k, f)| Some((k, f, choices(f)?)));
    let (key, field, mut options) = choice.next()?;
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
    let ElicitationPropertySchema::String(s) = field else {
        unreachable!("choices are strings");
    };
    let message = first_line(&request.message);
    let title = (s.title.as_deref()).filter(|t| !t.is_empty() && first_line(t) != message);
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
    };
    Some((question, tool))
}

impl Question {
    /// The block's rows, each with the length of its text before a dim suffix.
    pub(super) fn rows(&self) -> Vec<(String, usize)> {
        let full = |text: String| {
            let len = text.chars().count();
            (text, len)
        };
        let mut rows = vec![full(format!("Question: {}", self.message))];
        rows.extend(self.title.clone().map(full));
        for (i, (_, title, description)) in self.options.iter().enumerate() {
            let (text, len) = full(format!("{}. {}", i + 1, first_line(title)));
            match description {
                Some(d) => rows.push((format!("{text}  {}", first_line(d)), len)),
                None => rows.push((text, len)),
            }
        }
        if self.own.is_some() {
            rows.push(full("o. Own answer".into()));
        }
        let keys = match self.options.len() {
            1 => "1".into(),
            n => format!("1-{n}"),
        };
        rows.push((
            format!("{keys} answer   RET answer with the row at point   C-c C-k skip"),
            0,
        ));
        rows
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

    /// `answer`'s content and the text its answered line shows.
    pub(super) fn content(
        &self,
        answer: &Answer,
    ) -> (BTreeMap<String, ElicitationContentValue>, String) {
        let mut content = BTreeMap::new();
        let shown = match (answer, &self.own) {
            (Answer::Option(i), _) => {
                let (value, title, _) = &self.options[*i];
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
        let rows: Vec<_> = q.rows().into_iter().map(|r| r.0).collect();
        assert_eq!(
            rows,
            [
                "Question: Pick one?",
                "Pick",
                "1. A  First",
                "2. B",
                "o. Own answer",
                "1-2 answer   RET answer with the row at point   C-c C-k skip"
            ]
        );
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
            q.rows().last().unwrap().0,
            "1 answer   RET answer with the row at point   C-c C-k skip"
        );
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
            json!({"a": {"type": "array", "items": {"anyOf": one_of()}}}),
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
