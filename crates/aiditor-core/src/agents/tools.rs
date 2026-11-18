//! Tool calls: each one's block in the session, its output, and the terminal a command's call
//! runs in.

use super::*;

/// A tool call's block: a header line, then its content when it has any.
pub(super) struct Tool {
    /// The call, with every update merged in.
    pub(super) call: ToolCall,
    /// The header's length in chars.
    pub(super) header: usize,
    /// Whether its content shows.
    pub(super) open: bool,
    /// When it arrived; `None` when replayed.
    pub(super) arrived: Option<Instant>,
    /// Its closed permission waits.
    pub(super) waited: Duration,
    /// How long it ran, once it finished.
    ran: Option<Duration>,
    /// When it first finished.
    pub(super) ended: Option<Instant>,
    /// The group it is a row of.
    pub(super) group: Option<usize>,
    /// Its time: at the right of its header in a group, else on the line under its block.
    pub(super) time: String,
    /// The terminal it announced, whose output it shows.
    pub(super) term: Option<Term>,
    /// Its terminal changed since the block was last rendered.
    pub(super) dirty: bool,
    /// The faces of the SGR escapes and diffs in its content, from the content's start.
    pub(super) faces: Faces,
    /// Where a command's output lines start in its content.
    pub(super) output: Option<usize>,
    /// The buffer showing its full output.
    output_buffer: Option<BufferId>,
    /// The indexed file its header links to, and the line, from 1.
    pub(super) link: Option<(PathBuf, Option<usize>)>,
    /// A closed question's line stands for it, so it shows no row.
    pub(super) replaced: bool,
}

/// A command's terminal, from the `_meta` extension (#100).
pub(super) struct Term {
    pub(super) id: String,
    pub(super) sgr: Sgr,
    processor: Processor,
    rendered: Option<TermRender>,
    /// `exit <code>` or `signal <name>` once it failed.
    pub(super) exit: Option<String>,
}

struct TermRender {
    bytes: usize,
    chars: usize,
    at: usize,
    pub(super) faces: usize,
    fence: usize,
}

/// The terminal a call announces: its `_meta.terminal_info`, else its first terminal item.
fn terminal_id(call: &ToolCall) -> Option<String> {
    let info = (call.meta.as_ref()).and_then(|m| m.get("terminal_info")?["terminal_id"].as_str());
    let item = call.content.iter().find_map(|item| match item {
        ToolCallContent::Terminal(t) => Some(&*t.terminal_id.0),
        _ => None,
    });
    info.or(item).map(Into::into)
}

/// The shown call whose terminal `value`'s `terminal_id` names.
fn terminal_tool<'a>(
    tools: &'a mut HashMap<ToolCallId, Tool>,
    value: &Value,
) -> Option<&'a mut Tool> {
    let id = value["terminal_id"].as_str()?;
    (tools.values_mut()).find(|t| t.term.as_ref().is_some_and(|t| t.id == id))
}

impl Core {
    /// Merges a `tool_call_update` into its call, or starts a call from one with a title.
    pub(super) fn tool_update(&mut self, id: AgentId, update: ToolCallUpdate) {
        let known = (self.agents.get(&id))
            .and_then(|s| s.tools.get(&update.tool_call_id))
            .map(|t| t.call.clone());
        let call = match known {
            Some(mut call) => {
                call.update(update.fields);
                call
            }
            None => match ToolCall::try_from(update) {
                Ok(call) => call,
                Err(_) => return,
            },
        };
        self.show_tool(id, call);
    }

    /// Puts `call` above session `id`'s prompt line as a new block, or rewrites its block in
    /// place, keeping whether it is folded.
    pub(super) fn show_tool(&mut self, id: AgentId, call: ToolCall) {
        let known =
            (self.agents.get(&id)).is_some_and(|s| s.tools.contains_key(&call.tool_call_id));
        let new = !known;
        if new {
            self.end_stream(id);
        }
        let Some(session) = self.agents.get_mut(&id) else {
            return;
        };
        let Some(buffer) = self.buffers.get_mut(session.buffer) else {
            return;
        };
        let term = (session.tools.get_mut(&call.tool_call_id))
            .and_then(|t| t.term.take())
            .or_else(|| {
                terminal_id(&call).map(|id| Term {
                    id,
                    sgr: Sgr::default(),
                    processor: Processor::new(),
                    rendered: None,
                    exit: None,
                })
            });
        let now = self.now.expect("set by handle");
        let known = session.tools.get(&call.tool_call_id);
        let arrived = known.map_or(session.live().then_some(now), |t| t.arrived);
        let waited = known.map_or(Duration::ZERO, |t| t.waited);
        let asked = (session.asks()).find(|(_, a)| a.tool.as_ref() == Some(&call.tool_call_id));
        let waiting = asked.map(|(_, a)| now - a.since);
        let done = finished(call.status);
        let ended = known.and_then(|t| t.ended).or(done.then_some(now));
        let group = known.map_or(session.group, |t| t.group);
        let ran = known.and_then(|t| t.ran).or_else(|| {
            let spent = (now - arrived?).checked_sub(waited + waiting.unwrap_or_default());
            done.then(|| spent.unwrap_or_default())
        });
        let exit = term.as_ref().and_then(|t| t.exit.clone());
        let time = match (waiting, exit, ran) {
            (Some(wait), ..) => waiting_text(wait),
            (None, Some(exit), _) => exit,
            (None, None, Some(ran)) => short_duration(ran),
            (None, None, None) => String::new(),
        };
        let content = tool_content(&call, term.as_ref());
        let index = self.index.get(&session.project);
        let link = tool_path(&call, term.as_ref())
            .and_then(|(path, line)| Some((index?.file(path)?, line.map(|l| l as usize))));
        let call_id = call.tool_call_id.clone();
        let tool = Tool {
            call,
            header: 0,
            open: false,
            arrived,
            waited,
            ran,
            ended,
            group,
            time,
            term,
            dirty: false,
            faces: content.faces,
            output: content.output,
            output_buffer: None,
            link,
            replaced: false,
        };
        let mut tool = match session.tools.remove(&call_id) {
            Some(old) => Tool {
                open: old.open,
                output_buffer: old.output_buffer,
                replaced: old.replaced,
                ..tool
            },
            None => tool,
        };
        let header = session.tool_header(&tool).chars().count();
        let len = header + content.text.chars().count() + usize::from(!content.text.is_empty());
        tool.header = header;
        let call = &tool.call;
        if let Some(term) = &mut tool.term {
            let output = term.sgr.text.strip_suffix('\n').unwrap_or(&term.sgr.text);
            let command = tool_command(call, Some(&*term)).map_or(0, fence_len);
            let fence = (term.sgr.backticks.max(2) + 1).max(command);
            let chars = term.sgr.chars - usize::from(term.sgr.text.ends_with('\n'));
            term.rendered = (!term.sgr.text.is_empty()).then(|| TermRender {
                bytes: output.len(),
                chars,
                at: len - header - 1 - fence - 1 - chars,
                faces: term.sgr.faces.len(),
                fence,
            });
        }
        let rendered = session.render_tool(&tool, &content.text);
        let replaced = tool.replaced;
        session.tools.insert(call_id.clone(), tool);
        let path = match new {
            false => session.tool_path(&call_id),
            true => session.push(buffer, "", BlockKind::Tool(call_id.clone()), true),
        };
        session.write(buffer, path, rendered, &Look::at(now));
        if replaced && group.is_none_or(|g| session.group_open(g)) {
            let block = session.block(path);
            let range = block.range(buffer);
            hide_replaced(buffer, &session.tools[&call_id], range, block.meta);
        }
        if let Some(g) = group {
            session.show_group(buffer, g);
        }
        if new && session.forms_group() {
            session.form_group(buffer);
            let g = session.group.expect("just formed");
            for call in session.group_calls(g) {
                self.show_tool(id, self.agents[&id].tools[&call].call.clone());
            }
            if let Some((session, buffer)) = self.session_buffer(id) {
                session.render_group(buffer, g);
            }
        }
    }

    /// Appends the command output in `meta` to the terminals it names and records their exit,
    /// for their calls to show with the next frame; drops what names no shown call's terminal.
    pub(super) fn terminal_meta(&mut self, id: AgentId, meta: Option<&Map<String, Value>>) {
        let (Some(session), Some(meta)) = (self.agents.get_mut(&id), meta) else {
            return;
        };
        for key in ["terminal_output", "terminal_output_delta"] {
            let Some(value) = meta.get(key) else {
                continue;
            };
            if let (Some(tool), Some(data)) = (
                terminal_tool(&mut session.tools, value),
                value["data"].as_str(),
            ) {
                let term = tool.term.as_mut().expect("found by it");
                term.processor.advance(&mut term.sgr, data.as_bytes());
                tool.dirty = true;
            }
        }
        if let Some(value) = meta.get("terminal_exit")
            && let Some(tool) = terminal_tool(&mut session.tools, value)
        {
            let term = tool.term.as_mut().expect("found by it");
            term.rendered = None;
            term.exit = match (value["signal"].as_str(), value["exit_code"].as_i64()) {
                (Some(signal), _) => Some(format!("signal {signal}")),
                (None, Some(code)) if code != 0 => Some(format!("exit {code}")),
                _ => None,
            };
            tool.dirty = true;
        }
    }

    /// Appends a command's new output to its block: a folded call's, or an open call's past the
    /// cut, into its hidden lines; false for the rest, which `show_tool` rebuilds.
    pub(super) fn append_tool_output(&mut self, id: AgentId, call: &ToolCallId) -> bool {
        let now = self.now();
        let session = self.agents.get_mut(&id).expect("known session");
        let path = session.tool_path(call);
        let buffer = self
            .buffers
            .get_mut(session.buffer)
            .expect("session buffer");
        let Range { start, end } = session.range(buffer, path);
        let tool = session.tools.get_mut(call).expect("dirty call");
        let Some(term) = &mut tool.term else {
            return false;
        };
        let Some(rendered) = &mut term.rendered else {
            return false;
        };
        if term.sgr.backticks >= rendered.fence {
            return false;
        }
        let output = term.sgr.text.strip_suffix('\n').unwrap_or(&term.sgr.text);
        let tail = &output[rendered.bytes..];
        let chars = term.sgr.chars - usize::from(term.sgr.text.ends_with('\n'));
        let at = end - rendered.fence - 1;
        // An open call's hidden lines end where its output does, under the `… N more lines` fold.
        let fold = match tool.open {
            true => match buffer.fold_ending_at(at) {
                Some(fold) => Some(fold),
                None => return false,
            },
            false => None,
        };
        // The closing lines keep their text and properties, past the new output.
        let shift = tail.chars().count();
        let closing = (buffer.properties_near(at..end).into_iter())
            .filter(|(r, _)| r.start >= at && r.end <= end)
            .map(|(r, p)| (r.start - at + shift..r.end - at + shift, p.clone()))
            .collect();
        let text = format!("{tail}{}", buffer.text().slice(at..end));
        let grew = chars - rendered.chars;
        let keep = rendered.faces.saturating_sub(1);
        tool.faces
            .truncate(tool.faces.len() - rendered.faces + keep);
        tool.faces.extend(
            term.sgr.faces[keep..]
                .iter()
                .map(|(r, f)| (rendered.at + r.start..rendered.at + r.end, f.clone())),
        );
        (rendered.bytes, rendered.chars, rendered.faces) =
            (output.len(), chars, term.sgr.faces.len());
        tool.dirty = false;
        let tail = Rendered {
            from: at - start,
            text,
            props: closing,
            meta: None,
        };
        session.put(buffer, path, tail);
        if let Some((fold, from)) = fold {
            let text = buffer.text();
            let lines = text.char_to_line(at + grew) - text.char_to_line(from) + 1;
            buffer.extend_fold(fold, at + grew, more_lines(lines));
        }
        #[cfg(debug_assertions)]
        session.check(buffer, path, &Look::at(now));
        #[cfg(not(debug_assertions))]
        let _ = now;
        true
    }
}

impl Ctx<'_> {
    /// The tool call whose header point is on.
    pub(super) fn tool_at_point(&self) -> Option<(AgentId, ToolCallId)> {
        let core = &*self.core;
        let point = core.point();
        let buffer = core.windows.window.buffer();
        let b = core.buffers.get(buffer)?;
        let (id, session) = core.session_of(buffer)?;
        Some((id, session.tool_at(b, point)?.clone()))
    }

    /// Shows `call`'s full output in its own read-only buffer, refreshed from the call.
    // shortcut: a snapshot, not following a running command; RET refreshes it.
    pub(super) fn show_tool_output(
        &mut self,
        id: AgentId,
        call: &ToolCallId,
    ) -> Result<(), String> {
        let core = &mut *self.core;
        let workspace = core.current_workspace()?.name.clone();
        let session = core.agents.get_mut(&id).expect("found by the caller");
        let tool = session.tools.get_mut(call).expect("found by the caller");
        let term = tool.term.as_ref();
        let (text, faces) = tool_output(&tool.call, term).ok_or("No output")?;
        let target = session_title(Some(&tool_target(&tool.call, term, &session.root)));
        let found = tool
            .output_buffer
            .filter(|b| core.buffers.get(*b).is_some());
        let buffer = found.unwrap_or_else(|| {
            let name = format!("*{} output: {target}*<{workspace}>", session.agent);
            let id = core.buffers.create(&name);
            core.buffers.get_mut(id).expect("just created").read_only = true;
            id
        });
        tool.output_buffer = Some(buffer);
        let b = core.buffers.get_mut(buffer).expect("live");
        b.set_text(&text);
        b.remove_properties(0..b.len_chars());
        for (r, face) in faces {
            (b.add_property(r, Prop::Named(face))).expect("inside the text");
        }
        self.display_buffer(buffer);
        Ok(())
    }

    /// `agent-toggle-tool-call`: shows or hides the content of the tool call or thought row whose
    /// header point is on, or hides that of the open one point is in, moving point to its header.
    /// Elsewhere in a group, it opens or folds the group. In a block of several questions, it
    /// opens the next one.
    pub(crate) fn agent_toggle_tool_call(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let (point, look) = (core.point(), Look::at(core.now()));
        let buffer = core.windows.window.buffer();
        let agent = core.buffers.get(buffer).and_then(Buffer::agent);
        let session = agent.and_then(|id| Some((id, core.agents.get_mut(&id)?)));
        let b = core
            .buffers
            .get_mut(buffer)
            .expect("current buffer is live");
        let (id, session) = session.ok_or("No tool call here")?;
        // Point past the line it ends on goes back to its start.
        let back = |(start, line): (usize, usize)| (point > line).then_some(start);
        let to = match session.block_at(b, point).ok_or("No tool call here")? {
            At::Ask(at) => {
                self.open_question(id, at, |q| {
                    q.open = Some(q.open.map_or(0, |i| (i + 1) % q.fields.len()))
                });
                return Ok(());
            }
            At::Paste(i) => Some(session.toggle_paste(b, i)),
            At::Answer(at) => back(session.toggle_answer(b, at, &look)),
            At::Thought(at) => back(session.toggle_thought(b, at)),
            At::Group(g) => match session.toggle_group(b, g) {
                (start, line, false) => back((start, line)),
                _ => None,
            },
            At::Tool(call) => {
                let path = session.tool_path(&call);
                let range = session.range(b, path);
                let tool = session.tools.get_mut(&call).expect("just found");
                // Without content there is nothing to show, so it stays folded for when content
                // comes.
                tool.open = !tool.open && range.len() > tool.header;
                let mut rendered = session.render(path, &look);
                rendered.meta = None;
                session.write(b, path, rendered, &look);
                back((range.start, range.start + session.tools[&call].header))
            }
        };
        match to {
            Some(start) => self.goto(start),
            None => Ok(()),
        }
    }
}
