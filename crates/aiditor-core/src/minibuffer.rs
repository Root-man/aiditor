//! Prompts read in the minibuffer, with history and completion.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nucleo::Nucleo;
use nucleo::pattern::{CaseMatching, Normalization};

use crate::command::{Args, Command};
use crate::frame::window_display;
use crate::timer::TimerId;
use crate::window::{Window, WindowId};
use crate::{Core, Ctx, Effect, Effects, FileOp, Msg, display};

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
    /// The candidate last previewed; `None` before any and with no selection.
    previewed: Option<String>,
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
}

/// The open prompt, each prompt's history and the completion matcher.
#[derive(Default)]
pub(crate) struct Prompts {
    /// The prompt being answered, while the minibuffer is open.
    pub(crate) minibuffer: Option<Minibuffer>,
    /// Each prompt's answers, newest first.
    history: HashMap<String, Vec<String>>,
    /// The fuzzy matcher for completion, made on first use.
    pub(crate) matcher: Option<Nucleo<(String, String)>>,
    /// The timer that polls the matcher again while it is still matching.
    repoll: Option<TimerId>,
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
        let candidates = match choices {
            Some(choices) => Some(choices.iter().map(|c| c.to_string()).collect()),
            // No candidates is a plain prompt, so one command can mix completing and free text.
            None => (command.spec.completion)
                .map(|candidates| candidates(core, &args))
                .filter(|c: &Vec<String>| !c.is_empty()),
        };
        let completion = (candidates.is_some() || start.is_some()).then(|| {
            // One matcher for the core, as each one starts a thread pool.
            let matcher = core.prompts.matcher.get_or_insert_with(|| {
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
                rows: 0,
                // Nothing listed yet; `poll_completion` asks for the start directory.
                dir: files.then(String::new),
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
        core.prompts.minibuffer = Some(Minibuffer {
            command: command.name,
            args,
            prompt,
            from: core.selected,
            index: 0,
            input: String::new(),
            completion,
            history: choices.is_none(),
            saved: None,
            previewed: None,
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
        if completion.is_some_and(|c| c.dir.is_some())
            && !directories
            && let Some(dir) = selected.as_ref().filter(|s| s.ends_with('/'))
        {
            return self.set_text(dir);
        }
        let text = selected.unwrap_or_else(|| self.core.text());
        let mb = self.core.close_minibuffer().ok_or("Not in a minibuffer")?;
        if mb.history {
            let history = self.core.prompts.history.entry(mb.prompt).or_default();
            if !text.is_empty() && history.first() != Some(&text) {
                history.insert(0, text.clone());
                history.truncate(HISTORY_MAX);
            }
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
            .prompts
            .minibuffer
            .as_ref()
            .is_some_and(|mb| mb.completion.is_some())
        {
            return;
        }
        let text = self.text();
        let Some(completion) =
            (self.prompts.minibuffer.as_mut()).and_then(|mb| mb.completion.as_mut())
        else {
            return;
        };
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
        if let (Some(completion), Some(matcher)) = (&mut mb.completion, &mut self.prompts.matcher) {
            matcher.tick(0);
            let snapshot = matcher.snapshot();
            let matched = snapshot.matched_item_count() as usize;
            let rows = completion.top.min(matched)..(completion.top + COMPLETION_ROWS).min(matched);
            let items = snapshot.matched_items(rows.start as u32..rows.end as u32);
            (candidates, annotations) = items.map(|item| item.data.clone()).unzip();
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
        let (window, buffers, settings) = (&mut self.window, &self.buffers, &self.settings);
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

    /// Shows the buffer the command's preview gives for the selected candidate in the window
    /// the minibuffer was opened from, or that window as it was when it gives none.
    pub(crate) fn preview(&mut self) {
        let Some(mb) = &self.prompts.minibuffer else {
            return;
        };
        // A handler's own question completes over its choices, not the spec's candidates.
        let Some(preview) = self.commands[mb.command]
            .spec
            .preview
            .filter(|_| mb.history)
        else {
            return;
        };
        let candidate = self.prompts.selected_candidate();
        if candidate == mb.previewed {
            return;
        }
        self.unpreview();
        let id = candidate.as_deref().and_then(|c| preview(self, c));
        let mb = self.prompts.minibuffer.as_mut().expect("checked above");
        mb.previewed = candidate;
        let from = (self.others.get_mut(&mb.from)).expect("the minibuffer's window is live");
        if let Some(id) = id.filter(|&id| id != from.buffer()) {
            // Swapped out whole, so its point, mark, ring and scroll stay as markers until restored.
            mb.saved = Some(std::mem::replace(from, Window::new(&mut self.buffers, id)));
        }
    }

    /// Puts back the window a preview stands in for.
    fn unpreview(&mut self) {
        let Some(mb) = self.prompts.minibuffer.as_mut() else {
            return;
        };
        if let Some(saved) = mb.saved.take() {
            let from = (self.others.get_mut(&mb.from)).expect("the minibuffer's window is live");
            std::mem::replace(from, saved).release(&mut self.buffers);
        }
    }

    /// Closes the minibuffer, selecting the window it was opened from again, as it was before
    /// any preview.
    pub(crate) fn close_minibuffer(&mut self) -> Option<Minibuffer> {
        self.unpreview();
        let mb = self.prompts.minibuffer.take()?;
        self.select(mb.from);
        let window = (self.others.remove(&MINIBUFFER)).expect("the minibuffer window is live");
        _ = self.buffers.kill(window.buffer());
        Some(mb)
    }
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::command::ArgSpec;
    use crate::command::Args;
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
            got.ends_with(&format!("strings: {} }}", strings(&["hello"]))),
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
                    "Args {{ prefix: Some(Number(3)), region: None, strings: {} }}",
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
        let (w, b) = (&h.core.window, &h.core.buffers);
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
                preview: Some(|core, c| core.buffers.find_here(c)),
                ..ArgSpec::default()
            },
        );
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        let [apple, banana] = ["apple", "banana"].map(|n| h.core.buffers.create(n));
        h.keys("C-x 2 C-SPC M-f C-SPC C-u 40 C-n C-SPC M-f C-SPC C-f C-SPC C-f");
        let before = windows(&mut h);
        let (file, point) = (h.core.window.buffer(), h.point());
        let shown = |h: &Harness| h.core.others[&WindowId(0)].buffer();
        h.keys("C-c p");
        assert_eq!(shown(&h), apple, "the first match shows at once");
        h.keys("C-n");
        assert_eq!(shown(&h), banana);
        h.keys("C-n");
        assert_eq!(shown(&h), file, "cherry has no buffer");
        h.keys("C-p zzz");
        assert_eq!(shown(&h), file, "no match");
        assert_eq!(h.core.others[&WindowId(0)].point(&h.core.buffers), point);
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
        let (main, w) = (h.core.workspace(), h.core.new_workspace("w", "p"));
        h.core.switch_workspace(main).unwrap();
        // The click selected window 1, so the prompt opens from it.
        let before = h.core.window.buffer();
        h.keys("C-c p C-n");
        assert_ne!(h.core.others[&WindowId(1)].buffer(), before);
        h.core.switch_workspace(w).unwrap();
        h.core.switch_workspace(main).unwrap();
        assert_eq!(
            h.core.window.buffer(),
            before,
            "a workspace switch restores"
        );
        assert!(h.core.minibuffer().is_none());
    }

    #[test]
    fn a_command_s_own_question_does_not_preview() {
        let mut h = on("");
        h.core.register(crate::command::Command {
            name: "test-cmd",
            spec: ArgSpec {
                preview: Some(|core, c| core.buffers.find_here(c)),
                ..ArgSpec::default()
            },
            handler: |ctx, args| {
                let command = ctx.core.commands["test-cmd"];
                ctx.read_string(command, args, "Pick? ".into(), Some(&["apple"]))
            },
        });
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        h.core.buffers.create("apple");
        let file = h.core.window.buffer();
        h.keys("C-c p");
        assert_eq!(h.core.others[&WindowId(0)].buffer(), file);
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
