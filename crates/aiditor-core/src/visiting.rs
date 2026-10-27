//! Visiting, saving, switching and killing buffers, and quitting.

use std::path::{Path, PathBuf};

use crate::buffer::{self, Buffer, BufferId, Buffers};
use crate::command::Args;
use crate::workspace::WorkspaceId;
use crate::{Core, Ctx, Effect, FileOp, VisitIntent};

pub(crate) struct PendingVisit {
    pub intent: VisitIntent,
    pub path: PathBuf,
    pub destination: Option<(usize, usize)>,
}

impl Ctx<'_> {
    /// Handles the file service's answer to a save of buffer `id` at `tick`, and the quit
    /// waiting on it.
    pub(crate) fn on_saved(&mut self, id: BufferId, tick: u64, result: Result<u64, String>) {
        let ok = result.is_ok();
        match (self.core.buffers.get_mut(id), result) {
            (None, _) => {}
            (Some(buffer), Ok(hash)) => {
                (buffer.disk, buffer.deleted, buffer.conflict) = (Some(hash), false, None);
                // Edits made while the write ran are still unsaved.
                if buffer.tick() == tick {
                    buffer.set_saved();
                }
                let path = buffer.file.as_deref().unwrap_or(Path::new(""));
                let msg = format!("Wrote {}", path.display());
                self.core.echo(msg);
            }
            (Some(buffer), Err(err)) => {
                // A change on disk the watcher missed becomes a conflict.
                if let Some(path) = buffer.file.clone() {
                    self.effects.push(Effect::File(FileOp::Check(vec![path])));
                }
                self.core.echo(err)
            }
        }
        let core = &mut *self.core;
        if let Some(at) = core.quitting.iter().position(|&b| b == id) {
            // A failed save, or edits made while it ran, cancel the quit.
            if ok && !core.buffers.get(id).is_some_and(Buffer::modified) {
                core.quitting.swap_remove(at);
                if core.quitting.is_empty() {
                    match core.then_kill.take() {
                        Some(workspace) => {
                            if let Err(err) = core.kill_workspace(workspace) {
                                core.echo(err);
                            }
                        }
                        None => self.effects.push(Effect::Quit),
                    }
                }
            } else {
                core.quitting.clear();
                core.then_kill = None;
            }
        }
    }

    /// Kills the buffer `name`, asking first when it is a modified file buffer, a live terminal
    /// or a live agent session's log; an unkillable one is refused.
    pub(crate) fn kill_buffer(&mut self, args: Args) -> Result<(), String> {
        let name = &args.strings[0];
        let id =
            (self.core.buffers.find_here(name)).ok_or_else(|| format!("No such buffer {name}"))?;
        let buffer = self.core.buffers.get(id).expect("found");
        if buffer.unkillable {
            return Err(format!("Can't kill {name}"));
        }
        let prompt = match self.core.live_terminal(id) || self.core.live_agent(id) {
            true => Some(format!("Buffer {name} has a running process; kill it? ")),
            false => (buffer.file.is_some() && buffer.modified())
                .then(|| "Buffer modified; kill anyway? ".into()),
        };
        if let Some(prompt) = prompt {
            match args.strings.get(1).map(String::as_str) {
                None => {
                    let command = self.core.commands["kill-buffer"];
                    return self.read_string(command, args, prompt, Some(&["no", "yes"]));
                }
                Some("yes") => {}
                Some(_) => return Ok(()),
            }
        }
        self.core.kill_buffer(id);
        Ok(())
    }

    /// Reverts the current buffer from its file, asking first when it is modified.
    pub(crate) fn revert_buffer(&mut self, args: Args) -> Result<(), String> {
        let buffer = self.core.current();
        let path =
            (buffer.file.clone()).ok_or("Buffer does not seem to be associated with any file")?;
        if buffer.modified() {
            match args.strings.first() {
                None => {
                    let command = self.core.commands["revert-buffer"];
                    let prompt = format!("Revert buffer from file {}? ", path.display());
                    return self.read_string(command, args, prompt, Some(&["Yes", "No"]));
                }
                Some(answer) if answer.eq_ignore_ascii_case("yes") => {}
                Some(_) => return Ok(()),
            }
        }
        let buffer = self.core.current();
        self.effects.push(Effect::File(FileOp::Revert {
            buffer: self.core.windows.window.buffer(),
            tick: buffer.tick(),
            path,
            text: buffer.text().clone(),
        }));
        Ok(())
    }

    /// Asks how to resolve the current buffer's conflict with its file, as `command`
    /// (`resolve-conflict` or `save-buffer`), then does what the answer says; `save-buffer`
    /// saves after `Keep mine`, and saves at once when there is no conflict.
    pub(crate) fn resolve_conflict(&mut self, command: &str, args: Args) -> Result<(), String> {
        let save = command == "save-buffer";
        let id = self.core.windows.window.buffer();
        let buffer = self.core.current_mut();
        let (Some(path), Some(hash)) = (buffer.file.clone(), buffer.conflict) else {
            // An answer arriving after the conflict ended does nothing more.
            return match (save, args.strings.is_empty()) {
                (true, _) => self.save(id),
                (false, true) => Err("No conflict with the file on disk".into()),
                (false, false) => Ok(()),
            };
        };
        let Some(answer) = args.strings.first() else {
            let command = self.core.commands[command];
            let prompt = format!("{} changed on disk: ", path.display());
            // Safe to destructive, so `RET` right away loses nothing.
            let choices = ["Compare", "Take disk", "Keep mine"];
            return self.read_string(command, args, prompt, Some(&choices));
        };
        match answer.to_ascii_lowercase().as_str() {
            "keep mine" => {
                // The disk version becomes the save's base, so the save overwrites it.
                (buffer.disk, buffer.conflict) = (Some(hash), None);
                if save {
                    return self.save(id);
                }
            }
            "take disk" => {
                let yes = vec!["Yes".to_string()];
                return self.revert_buffer(Args {
                    strings: yes,
                    ..Args::default()
                });
            }
            "compare" => return self.diff_buffer_with_file(),
            _ => {}
        }
        Ok(())
    }

    /// Asks the file service to diff the current buffer against its file, for `on_diffed`.
    pub(crate) fn diff_buffer_with_file(&mut self) -> Result<(), String> {
        let buffer = self.core.current();
        let path =
            (buffer.file.clone()).ok_or("Buffer does not seem to be associated with any file")?;
        self.effects.push(Effect::File(FileOp::Diff {
            buffer: self.core.windows.window.buffer(),
            name: buffer.name.clone(),
            path,
            text: buffer.text().clone(),
        }));
        Ok(())
    }

    /// Shows the file service's diff of buffer `id` in the workspace's read-only `*Diff*`, in
    /// another window; echoes `No differences` or why there is no diff.
    pub(crate) fn on_diffed(&mut self, id: BufferId, result: Result<String, String>) {
        let core = &mut *self.core;
        // A killed buffer, or one of a workspace no longer current, has nowhere to show it.
        if core.buffers.get(id).is_none() || !core.buffers.is_here(id) {
            return;
        }
        let diff = match result {
            Ok(diff) if diff.is_empty() => return core.echo("No differences".into()),
            Ok(diff) => diff,
            Err(err) => return core.echo(err),
        };
        // By mode, not name: another workspace's `*Diff*` makes this one's `*Diff*<2>`.
        let found = (core.buffers.recent())
            .find(|&(b, buffer)| buffer.major_mode == "diff-mode" && core.buffers.is_here(b));
        let diff_id = found.map(|(b, _)| b).unwrap_or_else(|| {
            let b = core.buffers.create("*Diff*");
            let buffer = core.buffers.get_mut(b).expect("just created");
            (buffer.read_only, buffer.major_mode) = (true, "diff-mode");
            b
        });
        (core.buffers.get_mut(diff_id).expect("live")).set_text(&diff);
        // shortcut: an answer arriving mid-prompt leaves `*Diff*` unshown (`C-x b` finds it); show it on close if that bites.
        if self.outside_minibuffer("").is_ok() {
            self.display_buffer(diff_id);
        }
    }

    /// Visits `name`, taken from the current buffer's directory: switches to a buffer visiting
    /// it, or asks the app to read it.
    pub(crate) fn find_file(&mut self, name: &str) -> Result<(), String> {
        let path = self.core.directory().join(name);
        self.core.pending_visit = None;
        match self.core.buffers.visiting(&path) {
            Some(id) => self.core.switch_to(id),
            None => {
                self.core.visits += 1;
                let intent = VisitIntent {
                    request: self.core.visits,
                    workspace: self.core.buffers.workspace,
                    window: self.core.windows.selected,
                };
                self.core.pending_visit = Some(PendingVisit {
                    intent,
                    path: path.clone(),
                    destination: None,
                });
                self.effects
                    .push(Effect::File(FileOp::Open { intent, path }));
            }
        }
        Ok(())
    }

    /// Offers to save each modified file buffer, asks before killing live terminals and agents,
    /// then quits once those saves succeed.
    pub(crate) fn save_buffers_kill_terminal(&mut self, mut args: Args) -> Result<(), String> {
        let command = "save-buffers-kill-terminal";
        // Marks the processes answer, the last string, so `save_some`'s pairs don't shift.
        const PROCESSES: &str = "\0processes";
        let n = args.strings.len();
        let processes = (n >= 2 && args.strings[n - 2] == PROCESSES).then(|| {
            let answer = args.strings.pop();
            args.strings.pop();
            answer.expect("two strings")
        });
        let Some(saving) = self.save_some(command, args.clone(), 0, |_| true, "exit")? else {
            return Ok(());
        };
        if !self.core.terminals.is_empty() || self.core.live_agents() {
            match processes {
                None => {
                    args.strings.push(PROCESSES.into());
                    let prompt = "Active processes exist; kill them and exit anyway? ".into();
                    let command = self.core.commands[command];
                    return self.read_string(command, args, prompt, Some(&["No", "Yes"]));
                }
                Some(answer) if answer.eq_ignore_ascii_case("yes") => {}
                Some(_) => {
                    // As declining to exit anyway, the buffers answered `Yes` are still saved.
                    return saving.iter().try_for_each(|&id| self.save(id));
                }
            }
        }
        for &id in &saving {
            self.save(id)?;
        }
        match saving.is_empty() {
            true => self.effects.push(Effect::Quit),
            false => (self.core.quitting, self.core.then_kill) = (saving, None),
        }
        Ok(())
    }

    /// Asks `command`'s questions about the modified file buffers `of` takes: whether to save
    /// each, then, if any was declined, whether to `verb` anyway. The strings after the first
    /// `skip` are name/answer pairs for the buffers asked about, then the `verb anyway` answer.
    /// Returns the buffers answered `Yes`, for the caller to save, once the command should go
    /// ahead; `None` while asking, or when declined, having saved them.
    pub(crate) fn save_some(
        &mut self,
        command: &str,
        mut args: Args,
        skip: usize,
        of: impl Fn(&Buffer) -> bool,
        verb: &str,
    ) -> Result<Option<Vec<BufferId>>, String> {
        let command = self.core.commands[command];
        let answers = &args.strings[skip..];
        let asked = |name: &str| answers.as_chunks::<2>().0.iter().any(|p| p[0] == name);
        let next = (self.core.buffers.recent())
            .find(|(_, b)| of(b) && b.file.is_some() && b.modified() && !asked(&b.name))
            .map(|(_, b)| (b.name.clone(), b.file.clone().expect("a file buffer")));
        let paired = answers.len().is_multiple_of(2);
        if paired && let Some((name, path)) = next {
            args.strings.push(name);
            let prompt = format!("Save file {}? ", path.display());
            return (self.read_string(command, args, prompt, Some(&["Yes", "No"]))).map(|()| None);
        }
        let yes: Vec<String> = (answers.as_chunks::<2>().0.iter())
            .filter(|p| p[1].eq_ignore_ascii_case("yes"))
            .map(|p| p[0].clone())
            .collect();
        let declined = yes.len() < answers.len() / 2;
        if declined && paired {
            let prompt = format!("Modified buffers exist; {verb} anyway? ");
            return (self.read_string(command, args, prompt, Some(&["No", "Yes"]))).map(|()| None);
        }
        let last_yes = (answers.last()).is_some_and(|a| a.eq_ignore_ascii_case("yes"));
        let core = &mut *self.core;
        let saving: Vec<BufferId> = (yes.iter())
            .filter_map(|name| core.buffers.find(name))
            .filter(|&id| core.buffers.get(id).is_some_and(Buffer::modified))
            .collect();
        let go = !declined || last_yes;
        // Declining to go ahead still saves the buffers answered `Yes`.
        if !go {
            for &id in &saving {
                self.save(id)?;
            }
        }
        Ok(go.then_some(saving))
    }

    pub(crate) fn save(&mut self, id: BufferId) -> Result<(), String> {
        let buffer = self.core.buffers.get_mut(id).expect("saved buffer is live");
        let Some(path) = buffer.file.clone() else {
            tracing::info!(name = buffer.name, "buffer visits no file");
            return Ok(());
        };
        // A deleted file is written back even unmodified, since nothing on disk is lost.
        if !buffer.modified() && !buffer.deleted {
            return Err("(No changes need to be saved)".into());
        }
        let len = buffer.text().len_chars();
        if len > 0 && buffer.text().char(len - 1) != '\n' {
            buffer.insert(len, "\n")?;
        }
        self.effects.push(Effect::File(FileOp::Save {
            buffer: id,
            tick: buffer.tick(),
            path,
            text: buffer.text().clone(),
            crlf: buffer.crlf,
            bom: buffer.bom,
            hash: buffer.disk,
        }));
        Ok(())
    }
}

impl Core {
    /// The current buffer's directory: its file's, its terminal's, else the working directory.
    pub(crate) fn directory(&self) -> PathBuf {
        (self.buffer_directory()).unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    /// The current buffer's file's directory, or its terminal's.
    pub(crate) fn buffer_directory(&self) -> Option<PathBuf> {
        let buffer = self.current();
        (buffer.file.as_deref().and_then(Path::parent))
            .or_else(|| buffer.terminal()?.cwd())
            .map(Path::to_path_buf)
    }

    /// Applies the file service's answer to a revert of buffer `id` at `tick` as one undo group,
    /// unless the buffer changed since; errors are echoed.
    pub(crate) fn on_reverted(
        &mut self,
        id: BufferId,
        tick: u64,
        result: Result<buffer::Revert, String>,
    ) {
        let revert = match result {
            Ok(revert) => revert,
            Err(err) => return self.echo(err),
        };
        // Edits made since, or a kill, leave the diff nothing to apply to.
        if self.buffers.get(id).is_none_or(|b| b.tick() != tick) {
            return;
        }
        let point = |core: &Core| match core.windows.window.buffer() == id {
            true => core.windows.window.point(&core.buffers),
            false => {
                let b = core.buffers.get(id).expect("live");
                b.marker(b.point).unwrap_or(0)
            }
        };
        let before = point(self);
        let buffer = self.buffers.get_mut(id).expect("live");
        buffer.undo_boundary(before);
        buffer
            .replace(&revert.edits)
            .expect("the diff is of this text");
        let after = point(self);
        let buffer = self.buffers.get_mut(id).expect("live");
        buffer.undo_boundary(after);
        (buffer.disk, buffer.crlf, buffer.bom) = (Some(revert.hash), revert.crlf, revert.bom);
        buffer.conflict = None;
        buffer.set_saved();
    }

    pub(crate) fn on_opened(
        &mut self,
        intent: VisitIntent,
        path: PathBuf,
        result: Result<Option<(Vec<u8>, u64)>, String>,
    ) {
        let valid = self.pending_visit.as_ref().is_some_and(|pending| {
            pending.intent == intent
                && pending.path == path
                && intent.workspace == self.buffers.workspace
                && intent.window == self.windows.selected
        });
        if !valid {
            return;
        }
        let pending = self.pending_visit.take().expect("validated visit");
        match result {
            Ok(file) => {
                self.visit(&path, file, true);
                if self.buffers.visiting(&path) == Some(self.windows.window.buffer())
                    && let Some((line, col)) = pending.destination
                {
                    self.goto_line_col(line, col);
                }
            }
            Err(err) => self.echo(err),
        }
    }

    /// Visits `path` in a new current buffer, given the file's bytes and their hash, or `None`
    /// when there is no file. A binary or non-UTF-8 file opens empty and read-only.
    pub fn open(&mut self, path: &Path, file: Option<(Vec<u8>, u64)>) {
        self.visit(path, file, false)
    }

    /// `open`, but a buffer already visiting `path` is switched to, and with `refuse` a binary
    /// or non-UTF-8 file is not opened; either way the echo area says why.
    pub(crate) fn visit(&mut self, path: &Path, file: Option<(Vec<u8>, u64)>, refuse: bool) {
        // Two finds of one file can race; the later one switches to the first's buffer.
        if let Some(id) = self.buffers.visiting(path) {
            return self.switch_to(id);
        }
        let disk = file.as_ref().map(|(_, hash)| *hash);
        let (text, crlf, bom, refused) = match file.map(|(bytes, _)| buffer::decode(&bytes)) {
            None => (String::new(), false, false, None),
            Some(Ok((text, crlf, bom))) => (text, crlf, bom, None),
            Some(Err(reason)) => (String::new(), false, false, Some(reason)),
        };
        if let Some(reason) = refused {
            let opened = if refuse { "" } else { ", opened read-only" };
            self.echo(format!("{}: {reason}{opened}", path.display()));
            if refuse {
                return;
            }
        }
        let id = self.buffers.create_for_file(path, &text);
        let buffer = self.buffers.get_mut(id).expect("just created");
        (buffer.crlf, buffer.bom, buffer.disk) = (crlf, bom, disk);
        buffer.read_only = refused.is_some();
        self.switch_to(id);
    }

    /// Shows buffer `id` in the selected window, as a jump, and makes it the most recent.
    pub(crate) fn switch_to(&mut self, id: BufferId) {
        self.pending_visit = None;
        if self.windows.window.buffer() != id {
            self.push_jump();
        }
        self.show_buffer(id);
    }

    /// Shows buffer `id` in the selected window and makes it the most recent.
    pub(crate) fn show_buffer(&mut self, id: BufferId) {
        self.buffers.raise(id);
        if self.windows.window.buffer() != id {
            self.pending_visit = None;
            self.windows.window.show(&mut self.buffers, id);
        }
    }

    /// Switches to the buffer `name`, making an empty one if there is none.
    pub(crate) fn switch_to_name(&mut self, name: &str) {
        let id = (self.buffers.find_here(name)).unwrap_or_else(|| self.buffers.create(name));
        self.switch_to(id);
    }

    /// The names of the current workspace's buffers and `*Messages*` not hidden by a leading
    /// space, most recently shown first, with the current buffer first or last.
    pub(crate) fn buffer_names(&self, current_first: bool) -> Vec<String> {
        let current = self.windows.window.buffer();
        let listed = self
            .buffers
            .recent()
            .filter(|&(id, b)| !b.name.starts_with(' ') && self.buffers.is_here(id));
        let (this, others): (Vec<_>, Vec<_>) = listed.partition(|(id, _)| *id == current);
        let names = match current_first {
            true => [this, others].concat(),
            false => [others, this].concat(),
        };
        names.into_iter().map(|(_, b)| b.name.clone()).collect()
    }

    /// Kills buffer `id`; windows showing it show the most recent listed buffer left of their
    /// workspace or `*Messages*`, or a new scratch in the current workspace.
    pub(crate) fn kill_buffer(&mut self, id: BufferId) {
        if self.windows.window.buffer() == id {
            self.pending_visit = None;
        }
        let next = replacement(&self.buffers, id, self.buffers.workspace);
        // Killed first, so a new scratch gets its name.
        _ = self.buffers.kill(id);
        let next = next.unwrap_or_else(|| {
            let name = self.current_workspace().name.clone();
            self.buffers.create_scratch(&name)
        });
        self.windows.replace_buffer(&mut self.buffers, id, next);
        for workspace in &mut self.workspaces {
            let Some(windows) = &mut workspace.windows else {
                continue;
            };
            // Every live workspace keeps its `*scratch*`, so `next` is only a guard.
            let theirs = replacement(&self.buffers, id, workspace.id).unwrap_or(next);
            windows.replace_buffer(&mut self.buffers, id, theirs);
        }
    }
}

/// The most recent listed buffer but `id` that belongs to `workspace` or to none.
fn replacement(buffers: &Buffers, id: BufferId, workspace: WorkspaceId) -> Option<BufferId> {
    (buffers.recent())
        .find(|&(b, buffer)| {
            b != id
                && !buffer.name.starts_with(' ')
                && buffer.workspace.is_none_or(|w| w == workspace)
        })
        .map(|(b, _)| b)
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::command::Args;
    use crate::harness::Harness;
    use crate::harness::ask_completing;
    use crate::harness::fruit;
    use crate::harness::listed;
    use crate::harness::name;
    use crate::harness::on;
    use crate::harness::strings;
    use crate::window::WindowId;

    const LINES: &str = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";

    /// `a.txt` open with lines `a` to `j`, and point at column 1 of line `f`.
    fn reverting() -> Harness {
        let mut h = Harness::new(&[("a.txt", LINES)]);
        h.open("a.txt");
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, 11)
            .unwrap();
        h
    }

    fn revert(h: &mut Harness, disk: &str) {
        h.prepare_revert("a.txt", Some(disk.as_bytes()));
        h.keys("M-x revert-buffer RET");
    }

    fn pending_open(h: &mut Harness, file: &str) -> (crate::VisitIntent, std::path::PathBuf) {
        h.collect_effects = true;
        h.call(
            "find-file",
            Args {
                strings: vec![h.path(file).to_string_lossy().into_owned()],
                ..Args::default()
            },
        );
        std::mem::take(&mut h.effects)
            .into_iter()
            .find_map(|effect| match effect {
                crate::Effect::File(crate::FileOp::Open { intent, path }) => Some((intent, path)),
                _ => None,
            })
            .expect("a visit read")
    }

    fn opened(intent: crate::VisitIntent, path: std::path::PathBuf, text: &str) -> Msg {
        Msg::Opened {
            intent,
            path,
            result: Ok(Some((
                text.as_bytes().to_vec(),
                crate::buffer::hash(text.as_bytes()),
            ))),
        }
    }

    #[test]
    fn a_visit_keeps_its_owner_through_ordinary_edits_and_validates_text() {
        let mut h = Harness::new(&[]);
        let (intent, path) = pending_open(&mut h, "new.txt");
        let origin = h.core.windows.window.buffer();
        h.keys("editing");
        h.send(opened(intent, path.clone(), "visited"));
        assert_eq!(h.core.current().file, Some(path));
        assert_eq!(h.core.current().workspace, Some(intent.workspace));
        assert_eq!(h.text(), "visited");
        assert_eq!(
            h.core.buffers.get(origin).unwrap().text().to_string(),
            "editing"
        );
        let (intent, path) = pending_open(&mut h, "binary");
        h.send(opened(intent, path, "a\0b"));
        assert_eq!(h.text(), "visited");
        assert!(h.core.message().unwrap().ends_with(": binary file"));
    }

    #[test]
    fn abandoned_visits_leave_navigation_prompts_and_echo_unchanged() {
        let changes: [fn(&mut Harness); 7] = [
            |h| {
                let origin = h.core.workspace();
                h.new_workspace("away", "home");
                h.core.switch_workspace(origin).unwrap();
            },
            |h| {
                h.keys("C-x 2 C-x o C-x o");
            },
            |h| {
                h.keys("C-x 2 C-x 0");
            },
            |h| {
                let origin = h.core.workspace();
                h.new_workspace("away", "home");
                h.core.kill_workspace(origin).unwrap();
            },
            |h| {
                h.core.switch_to_name("another");
                h.core.switch_to_name("*scratch*");
            },
            |h| {
                h.keys("C-x b");
            },
            |h| {
                h.core.kill_buffer(h.core.windows.window.buffer());
            },
        ];
        for change in changes {
            for error in [false, true] {
                let mut h = Harness::new(&[]);
                let (intent, path) = pending_open(&mut h, "late.txt");
                change(&mut h);
                h.core.echo("new interaction".into());
                let before = (
                    h.core.workspace(),
                    h.core.windows.selected,
                    h.text(),
                    h.point(),
                    h.core.buffers.recent().count(),
                );
                h.send(if error {
                    Msg::Opened {
                        intent,
                        path,
                        result: Err("obsolete error".into()),
                    }
                } else {
                    opened(intent, path, "late")
                });
                assert_eq!(
                    (
                        h.core.workspace(),
                        h.core.windows.selected,
                        h.text(),
                        h.point(),
                        h.core.buffers.recent().count()
                    ),
                    before
                );
                assert_eq!(h.core.message(), Some("new interaction"));
            }
        }
    }

    #[test]
    fn a_newer_same_path_visit_owns_its_reply_and_point() {
        let mut h = Harness::new(&[]);
        let (first, path) = pending_open(&mut h, "same");
        h.core.pending_visit.as_mut().unwrap().destination = Some((1, 2));
        let (second, _) = pending_open(&mut h, "same");
        h.send(opened(first, path.clone(), "old\n  hit"));
        assert_eq!(h.text(), "");
        h.send(opened(second, path.clone(), "new\n  hit"));
        assert_eq!((h.text(), h.point()), ("new\n  hit".into(), 0));
        h.keys("C-f");
        h.send(opened(first, path, "old"));
        assert_eq!((h.text(), h.point()), ("new\n  hit".into(), 1));
    }

    #[test]
    fn point_stays_on_its_line_after_a_revert_changes_lines_above() {
        let mut h = reverting();
        let disk = "a\n1\n2\n3\n4\nd\ne\nf\ng\nh\ni\nj\n";
        revert(&mut h, disk);
        assert_eq!(h.text(), disk);
        assert_eq!(h.core.current().line_col(h.point()), Ok((7, 1)));
        assert!(!h.core.current().modified());
        assert_eq!(h.core.minibuffer().map(|m| m.prompt.clone()), None);
    }

    #[test]
    fn a_revert_keeps_the_mark_and_other_windows_on_their_lines() {
        let mut h = reverting();
        h.core
            .windows
            .window
            .set_mark(&mut h.core.buffers, 14)
            .unwrap();
        h.keys("C-x 2");
        let other = *h.core.windows.others.keys().next().unwrap();
        let w = &h.core.windows.others[&other];
        w.set_point(&mut h.core.buffers, 16).unwrap();
        revert(&mut h, &LINES[2..]);
        assert_eq!(
            h.core.windows.window.mark(&h.core.buffers),
            Some(12),
            "on h"
        );
        assert_eq!(
            h.core.windows.others[&other].point(&h.core.buffers),
            14,
            "on i"
        );
        assert_eq!(h.point(), 9, "still at column 1 of f");
    }

    #[test]
    fn point_in_a_changed_line_goes_to_its_replacement() {
        let mut h = reverting();
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, 5)
            .unwrap();
        revert(&mut h, &LINES.replace("c\n", "ccc\n"));
        assert_eq!(h.point(), 4);
    }

    #[test]
    fn a_revert_undoes_in_one_step_and_saves_afterwards() {
        let mut h = reverting();
        h.keys("x");
        revert(&mut h, "z\n");
        h.keys("RET");
        assert_eq!(h.text(), "z\n");
        assert!(!h.core.current().modified());
        h.keys("C-/");
        assert_eq!(h.text(), LINES.replace("f\n", "fx\n"));
        h.save_reply(LINES);
        h.keys("C-/ C-x C-s");
        assert_eq!(
            h.submitted("a.txt"),
            LINES,
            "the revert took the disk's hash"
        );
    }

    #[test]
    fn a_revert_takes_the_file_s_line_ending() {
        let mut h = reverting();
        revert(&mut h, &LINES.replace('\n', "\r\n"));
        assert_eq!(h.text(), LINES);
        assert!(h.display(5).mode_line.contains("(DOS)"));
    }

    #[test]
    fn a_revert_of_edited_text_is_dropped() {
        let mut h = reverting();
        h.prepare_revert("a.txt", Some(b"z\n"));
        let mut ctx = h.ctx();
        ctx.call("revert-buffer", Args::default());
        let effects = ctx.effects;
        h.keys("x");
        h.run(effects);
        assert_eq!(h.text(), LINES.replace("f\n", "fx\n"));
        assert!(h.core.current().modified());
        // Nor is one for a killed buffer.
        let mut ctx = h.ctx();
        ctx.call("revert-buffer", Args::default());
        let effects = ctx.effects;
        let id = h.core.windows.window.buffer();
        h.core.kill_buffer(id);
        h.run(effects);
        assert!(h.core.buffers.get(id).is_none());
    }

    #[test]
    fn a_read_only_buffer_reverts() {
        let mut h = reverting();
        crate::harness::buffer(&mut h).read_only = true;
        revert(&mut h, "z\n");
        assert_eq!(h.text(), "z\n");
        assert!(h.core.current().read_only);
    }

    #[test]
    fn reverting_a_modified_buffer_asks() {
        let mut h = reverting();
        h.keys("x");
        revert(&mut h, "z\n");
        let prompt = format!("Revert buffer from file {}? ", h.path("a.txt").display());
        assert_eq!(h.core.minibuffer().unwrap().prompt, prompt);
        assert_eq!(listed(&mut h).0, ["Yes", "No"]);
        h.keys("No RET");
        assert!(h.core.current().modified());
        h.keys("M-x revert-buffer RET y RET");
        assert_eq!(h.text(), "z\n");
    }

    #[test]
    fn revert_buffer_fails_without_a_file_to_take() {
        let mut h = Harness::new(&[]);
        h.keys("M-x revert-buffer RET");
        assert!(h.core.current().name.starts_with("*scratch*"));
        let no_file = "Buffer does not seem to be associated with any file";
        assert_eq!(h.core.message(), Some(no_file));
        let mut h = reverting();
        h.prepare_revert("a.txt", None);
        h.keys("M-x revert-buffer RET");
        let gone = format!("File {} no longer exists", h.path("a.txt").display());
        assert_eq!(h.core.message(), Some(gone.as_str()));
        revert(&mut h, "a\0");
        let binary = format!("{}: binary file", h.path("a.txt").display());
        assert_eq!(h.core.message(), Some(binary.as_str()));
        assert_eq!(h.text(), LINES);
    }

    /// The text of the current workspace's `*Diff*`, if there is one.
    fn diff_text(h: &Harness) -> Option<String> {
        let id = h.core.buffers.find_here("*Diff*")?;
        Some(h.core.buffers.get(id).unwrap().text().to_string())
    }

    /// The buffer names the windows show, the selected one first.
    fn shown(h: &Harness) -> Vec<String> {
        let ws = std::iter::once(&h.core.windows.window).chain(h.core.windows.others.values());
        let names = ws.map(|w| h.core.buffers.get(w.buffer()).unwrap().name.clone());
        names.collect()
    }

    #[test]
    fn diff_buffer_with_file_shows_the_disk_against_the_buffer_in_another_window() {
        let mut h = Harness::new(&[("a.txt", LINES)]);
        h.open("a.txt");
        h.keys("C-n C-n C-n C-n e");
        let diff = format!(
            "--- {}\n+++ a.txt\n@@ -2,7 +2,7 @@\n b\n c\n d\n-e\n+ee\n f\n g\n h\n",
            h.path("a.txt").display()
        );
        h.diffs.insert(h.path("a.txt"), Ok(diff.clone()));
        h.keys("M-x diff-buffer-with-file RET");
        assert_eq!(diff_text(&h), Some(diff.clone()));
        assert_eq!(shown(&h), ["a.txt", "*Diff*"]);
        let buffer = h.core.current();
        assert!(buffer.modified() && h.text() == LINES.replace("e\n", "ee\n"));
        // Read-only, with nothing to undo.
        h.keys("C-x o x");
        assert_eq!(h.core.message(), Some("Buffer is read-only"));
        assert!(!h.core.current().modified());
        h.keys("C-/");
        assert_eq!(h.core.message(), Some("No further undo information"));
        // A second run refills the same buffer in the same window.
        h.diffs
            .insert(h.path("a.txt"), Ok(diff.replace("+ee\n", "+\n")));
        h.keys("C-x o C-a C-k M-x diff-buffer-with-file RET");
        let diff = diff_text(&h).unwrap();
        assert!(diff.ends_with(" d\n-e\n+\n f\n g\n h\n"), "{diff}");
        assert_eq!(shown(&h), ["a.txt", "*Diff*"]);
        let diffs = h
            .core
            .buffers
            .recent()
            .filter(|(_, b)| b.major_mode == "diff-mode");
        assert_eq!(diffs.count(), 1);
    }

    #[test]
    fn diff_buffer_with_file_says_when_there_is_nothing_to_show() {
        let mut h = Harness::new(&[]);
        h.keys("M-x diff-buffer-with-file RET");
        let no_file = "Buffer does not seem to be associated with any file";
        assert_eq!(h.core.message(), Some(no_file));
        let mut h = reverting();
        h.diffs.insert(h.path("a.txt"), Ok(String::new()));
        h.keys("M-x diff-buffer-with-file RET");
        assert_eq!(h.core.message(), Some("No differences"));
        assert_eq!(shown(&h), ["a.txt"]);
        h.diffs.insert(
            h.path("a.txt"),
            Err(format!("{}: binary file", h.path("a.txt").display())),
        );
        h.keys("M-x diff-buffer-with-file RET");
        let binary = format!("{}: binary file", h.path("a.txt").display());
        assert_eq!(h.core.message(), Some(binary.as_str()));
        h.diffs.insert(
            h.path("a.txt"),
            Err(format!(
                "File {} no longer exists",
                h.path("a.txt").display()
            )),
        );
        h.keys("M-x diff-buffer-with-file RET");
        let gone = format!("File {} no longer exists", h.path("a.txt").display());
        assert_eq!(h.core.message(), Some(gone.as_str()));
        assert_eq!((diff_text(&h), shown(&h)), (None, vec!["a.txt".into()]));
    }

    #[test]
    fn a_diff_for_a_killed_buffer_or_another_workspace_is_dropped() {
        let mut h = reverting();
        h.diffs.insert(
            h.path("a.txt"),
            Ok("--- file\n+++ buffer\n-old\n+new\n".into()),
        );
        h.keys("x");
        let mut ctx = h.ctx();
        ctx.call("diff-buffer-with-file", Args::default());
        let effects = ctx.effects;
        let id = h.core.windows.window.buffer();
        h.core.kill_buffer(id);
        h.run(effects);
        assert!(h.core.buffers.find("*Diff*").is_none());
        let mut h = reverting();
        h.diffs.insert(
            h.path("a.txt"),
            Ok("--- file\n+++ buffer\n-old\n+new\n".into()),
        );
        h.keys("x");
        let mut ctx = h.ctx();
        ctx.call("diff-buffer-with-file", Args::default());
        let effects = ctx.effects;
        h.new_workspace("w", "home");
        h.run(effects);
        assert!(h.core.buffers.find("*Diff*").is_none());
    }

    #[test]
    fn opens_edits_and_saves_a_file() {
        let mut h = Harness::new(&[("a.txt", "hello")]);
        h.open("a.txt");
        assert_eq!(h.core.current().name, "a.txt");
        assert_eq!(h.core.current().file, Some(h.path("a.txt")));
        assert!(!h.core.current().modified());
        assert_eq!(h.point(), 0);
        h.keys("<right> X");
        assert!(h.core.current().modified());
        assert_eq!(h.core.current().disk, Some(crate::buffer::hash(b"hello")));
        h.save_reply("hXello\n");
        h.keys("C-x C-s");
        assert_eq!(h.submitted("a.txt"), "hXello\n");
        assert_eq!(h.text(), "hXello\n");
        assert_eq!(h.point(), 2);
        assert!(!h.core.current().modified());
        let wrote = format!("Wrote {}", h.path("a.txt").display());
        assert_eq!(h.core.message(), Some(wrote.as_str()));
        h.keys("C-x C-s");
        assert_eq!(h.core.message(), Some("(No changes need to be saved)"));
    }

    #[test]
    fn refuses_to_save_over_a_file_changed_on_disk() {
        let mut h = Harness::new(&[("a.txt", "old\n")]);
        h.open("a.txt");
        h.saves.push_back(Err(format!(
            "{} changed on disk; not saved",
            h.path("a.txt").display()
        )));
        h.keys("x C-x C-s");
        assert_eq!(h.core.current().disk, Some(crate::buffer::hash(b"old\n")));
        assert!(h.core.current().modified());
        assert!(
            h.core
                .message()
                .unwrap()
                .ends_with("changed on disk; not saved")
        );
        // A file that appeared after it was opened as missing.
        h.open("new.txt");
        h.saves.push_back(Err(format!(
            "{} changed on disk; not saved",
            h.path("new.txt").display()
        )));
        h.keys("x C-x C-s");
        assert!(h.core.current().modified());
        assert_eq!(h.core.current().disk, None);
    }

    #[test]
    fn saves_keep_the_bom_and_crlf() {
        let mut h = Harness::new(&[("a.txt", "\u{feff}a\r\nb\r\n")]);
        h.open("a.txt");
        assert_eq!(h.text(), "a\nb\n");
        assert_eq!(h.display(5).mode_line, "-- a.txt (DOS)  1:0");
        h.save_reply("\u{feff}xa\r\nb\r\n");
        h.keys("x C-x C-s");
        assert_eq!(
            h.core.current().disk,
            Some(crate::buffer::hash("\u{feff}xa\r\nb\r\n".as_bytes()))
        );
        assert!(h.core.current().crlf && h.core.current().bom);
        // The saved hash is the next save's base.
        h.save_reply("\u{feff}xya\r\nb\r\n");
        h.keys("y C-x C-s");
        assert_eq!(
            h.core.current().disk,
            Some(crate::buffer::hash("\u{feff}xya\r\nb\r\n".as_bytes()))
        );
    }

    #[test]
    fn edits_during_a_save_stay_unsaved() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        h.keys("x");
        let mut ctx = h.ctx();
        ctx.call("save-buffer", Args::default());
        let effects = ctx.effects;
        h.keys("y");
        h.save_reply("x\n");
        h.run(effects);
        assert_eq!(h.submitted("a.txt"), "x\n");
        assert!(h.core.current().modified());
        h.save_reply("xy\n");
        h.keys("C-x C-s");
        assert_eq!(h.submitted("a.txt"), "xy\n");
    }

    #[test]
    fn binary_and_non_utf8_files_open_empty_and_read_only() {
        for (name, bytes) in [("a.bin", &b"a\0b"[..]), ("l1.txt", b"caf\xe9")] {
            let mut h = Harness::new(&[]);
            h.read_reply(name, Some(bytes));
            h.open(name);
            assert_eq!(h.text(), "");
            assert!(h.core.message().unwrap().contains(", opened read-only"));
            assert_eq!(h.display(5).mode_line, format!("%% {name}  1:0"));
            h.keys("x");
            assert_eq!(h.core.message(), Some("Buffer is read-only"));
            h.keys("C-x C-s");
            assert!(h.core.current().read_only);
        }
    }

    #[test]
    fn missing_file_opens_empty_and_is_written_on_save() {
        let mut h = Harness::new(&[]);
        h.open("new.txt");
        assert_eq!(h.text(), "");
        assert_eq!(h.core.current().disk, None);
        h.save_reply("hi\n");
        h.keys("hi C-x C-s");
        assert_eq!(h.submitted("new.txt"), "hi\n");
    }

    #[test]
    fn failed_save_keeps_the_buffer_modified() {
        let mut h = Harness::new(&[]);
        h.open("no/such/dir/a.txt");
        h.saves.push_back(Err("No such file or directory".into()));
        h.keys("x C-x C-s");
        assert!(h.core.current().modified());
        assert_eq!(h.text(), "x\n");
        assert!(h.core.message().unwrap().contains("No such file"));
    }

    #[test]
    fn c_x_b_lists_buffers_most_recent_first_and_goes_back() {
        let mut h = Harness::new(&[("a", "1"), ("b", "2")]);
        h.open("a").open("b");
        h.core.buffers.create(" hidden");
        h.keys("C-x b");
        assert_eq!(
            listed(&mut h),
            (
                vec!["a".into(), "*scratch*<main>".into(), "b".into()],
                Some(0)
            )
        );
        h.keys("RET");
        assert_eq!(name(&h), "a");
        h.keys("C-x b RET");
        assert_eq!(name(&h), "b");
        h.keys("C-x b notes RET");
        assert_eq!((name(&h), h.text().as_str()), ("notes", ""));
    }

    #[test]
    fn c_x_b_previews_the_selected_buffer() {
        let mut h = Harness::new(&[("a", "1"), ("b", "2")]);
        h.open("a").open("b");
        let shown = |h: &Harness| {
            let id = h.core.windows.others[&h.core.windows.tree.leaves()[0]].buffer();
            h.core.buffers.get(id).unwrap().name.clone()
        };
        h.keys("C-x b");
        assert_eq!(shown(&h), "a", "the previous buffer shows at once");
        h.keys("C-n");
        assert_eq!(shown(&h), "*scratch*<main>");
        h.keys("C-g");
        assert_eq!(name(&h), "b");
        h.keys("C-x b");
        // `Quit` made `*Messages*`.
        assert_eq!(
            listed(&mut h).0,
            ["a", "*scratch*<main>", "*Messages*", "b"]
        );
        for keys in ["C-x 4 b", "C-x k"] {
            h.keys(&format!("C-g {keys} C-n"));
            assert_eq!(shown(&h), "b", "{keys} does not preview");
        }
    }

    #[test]
    fn c_x_b_and_c_x_k_see_only_this_workspace_s_buffers() {
        // `x` matches neither `*scratch*<w>` nor `*Messages*`, so `RET` takes it as typed.
        let mut h = Harness::new(&[("x", "1")]);
        h.open("x");
        let main = h.core.workspace();
        let w = h.new_workspace("w", "p");
        h.keys("C-x k x RET");
        assert_eq!(h.core.message(), Some("No such buffer x"));
        h.keys("C-x b");
        assert_eq!(listed(&mut h).0, ["*Messages*", "*scratch*<w>"]);
        h.keys("C-g C-x b x RET");
        assert_eq!(
            (name(&h), h.text().as_str(), h.core.current().workspace),
            ("x<2>", "", Some(w))
        );
        let x = h.core.buffers.find("x").unwrap();
        let x = h.core.buffers.get(x).unwrap();
        assert_eq!(
            (x.text().to_string(), x.workspace),
            ("1".into(), Some(main))
        );
        h.keys("C-x 4 b");
        assert!(!listed(&mut h).0.contains(&"x".into()));
    }

    #[test]
    fn two_files_with_one_name_get_unique_buffer_names() {
        let mut h = Harness::new(&[("a/x", ""), ("b/x", "")]);
        h.open("a/x").open("b/x");
        assert_eq!(h.core.buffer_names(true), ["x<2>", "x", "*scratch*<main>"]);
    }

    #[test]
    fn switching_back_restores_point() {
        let mut h = Harness::new(&[("a", "hello")]);
        h.open("a");
        h.keys("C-f C-f C-f C-x b RET");
        assert_eq!((name(&h), h.point()), ("*scratch*<main>", 0));
        h.keys("C-x b a RET");
        assert_eq!((name(&h), h.point()), ("a", 3));
    }

    #[test]
    fn c_x_4_b_switches_the_next_window() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a");
        h.keys("C-x 4 b RET");
        let leaves = h.core.windows.tree.leaves();
        assert_eq!((leaves.len(), h.core.windows.selected), (2, leaves[1]));
        assert_eq!(name(&h), "*scratch*<main>");
        let upper = h.core.windows.others[&leaves[0]].buffer();
        assert_eq!(h.core.buffers.get(upper).unwrap().name, "a");
        // With two windows it uses the next one rather than splitting again.
        h.keys("C-x 4 b RET");
        assert_eq!(
            (h.core.windows.tree.leaves().len(), h.core.windows.selected),
            (2, leaves[0])
        );
    }

    #[test]
    fn c_x_k_kills_the_current_buffer_in_every_window() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a");
        h.keys("C-x 2 C-x k");
        assert_eq!(listed(&mut h).0, ["a", "*scratch*<main>"]);
        h.keys("RET");
        assert_eq!(
            (name(&h), h.core.buffers.find("a")),
            ("*scratch*<main>", None)
        );
        let other = h.core.windows.others[&WindowId(1)].buffer();
        assert_eq!(h.core.buffers.get(other).unwrap().name, "*scratch*<main>");
        h.keys("C-x k nope RET");
        assert_eq!(h.core.message(), Some("No such buffer nope"));
        // That message made *Messages*.
        assert_eq!(h.core.buffers.recent().count(), 2);
        h.keys("C-x k *Messages* RET");
        assert_eq!(h.core.message(), Some("Can't kill *Messages*"));
        h.keys("x C-x k RET");
        assert_eq!(h.core.message(), Some("Can't kill *scratch*<main>"));
        assert_eq!((name(&h), h.text().as_str()), ("*scratch*<main>", "x"));
        assert_eq!(h.core.buffers.recent().count(), 2);
    }

    #[test]
    fn killing_a_modified_file_buffer_asks() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a");
        h.keys("x C-x k RET");
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Buffer modified; kill anyway? "
        );
        assert_eq!(listed(&mut h).0, ["no", "yes"]);
        h.keys("RET");
        assert_eq!((name(&h), h.text().as_str()), ("a", "x1"));
        h.keys("C-x k RET yes RET");
        assert_eq!(name(&h), "*scratch*<main>");
        assert_eq!(h.core.buffers.find("a"), None);
    }

    #[test]
    fn quitting_with_nothing_modified_quits_at_once() {
        for keys in ["C-x C-c", "s-q"] {
            let mut h = Harness::new(&[]);
            h.keys(keys);
            assert!(h.quit, "{keys}");
        }
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a");
        // `*scratch*` visits no file, so it is not asked about.
        h.keys("C-x b RET x C-x C-c");
        assert!(h.core.minibuffer().is_none());
        assert!(h.quit);
    }

    #[test]
    fn quitting_offers_to_save_each_modified_file() {
        let mut h = Harness::new(&[("a", "1"), ("b", "2")]);
        h.open("a").keys("x").open("b").keys("y C-x C-c");
        let prompt = |h: &mut Harness, name: &str| {
            let path = h.path(name);
            assert_eq!(
                h.core.minibuffer().unwrap().prompt,
                format!("Save file {}? ", path.display())
            );
        };
        h.save_reply("y2\n");
        h.save_reply("x1\n");
        prompt(&mut h, "b");
        assert_eq!(listed(&mut h).0, ["Yes", "No"]);
        h.keys("RET");
        prompt(&mut h, "a");
        // Lowercase text still picks a candidate.
        h.keys("y RET");
        assert_eq!(
            (h.submitted("a").as_str(), h.submitted("b").as_str()),
            ("x1\n", "y2\n")
        );
        assert!(h.quit);
    }

    #[test]
    fn confirmations_keep_no_history() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a").keys("x C-x C-c no RET no RET C-x C-c M-p");
        assert!(
            h.core
                .minibuffer()
                .unwrap()
                .prompt
                .starts_with("Save file ")
        );
        assert_eq!(
            h.core.message(),
            Some("Beginning of history; no preceding item")
        );
    }

    #[test]
    fn declining_keeps_the_editor_open_unless_confirmed() {
        let mut h = Harness::new(&[("a", "1"), ("b", "2")]);
        h.save_reply("x1\n");
        h.open("a").keys("x").open("b").keys("y C-x C-c no RET RET");
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Modified buffers exist; exit anyway? "
        );
        assert_eq!(listed(&mut h).0, ["No", "Yes"]);
        h.keys("RET");
        // The answered `Yes` is still saved; the declined buffer is left alone.
        assert!(!h.quit && h.core.minibuffer().is_none());
        assert_eq!(h.writes, [(h.path("a"), "x1\n".into())]);
        assert!(h.core.current().modified());
        h.keys("C-x C-c no RET yes RET");
        assert!(h.quit);
        assert_eq!(h.core.current().disk, Some(crate::buffer::hash(b"2")));
    }

    #[test]
    fn a_failed_save_cancels_the_quit() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a").keys("x");
        h.saves.push_back(Err(format!(
            "{} changed on disk; not saved",
            h.path("a").display()
        )));
        h.keys("C-x C-c RET");
        let msg = format!("{} changed on disk; not saved", h.path("a").display());
        assert_eq!(h.core.message(), Some(msg.as_str()));
        assert!(!h.quit);
        // A later save that succeeds does not quit.
        h.save_reply("x1\n");
        h.keys("C-x C-s");
        assert!(!h.quit);
    }

    #[test]
    fn a_close_request_quits_the_same_way() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a").keys("x");
        h.send(Msg::Quit);
        h.save_reply("x1\n");
        assert!(h.core.minibuffer().is_some() && !h.quit);
        h.keys("RET");
        assert!(h.quit);
    }

    /// A harness visiting `d/a.txt`, with `d` holding `a.txt`, `b.txt` and `sub/c.txt`.
    fn in_dir() -> Harness {
        let mut h = Harness::new(&[("d/a.txt", "a"), ("d/b.txt", "hi"), ("d/sub/c.txt", "")]);
        h.listing("d", &["a.txt", "b.txt", "sub/"]);
        h.listing("d/sub", &["c.txt"]);
        h.read_reply("d/bt", None);
        h.reads.insert(
            h.path("d/"),
            Err(format!("{}: Is a directory", h.path("d/").display())),
        );
        h.open("d/a.txt");
        h
    }

    /// Types the temp dir's path for `name` as the platform's text input would.
    fn type_path(h: &mut Harness, name: &str) {
        let text = typed(h, name);
        h.send(Msg::Insert {
            text,
            replace: None,
        });
    }

    /// The temp dir's path for `name`, as typed at a prompt.
    fn typed(h: &Harness, name: &str) -> String {
        h.path(name).to_string_lossy().into_owned()
    }

    #[test]
    fn find_file_starts_at_the_buffer_directory_and_lists_it() {
        let mut h = in_dir();
        h.keys("C-x C-f");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Find file: ");
        assert_eq!(h.text(), typed(&h, "d/"));
        assert_eq!(listed(&mut h).0, ["a.txt", "b.txt", "sub/"]);
        h.keys("b");
        assert_eq!(listed(&mut h).0, ["b.txt", "sub/"]);
        h.keys("DEL sub/");
        assert_eq!(listed(&mut h).0, ["c.txt"]);
        h.keys("C-g");
        // A buffer without a file starts at the working directory.
        h.keys("C-x b *scratch* RET C-x C-f");
        let cwd = std::env::current_dir().unwrap().join("");
        assert_eq!(h.text(), cwd.to_string_lossy());
    }

    #[test]
    fn find_file_previews_the_selected_file_but_not_a_directory() {
        let mut h = in_dir();
        h.read_reply("d/bin", Some(b"a\0b".as_ref()));
        let shown = |h: &Harness| {
            let id = h.core.windows.others[&h.core.windows.tree.leaves()[0]].buffer();
            h.core.buffers.get(id).unwrap().text().to_string()
        };
        h.keys("C-x C-f b.txt");
        assert_eq!(shown(&h), "hi");
        h.keys("C-a C-k");
        type_path(&mut h, "d/sub/");
        assert_eq!(shown(&h), "", "c.txt is empty");
        h.keys("C-a C-k");
        type_path(&mut h, "d/su");
        assert_eq!(listed(&mut h).0, ["sub/"]);
        assert_eq!(shown(&h), "a", "a directory shows the window as it was");
        h.keys("bin");
        assert_eq!(shown(&h), "a", "a binary file too");
    }

    #[test]
    fn find_file_visits_an_existing_or_a_new_file() {
        let mut h = in_dir();
        h.keys("C-x C-f b.txt RET");
        assert_eq!((name(&h), h.text().as_str()), ("b.txt", "hi"));
        assert_eq!(h.core.current().file, Some(h.path("d/b.txt")));
        assert!(!h.core.current().modified());
        // M-RET keeps a new name that fuzzy-matches b.txt.
        h.keys("C-x C-f bt M-RET");
        assert_eq!((name(&h), h.text().as_str()), ("bt", ""));
        assert_eq!(h.core.current().disk, None);
        h.save_reply("x\n");
        h.keys("x C-x C-s");
        assert_eq!(h.submitted("d/bt"), "x\n");
    }

    #[test]
    fn tab_completes_and_ret_enters_a_directory() {
        let mut h = in_dir();
        h.keys("C-x C-f b TAB");
        assert_eq!(h.text(), typed(&h, "d/b.txt"));
        h.keys("C-a C-k");
        type_path(&mut h, "d/s");
        h.keys("RET");
        assert_eq!(h.text(), typed(&h, "d/sub/"));
        assert_eq!(listed(&mut h).0, ["c.txt"]);
        h.keys("RET");
        assert_eq!(name(&h), "c.txt");
    }

    #[test]
    fn find_file_switches_to_a_visited_file() {
        let mut h = in_dir();
        h.keys("x C-x b *scratch* RET C-x C-f C-a C-k");
        type_path(&mut h, "d/a.txt");
        h.keys("RET");
        assert_eq!((name(&h), h.text().as_str()), ("a.txt", "xa"));
        assert_eq!(h.core.buffers.recent().count(), 2);
        // An obsolete read leaves the reused buffer intact.
        h.send(Msg::Opened {
            intent: crate::VisitIntent {
                request: 0,
                workspace: h.core.workspace(),
                window: h.core.windows.selected,
            },
            path: h.path("d/a.txt"),
            result: Ok(None),
        });
        assert_eq!(
            (h.core.buffers.recent().count(), h.text().as_str()),
            (2, "xa")
        );
    }

    #[test]
    fn find_file_refuses_binary_files_and_directories() {
        let mut h = in_dir();
        h.read_reply("d/bin", Some(b"a\0b".as_ref()));
        h.keys("C-x C-f bin M-RET");
        assert_eq!(name(&h), "a.txt");
        let msg = format!("{}: binary file", h.path("d/bin").display());
        assert_eq!(h.core.message(), Some(msg.as_str()));
        h.keys("C-x C-f M-RET");
        assert_eq!(name(&h), "a.txt");
        assert!(h.core.message().unwrap().starts_with(&typed(&h, "d/")));
        // a.txt, *scratch* and *Messages*.
        assert_eq!(h.core.buffers.recent().count(), 3);
    }

    #[test]
    fn the_completion_list_keeps_its_height_while_open() {
        let names: Vec<_> = (0..10)
            .map(|i| format!("d/ten/{i}"))
            .chain((0..5).map(|i| format!("d/five/{i}")))
            .collect();
        let files: Vec<_> = names
            .iter()
            .map(|n| (n.as_str(), ""))
            .chain([("d/a.txt", "")])
            .collect();
        let mut h = Harness::new(&files);
        h.listing("d", &["a.txt", "five/", "ten/"]);
        h.listing("d/ten", &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]);
        h.listing("d/five", &["0", "1", "2", "3", "4"]);
        h.open("d/a.txt");
        h.keys("C-x C-f ten/");
        let mb = h.core.minibuffer().unwrap();
        assert_eq!((mb.candidates.len(), mb.rows), (10, 10));
        h.keys("DEL five/");
        let mb = h.core.minibuffer().unwrap();
        assert_eq!((mb.candidates.len(), mb.rows), (5, 10));
        h.keys("C-g");
        ask_completing(&mut h, fruit);
        h.keys("C-c p");
        assert_eq!(
            h.core.minibuffer().unwrap().rows,
            3,
            "the next minibuffer starts afresh"
        );
    }

    #[test]
    fn a_listing_error_shows_and_a_late_listing_is_dropped() {
        let mut h = in_dir();
        h.listings.insert(
            h.path("d/nope/"),
            Err(format!(
                "{}: missing directory",
                h.path("d/nope/").display()
            )),
        );
        h.keys("C-x C-f nope/");
        assert!(h.core.message().unwrap().starts_with(&typed(&h, "d/nope/")));
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
        h.send(Msg::Listed {
            request: 1,
            result: Ok(vec!["stale".into()]),
        });
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
    }

    /// The ids of the listings the core asked for since the last call, with collected effects.
    fn listing_requests(h: &mut Harness) -> Vec<u64> {
        (h.effects.drain(..))
            .filter_map(|e| match e {
                crate::Effect::File(crate::FileOp::ListDir { request, .. }) => Some(request),
                _ => None,
            })
            .collect()
    }

    fn deliver(h: &mut Harness, request: u64, names: &[&str]) {
        let result = Ok(names.iter().map(|n| (*n).into()).collect());
        h.send(Msg::Listed { request, result });
    }

    #[test]
    fn a_reopened_prompt_ignores_the_old_listing() {
        let mut h = in_dir();
        h.collect_effects = true;
        h.keys("C-x C-f");
        let first = listing_requests(&mut h);
        h.keys("C-g C-x C-f");
        let second = listing_requests(&mut h);
        h.collect_effects = false;
        deliver(&mut h, first[0], &["old.txt"]);
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
        deliver(&mut h, second[0], &["new.txt"]);
        assert_eq!(listed(&mut h).0, ["new.txt"]);
    }

    #[test]
    fn a_listing_from_before_the_text_came_back_is_ignored() {
        let mut h = in_dir();
        h.collect_effects = true;
        h.keys("C-x C-f");
        let first = listing_requests(&mut h);
        h.keys("s u b / DEL");
        let later = listing_requests(&mut h);
        assert_eq!(later.len(), 2, "sub/, then the directory again");
        h.collect_effects = false;
        deliver(&mut h, later[1], &["new.txt"]);
        deliver(&mut h, first[0], &["old.txt"]);
        deliver(&mut h, later[0], &["c.txt"]);
        assert_eq!(listed(&mut h).0, ["new.txt"]);
    }

    #[test]
    fn a_relative_name_lists_and_visits_from_the_buffer_directory() {
        let mut h = in_dir();
        h.keys("C-x C-f C-a C-k sub/");
        assert_eq!(listed(&mut h).0, ["c.txt"]);
        h.keys("RET");
        assert_eq!(h.core.current().file, Some(h.path("d/sub/c.txt")));
    }

    #[test]
    fn m_ret_returns_the_text_as_typed() {
        let mut h = on("");
        ask_completing(&mut h, fruit);
        h.keys("C-c p ch M-RET");
        assert!(h.core.message().unwrap().contains(&strings(&["ch"])));
    }

    #[test]
    fn del_deletes_a_whole_directory_in_a_file_prompt() {
        let mut h = in_dir();
        h.keys("C-x C-f sub/");
        h.keys("DEL");
        assert_eq!(h.text(), typed(&h, "d/"));
        h.keys("sub");
        h.keys("DEL");
        assert_eq!(h.text(), typed(&h, "d/"));
        // A file, a name nothing lists, or point before the end: one char.
        h.keys("b.txt DEL");
        assert_eq!(h.text(), typed(&h, "d/b.tx"));
        h.keys("C-a C-k");
        type_path(&mut h, "d/sub/");
        h.keys("<left> DEL");
        assert_eq!(h.text(), typed(&h, "d/su/"));
        // A prefix argument or an active region keeps plain DEL.
        h.keys("C-e b/ C-u 1 DEL");
        assert_eq!(h.text(), typed(&h, "d/su/b"));
        h.keys("/ C-SPC DEL");
        assert_eq!(h.text(), typed(&h, "d/su/b"));
        // Other completing prompts delete a char.
        h.keys("C-g");
        ask_completing(&mut h, fruit);
        h.keys("C-c p ab/ DEL");
        assert_eq!(h.text(), "ab");
    }
}
