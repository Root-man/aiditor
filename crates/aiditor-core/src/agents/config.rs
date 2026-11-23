//! A session's config options (model, effort, mode) and its mode line.

use super::*;

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
pub(super) fn mode_option(
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
pub(super) fn values(select: &SessionConfigSelect) -> Vec<&SessionConfigValueId> {
    options(select).into_iter().map(|o| &o.value).collect()
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

impl Session {
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
            optional: 0..0,
        }
    }
}

impl Core {
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
}

impl Ctx<'_> {
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
}
