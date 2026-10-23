//! Prompts read in the minibuffer, with history and completion.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use nucleo::Nucleo;
use nucleo::pattern::{CaseMatching, Normalization};

use crate::buffer::{BufferId, file_name};
use crate::command::{Args, Command, Preview};
use crate::frame::window_display;
use crate::timer::TimerId;
use crate::window::{Window, WindowId};
use crate::{Core, Ctx, Effect, Effects, FileOp, Msg, PreviewRequest, display};

/// How many answers each prompt's history keeps, Emacs's `history-length`.
const HISTORY_MAX: usize = 100;

/// How many candidates the completion list shows, Vertico's `vertico-count`.
const COMPLETION_ROWS: usize = 10;

/// How long the core waits for the matcher before trying again on a timer.
const REPOLL: Duration = Duration::from_millis(16);

/// The minibuffer's window, kept outside the split tree.
const MINIBUFFER: WindowId = WindowId(usize::MAX);

/// A prompt waiting in the minibuffer for the command that asked.
pub(crate) struct Minibuffer {
    session: u64,
    command: &'static str,
    /// The args so far; the answer becomes the next string.
    args: Args,
    prompt: String,
    /// The window to select again on closing.
    from: WindowId,
    /// The history entry shown, 1 the newest; 0 is the text typed before moving.
    index: usize,
    /// What was typed before the first history move.
    input: String,
    /// The completion list, when the command completes over candidates.
    completion: Option<Completion>,
    /// Whether the answer joins the prompt's history; a handler's own question keeps none.
    history: bool,
    /// The window it was opened from, as it was, while a preview stands in for it.
    saved: Option<Window>,
    /// What was last previewed; `None` before any and with no selection.
    previewed: Option<Preview>,
    /// The file whose read for a preview is awaited.
    reading: Option<PreviewRequest>,
    /// The hidden buffer a previewed file was read into, killed on closing.
    peek: Option<BufferId>,
}

/// The selection in a completing minibuffer's list of matches.
struct Completion {
    /// The text the matcher's pattern was last set from.
    pattern: String,
    /// The selected match, best first.
    selected: usize,
    /// The first match shown, so the shown rows hold the selection.
    top: usize,
    /// The most rows shown since the minibuffer opened, Emacs's `grow-only` mini-window.
    rows: usize,
    /// For file names, the text through its last `/`: the directory listed or awaited.
    dir: Option<String>,
    /// Whether `dir`'s listing arrived, so a second answer is dropped.
    listed: bool,
    /// The directory a relative `dir` is listed from, as `find-file` visits from it.
    base: PathBuf,
    /// The command's own matches, in place of the matcher's.
    filtered: Option<Filtered>,
}

/// A command's candidates matched by its spec's `filter`.
struct Filtered {
    items: Vec<String>,
    notes: Vec<String>,
    /// The indices of the matching items, in the filter's order.
    matched: Vec<usize>,
    /// The item last selected, kept while nothing matches.
    anchor: usize,
}

/// The open prompt, each prompt's history and the completion matcher.
#[derive(Default)]
pub(crate) struct Prompts {
    sessions: u64,
    preview_revision: u64,
    active_preview: Option<PreviewRequest>,
    pending_preview: Option<PreviewRequest>,
    /// The prompt being answered, while the minibuffer is open.
    pub(crate) minibuffer: Option<Minibuffer>,
    /// Each prompt's answers, newest first.
    history: HashMap<String, Vec<String>>,
    /// The fuzzy matcher for completion, made on first use.
    pub(crate) matcher: Option<Nucleo<(String, String)>>,
    /// The timer that polls the matcher again while it is still matching.
    repoll: Option<TimerId>,
    /// The text of the last minibuffer closed, as ivy's `ivy-text` outlives the read.
    pub(crate) last_text: String,
}

impl Prompts {
    /// Whether the open prompt completes directories only.
    fn directories(&self, commands: &HashMap<&'static str, Command>) -> bool {
        self.minibuffer.as_ref().is_some_and(|mb| {
            mb.completion.as_ref().is_some_and(|c| c.dir.is_some())
                && commands[mb.command].spec.directories
        })
    }

    /// How many candidates match, 0 when not completing.
    fn matches(&self) -> usize {
        let completion = self
            .minibuffer
            .as_ref()
            .and_then(|mb| mb.completion.as_ref());
        match (completion, &self.matcher) {
            (
                Some(Completion {
                    filtered: Some(f), ..
                }),
                _,
            ) => f.matched.len(),
            (Some(_), Some(m)) => m.snapshot().matched_item_count() as usize,
            _ => 0,
        }
    }

    /// Adds `text` to the newest end of `prompt`'s history, unless empty or the newest already.
    fn remember(&mut self, prompt: &str, text: String) {
        let history = self.history.entry(prompt.to_string()).or_default();
        if !text.is_empty() && history.first() != Some(&text) {
            history.insert(0, text);
            history.truncate(HISTORY_MAX);
        }
    }

    /// The directory a file prompt's candidates are relative to.
    pub(crate) fn completion_base(&self) -> Option<&Path> {
        let completion = self.minibuffer.as_ref()?.completion.as_ref()?;
        completion
            .dir
            .is_some()
            .then_some(completion.base.as_path())
    }

    /// The command the open prompt answers.
    pub(crate) fn command(&self) -> Option<&'static str> {
        self.minibuffer.as_ref().map(|mb| mb.command)
    }

    /// The selected candidate, if the minibuffer is completing and anything matches.
    pub(crate) fn selected_candidate(&self) -> Option<String> {
        let completion = self.minibuffer.as_ref()?.completion.as_ref()?;
        if let Some(f) = &completion.filtered {
            let last = f.matched.len().checked_sub(1)?;
            return Some(f.items[f.matched[completion.selected.min(last)]].clone());
        }
        let snapshot = self.matcher.as_ref()?.snapshot();
        let last = snapshot.matched_item_count().checked_sub(1)?;
        let item = snapshot.get_matched_item((completion.selected as u32).min(last))?;
        let name = &item.data.0;
        Some(match &completion.dir {
            Some(dir) => format!("{dir}{name}"),
            None => name.clone(),
        })
    }
}

impl Completion {
    /// Selects match `selected`, scrolling the shown rows only as far as needed.
    fn select(&mut self, selected: usize) {
        self.selected = selected;
        self.top = self
            .top
            .min(selected)
            .max((selected + 1).saturating_sub(COMPLETION_ROWS));
    }
}

impl Ctx<'_> {
    /// Adds `dir`'s listing to the completion that awaits it, or shows why it failed.
    pub(crate) fn on_listed(&mut self, dir: PathBuf, result: Result<Vec<String>, String>) {
        let core = &mut *self.core;
        let awaited = (core.prompts.minibuffer.as_mut())
            .and_then(|mb| mb.completion.as_mut())
            .filter(|c| !c.listed && c.dir.as_ref().map(|d| c.base.join(d)) == Some(dir.clone()));
        let Some(completion) = awaited else {
            return;
        };
        completion.listed = true;
        let directories = core.prompts.directories(&core.commands);
        match result {
            Ok(names) => {
                let matcher = core
                    .prompts
                    .matcher
                    .as_ref()
                    .expect("completing has a matcher");
                let injector = matcher.injector();
                // A directory prompt offers the listed directory itself first, as `./`.
                let here = directories.then(|| "./".to_string());
                let names = names
                    .into_iter()
                    .filter(|n| !directories || n.ends_with('/'));
                for name in here.into_iter().chain(names) {
                    injector.push((name, String::new()), |(c, _), cols| {
                        cols[0] = c.as_str().into()
                    });
                }
            }
            Err(err) => core.echo(err),
        }
    }

    /// Opens the minibuffer at `prompt`; its answer then calls `command` again. `choices`, for
    /// a handler's own question, replace the spec's completion and keep the answer out of history.
    pub(crate) fn read_string(
        &mut self,
        command: Command,
        args: Args,
        prompt: String,
        choices: Option<&[&str]>,
    ) -> Result<(), String> {
        let core = &mut *self.core;
        if core.prompts.minibuffer.is_some() {
            return Err("Command attempted to use minibuffer while in minibuffer".into());
        }
        let base = core.directory();
        let files = command.spec.files == Some(args.strings.len());
        let start = files.then(|| base.join("").to_string_lossy().into_owned());
        let mut candidates = match choices {
            Some(choices) => Some(choices.iter().map(|c| c.to_string()).collect()),
            // No candidates is a plain prompt, so one command can mix completing and free text.
            None => (command.spec.completion)
                .map(|candidates| candidates(core, &args))
                .filter(|c: &Vec<String>| !c.is_empty()),
        };
        let note = |core: &Core, c: &String| {
            (command.spec.annotation.and_then(|f| f(core, c))).unwrap_or_default()
        };
        let filtered = match (command.spec.filter, choices) {
            // A filtering prompt lists even no candidates, so it still requires a match.
            (Some(filter), None) => {
                let items = candidates.take().unwrap_or_default();
                Some(Filtered {
                    notes: items.iter().map(|c| note(core, c)).collect(),
                    matched: filter("", &items),
                    items,
                    anchor: 0,
                })
            }
            _ => None,
        };
        let preselect = command.spec.preselect.map_or(0, |f| f(core, &args));
        // A filtering command took its candidates; the rest go to the matcher.
        let matching = candidates.is_some() || start.is_some();
        if matching {
            // One matcher for the core, as each one starts a thread pool.
            let matcher = core.prompts.matcher.get_or_insert_with(|| {
                Nucleo::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 1)
            });
            matcher.restart(true);
            (matcher.pattern).reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
            let injector = matcher.injector();
            for c in candidates.into_iter().flatten() {
                let note = note(core, &c);
                injector.push((c, note), |(c, _), cols| cols[0] = c.as_str().into());
            }
        }
        let completion = (matching || filtered.is_some()).then(|| {
            let mut completion = Completion {
                pattern: String::new(),
                selected: 0,
                top: 0,
                rows: 0,
                // Nothing listed yet; `poll_completion` asks for the start directory.
                dir: files.then(String::new),
                listed: false,
                base,
                filtered,
            };
            completion.select(preselect);
            completion
        });
        // A fresh buffer per prompt, so undo and markers never carry over.
        let id = core.buffers.create(" *Minibuf-1*");
        let buffer = core.buffers.get_mut(id).expect("just created");
        buffer.major_mode = "minibuffer-mode";
        if completion.is_some() {
            buffer.minor_modes.push("minibuffer-completion");
        }
        // So a keymap named after the command binds keys at its prompts, as ivy's `:keymap`.
        buffer.minor_modes.push(command.name);
        let window = Window::new(&mut core.buffers, id);
        core.windows.others.insert(MINIBUFFER, window);
        core.prompts.sessions += 1;
        core.prompts.minibuffer = Some(Minibuffer {
            session: core.prompts.sessions,
            command: command.name,
            args,
            prompt,
            from: core.windows.selected,
            index: 0,
            input: String::new(),
            completion,
            history: choices.is_none(),
            saved: None,
            previewed: None,
            reading: None,
            peek: None,
        });
        core.select(MINIBUFFER);
        match start {
            Some(text) => self.set_text(&text),
            None => Ok(()),
        }
    }

    /// Closes the minibuffer and calls its command with the selected candidate, or with `input`
    /// or no match the text, as the next string. A selected directory is entered instead, except
    /// in a directory prompt, which returns it.
    pub(crate) fn exit_minibuffer(&mut self, input: bool) -> Result<(), String> {
        let core = &*self.core;
        let directories = core.prompts.directories(&core.commands);
        let selected = (!input)
            .then(|| self.core.prompts.selected_candidate())
            .flatten();
        let completion = self
            .core
            .prompts
            .minibuffer
            .as_ref()
            .and_then(|mb| mb.completion.as_ref());
        let filtered = completion.is_some_and(|c| c.filtered.is_some());
        if filtered && selected.is_none() {
            return Err("Match required".into());
        }
        if completion.is_some_and(|c| c.dir.is_some())
            && !directories
            && let Some(dir) = selected.as_ref().filter(|s| s.ends_with('/'))
        {
            return self.set_text(dir);
        }
        let text = selected.unwrap_or_else(|| self.core.text());
        let mb = self.core.close_minibuffer().ok_or("Not in a minibuffer")?;
        // A filtering prompt's typed text was remembered on closing.
        if mb.history && !filtered {
            self.core.prompts.remember(&mb.prompt, text.clone());
        }
        let mut args = mb.args;
        args.strings.push(text);
        self.core.this_command = Some(mb.command);
        self.call(mb.command, args);
        Ok(())
    }

    /// Replaces the minibuffer's text with the history entry `n` older, newer when negative.
    pub(crate) fn history_element(&mut self, n: i64) -> Result<(), String> {
        let text = self.core.text();
        let prompts = &mut self.core.prompts;
        let mb = prompts.minibuffer.as_mut().ok_or("Not in a minibuffer")?;
        let items = prompts
            .history
            .get(&mb.prompt)
            .map_or(&[][..], Vec::as_slice);
        let target = (mb.index as i64).saturating_add(n);
        if target > items.len() as i64 {
            return Err("Beginning of history; no preceding item".into());
        }
        if target < 0 {
            return Err("End of history; no default available".into());
        }
        if mb.index == 0 {
            mb.input = text;
        }
        mb.index = target as usize;
        let new = match mb.index {
            0 => mb.input.clone(),
            i => items[i - 1].clone(),
        };
        self.set_text(&new)
    }

    /// Replaces the current buffer's text with `new`, point at its end.
    pub(crate) fn set_text(&mut self, new: &str) -> Result<(), String> {
        let buffer = self.core.current_mut();
        let len = buffer.len_chars();
        buffer.delete(0..len)?;
        buffer.insert(0, new)?;
        self.goto(new.chars().count())
    }

    /// Moves the completion selection `n` matches down, up when negative, stopping at the ends.
    pub(crate) fn next_completion(&mut self, n: i64) -> Result<(), String> {
        let count = self.core.prompts.matches();
        let completion = (self.core.prompts.minibuffer.as_mut())
            .and_then(|mb| mb.completion.as_mut())
            .ok_or("Not completing")?;
        if count == 0 {
            return Err("No match".into());
        }
        let target = (completion.selected as i64).saturating_add(n);
        completion.select(target.clamp(0, count as i64 - 1) as usize);
        Ok(())
    }

    /// Fails with `err` while the minibuffer is selected.
    pub(crate) fn outside_minibuffer(&self, err: &str) -> Result<(), String> {
        match self.core.windows.selected == MINIBUFFER {
            true => Err(err.into()),
            false => Ok(()),
        }
    }
}

impl Core {
    /// Matches the completing minibuffer's text against its candidates, waiting up to 10 ms,
    /// and polls again on a timer while the matcher is still running. A file prompt matches the
    /// text after the last `/`, asking for a listing when the part before it changes.
    pub(crate) fn poll_completion(&mut self, now: Instant, effects: &mut Effects) {
        self.poll_grep(now, effects);
        // Checked before copying the text: this runs after every message.
        if !self
            .prompts
            .minibuffer
            .as_ref()
            .is_some_and(|mb| mb.completion.is_some())
        {
            return;
        }
        let text = self.text();
        let Some(mb) = self.prompts.minibuffer.as_mut() else {
            return;
        };
        let Some(completion) = mb.completion.as_mut() else {
            return;
        };
        if let Some(f) = &mut completion.filtered {
            if completion.pattern != text {
                if let Some(last) = f.matched.len().checked_sub(1) {
                    f.anchor = f.matched[completion.selected.min(last)];
                }
                let filter = self.commands[mb.command].spec.filter;
                f.matched = filter.expect("filtered by the spec")(&text, &f.items);
                // ivy's swiper rule: the selected item if it still matches, else the next one.
                let next = f.matched.iter().position(|&i| i >= f.anchor);
                let next = next.unwrap_or(f.matched.len().saturating_sub(1));
                completion.select(next);
                completion.pattern = text;
            }
            return;
        }
        let matcher = self
            .prompts
            .matcher
            .as_mut()
            .expect("a completing minibuffer has a matcher");
        let (mut pattern, mut fresh) = (text.as_str(), false);
        if let Some(dir) = &mut completion.dir {
            let (head, tail) = text.split_at(text.rfind('/').map_or(0, |i| i + 1));
            pattern = tail;
            if head != dir {
                *dir = head.to_string();
                completion.listed = false;
                fresh = true;
                matcher.restart(true);
                if !head.is_empty() {
                    effects.push(Effect::File(FileOp::ListDir(completion.base.join(head))));
                }
            }
        }
        if fresh || completion.pattern != pattern {
            // Text that extends the old pattern lets nucleo narrow the last matches.
            let append = !fresh && pattern.starts_with(&completion.pattern);
            (matcher.pattern).reparse(
                0,
                pattern,
                CaseMatching::Smart,
                Normalization::Smart,
                append,
            );
            completion.pattern = pattern.to_string();
            completion.select(0);
        }
        if let Some(id) = self.prompts.repoll.take() {
            self.timers.cancel(id);
        }
        if matcher.tick(10).running {
            self.prompts.repoll = Some(self.timers.start(now, REPOLL, None, Msg::Tick));
        }
    }

    /// In a file prompt with point at the end of a text that ends in `/` or names a listed
    /// directory, the start of that last component: where `DEL` deletes back to.
    pub(crate) fn component_start(&self) -> Option<usize> {
        let completion = self.prompts.minibuffer.as_ref()?.completion.as_ref()?;
        let (dir, matcher) = (completion.dir.as_ref()?, self.prompts.matcher.as_ref()?);
        let text = self.text();
        if self.point() != text.chars().count() {
            return None;
        }
        let component = match text.strip_suffix('/') {
            Some(component) => component,
            None => {
                // The listing already loaded says whether the last component is a directory.
                let name = format!("{}/", text.strip_prefix(dir.as_str())?);
                let injector = matcher.injector();
                let listed = (0..injector.injected_items())
                    .any(|i| injector.get(i).is_some_and(|item| item.data.0 == name));
                listed.then_some(text.as_str())?
            }
        };
        let start = component.rfind('/')? + 1;
        Some(text[..start].chars().count())
    }

    /// The open minibuffer's prompt and the line holding its point.
    pub(crate) fn minibuffer(&mut self) -> Option<display::Minibuffer> {
        let mb = self.prompts.minibuffer.as_mut()?;
        let prompt = mb.prompt.to_string();
        let (mut candidates, mut annotations, mut selected) = (Vec::new(), Vec::new(), None);
        let (mut count, mut shown) = (String::new(), 0);
        if let Some(completion) = &mut mb.completion {
            let range = |matched: usize| {
                completion.top.min(matched)..(completion.top + COMPLETION_ROWS).min(matched)
            };
            let (matched, rows) = match (&completion.filtered, self.prompts.matcher.as_mut()) {
                (Some(f), _) => {
                    let rows = range(f.matched.len());
                    let items = f.matched[rows.clone()].iter();
                    (candidates, annotations) = items
                        .map(|&i| (f.items[i].clone(), f.notes[i].clone()))
                        .unzip();
                    (f.matched.len(), rows)
                }
                (None, Some(matcher)) => {
                    matcher.tick(0);
                    let snapshot = matcher.snapshot();
                    let matched = snapshot.matched_item_count() as usize;
                    let rows = range(matched);
                    let items = snapshot.matched_items(rows.start as u32..rows.end as u32);
                    (candidates, annotations) = items.map(|item| item.data.clone()).unzip();
                    (matched, rows)
                }
                (None, None) => (0, 0..0),
            };
            let number = (matched > 0).then(|| completion.selected.min(matched - 1));
            selected = number.map(|n| n - rows.start);
            count = format!("{}/{matched} ", number.map_or(0, |n| n + 1));
            completion.rows = completion.rows.max(rows.len());
            shown = completion.rows;
        }
        // The minibuffer is selected while open; one row, as the height leaves out a mode line.
        let height = 2. * self.settings.line_height();
        let (width, _, cell_width) = self.frame_size;
        let cols =
            ((width / cell_width) as usize).saturating_sub(count.len() + prompt.chars().count());
        let (window, buffers, settings) = (&mut self.windows.window, &self.buffers, &self.settings);
        let display = window_display(window, buffers, settings, height, cols);
        Some(display::Minibuffer {
            count,
            prompt,
            display,
            candidates,
            annotations,
            selected,
            rows: shown,
        })
    }

    /// Shows what the command's preview gives for the selected candidate in the window the
    /// minibuffer was opened from, or that window as it was when it gives nothing. A file no
    /// buffer visits is read first, the window keeping what it shows meanwhile.
    pub(crate) fn preview(&mut self, effects: &mut Effects) {
        let Some(mb) = &self.prompts.minibuffer else {
            return;
        };
        let spec = &self.commands[mb.command].spec;
        // A handler's own question completes over its choices, not the spec's candidates, unless
        // the command declares no prompts, so that question is its only one.
        let Some(preview) = spec
            .preview
            .filter(|_| mb.history || spec.prompts.is_empty())
        else {
            return;
        };
        // Compared by value, as a position also follows the text.
        let shown = (self.prompts.selected_candidate()).and_then(|c| preview(self, &c));
        if shown == mb.previewed {
            return;
        }
        let (id, at) = match shown.clone() {
            Some(Preview::Buffer(id)) => (Some(id), None),
            Some(Preview::Point(p)) => (self.origin().map(Window::buffer), Some(p)),
            // The peek buffer visits its file too, so this finds it once read.
            Some(Preview::File(path) | Preview::FileAt(path, ..)) => {
                match self.buffers.visiting(&path) {
                    Some(id) => {
                        let at = match shown {
                            Some(Preview::FileAt(_, line, col)) => {
                                let buffer = self.buffers.get(id).expect("visiting is live");
                                // A line past the end, the file having changed since, is the end.
                                Some(buffer.char_idx(line, col).unwrap_or(buffer.len_chars()))
                            }
                            _ => None,
                        };
                        (Some(id), at)
                    }
                    None => {
                        let mb = self.prompts.minibuffer.as_mut().expect("checked above");
                        mb.previewed = shown;
                        self.request_preview(path, effects);
                        return;
                    }
                }
            }
            None => (None, None),
        };
        self.prompts.pending_preview = None;
        self.prompts
            .minibuffer
            .as_mut()
            .expect("previewing")
            .reading = None;
        if let (Some(id), Some(p)) = (id, at) {
            self.preview_at(id, p);
            let mb = self.prompts.minibuffer.as_mut().expect("checked above");
            mb.previewed = shown;
            return;
        }
        self.unpreview();
        let mb = self.prompts.minibuffer.as_mut().expect("checked above");
        mb.previewed = shown;
        let from =
            (self.windows.others.get_mut(&mb.from)).expect("the minibuffer's window is live");
        if let Some(id) = id.filter(|&id| id != from.buffer()) {
            // Swapped out whole, so its point, mark, ring and scroll stay as markers until restored.
            mb.saved = Some(std::mem::replace(from, Window::new(&mut self.buffers, id)));
        }
    }

    fn request_preview(&mut self, path: PathBuf, effects: &mut Effects) {
        let prompts = &mut self.prompts;
        let mb = prompts.minibuffer.as_mut().expect("previewing");
        let reusable = prompts
            .active_preview
            .iter()
            .chain(prompts.pending_preview.iter())
            .find(|r| r.session == mb.session && r.path == path)
            .cloned();
        let request = reusable.unwrap_or_else(|| {
            prompts.preview_revision += 1;
            PreviewRequest {
                session: mb.session,
                revision: prompts.preview_revision,
                path,
            }
        });
        mb.reading = Some(request.clone());
        if prompts.active_preview.is_none() {
            prompts.active_preview = Some(request.clone());
            effects.push(Effect::File(FileOp::Preview(request)));
        } else if prompts.active_preview.as_ref() == Some(&request) {
            prompts.pending_preview = None;
        } else {
            prompts.pending_preview = Some(request);
        }
    }

    pub(crate) fn on_previewed(
        &mut self,
        request: PreviewRequest,
        result: Result<Option<ropey::Rope>, String>,
        effects: &mut Effects,
    ) {
        if self.prompts.active_preview.as_ref() != Some(&request) {
            return;
        }
        self.prompts.active_preview = None;
        if self
            .prompts
            .minibuffer
            .as_ref()
            .is_some_and(|mb| mb.reading.as_ref() == Some(&request))
        {
            self.unpreview();
            let mb = self.prompts.minibuffer.as_mut().expect("previewing");
            mb.reading = None;
            if let Some(old) = mb.peek.take() {
                _ = self.buffers.kill(old);
            }
            if let Ok(Some(text)) = result {
                let id = self.buffers.create_for_prepared_file(&request.path, text);
                self.buffers
                    .rename(id, &format!(" {}", file_name(&request.path)));
                self.buffers.get_mut(id).expect("just created").read_only = true;
                let mb = self.prompts.minibuffer.as_mut().expect("previewing");
                mb.peek = Some(id);
                mb.previewed = None;
            }
        }
        if let Some(next) = self.prompts.pending_preview.take() {
            self.prompts.active_preview = Some(next.clone());
            effects.push(Effect::File(FileOp::Preview(next)));
        }
    }

    /// Adds `candidates` to the open minibuffer's list when `command` opened it completing.
    pub(crate) fn add_candidates(&mut self, command: &str, candidates: Vec<String>) {
        let open = (self.prompts.minibuffer.as_ref())
            .is_some_and(|mb| mb.command == command && mb.completion.is_some());
        let Some(matcher) = self.prompts.matcher.as_ref().filter(|_| open) else {
            return;
        };
        let injector = matcher.injector();
        for c in candidates {
            injector.push((c, String::new()), |(c, _), cols| {
                cols[0] = c.as_str().into()
            });
        }
    }

    /// Replaces the items of the filtering prompt `command` opened, or adds to them, every item
    /// listed; a replaced list selects its first.
    pub(crate) fn set_items(&mut self, command: &str, items: Vec<String>, replace: bool) {
        let completion = (self.prompts.minibuffer.as_mut())
            .filter(|mb| mb.command == command)
            .and_then(|mb| mb.completion.as_mut());
        let Some(completion) = completion.filter(|c| c.filtered.is_some()) else {
            return;
        };
        if replace {
            completion.select(0);
        }
        let f = completion.filtered.as_mut().expect("checked above");
        if replace {
            f.items.clear();
            f.notes.clear();
        }
        f.notes.extend(items.iter().map(|_| String::new()));
        f.items.extend(items);
        f.matched = (0..f.items.len()).collect();
    }

    /// Shows point at `p` of buffer `id` in the window the minibuffer was opened from, standing
    /// in a copy of it for its own buffer. A stand-in already on `id` keeps its scroll, as swiper
    /// keeps its window start.
    // shortcut: releasing the stand-in leaves `id`'s remembered point at the preview; save and restore it if reopened buffers start in odd places.
    fn preview_at(&mut self, id: BufferId, p: usize) {
        let mb = self.prompts.minibuffer.as_ref().expect("previewing");
        if mb.saved.is_none() || self.windows.others[&mb.from].buffer() != id {
            self.unpreview();
            let mb = self.prompts.minibuffer.as_mut().expect("previewing");
            let from =
                (self.windows.others.get_mut(&mb.from)).expect("the minibuffer's window is live");
            // A copy, so another buffer's stand-in keeps the rows it centers in.
            let mut stand_in = from.split(&mut self.buffers);
            if stand_in.buffer() != id {
                stand_in.show(&mut self.buffers, id);
            }
            mb.saved = Some(std::mem::replace(from, stand_in));
        }
        let mb = self.prompts.minibuffer.as_ref().expect("previewing");
        let from =
            (self.windows.others.get_mut(&mb.from)).expect("the minibuffer's window is live");
        if let Err(err) = from.jump(&mut self.buffers, p) {
            tracing::debug!(%err, "preview outside the buffer");
        }
    }

    /// The window the open minibuffer was opened from, as it was before any preview.
    pub(crate) fn origin(&self) -> Option<&Window> {
        let mb = self.prompts.minibuffer.as_ref()?;
        mb.saved
            .as_ref()
            .or_else(|| self.windows.others.get(&mb.from))
    }

    /// Puts back the window a preview stands in for.
    fn unpreview(&mut self) {
        let Some(mb) = self.prompts.minibuffer.as_mut() else {
            return;
        };
        if let Some(saved) = mb.saved.take() {
            let from =
                (self.windows.others.get_mut(&mb.from)).expect("the minibuffer's window is live");
            std::mem::replace(from, saved).release(&mut self.buffers);
        }
    }

    /// Closes the minibuffer, selecting the window it was opened from again, as it was before
    /// any preview.
    pub(crate) fn close_minibuffer(&mut self) -> Option<Minibuffer> {
        self.unpreview();
        let mb = self.prompts.minibuffer.take()?;
        self.prompts.pending_preview = None;
        if let Some(peek) = mb.peek {
            _ = self.buffers.kill(peek);
        }
        self.select(mb.from);
        let window =
            (self.windows.others.remove(&MINIBUFFER)).expect("the minibuffer window is live");
        let buffer = self
            .buffers
            .get(window.buffer())
            .expect("minibuffer buffer is live");
        let text = buffer.text().to_string();
        // A filtering prompt remembers what was typed however it closes, as swiper does.
        if mb.history && mb.completion.as_ref().is_some_and(|c| c.filtered.is_some()) {
            self.prompts.remember(&mb.prompt, text.clone());
        }
        self.prompts.last_text = text;
        _ = self.buffers.kill(window.buffer());
        Some(mb)
    }
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::command::ArgSpec;
    use crate::command::Args;
    use crate::command::Preview;
    use crate::harness::Harness;
    use crate::harness::ask;
    use crate::harness::ask_completing;
    use crate::harness::bind_mode;
    use crate::harness::buffer;
    use crate::harness::fruit;
    use crate::harness::listed;
    use crate::harness::on;
    use crate::harness::strings;
    use crate::harness::test_cmd;
    use crate::minibuffer::HISTORY_MAX;
    use crate::window::WindowId;

    fn pending_previews() -> Harness {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        test_cmd(
            &mut h,
            ArgSpec {
                prompts: &["Pick: "],
                completion: Some(fruit),
                preview: Some(|_, candidate| {
                    Some(Preview::File(std::path::Path::new("/").join(candidate)))
                }),
                ..ArgSpec::default()
            },
        );
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        h.keys("C-c p");
        h
    }

    fn reads(h: &mut Harness) -> Vec<crate::PreviewRequest> {
        std::mem::take(&mut h.effects)
            .into_iter()
            .filter_map(|e| match e {
                crate::Effect::File(crate::FileOp::Preview(r)) => Some(r),
                _ => None,
            })
            .collect()
    }

    fn answer_preview(h: &mut Harness, request: crate::PreviewRequest, text: &str) {
        h.send(Msg::Previewed {
            request,
            result: Ok(Some(
                crate::buffer::prepare_preview(text.as_bytes()).unwrap(),
            )),
        });
    }

    #[test]
    fn preview_reads_coalesce_to_the_latest_selection_and_input_keeps_working() {
        let mut h = pending_previews();
        let first = reads(&mut h).pop().unwrap();
        h.keys("C-n C-n");
        assert_eq!(
            h.core.prompts.selected_candidate().as_deref(),
            Some("cherry")
        );
        assert_eq!(reads(&mut h).len(), 0);
        answer_preview(&mut h, first, "obsolete apple");
        let next = reads(&mut h);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].path, std::path::Path::new("/cherry"));
        assert_eq!(h.core.prompts.minibuffer.as_ref().unwrap().peek, None);
        answer_preview(&mut h, next[0].clone(), "fresh cherry");
        let peek = h.core.prompts.minibuffer.as_ref().unwrap().peek.unwrap();
        assert_eq!(
            h.core.buffers.get(peek).unwrap().text().to_string(),
            "fresh cherry"
        );
        h.keys("C-g typed");
        assert_eq!(h.text(), "typed");
        assert_eq!(reads(&mut h).len(), 0);
    }

    #[test]
    fn reopened_same_path_has_its_own_request_and_closed_queues_do_not_run() {
        let mut h = pending_previews();
        let old = reads(&mut h).pop().unwrap();
        h.keys("C-n C-g C-c p");
        assert_eq!(reads(&mut h).len(), 0);
        answer_preview(&mut h, old.clone(), "old session");
        let current = reads(&mut h).pop().unwrap();
        assert_eq!(current.path, old.path);
        assert!(current.session > old.session);
        assert_eq!(h.core.prompts.minibuffer.as_ref().unwrap().peek, None);
        answer_preview(&mut h, old, "duplicate old session");
        answer_preview(&mut h, current, "current session");
        let peek = h.core.prompts.minibuffer.as_ref().unwrap().peek.unwrap();
        assert_eq!(
            h.core.buffers.get(peek).unwrap().text().to_string(),
            "current session"
        );
        h.keys("C-n C-n");
        let active = reads(&mut h).pop().unwrap();
        h.keys("C-g");
        answer_preview(&mut h, active, "closed");
        assert_eq!(reads(&mut h).len(), 0);
        assert_eq!(h.text(), "");
    }

    #[test]
    fn returning_to_an_active_file_reuses_its_read() {
        let mut h = pending_previews();
        let first = reads(&mut h).pop().unwrap();
        h.keys("C-n C-p");
        answer_preview(&mut h, first, "apple");
        assert_eq!(reads(&mut h).len(), 0);
        let peek = h.core.prompts.minibuffer.as_ref().unwrap().peek.unwrap();
        assert_eq!(
            h.core.buffers.get(peek).unwrap().text().to_string(),
            "apple"
        );
    }

    #[test]
    fn a_key_script_answers_a_prompt() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p hello");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Text: ");
        assert_eq!(h.text(), "hello");
        h.keys("RET");
        let got = h.core.message().unwrap().to_string();
        assert!(
            got.ends_with(&format!(
                "strings: {}, project: None }}",
                strings(&["hello"])
            )),
            "{got}"
        );
        assert!(h.core.minibuffer().is_none());
        assert_eq!(
            (h.text().as_str(), h.core.current().modified()),
            ("text", false)
        );
        assert_eq!(h.core.last_command(), Some("test-cmd"));
    }

    #[test]
    fn prompts_chain_and_keep_the_prefix_argument() {
        let mut h = on("");
        ask(&mut h, &["A: ", "B: "]);
        h.keys("C-u 3 C-c p a RET");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "B: ");
        h.keys("b RET");
        assert_eq!(
            h.core.message(),
            Some(
                format!(
                    "Args {{ prefix: Some(Number(3)), region: None, strings: {}, project: None }}",
                    strings(&["a", "b"])
                )
                .as_str()
            )
        );
    }

    #[test]
    fn c_g_aborts_the_minibuffer() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p abc C-g");
        assert!(h.core.minibuffer().is_none());
        assert_eq!(
            (h.text().as_str(), h.core.message()),
            ("text", Some("Quit"))
        );
    }

    #[test]
    fn editing_keys_work_in_the_minibuffer() {
        let mut h = on("");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p helo <left> l C-a x DEL C-e");
        assert_eq!(h.text(), "hello");
        h.keys("RET");
        assert!(h.core.message().unwrap().contains(&strings(&["hello"])));
    }

    #[test]
    fn each_prompt_keeps_a_history() {
        let mut h = on("");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p one RET C-c p two RET C-c p RET C-c p two RET C-c p x");
        h.keys("M-p");
        assert_eq!((h.text().as_str(), h.point()), ("two", 3));
        h.keys("M-p");
        assert_eq!(h.text(), "one", "empty and repeated answers are not kept");
        h.keys("M-p");
        assert_eq!(
            h.core.message(),
            Some("Beginning of history; no preceding item")
        );
        assert_eq!(h.text(), "one");
        h.keys("C-u 2 M-n");
        assert_eq!(h.text(), "x", "back to what was typed");
        h.keys("M-n");
        assert_eq!(
            h.core.message(),
            Some("End of history; no default available")
        );
        assert_eq!(h.text(), "x");
        h.keys("C-g");
        ask(&mut h, &["Other: "]);
        h.keys("C-c p M-p");
        assert_eq!(
            h.core.message(),
            Some("Beginning of history; no preceding item")
        );
    }

    #[test]
    fn a_command_asks_with_its_own_candidates() {
        let mut h = on("");
        h.core.register(crate::command::Command {
            name: "test-cmd",
            spec: ArgSpec::default(),
            handler: |ctx, args| {
                let command = ctx.core.commands["test-cmd"];
                let (prompt, choices): (&str, &[&str]) = match args.strings.len() {
                    0 => ("First? ", &["a", "b"]),
                    1 => ("Second? ", &["x", "y", "z"]),
                    2 => ("Third? ", &["no", "yes"]),
                    _ => {
                        ctx.core.message = Some(format!("{args:?}"));
                        return Ok(());
                    }
                };
                ctx.read_string(command, args, prompt.into(), Some(choices))
            },
        });
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        h.keys("C-c p");
        assert_eq!(listed(&mut h).0, ["a", "b"]);
        h.keys("b RET");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Second? ");
        assert_eq!(listed(&mut h).0, ["x", "y", "z"]);
        h.keys("RET");
        assert_eq!(listed(&mut h).0, ["no", "yes"]);
        h.keys("RET");
        assert!(
            h.core
                .message()
                .unwrap()
                .contains(&strings(&["b", "x", "no"]))
        );
    }

    #[test]
    fn history_keeps_a_hundred() {
        let mut h = on("");
        ask(&mut h, &["Text: "]);
        for i in 0..=HISTORY_MAX {
            h.keys(&format!("C-c p {i} RET"));
        }
        h.keys("C-c p C-u 100 M-p");
        assert_eq!(h.text(), "1");
        h.keys("M-p");
        assert_eq!(
            h.core.message(),
            Some("Beginning of history; no preceding item")
        );
    }

    #[test]
    fn the_minibuffer_stays_put_while_open() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        h.keys("C-x 2 C-c p");
        for keys in ["C-x 2", "C-x 3", "C-x o", "C-x 0", "C-x 1"] {
            h.keys(keys);
            assert!(h.core.message().is_some(), "{keys}");
            assert_eq!(h.frame(800., 600.).windows.len(), 2, "{keys}");
            assert!(h.core.minibuffer().is_some(), "{keys}");
        }
        // A prompt from the minibuffer is refused.
        h.call("test-cmd", Args::default());
        assert_eq!(
            h.core.message(),
            Some("Command attempted to use minibuffer while in minibuffer")
        );
        h.keys("ab").send(Msg::Click {
            window: WindowId(0),
            line: 0,
            col: 2,
        });
        assert!(h.core.minibuffer().is_none());
        assert_eq!((h.text().as_str(), h.point()), ("text", 2));
        assert_eq!(h.core.message(), None, "the command did not run");
    }

    #[test]
    fn the_frame_shows_the_minibuffer() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p hi");
        let frame = h.frame(800., 600.);
        assert!(frame.windows.iter().all(|w| !w.selected));
        insta::assert_debug_snapshot!(frame.minibuffer);
        h.keys("C-g");
        assert_eq!(h.frame(800., 600.).minibuffer, None);
    }

    #[test]
    fn typing_filters_candidates() {
        let mut h = on("");
        ask_completing(&mut h, fruit);
        h.keys("C-c p");
        assert_eq!(listed(&mut h), (fruit(&h.core, &Args::default()), Some(0)));
        h.keys("an");
        assert_eq!(listed(&mut h), (vec!["banana".into()], Some(0)));
        h.keys("DEL DEL ch");
        assert_eq!(listed(&mut h).0, ["cherry"]);
        h.keys("C-g");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p");
        assert_eq!(listed(&mut h), (vec![], None), "a plain prompt has no list");
    }

    #[test]
    fn c_n_and_c_p_move_the_selection() {
        let mut h = on("");
        ask_completing(&mut h, fruit);
        h.keys("C-c p C-n C-n C-n");
        assert_eq!(listed(&mut h).1, Some(2), "stops at the last");
        h.keys("C-p");
        assert_eq!(listed(&mut h).1, Some(1));
        h.keys("C-u 5 C-p");
        assert_eq!(listed(&mut h).1, Some(0), "stops at the first");
        h.keys("C-n a");
        assert_eq!(listed(&mut h).1, Some(0), "typing selects the first match");
        h.keys("zzz C-n");
        assert_eq!(h.core.message(), Some("No match"));
        h.keys("C-g");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p ab C-p C-n");
        assert_eq!(h.text(), "ab", "C-n and C-p keep their global bindings");
    }

    #[test]
    fn tab_completes_and_ret_picks_the_selection() {
        let mut h = on("");
        ask_completing(&mut h, fruit);
        h.keys("C-c p C-n TAB");
        assert_eq!((h.text().as_str(), h.point()), ("banana", 6));
        h.keys("C-g C-c p ch RET");
        assert!(h.core.message().unwrap().contains(&strings(&["cherry"])));
        h.keys("C-c p zzz TAB");
        assert_eq!(h.core.message(), Some("No match"));
        h.keys("RET");
        assert!(h.core.message().unwrap().contains(&strings(&["zzz"])));
    }

    #[test]
    fn the_frame_shows_the_completion_list() {
        let mut h = on("");
        ask_completing(&mut h, |_, _| {
            (1..=15).map(|i| format!("item {i}")).collect()
        });
        h.keys("C-c p C-u 12 C-n");
        let mb = h.frame(800., 600.).minibuffer.unwrap();
        assert_eq!((mb.candidates.len(), mb.selected), (10, Some(9)));
        assert_eq!(mb.count, "13/15 ");
        insta::assert_debug_snapshot!((mb.candidates, mb.selected));
        h.keys("C-u 9 C-p");
        assert_eq!(
            listed(&mut h).1,
            Some(0),
            "scrolls up only as far as needed"
        );
        assert_eq!(listed(&mut h).0[0], "item 4");
    }

    #[test]
    fn annotations_are_listed_and_not_matched() {
        let mut h = on("");
        test_cmd(
            &mut h,
            ArgSpec {
                prompts: &["Pick: "],
                completion: Some(fruit),
                annotation: Some(|_, c| (c != "cherry").then(|| format!("{c}-note"))),
                ..ArgSpec::default()
            },
        );
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        h.keys("C-c p");
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(mb.annotations, ["apple-note", "banana-note", ""]);
        h.keys("note");
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
    }

    /// The frame's windows and the selected window's point, mark, active mark and mark ring.
    fn windows(h: &mut Harness) -> String {
        let frame = h.frame(800., 600.).windows;
        let (w, b) = (&h.core.windows.window, &h.core.buffers);
        format!(
            "{:?}",
            (frame, w.point(b), w.mark(b), w.active(), w.ring(b))
        )
    }

    #[test]
    fn preview_then_c_g_leaves_windows_as_they_were() {
        let text: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
        let mut h = on(&text.join("\n"));
        test_cmd(
            &mut h,
            ArgSpec {
                prompts: &["Pick: "],
                completion: Some(fruit),
                preview: Some(|core, c| core.buffers.find_here(c).map(Preview::Buffer)),
                ..ArgSpec::default()
            },
        );
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        let [apple, banana] = ["apple", "banana"].map(|n| h.core.buffers.create(n));
        h.keys("C-x 2 C-SPC M-f C-SPC C-u 40 C-n C-SPC M-f C-SPC C-f C-SPC C-f");
        let before = windows(&mut h);
        let (file, point) = (h.core.windows.window.buffer(), h.point());
        let shown = |h: &Harness| h.core.windows.others[&WindowId(0)].buffer();
        h.keys("C-c p");
        assert_eq!(shown(&h), apple, "the first match shows at once");
        h.keys("C-n");
        assert_eq!(shown(&h), banana);
        h.keys("C-n");
        assert_eq!(shown(&h), file, "cherry has no buffer");
        h.keys("C-p zzz");
        assert_eq!(shown(&h), file, "no match");
        assert_eq!(
            h.core.windows.others[&WindowId(0)].point(&h.core.buffers),
            point
        );
        h.keys("C-g");
        assert_eq!(windows(&mut h), before);
        assert_eq!(
            h.core.buffers.recent().next().unwrap().0,
            file,
            "not raised"
        );
        h.keys("C-c p C-n RET");
        assert!(h.core.message().unwrap().contains(&strings(&["banana"])));
        assert_eq!(
            windows(&mut h),
            before,
            "RET runs from the window as it was"
        );
        h.keys("C-c p C-n").send(Msg::Click {
            window: WindowId(1),
            line: 0,
            col: 0,
        });
        assert_eq!(shown(&h), file, "a click restores");
        let (main, w) = (h.core.workspace(), h.new_workspace("w", "p"));
        h.core.switch_workspace(main).unwrap();
        // The click selected window 1, so the prompt opens from it.
        let before = h.core.windows.window.buffer();
        h.keys("C-c p C-n");
        assert_ne!(h.core.windows.others[&WindowId(1)].buffer(), before);
        h.core.switch_workspace(w).unwrap();
        h.core.switch_workspace(main).unwrap();
        assert_eq!(
            h.core.windows.window.buffer(),
            before,
            "a workspace switch restores"
        );
        assert!(h.core.minibuffer().is_none());
    }

    #[test]
    fn a_command_s_own_question_previews_only_without_declared_prompts() {
        for (prompts, previews) in [(&["Pick: "][..], false), (&[][..], true)] {
            let mut h = on("");
            h.core.register(crate::command::Command {
                name: "test-cmd",
                spec: ArgSpec {
                    prompts,
                    preview: Some(|core, c| core.buffers.find_here(c).map(Preview::Buffer)),
                    ..ArgSpec::default()
                },
                handler: |ctx, args| {
                    let command = ctx.core.commands["test-cmd"];
                    ctx.read_string(command, args, "Pick? ".into(), Some(&["apple"]))
                },
            });
            bind_mode(&mut h, "global", "C-c p", "test-cmd");
            let apple = h.core.buffers.create("apple");
            let file = h.core.windows.window.buffer();
            h.keys(if previews { "C-c p" } else { "C-c p x RET" });
            let shown = h.core.windows.others[&WindowId(0)].buffer();
            assert_eq!(shown, if previews { apple } else { file }, "{prompts:?}");
        }
    }

    #[test]
    fn m_x_runs_a_command_by_name() {
        let mut h = on("abc");
        h.keys("C-u 2 M-x forward-char RET");
        assert_eq!(h.point(), 2);
        assert_eq!(h.core.last_command(), Some("forward-char"));
        h.keys("M-x split-below");
        assert_eq!(
            h.core.prompts.selected_candidate().as_deref(),
            Some("split-window-below")
        );
        h.keys("C-g M-x zzz-nothing RET");
        assert!(h.core.message().unwrap().contains("zzz-nothing"));
    }

    #[test]
    fn m_x_shows_each_commands_key() {
        let mut h = on("");
        let note = |h: &mut Harness, name: &str| {
            h.keys(&format!("M-x {name}"));
            let mb = h.core.minibuffer().unwrap();
            let i = mb.candidates.iter().position(|c| c == name).unwrap();
            let note = mb.annotations[i].clone();
            h.keys("C-g");
            note
        };
        assert_eq!(note(&mut h, "split-window-below"), "C-x 2");
        assert_eq!(
            note(&mut h, "undo"),
            "C-/",
            "shortest, then first in kbd order"
        );
        assert_eq!(note(&mut h, "exit-minibuffer"), "", "no key runs it here");
        // A key a minor mode binds to something else no longer runs the command.
        bind_mode(&mut h, "m", "C-x 2", "undo");
        buffer(&mut h).minor_modes.push("m");
        assert_eq!(note(&mut h, "split-window-below"), "");
    }
}
