//! Visiting, saving, switching and killing buffers, and quitting.

use std::path::{Path, PathBuf};

use crate::buffer::{self, Buffer, BufferId};
use crate::command::Args;
use crate::{Core, Ctx, Effect};

impl Ctx<'_> {
    /// Kills the buffer `name`, asking first when it is a modified file buffer.
    pub(crate) fn kill_buffer(&mut self, args: Args) -> Result<(), String> {
        let name = &args.strings[0];
        let id = (self.core.buffers.find(name)).ok_or_else(|| format!("No such buffer {name}"))?;
        let buffer = self.core.buffers.get(id).expect("found");
        if buffer.file.is_some() && buffer.modified() {
            match args.strings.get(1).map(String::as_str) {
                None => {
                    let command = self.core.commands["kill-buffer"];
                    return self.read_string(
                        command,
                        args,
                        "Buffer modified; kill anyway? ".into(),
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
            None => self.effects.push(Effect::Open(path)),
        }
        Ok(())
    }

    /// Offers to save each modified file buffer, then quits once those saves succeed. The
    /// strings are name/answer pairs for the buffers asked about, then the `exit anyway` answer.
    pub(crate) fn save_buffers_kill_terminal(&mut self, mut args: Args) -> Result<(), String> {
        let command = self.core.commands["save-buffers-kill-terminal"];
        let asked = |name: &str| args.strings.as_chunks::<2>().0.iter().any(|p| p[0] == name);
        let next = (self.core.buffers.recent())
            .find(|(_, b)| b.file.is_some() && b.modified() && !asked(&b.name))
            .map(|(_, b)| (b.name.clone(), b.file.clone().expect("a file buffer")));
        if args.strings.len().is_multiple_of(2)
            && let Some((name, path)) = next
        {
            args.strings.push(name);
            return self.read_string(command, args, format!("Save file {}? ", path.display()));
        }
        let yes: Vec<String> = (args.strings.as_chunks::<2>().0.iter())
            .filter(|p| p[1].eq_ignore_ascii_case("yes"))
            .map(|p| p[0].clone())
            .collect();
        let declined = yes.len() < args.strings.len() / 2;
        if declined && args.strings.len().is_multiple_of(2) {
            let prompt = "Modified buffers exist; exit anyway? ".into();
            return self.read_string(command, args, prompt);
        }
        let core = &mut *self.core;
        let saving: Vec<BufferId> = (yes.iter())
            .filter_map(|name| core.buffers.find(name))
            .filter(|&id| core.buffers.get(id).is_some_and(Buffer::modified))
            .collect();
        for &id in &saving {
            self.save(id)?;
        }
        // Declining to exit still saves the buffers answered `Yes`.
        if declined
            && args
                .strings
                .last()
                .is_none_or(|a| !a.eq_ignore_ascii_case("yes"))
        {
            return Ok(());
        }
        match saving.is_empty() {
            true => self.effects.push(Effect::Quit),
            false => self.core.quitting = saving,
        }
        Ok(())
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
        self.effects.push(Effect::Save {
            buffer: id,
            tick: buffer.tick(),
            path,
            bytes: buffer.encode(),
            hash: buffer.disk,
        });
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

    /// Shows buffer `id` in the selected window and makes it the most recent.
    pub(crate) fn switch_to(&mut self, id: BufferId) {
        self.buffers.raise(id);
        if self.window.buffer() != id {
            self.window.show(&mut self.buffers, id);
        }
    }

    /// Switches to the buffer `name`, making an empty one if there is none.
    pub(crate) fn switch_to_name(&mut self, name: &str) {
        let id = (self.buffers.find(name)).unwrap_or_else(|| self.buffers.create(name));
        self.switch_to(id);
    }

    /// The names of the buffers not hidden by a leading space, most recently shown first, with
    /// the current buffer first or last.
    pub(crate) fn buffer_names(&self, current_first: bool) -> Vec<String> {
        let current = self.window.buffer();
        let listed = self
            .buffers
            .recent()
            .filter(|(_, b)| !b.name.starts_with(' '));
        let (this, others): (Vec<_>, Vec<_>) = listed.partition(|(id, _)| *id == current);
        let names = match current_first {
            true => [this, others].concat(),
            false => [others, this].concat(),
        };
        names.into_iter().map(|(_, b)| b.name.clone()).collect()
    }

    /// Kills buffer `id`; windows showing it show the most recent listed buffer left, or a new
    /// `*scratch*`.
    fn kill_buffer(&mut self, id: BufferId) {
        let next = (self.buffers.recent())
            .find(|&(b, buffer)| b != id && !buffer.name.starts_with(' '))
            .map(|(b, _)| b);
        // Killed first, so a new `*scratch*` gets its name.
        _ = self.buffers.kill(id);
        let next = next.unwrap_or_else(|| self.buffers.create("*scratch*"));
        for window in std::iter::once(&mut self.window).chain(self.others.values_mut()) {
            if window.buffer() == id {
                window.show(&mut self.buffers, next);
            }
        }
    }
}
