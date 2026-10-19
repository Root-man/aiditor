//! Auto-saving buffers and recovering them after a crash.

use std::time::{Duration, Instant};

use crate::buffer::{Buffer, BufferId};
use crate::command::Args;
use crate::{Core, Ctx, Effect, Effects, Msg};

/// How long after the last edit buffers are auto-saved, and the most edits wait while typing goes on.
const AUTOSAVE_IDLE: Duration = Duration::from_secs(2);
const AUTOSAVE_MAX: Duration = Duration::from_secs(30);

impl Ctx<'_> {
    /// Asks about the first recovered file, then handles its answer (the only string) and asks
    /// about the next: `Yes` opens it as a modified buffer, anything else deletes it.
    pub(crate) fn recover_buffers(&mut self, args: Args) -> Result<(), String> {
        let core = &mut *self.core;
        if let Some(answer) = args.strings.first() {
            let r = core.recovered.remove(0);
            let mut keep = false;
            if answer.eq_ignore_ascii_case("yes") {
                let id = match &r.path {
                    Some(path) => {
                        core.visit(path, r.disk, false);
                        core.window.buffer()
                    }
                    None => match core.buffers.find(&r.name) {
                        Some(id) if !core.buffers.get(id).expect("found").modified() => id,
                        _ => core.buffers.create(&r.name),
                    },
                };
                core.switch_to(id);
                let point = core.window.point(&core.buffers);
                let buffer = core.buffers.get_mut(id).expect("just shown");
                let len = buffer.len_chars();
                buffer.undo_boundary(point);
                // Past read-only too: a file refused since keeps its text, though not auto-saved.
                buffer.delete(0..len).expect("in range");
                buffer.insert(0, &r.text).expect("in range");
                if r.path.is_some() {
                    buffer.disk = r.hash;
                }
                // Written under this session first, so the old file goes only once a copy exists.
                keep = !autosaves(buffer);
                self.dispatch(Msg::AutoSave);
            }
            if !keep {
                self.effects.push(Effect::DeleteRecovery(r.file));
            }
        }
        let Some(next) = self.core.recovered.first() else {
            return match args.strings.is_empty() {
                true => Err("No auto-saved buffers to recover".into()),
                false => Ok(()),
            };
        };
        let what = (next.path.as_ref()).map_or(next.name.clone(), |p| p.display().to_string());
        let command = self.core.commands["recover-buffers"];
        self.read_string(
            command,
            Args::default(),
            format!("Recover auto-saved {what}? "),
        )
    }
}

impl Core {
    /// After input, (re)starts the auto-save timers if a buffer has unwritten edits; deletes the
    /// recovery files of buffers gone or saved, and all of them before a quit.
    pub(crate) fn autosave(&mut self, now: Instant, input: bool, effects: &mut Effects) {
        let quit = effects.iter().position(|e| matches!(e, Effect::Quit));
        let buffers = &self.buffers;
        let stale: Vec<BufferId> = (self.autosaved.keys().copied())
            .filter(|&id| quit.is_some() || !buffers.get(id).is_some_and(autosaves))
            .collect();
        let at = quit.unwrap_or(effects.len());
        for &id in &stale {
            self.autosaved.remove(&id);
        }
        effects.splice(at..at, stale.into_iter().map(Effect::DeleteAutoSave));
        let unwritten = (self.buffers.recent())
            .any(|(id, b)| autosaves(b) && self.autosaved.get(&id) != Some(&b.tick()));
        if input && unwritten {
            if let Some(id) = self.autosave_idle.take() {
                self.timers.cancel(id);
            }
            self.autosave_idle = Some(self.timers.start(now, AUTOSAVE_IDLE, None, Msg::AutoSave));
            if self.autosave_max.is_none() {
                self.autosave_max = Some(self.timers.start(now, AUTOSAVE_MAX, None, Msg::AutoSave));
            }
        }
    }
}

/// Whether `buffer` has edits a crash would lose: modified, and not read-only or hidden.
pub(crate) fn autosaves(buffer: &Buffer) -> bool {
    buffer.modified() && !buffer.read_only && !buffer.name.starts_with(' ')
}
