//! Asks: an agent's permission requests and questions, shown in the session, and the commands
//! that answer them.

use super::*;

/// A permission request's or question's block, waiting for its answer.
pub(super) struct Ask {
    pub(super) request: RequestId,
    /// The tool call it asks about, which shows the wait.
    pub(super) tool: Option<ToolCallId>,
    pub(super) since: Instant,
    /// The tool call's title, or the question's message, for the block.
    pub(super) title: String,
    pub(super) body: Body,
}

/// What an `Ask` asks.
pub(super) enum Body {
    Permission(Vec<PermissionOption>),
    Question(questions::Question),
}

impl Body {
    /// Its block's rows, under `title`.
    pub(super) fn rows(&self, title: &str) -> Vec<(String, usize)> {
        match self {
            Body::Permission(options) => ask_rows(title, options),
            Body::Question(question) => question.lines().into_iter().map(|(t, _)| (t, 0)).collect(),
        }
    }

    /// Its block's properties, under `title`.
    pub(super) fn props(&self, title: &str) -> Props {
        match self {
            Body::Permission(_) => rows_props(&self.rows(title)),
            Body::Question(question) => question.props(),
        }
    }
}

impl Ask {
    pub(super) fn question(&self) -> Option<&questions::Question> {
        match &self.body {
            Body::Question(q) => Some(q),
            Body::Permission(_) => None,
        }
    }

    /// Whether the turn waits for it: anything but a non-blocking question.
    pub(super) fn blocks(&self) -> bool {
        !self.question().is_some_and(|q| q.nonblocking)
    }

    /// Whether it is a permission request about `tool`, whose row then shows under its folded
    /// group; a question's block shows there in place of the row.
    pub(super) fn holds(&self, tool: &ToolCallId) -> bool {
        self.question().is_none() && self.tool.as_ref() == Some(tool)
    }

    /// The answer that cancels it, and the line its block becomes.
    pub(super) fn cancelled(&self) -> (ClientResponse, String) {
        match &self.body {
            Body::Permission(_) => (
                ClientResponse::RequestPermissionResponse(RequestPermissionResponse::new(
                    RequestPermissionOutcome::Cancelled,
                )),
                CANCELLED.into(),
            ),
            Body::Question(_) => (
                ClientResponse::CreateElicitationResponse(CreateElicitationResponse::new(
                    ElicitationAction::Cancel,
                )),
                "✗ Cancelled".into(),
            ),
        }
    }
}

/// A closed question's line, `▸ <mark> <label>  <outcome>` and its time, with the rows of what
/// it sent under it.
pub(super) struct Answered {
    /// The line and the rows, without its time.
    pub(super) text: String,
    /// Where the label and the outcome end.
    pub(super) label: usize,
    pub(super) outcome: usize,
    /// Its clock time: at the right of the line in a group, else on the line under its block.
    pub(super) time: Option<String>,
    /// The rows' name width.
    pub(super) name: usize,
    pub(super) mark: FaceName,
    pub(super) open: bool,
}

impl Core {
    /// Puts permission request `ask` above session `id`'s prompt line as a block of its own,
    /// pending until answered.
    pub(super) fn show_permission(
        &mut self,
        id: AgentId,
        request: RequestId,
        ask: RequestPermissionRequest,
    ) {
        let Some(session) = self.agents.get(&id) else {
            return;
        };
        let call = &ask.tool_call;
        let shown = session.tools.get(&call.tool_call_id).map(|t| &t.call.title);
        let title = (call.fields.title.as_ref().or(shown))
            .map_or_else(|| call.tool_call_id.to_string(), |t| first_line(t));
        let tool = Some(ask.tool_call.tool_call_id);
        self.show_ask(id, request, tool, title, Body::Permission(ask.options));
    }

    /// Puts question `question` above session `id`'s prompt line, pending until answered. Point
    /// moves to it when the prompt area is empty; otherwise the echo line says it waits.
    pub(super) fn show_question(
        &mut self,
        id: AgentId,
        request: RequestId,
        (question, tool): (questions::Question, Option<ToolCallId>),
    ) {
        let (title, first) = (question.message.clone(), question.first());
        let nonblocking = question.nonblocking;
        self.show_ask(id, request, tool, title, Body::Question(question));
        let Some(session) = self.agents.get(&id) else {
            return;
        };
        let (buffer, (ask, _)) = (session.buffer, session.asks().last().expect("just shown"));
        let b = self.buffers.get(buffer).expect("shown in it");
        if nonblocking || b.prompt_start() < b.len_chars() {
            if self.windows.window.buffer() == buffer {
                self.echo("Question waiting: C-c C-q goes to it".into());
            }
            return;
        }
        let start = session.range(b, ask).start;
        let at = b.text().line_to_char(b.text().char_to_line(start) + first) + 1;
        let b = self.buffers.get_mut(buffer).expect("shown in it");
        let end = b.len_chars();
        (b.set_marker(b.point, at)).expect("in the text");
        for window in self.windows.windows_mut() {
            if window.buffer() == buffer {
                // Scrolled to the prompt, then to the header, so the block below point shows.
                for p in [end, start] {
                    (window.set_point(&mut self.buffers, p)).expect("in the text");
                    window.scroll(&self.buffers, window.rows());
                }
                (window.set_point(&mut self.buffers, at)).expect("in the text");
            }
        }
    }

    /// Puts `body` above session `id`'s prompt line as a block of its own, pending until
    /// answered.
    fn show_ask(
        &mut self,
        id: AgentId,
        request: RequestId,
        tool: Option<ToolCallId>,
        title: String,
        body: Body,
    ) {
        self.end_stream(id);
        let now = self.now();
        let Some((session, buffer)) = self.session_buffer(id) else {
            return;
        };
        // A non-blocking question stands apart, so the turn's later calls group below it.
        let alone = matches!(&body, Body::Question(q) if q.nonblocking) && tool.is_none();
        if alone {
            session.end_run(buffer);
        }
        let shown = (tool.as_ref()).and_then(|t| session.tools.get(t).map(|t| t.call.clone()));
        let ask = Ask {
            tool,
            since: now,
            request,
            title,
            body,
        };
        let path = session.push(buffer, "", BlockKind::Ask(ask), true);
        let rendered = session.render(path, &Look::at(now));
        session.write(buffer, path, rendered, &Look::at(now));
        if alone {
            session.end_run(buffer);
        }
        if let Some(g) = session.group {
            session.show_group(buffer, g);
        }
        if let Some(call) = shown {
            self.show_tool(id, call);
        }
    }

    /// Adds `ask`'s wait, just closed, to its tool call and shows the call without it, or not at
    /// all for a question, its block at `path` now an answer.
    // shortcut: overlapping asks on one call each add their wait, so the run time can read low;
    // both adapters ask once per call.
    pub(super) fn close_wait(&mut self, id: AgentId, ask: &Ask, path: Option<BlockPath>) {
        let now = self.now();
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        if let (Some(buffer), Some((g, Some(_)))) = (self.buffers.get_mut(session.buffer), path) {
            session.show_group(buffer, g);
        }
        if let Some(tool) = ask.tool.as_ref().and_then(|t| session.tools.get_mut(t)) {
            tool.waited += now - ask.since;
            tool.replaced |= ask.question().is_some();
            let call = tool.call.clone();
            self.show_tool(id, call);
        }
    }
}

impl Ctx<'_> {
    /// Closes session `id`'s pending request `at`, leaving the dim `line` in place of its block,
    /// and sends `response` unless the agent withdrew it. Point in a question's block moves to
    /// the prompt's end.
    pub(super) fn close_ask(
        &mut self,
        id: AgentId,
        at: BlockPath,
        line: &str,
        response: Option<ClientResponse>,
    ) {
        let inside = self.ask_at_point().is_some_and(|(_, a, _)| a == at)
            && self.core.agents[&id].ask(at).question().is_some();
        if inside {
            let end = self.core.current().len_chars();
            _ = self.goto(end);
        }
        let core = &mut *self.core;
        let (clock, look) = (core.clock(), Look::at(core.now()));
        let session = core
            .agents
            .get_mut(&id)
            .expect("a pending request's session");
        let b = (core.buffers.get_mut(session.buffer)).expect("live with its session");
        let ask = session.settle(b, at, line, clock.as_deref(), &look);
        core.close_wait(id, &ask, Some(at));
        if let Some(response) = response {
            self.effects.push(Effect::Agent(AgentOp::Reply {
                id,
                request: ask.request,
                result: Ok(response),
            }));
        }
    }

    /// Answers session `id`'s pending permission request `at` with its option `option`, or
    /// `cancelled`, leaving its answer line in place of its block.
    pub(super) fn answer_ask(&mut self, id: AgentId, at: BlockPath, option: Option<usize>) {
        let ask = &self.core.agents[&id].ask(at);
        let Body::Permission(options) = &ask.body else {
            unreachable!("answered as a permission request");
        };
        let (response, line) = match option.map(|i| &options[i]) {
            Some(o) => (
                ClientResponse::RequestPermissionResponse(RequestPermissionResponse::new(
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                        o.option_id.clone(),
                    )),
                )),
                answered(o.kind).to_owned(),
            ),
            None => ask.cancelled(),
        };
        self.close_ask(id, at, &line, Some(response));
    }

    /// Answers session `id`'s pending question `at` with `answer`.
    fn answer_question(&mut self, id: AgentId, at: BlockPath, answer: questions::Answer) {
        let ask = &self.core.agents[&id].ask(at);
        let question = ask.question().expect("answered as a question");
        let (content, shown, sent) = question.content(&answer);
        let line = format!("✓ {shown}");
        let accept = ElicitationAcceptAction::new().content(content);
        let ask = self.core.agents.get_mut(&id).map(|s| s.ask_mut(at));
        if let Some(Ask {
            body: Body::Question(question),
            ..
        }) = ask
        {
            question.sent = sent;
        }
        let response = ClientResponse::CreateElicitationResponse(CreateElicitationResponse::new(
            ElicitationAction::Accept(accept),
        ));
        self.close_ask(id, at, &line, Some(response));
    }

    /// Ticks or unticks option `option` of session `id`'s pending multi-select question `at`,
    /// or its own answer with `None`, which `own` sets.
    fn tick_question(
        &mut self,
        id: AgentId,
        at: BlockPath,
        option: Option<usize>,
        own: Option<String>,
    ) {
        let session = self
            .core
            .agents
            .get_mut(&id)
            .expect("a pending question's session");
        let Body::Question(question) = &mut session.ask_mut(at).body else {
            unreachable!("ticked as a question");
        };
        let field = question.field_mut().expect("ticked in the open field");
        let multi = (field.multi.as_mut()).expect("ticked as a multi-select");
        match option {
            Some(i) => multi.ticked[i] = !multi.ticked[i],
            None => multi.own = own,
        }
        self.redraw_question(id, at);
    }

    /// Records `answer` for the open single-choice field of session `id`'s pending block of
    /// several questions `at`, and opens the next field.
    fn record_answer(&mut self, id: AgentId, at: BlockPath, answer: questions::Answer) {
        self.open_question(id, at, |q| {
            let open = q.open.expect("answered in the open field");
            q.fields[open].chosen = Some(answer);
            // Past the last, every field folds until TAB opens one.
            q.open = Some(open + 1).filter(|&i| i < q.fields.len());
        });
    }

    /// Changes session `id`'s pending question `at` with `change`, which may open another field,
    /// then redraws it with point on the open field's first option.
    pub(super) fn open_question(
        &mut self,
        id: AgentId,
        at: BlockPath,
        change: impl FnOnce(&mut questions::Question),
    ) {
        let session = self
            .core
            .agents
            .get_mut(&id)
            .expect("a pending question's session");
        let Body::Question(question) = &mut session.ask_mut(at).body else {
            unreachable!("opened as a question");
        };
        change(question);
        let first = question.first();
        let Some(start) = self.redraw_question(id, at) else {
            return;
        };
        let text = self.core.current().text();
        let at = text.line_to_char(text.char_to_line(start) + first) + 1;
        _ = self.goto(at);
    }

    /// Draws session `id`'s pending question `at` again after a change, returning where its
    /// block starts. A block of the same length changes char for char, so point stays.
    fn redraw_question(&mut self, id: AgentId, at: BlockPath) -> Option<usize> {
        let point = self.core.point();
        let selected = self.core.windows.window.buffer();
        let look = Look::at(self.core.now());
        let session = self
            .core
            .agents
            .get_mut(&id)
            .expect("a pending question's session");
        let buffer = self.core.buffers.get_mut(session.buffer)?;
        let Range { start, end } = session.range(buffer, at);
        let rendered = session.render(at, &look);
        let mut kept = None;
        // Point's line and column in the block, kept as its lines change length.
        let text = buffer.text();
        if rendered.text.chars().count() != end - start
            && selected == session.buffer
            && (start..=end).contains(&point)
        {
            let line = text.char_to_line(point);
            kept = Some((
                line - text.char_to_line(start),
                point - text.line_to_char(line),
            ));
        }
        session.write(buffer, at, rendered, &look);
        if let (g, Some(_)) = at {
            session.show_group(buffer, g);
        }
        if let Some((line, column)) = kept {
            let text = self.core.current().text();
            let first = text.char_to_line(start) + line;
            let end = text.line_to_char(first + 1).saturating_sub(1);
            _ = self.goto((text.line_to_char(first) + column).min(end));
        }
        Some(start)
    }

    /// Answers session `id`'s pending request `at` with `cancelled` or `cancel`.
    pub(super) fn cancel_ask(&mut self, id: AgentId, at: BlockPath) {
        let (response, line) = self.core.agents[&id].ask(at).cancelled();
        self.close_ask(id, at, &line, Some(response));
    }

    /// Closes the block of the request `request` that session `id`'s agent withdrew.
    pub(crate) fn agent_withdrawn(&mut self, id: AgentId, request: RequestId) {
        let at = (self.core.agents.get(&id)).and_then(|s| {
            s.asks()
                .find(|(_, a)| a.request == request)
                .map(|(at, _)| at)
        });
        if let Some(at) = at {
            let question = self.core.agents[&id].ask(at).question().is_some();
            let line = if question {
                "○ Closed"
            } else {
                "○ withdrawn"
            };
            self.close_ask(id, at, line, None);
        }
    }

    /// `agent-allow-once`: answers the oldest pending permission request with its first
    /// `allow_once` option, passing questions by.
    pub(crate) fn agent_allow_once(&mut self) -> Result<(), String> {
        if let Some((id, at, _)) = self
            .ask_at_point()
            .filter(|&(id, at, _)| self.sendable(id, at))
        {
            let question = self.core.agents[&id].ask(at).question();
            if let Some(refusal) = question.and_then(|q| q.refusal()) {
                return Err(refusal);
            }
            self.answer_question(id, at, questions::Answer::Ticked);
            return Ok(());
        }
        let kind = PermissionOptionKind::AllowOnce;
        let (id, session) = self.core.current_session()?;
        let (at, ask, options) = (session.asks())
            .find_map(|(at, a)| match &a.body {
                Body::Permission(options) => Some((at, a, options)),
                Body::Question(_) => None,
            })
            .ok_or("No permission request is pending")?;
        let option = (options.iter().position(|o| o.kind == kind))
            .ok_or_else(|| format!("No {} option for {}", acp_name(kind), ask.title))?;
        self.answer_ask(id, at, Some(option));
        Ok(())
    }

    /// `agent-reject-once`: answers the pending request point is on, else the oldest: a
    /// permission request with its first `reject_once` option, a question with `decline`.
    pub(crate) fn agent_reject_once(&mut self) -> Result<(), String> {
        let kind = PermissionOptionKind::RejectOnce;
        let (id, at) = match self.ask_at_point() {
            Some((id, at, _)) => (id, at),
            None => {
                let (id, session) = self.core.current_session()?;
                let at = session
                    .asks()
                    .next()
                    .ok_or("No permission request is pending")?;
                (id, at.0)
            }
        };
        let ask = self.core.agents[&id].ask(at);
        let options = match &ask.body {
            Body::Permission(options) => options,
            Body::Question(_) => {
                let response = ClientResponse::CreateElicitationResponse(
                    CreateElicitationResponse::new(ElicitationAction::Decline),
                );
                self.close_ask(id, at, "– Skipped", Some(response));
                return Ok(());
            }
        };
        let option = (options.iter().position(|o| o.kind == kind))
            .ok_or_else(|| format!("No {} option for {}", acp_name(kind), ask.title))?;
        self.answer_ask(id, at, Some(option));
        Ok(())
    }

    /// `agent-goto-question`: moves point to the oldest pending question's first option.
    pub(crate) fn agent_goto_question(&mut self) -> Result<(), String> {
        let (_, session) = self.core.current_session()?;
        let (start, first) = (session.asks())
            .find_map(|(at, a)| Some((session.block(at).start, a.question()?.first())))
            .ok_or("No question is pending")?;
        let text = self.core.current().text();
        let start = (self.core.current().marker(start)).expect("live while pending");
        let at = text.line_to_char(text.char_to_line(start) + first) + 1;
        // Scrolled to the header first, so coming from below shows it.
        let (window, buffers) = (&mut self.core.windows.window, &mut self.core.buffers);
        (window.set_point(buffers, start)).expect("in the text");
        window.scroll(buffers, window.rows());
        self.goto(at)
    }

    /// `1` to `9` and `o` in a session: on a pending question's lines a digit answers it with
    /// that option and `o` reads an own answer; elsewhere they insert themselves.
    pub(crate) fn agent_question_key(&mut self, args: Args) -> Result<(), String> {
        let key = match self.core.this_keys.last().map(|key| key.code) {
            Some(crate::key::KeyCode::Char(c)) => c,
            _ => return Err("agent-question-key must be run by a character key".into()),
        };
        let asked = self.ask_at_point().and_then(|(id, at, _)| {
            let question = self.core.agents[&id].ask(at).question()?;
            let field = question.field();
            Some((
                id,
                at,
                field.map_or(0, |f| f.options.len()),
                field.is_some_and(|f| f.own.is_some()),
            ))
        });
        match (asked, key) {
            (Some((id, at, _, own)), 'o') => {
                if own {
                    self.own_answer(id, at);
                }
            }
            (Some((id, at, options, _)), digit) => {
                let n = digit.to_digit(10).map_or(0, |d| d as usize);
                if (1..=options).contains(&n) {
                    self.pick(id, at, n - 1);
                }
            }
            (None, _) => self.call("self-insert-command", args),
        }
        Ok(())
    }

    /// Answers session `id`'s pending question `at` with option `option`, or ticks it in a
    /// multi-select.
    pub(super) fn pick(&mut self, id: AgentId, at: BlockPath, option: usize) {
        self.choose(id, at, Some(option), questions::Answer::Option(option));
    }

    /// Ticks option `tick` (the own answer with `None`) of session `id`'s pending question
    /// `at` when its open field is a multi-select, records `answer` in a block of several, and
    /// otherwise answers with it.
    fn choose(
        &mut self,
        id: AgentId,
        at: BlockPath,
        tick: Option<usize>,
        answer: questions::Answer,
    ) {
        let question = self.core.agents[&id]
            .ask(at)
            .question()
            .expect("a question");
        if question.field().is_some_and(|f| f.multi.is_some()) {
            let own = match answer {
                questions::Answer::Own(text) => Some(text),
                _ => None,
            };
            self.tick_question(id, at, tick, own);
        } else if question.several() {
            self.record_answer(id, at, answer);
        } else {
            self.answer_question(id, at, answer);
        }
    }

    /// Reads an own answer to session `id`'s pending question `at`, starting from the one it has.
    pub(super) fn own_answer(&mut self, id: AgentId, at: BlockPath) {
        let question = self.core.agents[&id].ask(at).question();
        let own = (question.and_then(|q| q.field()?.own_text())).map(str::to_owned);
        self.call("agent-answer-own", Args::default());
        if let Some(own) = own {
            _ = self.set_text(&own);
        }
    }

    /// Whether session `id`'s pending request `at` is a question `C-c C-y` sends: a multi-select
    /// or a block of several.
    fn sendable(&self, id: AgentId, at: BlockPath) -> bool {
        (self.core.agents[&id].ask(at).question())
            .is_some_and(|q| q.several() || q.field().is_some_and(|f| f.multi.is_some()))
    }

    /// `agent-answer-own`: answers the question at point with `text`, in its adapter's shape.
    pub(crate) fn agent_answer_own(&mut self, text: &str) -> Result<(), String> {
        let (id, at) = self
            .ask_at_point()
            .filter(|&(id, at, _)| {
                (self.core.agents[&id].ask(at).question())
                    .is_some_and(|q| q.field().is_some_and(|f| f.own.is_some()))
            })
            .map(|(id, at, _)| (id, at))
            .ok_or("No question here")?;
        if text.trim().is_empty() {
            // An empty answer clears one kept while the block waits; a lone question has none.
            let question = self.core.agents[&id]
                .ask(at)
                .question()
                .expect("a question");
            if question.field().is_some_and(|f| f.multi.is_some()) {
                self.tick_question(id, at, None, None);
            } else if question.several() {
                let Body::Question(q) = &mut self
                    .core
                    .agents
                    .get_mut(&id)
                    .expect("live")
                    .ask_mut(at)
                    .body
                else {
                    unreachable!("a question");
                };
                q.field_mut().expect("read in the open field").chosen = None;
                self.redraw_question(id, at);
            } else {
                return Err("Answer is empty".into());
            }
            return Ok(());
        }
        self.choose(id, at, None, questions::Answer::Own(text.trim().into()));
        Ok(())
    }

    /// The session, pending request and line of its block point is on.
    pub(super) fn ask_at_point(&self) -> Option<(AgentId, BlockPath, usize)> {
        let core = &*self.core;
        let (point, buffer) = (core.point(), core.windows.window.buffer());
        let b = core.buffers.get(buffer)?;
        let (id, session) = core.session_of(buffer)?;
        session.asks().find_map(|(at, _)| {
            let Range { start, end } = session.range(b, at);
            if !(start..=end).contains(&point) {
                return None;
            }
            let line = b.text().slice(start..point).chars().filter(|&c| c == '\n');
            Some((id, at, line.count()))
        })
    }
}
