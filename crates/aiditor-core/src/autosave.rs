//! Auto-saving buffers and recovering them after a crash.

use std::time::{Duration, Instant};

use crate::buffer::{Buffer, BufferId};
use crate::command::Args;
use crate::{Core, Ctx, Effect, Effects, Msg, RecoveryOp};

/// How long after the last edit buffers are auto-saved, and the most edits wait while typing goes on.
const AUTOSAVE_IDLE: Duration = Duration::from_secs(2);
const AUTOSAVE_MAX: Duration = Duration::from_secs(30);

impl Ctx<'_> {
    /// Writes recovery files for buffers edited since their last one, cancelling both timers.
    pub(crate) fn write_autosaves(&mut self) {
        let core = &mut *self.core;
        for id in [core.autosave_idle.take(), core.autosave_max.take()]
            .into_iter()
            .flatten()
        {
            core.timers.cancel(id);
        }
        for (id, b) in core.buffers.recent() {
            if autosaves(b) && core.autosaved.get(&id) != Some(&b.tick()) {
                core.autosaved.insert(id, b.tick());
                self.effects.push(Effect::Recovery(RecoveryOp::AutoSave {
                    buffer: id,
                    name: b.name.clone(),
                    path: b.file.clone(),
                    hash: b.disk,
                    text: b.text().clone(),
                }));
            }
        }
    }

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
                self.write_autosaves();
            }
            if !keep {
                self.effects
                    .push(Effect::Recovery(RecoveryOp::DeleteRecovery(r.file)));
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
            Some(&["Yes", "No"]),
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
        effects.splice(
            at..at,
            (stale.into_iter()).map(|id| Effect::Recovery(RecoveryOp::DeleteAutoSave(id))),
        );
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
fn autosaves(buffer: &Buffer) -> bool {
    buffer.modified() && !buffer.read_only && !buffer.name.starts_with(' ')
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::Recovered;
    use crate::buffer::hash;
    use crate::harness::Harness;
    use crate::key::parse_keys;
    use crate::{Effect, FileOp};
    use std::path::PathBuf;
    use std::time::Duration;

    const SEC: Duration = Duration::from_secs(1);

    /// The recovery texts, by buffer name.
    fn recovered(h: &Harness) -> Vec<(String, Option<PathBuf>, String)> {
        let mut files: Vec<_> = h.recovery.values().cloned().collect();
        files.sort();
        files
    }

    #[test]
    fn an_edit_is_auto_saved_after_two_idle_seconds_and_removed_on_save() {
        let mut h = Harness::new(&[("a", "1\n")]);
        h.open("a").keys("x").advance(SEC);
        assert!(h.recovery.is_empty());
        h.keys("y").advance(SEC);
        assert!(h.recovery.is_empty(), "the second key restarted the delay");
        h.advance(SEC);
        let file = ("a".to_string(), Some(h.path("a")), "xy1\n".to_string());
        assert_eq!(recovered(&h), [file]);
        // Nothing new beside the file.
        assert_eq!(std::fs::read_dir(h.dir.path()).unwrap().count(), 1);
        h.keys("C-x C-s");
        assert!(h.recovery.is_empty());
        // Quiet once nothing is unwritten.
        h.advance(60 * SEC);
        assert!(h.recovery.is_empty() && h.core.next_deadline().is_none());
    }

    #[test]
    fn typing_without_pause_is_auto_saved_every_thirty_seconds() {
        let mut h = Harness::new(&[]);
        for _ in 0..29 {
            h.keys("x").advance(SEC);
        }
        assert!(h.recovery.is_empty());
        h.keys("y").advance(SEC);
        let text = format!("{}y", "x".repeat(29));
        assert_eq!(recovered(&h), [("*scratch*".into(), None, text)]);
    }

    #[test]
    fn messages_and_the_minibuffer_are_not_auto_saved() {
        let mut h = Harness::new(&[]);
        h.send(Msg::Log("hi".into()))
            .keys("M-x abc")
            .advance(3 * SEC);
        assert!(h.recovery.is_empty());
    }

    #[test]
    fn killing_or_quitting_removes_recovery_files() {
        let mut h = Harness::new(&[("a", "1\n")]);
        h.open("a").keys("x").advance(2 * SEC);
        assert_eq!(h.recovery.len(), 1);
        h.keys("C-x k RET yes RET");
        assert!(h.recovery.is_empty());
        h.keys("x")
            .advance(2 * SEC)
            .keys("C-x b z RET y")
            .advance(2 * SEC);
        assert_eq!(h.recovery.len(), 2);
        // The quit's own save runs while a recovery file is still there.
        h.open("a")
            .keys("w")
            .advance(2 * SEC)
            .keys("C-x C-c no RET yes RET");
        assert!(h.quit && h.recovery.is_empty());
    }

    #[test]
    fn edits_during_a_save_keep_the_recovery_file() {
        let mut h = Harness::new(&[("a", "1\n")]);
        h.open("a").keys("x").advance(2 * SEC);
        let (now, mut save) = (h.now, Vec::new());
        for key in parse_keys("C-x C-s").unwrap() {
            save.extend(h.core.handle(now, Msg::Key(key)));
        }
        let Some(Effect::File(FileOp::Save { buffer, tick, .. })) = save.pop() else {
            panic!("no save");
        };
        h.keys("z").send(Msg::Saved {
            buffer,
            tick,
            result: Ok(0),
        });
        assert!(h.core.current().modified() && h.recovery.len() == 1);
    }

    /// An earlier session's recovery file for `path` in the temp dir (`None`: `*scratch*`),
    /// auto-saved from a disk version holding `base`.
    fn left(h: &Harness, path: Option<&str>, base: &str, text: &str) -> Recovered {
        let path = path.map(|p| h.path(p));
        let disk = (path.as_ref()).and_then(|p| std::fs::read(p).ok());
        Recovered {
            file: format!("/recovery/{text}").into(),
            name: (path.as_ref()).map_or("*scratch*".into(), |_| "a".into()),
            hash: path.as_ref().map(|_| hash(base.as_bytes())),
            path,
            text: text.into(),
            disk: disk.map(|b| (b.clone(), hash(&b))),
        }
    }

    #[test]
    fn a_recovered_file_opens_modified_and_saves_over_the_unchanged_file() {
        let mut h = Harness::new(&[("a", "1\n")]);
        let r = left(&h, Some("a"), "1\n", "1\nx\n");
        let prompt = format!("Recover auto-saved {}? ", h.path("a").display());
        h.send(Msg::Recover(vec![r]));
        assert_eq!(h.core.minibuffer().map(|mb| mb.prompt), Some(prompt));
        h.keys("RET");
        let b = h.core.current();
        assert_eq!(b.file.as_deref(), Some(h.path("a").as_path()));
        assert!(b.modified() && h.text() == "1\nx\n");
        // Written again under this session before the old file goes.
        assert_eq!(
            recovered(&h),
            [("a".into(), Some(h.path("a")), "1\nx\n".into())]
        );
        assert_eq!(h.deleted, [PathBuf::from("/recovery/1\nx\n")]);
        h.keys("C-x C-s");
        assert_eq!(h.disk("a"), "1\nx\n");
        assert!(h.recovery.is_empty());
    }

    #[test]
    fn a_recovered_file_changed_on_disk_since_is_not_saved_over() {
        let mut h = Harness::new(&[("a", "2\n")]);
        let r = left(&h, Some("a"), "1\n", "1\nx\n");
        h.send(Msg::Recover(vec![r])).keys("RET C-x C-s");
        assert_eq!(h.disk("a"), "2\n");
        assert!(h.core.message().unwrap().contains("changed on disk"));
    }

    #[test]
    fn recovery_offers_each_file_in_turn_and_c_g_keeps_the_rest() {
        let mut h = Harness::new(&[("a", "1\n")]);
        h.send(Msg::Recover(Vec::new()));
        assert!(h.core.minibuffer().is_none() && h.core.message().is_none());
        let files = vec![
            left(&h, None, "", "notes"),
            left(&h, Some("a"), "1\n", "gone"),
            left(&h, None, "", "later"),
        ];
        h.send(Msg::Recover(files)).keys("RET No RET C-g");
        assert_eq!(h.core.current().name, "*scratch*");
        assert!(h.core.current().modified() && h.text() == "notes");
        assert!(h.core.buffers.visiting(&h.path("a")).is_none(), "declined");
        let deleted = ["/recovery/notes", "/recovery/gone"].map(PathBuf::from);
        assert_eq!(h.deleted, deleted);
        // A modified `*scratch*` is kept; the next one gets its own buffer.
        h.keys("M-x recover-buffers RET RET");
        assert_eq!(
            (h.core.current().name.as_str(), h.text()),
            ("*scratch*<2>", "later".into())
        );
        assert_eq!(h.deleted.len(), 3);
        h.keys("M-x recover-buffers RET");
        assert_eq!(h.core.message(), Some("No auto-saved buffers to recover"));
    }
}
