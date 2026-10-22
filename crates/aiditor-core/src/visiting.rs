//! Visiting, saving, switching and killing buffers, and quitting.

use std::path::{Path, PathBuf};

use crate::buffer::{self, Buffer, BufferId, Buffers};
use crate::command::Args;
use crate::workspace::WorkspaceId;
use crate::{Core, Ctx, Effect, FileOp};

impl Ctx<'_> {
    /// Handles the file service's answer to a save of buffer `id` at `tick`, and the quit
    /// waiting on it.
    pub(crate) fn on_saved(&mut self, id: BufferId, tick: u64, result: Result<u64, String>) {
        let ok = result.is_ok();
        match (self.core.buffers.get_mut(id), result) {
            (None, _) => {}
            (Some(buffer), Ok(hash)) => {
                buffer.disk = Some(hash);
                // Edits made while the write ran are still unsaved.
                if buffer.tick() == tick {
                    buffer.set_saved();
                }
                let path = buffer.file.as_deref().unwrap_or(Path::new(""));
                let msg = format!("Wrote {}", path.display());
                self.core.echo(msg);
            }
            (Some(_), Err(err)) => self.core.echo(err),
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

    /// Kills the buffer `name`, asking first when it is a modified file buffer; an unkillable
    /// one is refused.
    pub(crate) fn kill_buffer(&mut self, args: Args) -> Result<(), String> {
        let name = &args.strings[0];
        let id =
            (self.core.buffers.find_here(name)).ok_or_else(|| format!("No such buffer {name}"))?;
        let buffer = self.core.buffers.get(id).expect("found");
        if buffer.unkillable {
            return Err(format!("Can't kill {name}"));
        }
        if buffer.file.is_some() && buffer.modified() {
            match args.strings.get(1).map(String::as_str) {
                None => {
                    let command = self.core.commands["kill-buffer"];
                    return self.read_string(
                        command,
                        args,
                        "Buffer modified; kill anyway? ".into(),
                        Some(&["no", "yes"]),
                    );
                }
                Some("yes") => {}
                Some(_) => return Ok(()),
            }
        }
        self.core.kill_buffer(id);
        Ok(())
    }

    /// Visits `name`, taken from the current buffer's directory: switches to a buffer visiting
    /// it, or asks the app to read it.
    pub(crate) fn find_file(&mut self, name: &str) -> Result<(), String> {
        let path = self.core.directory().join(name);
        match self.core.buffers.visiting(&path) {
            Some(id) => self.core.switch_to(id),
            None => self.effects.push(Effect::File(FileOp::Open(path))),
        }
        Ok(())
    }

    /// Offers to save each modified file buffer, then quits once those saves succeed.
    pub(crate) fn save_buffers_kill_terminal(&mut self, args: Args) -> Result<(), String> {
        let command = "save-buffers-kill-terminal";
        let Some(saving) = self.save_some(command, args, 0, |_| true, "exit")? else {
            return Ok(());
        };
        match saving.is_empty() {
            true => self.effects.push(Effect::Quit),
            false => (self.core.quitting, self.core.then_kill) = (saving, None),
        }
        Ok(())
    }

    /// Asks `command`'s questions about the modified file buffers `of` takes: whether to save
    /// each, then, if any was declined, whether to `verb` anyway. The strings after the first
    /// `skip` are name/answer pairs for the buffers asked about, then the `verb anyway` answer.
    /// Starts the saves answered `Yes` and returns them once the command should go ahead;
    /// `None` while asking or when declined.
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
        for &id in &saving {
            self.save(id)?;
        }
        // Declining to go ahead still saves the buffers answered `Yes`.
        Ok((!declined || last_yes).then_some(saving))
    }

    pub(crate) fn save(&mut self, id: BufferId) -> Result<(), String> {
        let buffer = self.core.buffers.get_mut(id).expect("saved buffer is live");
        let Some(path) = buffer.file.clone() else {
            tracing::info!(name = buffer.name, "buffer visits no file");
            return Ok(());
        };
        if !buffer.modified() {
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
    /// The current buffer's directory: its file's, else the working directory.
    pub(crate) fn directory(&self) -> PathBuf {
        match self.current().file.as_deref().and_then(Path::parent) {
            Some(dir) => dir.to_path_buf(),
            None => std::env::current_dir().unwrap_or_default(),
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
        if self.window.buffer() != id {
            self.push_jump();
        }
        self.show_buffer(id);
    }

    /// Shows buffer `id` in the selected window and makes it the most recent.
    pub(crate) fn show_buffer(&mut self, id: BufferId) {
        self.buffers.raise(id);
        if self.window.buffer() != id {
            self.window.show(&mut self.buffers, id);
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
        let current = self.window.buffer();
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
        let next = replacement(&self.buffers, id, self.buffers.workspace);
        // Killed first, so a new scratch gets its name.
        _ = self.buffers.kill(id);
        let next = next.unwrap_or_else(|| {
            let name = self.current_workspace().name.clone();
            self.buffers.create_scratch(&name)
        });
        for window in std::iter::once(&mut self.window).chain(self.others.values_mut()) {
            if window.buffer() == id {
                window.show(&mut self.buffers, next);
            }
        }
        for workspace in &mut self.workspaces {
            let Some(windows) = &mut workspace.windows else {
                continue;
            };
            // Every live workspace keeps its `*scratch*`, so `next` is only a guard.
            let theirs = replacement(&self.buffers, id, workspace.id).unwrap_or(next);
            for window in windows.windows_mut() {
                if window.buffer() == id {
                    window.show(&mut self.buffers, theirs);
                }
            }
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
        assert_eq!(h.disk("a.txt"), "hello");
        h.keys("C-x C-s");
        assert_eq!(h.disk("a.txt"), "hXello\n");
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
        std::fs::write(h.path("a.txt"), "theirs\n").unwrap();
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "theirs\n");
        assert!(h.core.current().modified());
        assert!(
            h.core
                .message()
                .unwrap()
                .ends_with("changed on disk; not saved")
        );
        // A file that appeared after it was opened as missing.
        h.open("new.txt");
        std::fs::write(h.path("new.txt"), "theirs").unwrap();
        h.keys("x C-x C-s");
        assert_eq!(h.disk("new.txt"), "theirs");
    }

    #[test]
    fn saves_keep_the_bom_and_crlf() {
        let mut h = Harness::new(&[("a.txt", "\u{feff}a\r\nb\r\n")]);
        h.open("a.txt");
        assert_eq!(h.text(), "a\nb\n");
        assert_eq!(h.display(5).mode_line, "-- a.txt (DOS)  1:0");
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "\u{feff}xa\r\nb\r\n");
        // The saved hash is the next save's base.
        h.keys("y C-x C-s");
        assert_eq!(h.disk("a.txt"), "\u{feff}xya\r\nb\r\n");
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
        h.run(effects);
        assert_eq!(h.disk("a.txt"), "x\n");
        assert!(h.core.current().modified());
        h.keys("C-x C-s");
        assert_eq!(h.disk("a.txt"), "xy\n");
    }

    #[test]
    fn binary_and_non_utf8_files_open_empty_and_read_only() {
        for (name, bytes) in [("a.bin", &b"a\0b"[..]), ("l1.txt", b"caf\xe9")] {
            let mut h = Harness::new(&[]);
            std::fs::write(h.path(name), bytes).unwrap();
            h.open(name);
            assert_eq!(h.text(), "");
            assert!(h.core.message().unwrap().contains(", opened read-only"));
            assert_eq!(h.display(5).mode_line, format!("%% {name}  1:0"));
            h.keys("x");
            assert_eq!(h.core.message(), Some("Buffer is read-only"));
            h.keys("C-x C-s");
            assert_eq!(std::fs::read(h.path(name)).unwrap(), bytes);
        }
    }

    #[test]
    fn missing_file_opens_empty_and_is_written_on_save() {
        let mut h = Harness::new(&[]);
        h.open("new.txt");
        assert_eq!(h.text(), "");
        assert!(!h.path("new.txt").exists());
        h.keys("hi C-x C-s");
        assert_eq!(h.disk("new.txt"), "hi\n");
    }

    #[test]
    fn failed_save_keeps_the_buffer_modified() {
        let mut h = Harness::new(&[]);
        h.open("no/such/dir/a.txt");
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
            let id = h.core.others[&h.core.tree.leaves()[0]].buffer();
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
        let w = h.core.new_workspace("w", "p");
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
        let leaves = h.core.tree.leaves();
        assert_eq!((leaves.len(), h.core.selected), (2, leaves[1]));
        assert_eq!(name(&h), "*scratch*<main>");
        let upper = h.core.others[&leaves[0]].buffer();
        assert_eq!(h.core.buffers.get(upper).unwrap().name, "a");
        // With two windows it uses the next one rather than splitting again.
        h.keys("C-x 4 b RET");
        assert_eq!(
            (h.core.tree.leaves().len(), h.core.selected),
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
        let other = h.core.others[&WindowId(1)].buffer();
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
        prompt(&mut h, "b");
        assert_eq!(listed(&mut h).0, ["Yes", "No"]);
        h.keys("RET");
        prompt(&mut h, "a");
        // Lowercase text still picks a candidate.
        h.keys("y RET");
        assert_eq!(
            (h.disk("a").as_str(), h.disk("b").as_str()),
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
        h.open("a").keys("x").open("b").keys("y C-x C-c no RET RET");
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Modified buffers exist; exit anyway? "
        );
        assert_eq!(listed(&mut h).0, ["No", "Yes"]);
        h.keys("RET");
        // The answered `Yes` is still saved; the declined buffer is left alone.
        assert!(!h.quit && h.core.minibuffer().is_none());
        assert_eq!((h.disk("a").as_str(), h.disk("b").as_str()), ("x1\n", "2"));
        assert!(h.core.current().modified());
        h.keys("C-x C-c no RET yes RET");
        assert!(h.quit);
        assert_eq!(h.disk("b"), "2");
    }

    #[test]
    fn a_failed_save_cancels_the_quit() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a").keys("x");
        std::fs::write(h.path("a"), "theirs").unwrap();
        h.keys("C-x C-c RET");
        let msg = format!("{} changed on disk; not saved", h.path("a").display());
        assert_eq!(h.core.message(), Some(msg.as_str()));
        assert!(!h.quit);
        // A later save that succeeds does not quit.
        std::fs::write(h.path("a"), "1").unwrap();
        h.keys("C-x C-s");
        assert!(!h.quit);
    }

    #[test]
    fn a_close_request_quits_the_same_way() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a").keys("x");
        h.send(Msg::Quit);
        assert!(h.core.minibuffer().is_some() && !h.quit);
        h.keys("RET");
        assert!(h.quit);
    }

    /// A harness visiting `d/a.txt`, with `d` holding `a.txt`, `b.txt` and `sub/c.txt`.
    fn in_dir() -> Harness {
        let mut h = Harness::new(&[("d/a.txt", "a"), ("d/b.txt", "hi"), ("d/sub/c.txt", "")]);
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
        std::fs::write(h.path("d/bin"), b"a\0b").unwrap();
        let shown = |h: &Harness| {
            let id = h.core.others[&h.core.tree.leaves()[0]].buffer();
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
        assert!(!h.path("d/bt").exists());
        h.keys("x C-x C-s");
        assert_eq!(h.disk("d/bt"), "x\n");
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
        // A read that raced another find of the same file switches too.
        h.send(Msg::Opened {
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
        std::fs::write(h.path("d/bin"), b"a\0b").unwrap();
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
        h.keys("C-x C-f nope/");
        assert!(h.core.message().unwrap().starts_with(&typed(&h, "d/nope/")));
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
        h.send(Msg::Listed {
            dir: h.path("d"),
            result: Ok(vec!["stale".into()]),
        });
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
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
