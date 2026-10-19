//! Prompts read in the minibuffer, with history and completion.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nucleo::Nucleo;
use nucleo::pattern::{CaseMatching, Normalization};

use crate::command::{Args, Command};
use crate::frame::window_display;
use crate::window::{Window, WindowId};
use crate::{Core, Ctx, Effect, Effects, Msg, display};

/// How many answers each prompt's history keeps, Emacs's `history-length`.
pub(crate) const HISTORY_MAX: usize = 100;

/// How many candidates the completion list shows, Vertico's `vertico-count`.
const COMPLETION_ROWS: usize = 10;

/// How long the core waits for the matcher before trying again on a timer.
const REPOLL: Duration = Duration::from_millis(16);

/// The minibuffer's window, kept outside the split tree.
const MINIBUFFER: WindowId = WindowId(usize::MAX);

/// A prompt waiting in the minibuffer for the command that asked.
pub(crate) struct Minibuffer {
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
    pub(crate) completion: Option<Completion>,
}

/// The selection in a completing minibuffer's list of matches.
pub(crate) struct Completion {
    /// The text the matcher's pattern was last set from.
    pattern: String,
    /// The selected match, best first.
    selected: usize,
    /// The first match shown, so the shown rows hold the selection.
    top: usize,
    /// For file names, the text through its last `/`: the directory listed or awaited.
    pub(crate) dir: Option<String>,
    /// Whether `dir`'s listing arrived, so a second answer is dropped.
    pub(crate) listed: bool,
    /// The directory a relative `dir` is listed from, as `find-file` visits from it.
    pub(crate) base: PathBuf,
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
    /// Opens the minibuffer at `prompt`; its answer then calls `command` again.
    pub(crate) fn read_string(
        &mut self,
        command: Command,
        args: Args,
        prompt: String,
    ) -> Result<(), String> {
        let core = &mut *self.core;
        if core.minibuffer.is_some() {
            return Err("Command attempted to use minibuffer while in minibuffer".into());
        }
        let base = core.directory();
        let start = (command.spec.files).then(|| base.join("").to_string_lossy().into_owned());
        let candidates = command
            .spec
            .completion
            .map(|candidates| candidates(core, &args));
        let completion = (candidates.is_some() || start.is_some()).then(|| {
            // One matcher for the core, as each one starts a thread pool.
            let matcher = core.matcher.get_or_insert_with(|| {
                Nucleo::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 1)
            });
            matcher.restart(true);
            (matcher.pattern).reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
            let injector = matcher.injector();
            for c in candidates.into_iter().flatten() {
                let note = command.spec.annotation.and_then(|f| f(core, &c));
                injector.push((c, note.unwrap_or_default()), |(c, _), cols| {
                    cols[0] = c.as_str().into()
                });
            }
            Completion {
                pattern: String::new(),
                selected: 0,
                top: 0,
                // Nothing listed yet; `poll_completion` asks for the start directory.
                dir: command.spec.files.then(String::new),
                listed: false,
                base,
            }
        });
        // A fresh buffer per prompt, so undo and markers never carry over.
        let id = core.buffers.create(" *Minibuf-1*");
        let buffer = core.buffers.get_mut(id).expect("just created");
        buffer.major_mode = "minibuffer-mode";
        if completion.is_some() {
            buffer.minor_modes.push("minibuffer-completion");
        }
        let window = Window::new(&mut core.buffers, id);
        core.others.insert(MINIBUFFER, window);
        core.minibuffer = Some(Minibuffer {
            command: command.name,
            args,
            prompt,
            from: core.selected,
            index: 0,
            input: String::new(),
            completion,
        });
        core.select(MINIBUFFER);
        match start {
            Some(text) => self.set_text(&text),
            None => Ok(()),
        }
    }

    /// Closes the minibuffer and calls its command with the selected candidate, or with `input`
    /// or no match the text, as the next string. A selected directory is entered instead.
    pub(crate) fn exit_minibuffer(&mut self, input: bool) -> Result<(), String> {
        let selected = (!input).then(|| self.core.selected_candidate()).flatten();
        let completion = self
            .core
            .minibuffer
            .as_ref()
            .and_then(|mb| mb.completion.as_ref());
        if completion.is_some_and(|c| c.dir.is_some())
            && let Some(dir) = selected.as_ref().filter(|s| s.ends_with('/'))
        {
            return self.set_text(dir);
        }
        let text = selected.unwrap_or_else(|| self.core.text());
        let mb = self.core.close_minibuffer().ok_or("Not in a minibuffer")?;
        let history = self.core.history.entry(mb.prompt).or_default();
        if !text.is_empty() && history.first() != Some(&text) {
            history.insert(0, text.clone());
            history.truncate(HISTORY_MAX);
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
        let core = &mut *self.core;
        let mb = core.minibuffer.as_mut().ok_or("Not in a minibuffer")?;
        let items = core.history.get(&mb.prompt).map_or(&[][..], Vec::as_slice);
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
        let count = self.core.matches();
        let completion = (self.core.minibuffer.as_mut())
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
        match self.core.selected == MINIBUFFER {
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
        // Checked before copying the text: this runs after every message.
        if !self
            .minibuffer
            .as_ref()
            .is_some_and(|mb| mb.completion.is_some())
        {
            return;
        }
        let text = self.text();
        let Some(completion) = (self.minibuffer.as_mut()).and_then(|mb| mb.completion.as_mut())
        else {
            return;
        };
        let matcher = self
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
                    effects.push(Effect::ListDir(completion.base.join(head)));
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
        if let Some(id) = self.repoll.take() {
            self.timers.cancel(id);
        }
        if matcher.tick(10).running {
            self.repoll = Some(self.timers.start(now, REPOLL, None, Msg::Tick));
        }
    }

    /// How many candidates match, 0 when not completing.
    fn matches(&self) -> usize {
        match (&self.minibuffer, &self.matcher) {
            (
                Some(Minibuffer {
                    completion: Some(_),
                    ..
                }),
                Some(m),
            ) => m.snapshot().matched_item_count() as usize,
            _ => 0,
        }
    }

    /// The selected candidate, if the minibuffer is completing and anything matches.
    pub(crate) fn selected_candidate(&self) -> Option<String> {
        let completion = self.minibuffer.as_ref()?.completion.as_ref()?;
        let snapshot = self.matcher.as_ref()?.snapshot();
        let last = snapshot.matched_item_count().checked_sub(1)?;
        let item = snapshot.get_matched_item((completion.selected as u32).min(last))?;
        let name = &item.data.0;
        Some(match &completion.dir {
            Some(dir) => format!("{dir}{name}"),
            None => name.clone(),
        })
    }

    /// In a file prompt with point at the end of a text that ends in `/` or names a listed
    /// directory, the start of that last component: where `DEL` deletes back to.
    pub(crate) fn component_start(&self) -> Option<usize> {
        let completion = self.minibuffer.as_ref()?.completion.as_ref()?;
        let (dir, matcher) = (completion.dir.as_ref()?, self.matcher.as_ref()?);
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
    pub fn minibuffer(&mut self) -> Option<display::Minibuffer> {
        let mb = self.minibuffer.as_ref()?;
        let prompt = mb.prompt.to_string();
        let (mut candidates, mut annotations, mut selected) = (Vec::new(), Vec::new(), None);
        if let (Some(completion), Some(matcher)) = (&mb.completion, &mut self.matcher) {
            matcher.tick(0);
            let snapshot = matcher.snapshot();
            let count = snapshot.matched_item_count() as usize;
            let rows = completion.top.min(count)..(completion.top + COMPLETION_ROWS).min(count);
            let items = snapshot.matched_items(rows.start as u32..rows.end as u32);
            (candidates, annotations) = items.map(|item| item.data.clone()).unzip();
            selected = (count > 0).then(|| completion.selected.min(count - 1) - rows.start);
        }
        // The minibuffer is selected while open; one row, as the height leaves out a mode line.
        let height = 2. * self.settings.line_height();
        let (width, _, cell_width) = self.frame_size;
        let cols = ((width / cell_width) as usize).saturating_sub(prompt.chars().count());
        let (window, buffers, settings) = (&mut self.window, &self.buffers, &self.settings);
        let display = window_display(window, buffers, settings, height, cols);
        Some(display::Minibuffer {
            prompt,
            display,
            candidates,
            annotations,
            selected,
        })
    }

    /// Closes the minibuffer, selecting the window it was opened from again.
    pub(crate) fn close_minibuffer(&mut self) -> Option<Minibuffer> {
        let mb = self.minibuffer.take()?;
        self.select(mb.from);
        let window = (self.others.remove(&MINIBUFFER)).expect("the minibuffer window is live");
        _ = self.buffers.kill(window.buffer());
        Some(mb)
    }
}
