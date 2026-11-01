//! Auto-saving buffers and recovering them after a crash.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::buffer::{Buffer, BufferId};
use crate::command::Args;
use crate::{Core, Ctx, Effect, Effects, Msg, RecoveryOp};

/// How long after the last edit buffers are auto-saved, and the most edits wait while typing goes on.
const AUTOSAVE_IDLE: Duration = Duration::from_secs(2);
const AUTOSAVE_MAX: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(crate) struct State {
    confirmed: Option<u64>,
    pending: Option<Pending>,
    old: Vec<PathBuf>,
    retry: Option<Instant>,
    error: Option<String>,
}

struct Pending {
    request: u64,
    tick: u64,
    replaces: Vec<PathBuf>,
}

impl RecoveryOp {
    /// Correlates a service/enqueue failure without retaining the snapshot's text.
    pub fn failed(&self, reason: String) -> Option<Msg> {
        match self {
            Self::AutoSave {
                buffer,
                request,
                tick,
                ..
            } => Some(Msg::AutoSaved {
                buffer: *buffer,
                request: *request,
                tick: *tick,
                result: Err(reason),
            }),
            _ => None,
        }
    }
}

impl Ctx<'_> {
    pub(crate) fn write_autosaves(&mut self) {
        let core = &mut *self.core;
        if core.autosave_stopped {
            return;
        }
        for id in [
            core.autosave_idle.take(),
            core.autosave_max.take(),
            core.autosave_retry.take(),
        ]
        .into_iter()
        .flatten()
        {
            core.timers.cancel(id);
        }
        for (id, b) in core.buffers.recent() {
            if !autosaves(b) {
                continue;
            }
            let state = core.autosaved.entry(id).or_default();
            if state.confirmed == Some(b.tick())
                || state.pending.is_some()
                || state.retry.is_some_and(|at| at > self.now)
            {
                continue;
            }
            core.next_autosave += 1;
            let request = core.next_autosave;
            let tick = b.tick();
            let replaces = state.old.clone();
            state.pending = Some(Pending {
                request,
                tick,
                replaces: replaces.clone(),
            });
            state.retry = None;
            self.effects.push(Effect::Recovery(RecoveryOp::AutoSave {
                buffer: id,
                request,
                tick,
                name: b.name.clone(),
                path: b.file.clone(),
                hash: b.disk,
                text: b.text().clone(),
                replaces,
            }));
        }
    }

    pub(crate) fn auto_saved(
        &mut self,
        buffer: BufferId,
        request: u64,
        tick: u64,
        result: Result<Vec<(PathBuf, String)>, String>,
    ) {
        let core = &mut *self.core;
        let Some(state) = core.autosaved.get_mut(&buffer) else {
            return;
        };
        if !state
            .pending
            .as_ref()
            .is_some_and(|p| p.request == request && p.tick == tick)
        {
            return;
        }
        let pending = state.pending.take().unwrap();
        match result {
            Ok(cleanup) => {
                state.confirmed = Some(tick);
                state.retry = None;
                state.error = None;
                state.old.retain(|p| {
                    !pending.replaces.contains(p) || cleanup.iter().any(|(failed, _)| p == failed)
                });
                for (path, error) in cleanup {
                    core.echo(format!("Auto-save cleanup {}: {error}", path.display()));
                }
            }
            Err(error) => {
                state.retry = Some(self.now + AUTOSAVE_IDLE);
                if state.error.as_ref() != Some(&error) {
                    state.error = Some(error.clone());
                    if let Some(path) = core.buffers.get(buffer).and_then(|b| b.file.as_ref()) {
                        core.echo(format!("Auto-save {}: {error}", path.display()));
                    }
                }
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
                        core.windows.window.buffer()
                    }
                    None => match core.buffers.find(&r.name) {
                        Some(id) if !core.buffers.get(id).expect("found").modified() => id,
                        _ => core.buffers.create(&r.name),
                    },
                };
                core.switch_to(id);
                let point = core.windows.window.point(&core.buffers);
                let buffer = core.buffers.get_mut(id).expect("just shown");
                let len = buffer.len_chars();
                buffer.undo_boundary(point);
                // Past read-only too: a file refused since keeps its text.
                (buffer.replace(&[(0..len, r.text.clone())])).expect("in range");
                if r.path.is_some() {
                    buffer.disk = r.hash;
                }
                keep = r.path.is_some();
                if keep {
                    let old = &mut self.core.autosaved.entry(id).or_default().old;
                    if !old.contains(&r.file) {
                        old.push(r.file.clone());
                    }
                }
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
    pub(crate) fn autosave(&mut self, now: Instant, input: bool, effects: &mut Effects) {
        let quit = effects.iter().position(|e| matches!(e, Effect::Quit));
        self.autosave_stopped |= quit.is_some();
        let stale: Vec<_> = self
            .autosaved
            .keys()
            .copied()
            .filter(|&id| self.autosave_stopped || !self.buffers.get(id).is_some_and(autosaves))
            .collect();
        let mut cleanup = Vec::new();
        for id in stale {
            let state = self.autosaved.remove(&id).unwrap();
            cleanup.push(Effect::Recovery(RecoveryOp::DeleteAutoSave(id)));
            cleanup.extend(
                state
                    .old
                    .into_iter()
                    .map(|p| Effect::Recovery(RecoveryOp::DeleteRecovery(p))),
            );
        }
        let at = quit.unwrap_or(effects.len());
        effects.splice(at..at, cleanup);
        let unwritten = !self.autosave_stopped
            && self.buffers.recent().any(|(id, b)| {
                autosaves(b)
                    && self.autosaved.get(&id).is_none_or(|s| {
                        s.confirmed != Some(b.tick()) && s.pending.is_none() && s.retry.is_none()
                    })
            });
        if unwritten {
            if input || self.autosave_idle.is_none() {
                if let Some(id) = self.autosave_idle.take() {
                    self.timers.cancel(id);
                }
                self.autosave_idle =
                    Some(self.timers.start(now, AUTOSAVE_IDLE, None, Msg::AutoSave));
            }
            if self.autosave_max.is_none() {
                self.autosave_max = Some(self.timers.start(now, AUTOSAVE_MAX, None, Msg::AutoSave));
            }
        } else {
            for id in [self.autosave_idle.take(), self.autosave_max.take()]
                .into_iter()
                .flatten()
            {
                self.timers.cancel(id);
            }
        }
        if let Some(id) = self.autosave_retry.take() {
            self.timers.cancel(id);
        }
        if !self.autosave_stopped
            && let Some(at) = self.autosaved.values().filter_map(|s| s.retry).min()
        {
            self.autosave_retry = Some(self.timers.start(
                now,
                at.saturating_duration_since(now),
                None,
                Msg::AutoSave,
            ));
        }
    }
}

/// Whether `buffer` has edits a crash would lose: modified and visiting a file, as Emacs's
/// `auto-save-default` covers only file buffers.
fn autosaves(buffer: &Buffer) -> bool {
    buffer.modified() && buffer.file.is_some()
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::Recovered;
    use crate::buffer::hash;
    use crate::harness::Harness;
    use crate::key::parse_keys;
    use crate::{Effect, FileOp, RecoveryOp};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    const SEC: Duration = Duration::from_secs(1);

    /// The recovery texts, by buffer name.
    fn recovered(h: &Harness) -> Vec<(String, Option<PathBuf>, String)> {
        let mut files: Vec<_> = h.recovery.values().cloned().collect();
        files.sort();
        files
    }

    fn pending(h: &mut Harness) -> RecoveryOp {
        h.collect_effects = true;
        h.advance(2 * SEC);
        h.collect_effects = false;
        h.effects
            .drain(..)
            .find_map(|e| match e {
                Effect::Recovery(op @ RecoveryOp::AutoSave { .. }) => Some(op),
                _ => None,
            })
            .expect("auto-save requested")
    }

    fn success(op: &RecoveryOp) -> Msg {
        let RecoveryOp::AutoSave {
            buffer,
            request,
            tick,
            ..
        } = op
        else {
            panic!("not a snapshot")
        };
        Msg::AutoSaved {
            buffer: *buffer,
            request: *request,
            tick: *tick,
            result: Ok(Vec::new()),
        }
    }

    fn failures(h: &Harness) -> usize {
        let id = h.core.buffers.find("*Messages*").unwrap();
        h.core
            .buffers
            .get(id)
            .unwrap()
            .text()
            .to_string()
            .matches("Auto-save ")
            .count()
    }

    #[test]
    fn delayed_persistence_acknowledges_its_snapshot_and_newer_edits_follow_without_input() {
        let mut h = Harness::new(&[("a", "")]);
        h.open("a").keys("x");
        let op = pending(&mut h);
        h.keys("y").advance(40 * SEC);
        let state = &h.core.autosaved[&h.core.windows.window.buffer()];
        assert!(state.pending.is_some() && state.confirmed.is_none());
        h.run(vec![Effect::Recovery(op)]);
        assert_eq!(recovered(&h)[0].2, "x");
        h.advance(2 * SEC);
        assert_eq!(recovered(&h)[0].2, "xy");
    }

    #[test]
    fn an_older_attempt_cannot_acknowledge_a_retry_of_the_same_tick() {
        let mut h = Harness::new(&[("a", "")]);
        h.open("a").keys("x");
        let old = pending(&mut h);
        h.send(old.failed("blocked".into()).unwrap());
        let retry = pending(&mut h);
        h.send(success(&old));
        let id = h.core.windows.window.buffer();
        let state = h.core.autosaved[&id].pending.as_ref().unwrap();
        assert!(
            matches!(&retry, RecoveryOp::AutoSave { request, tick, .. } if state.request == *request && state.tick == *tick)
        );
        h.run(vec![Effect::Recovery(retry)]);
        assert_eq!(recovered(&h)[0].2, "x");
    }

    #[test]
    fn failure_deadlines_survive_input_and_report_changed_reasons_until_success() {
        let mut h = Harness::new(&[("a", "")]);
        h.open("a").keys("x");
        let op = pending(&mut h);
        h.send(op.failed("blocked".into()).unwrap());
        assert_eq!(failures(&h), 1);
        h.keys("y").advance(SEC).keys("C-b");
        h.collect_effects = true;
        h.advance(SEC);
        h.collect_effects = false;
        let op = h
            .effects
            .pop()
            .and_then(|e| match e {
                Effect::Recovery(op) => Some(op),
                _ => None,
            })
            .unwrap();
        assert!(matches!(&op, RecoveryOp::AutoSave { text, .. } if *text == "xy"));
        h.send(op.failed("blocked".into()).unwrap());
        assert_eq!(failures(&h), 1);
        let op = pending(&mut h);
        h.send(op.failed("changed reason".into()).unwrap());
        assert_eq!(failures(&h), 2);
        h.advance(2 * SEC);
        assert_eq!(recovered(&h)[0].2, "xy");
        h.keys("z");
        let op = pending(&mut h);
        h.send(op.failed("blocked".into()).unwrap());
        assert_eq!(failures(&h), 3);
    }

    #[test]
    fn retired_replies_leave_clean_killed_and_new_requests_untouched() {
        for (phase, action) in ["pending", "failed", "confirmed"]
            .into_iter()
            .flat_map(|phase| {
                [
                    "C-x C-s",
                    "M-x revert-buffer RET yes RET",
                    "C-x k RET yes RET",
                    "C-x C-c no RET yes RET",
                ]
                .into_iter()
                .map(move |action| (phase, action))
            })
        {
            let mut h = Harness::new(&[("a", "")]);
            h.open("a").keys("x");
            let op = pending(&mut h);
            let reply = success(&op);
            let late_failure = op.failed("late failure".into()).unwrap();
            if phase == "failed" {
                h.send(op.failed("blocked".into()).unwrap());
            }
            if phase == "confirmed" {
                h.run(vec![Effect::Recovery(op)]);
            }
            h.save_reply("x");
            h.prepare_revert("a", Some(b""));
            h.keys(action)
                .send(reply)
                .send(late_failure)
                .advance(40 * SEC);
            assert!(h.core.autosaved.is_empty(), "phase={phase} action={action}");
        }
        let mut h = Harness::new(&[("a", "")]);
        h.open("a").keys("x");
        let old = pending(&mut h);
        h.save_reply("x");
        h.keys("C-x C-s y");
        let new = pending(&mut h);
        h.send(success(&old));
        let id = h.core.windows.window.buffer();
        let request = h.core.autosaved[&id].pending.as_ref().unwrap().request;
        assert!(
            matches!(&new, RecoveryOp::AutoSave { request: current, .. } if request == *current)
        );
        h.run(vec![Effect::Recovery(new)]);
        assert_eq!(recovered(&h)[0].2, "xy\n");
    }

    #[test]
    fn multiple_recoveries_keep_each_copy_until_an_applicable_snapshot_succeeds() {
        let mut h = Harness::new(&[("a", "")]);
        let files = vec![
            left(&h, Some("a"), "", "one"),
            left(&h, Some("a"), "", "two"),
        ];
        h.collect_effects = true;
        h.send(Msg::Recover(files)).keys("RET");
        h.collect_effects = false;
        let old = h
            .effects
            .drain(..)
            .find_map(|e| match e {
                Effect::Recovery(op) => Some(op),
                _ => None,
            })
            .unwrap();
        h.keys("RET");
        let id = h.core.windows.window.buffer();
        assert_eq!(h.core.autosaved[&id].old.len(), 2);
        h.send(old.failed("blocked".into()).unwrap());
        let next = pending(&mut h);
        assert!(
            matches!(&next, RecoveryOp::AutoSave { replaces, text, .. } if replaces.len() == 2 && *text == "two")
        );
        h.run(vec![Effect::Recovery(next)]);
        assert_eq!(h.deleted.len(), 2);
        assert_eq!(recovered(&h)[0].2, "two");
    }

    #[test]
    fn older_success_and_cleanup_failures_preserve_later_recovery_copies() {
        let mut h = Harness::new(&[("a", "")]);
        let files = vec![
            left(&h, Some("a"), "", "one"),
            left(&h, Some("a"), "", "two"),
        ];
        h.collect_effects = true;
        h.send(Msg::Recover(files)).keys("RET");
        h.collect_effects = false;
        let op = h
            .effects
            .drain(..)
            .find_map(|e| match e {
                Effect::Recovery(op) => Some(op),
                _ => None,
            })
            .unwrap();
        h.keys("RET");
        h.run(vec![Effect::Recovery(op)]);
        let id = h.core.windows.window.buffer();
        assert_eq!(h.core.autosaved[&id].old, [PathBuf::from("/recovery/two")]);
        assert_eq!(recovered(&h)[0].2, "one");
        let op = pending(&mut h);
        let RecoveryOp::AutoSave {
            buffer,
            request,
            tick,
            ..
        } = &op
        else {
            panic!("snapshot")
        };
        h.recovery.get_mut(&id).unwrap().2 = "two".into();
        h.send(Msg::AutoSaved {
            buffer: *buffer,
            request: *request,
            tick: *tick,
            result: Ok(vec![("/recovery/two".into(), "cleanup blocked".into())]),
        });
        assert_eq!(recovered(&h)[0].2, "two");
        assert_eq!(h.core.autosaved[&id].confirmed, Some(*tick));
        assert_eq!(h.core.autosaved[&id].old, [PathBuf::from("/recovery/two")]);
        assert!(
            h.core
                .message()
                .unwrap()
                .contains("/recovery/two: cleanup blocked")
        );
        h.save_reply("two");
        h.keys("C-x C-s");
        assert!(h.deleted.contains(&PathBuf::from("/recovery/two")));
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
        h.save_reply("xy1\n");
        h.keys("C-x C-s");
        assert!(h.recovery.is_empty());
        // Quiet once nothing is unwritten.
        h.advance(60 * SEC);
        assert!(h.recovery.is_empty() && h.core.next_deadline().is_none());
    }

    #[test]
    fn typing_without_pause_is_auto_saved_every_thirty_seconds() {
        let mut h = Harness::new(&[("a", "")]);
        h.open("a");
        for _ in 0..29 {
            h.keys("x").advance(SEC);
        }
        assert!(h.recovery.is_empty());
        h.keys("y").advance(SEC);
        let text = format!("{}y", "x".repeat(29));
        assert_eq!(recovered(&h), [("a".into(), Some(h.path("a")), text)]);
    }

    #[test]
    fn scratch_messages_and_the_minibuffer_are_not_auto_saved() {
        let mut h = Harness::new(&[]);
        h.keys("x")
            .send(Msg::Log("hi".into()))
            .keys("M-x abc")
            .advance(3 * SEC);
        assert!(h.recovery.is_empty());
    }

    #[test]
    fn killing_or_quitting_removes_recovery_files() {
        let mut h = Harness::new(&[("a", "1\n"), ("b", "")]);
        h.open("a").keys("x").advance(2 * SEC);
        assert_eq!(h.recovery.len(), 1);
        h.keys("C-x k RET yes RET");
        assert!(h.recovery.is_empty());
        h.open("a").keys("x").advance(2 * SEC);
        h.open("b").keys("y").advance(2 * SEC);
        assert_eq!(h.recovery.len(), 2);
        // The quit's own save runs while a recovery file is still there.
        h.open("a")
            .keys("w")
            .advance(2 * SEC)
            .keys("C-x C-c no RET no RET yes RET");
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
        let disk = path
            .as_ref()
            .and_then(|p| h.reads.get(p))
            .and_then(|r| r.as_ref().ok())
            .and_then(|r| r.as_ref())
            .map(|(b, _)| b.clone());
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
        h.save_reply("1\nx\n");
        h.keys("C-x C-s");
        assert_eq!(h.submitted("a"), "1\nx\n");
        assert!(h.recovery.is_empty());
    }

    #[test]
    fn recovering_into_a_file_that_became_binary_keeps_the_text() {
        let mut h = Harness::new(&[]);
        h.read_reply("a", Some(&[0, 1, 2]));
        let r = left(&h, Some("a"), "1\n", "1\nx\n");
        h.send(Msg::Recover(vec![r])).keys("RET");
        let b = h.core.current();
        assert!(b.read_only && b.modified() && h.text() == "1\nx\n");
        assert_eq!(h.deleted, [PathBuf::from("/recovery/1\nx\n")]);
    }

    #[test]
    fn the_old_recovery_file_goes_with_this_sessions_write() {
        let mut h = Harness::new(&[("a", "1\n")]);
        let r = left(&h, Some("a"), "1\n", "2\n");
        h.collect_effects = true;
        h.send(Msg::Recover(vec![r])).keys("RET");
        let ops: Vec<_> = (h.effects.drain(..))
            .filter_map(|e| match e {
                Effect::Recovery(op) => Some(op),
                _ => None,
            })
            .collect();
        assert!(
            matches!(&ops[..], [RecoveryOp::AutoSave { replaces, .. }] if replaces == &[Path::new("/recovery/2\n")]),
            "no separate delete that could run after a failed write: {} ops",
            ops.len()
        );
    }

    #[test]
    fn a_recovered_buffer_can_be_compared_with_its_file() {
        let mut h = Harness::new(&[("a", "2\n")]);
        let r = left(&h, Some("a"), "1\n", "1\nx\n");
        h.diffs.insert(
            h.path("a"),
            Ok(format!(
                "--- {}\n+++ a\n@@ -1 +1,2 @@\n-2\n+1\n+x\n",
                h.path("a").display()
            )),
        );
        h.send(Msg::Recover(vec![r]))
            .keys("RET M-x diff-buffer-with-file RET");
        let id = h.core.buffers.find("*Diff*").unwrap();
        let diff = h.core.buffers.get(id).unwrap().text().to_string();
        let path = h.path("a").display().to_string();
        assert_eq!(
            diff,
            format!("--- {path}\n+++ a\n@@ -1 +1,2 @@\n-2\n+1\n+x\n")
        );
        assert!(h.core.current().modified() && h.text() == "1\nx\n");
    }

    #[test]
    fn a_recovered_file_changed_on_disk_since_is_not_saved_over() {
        let mut h = Harness::new(&[("a", "2\n")]);
        let r = left(&h, Some("a"), "1\n", "1\nx\n");
        h.saves.push_back(Err(format!(
            "{} changed on disk; not saved",
            h.path("a").display()
        )));
        h.send(Msg::Recover(vec![r])).keys("RET C-x C-s");
        assert_eq!(h.core.current().disk, Some(hash(b"1\n")));
        assert!(h.core.current().modified());
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
