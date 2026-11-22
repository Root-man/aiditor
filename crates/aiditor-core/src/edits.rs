//! Edits for `aiditor --wait` clients in terminals: their files open with `server-edit` on, and
//! finishing or aborting them answers the client.

use std::path::Path;

use crate::buffer::{BufferId, Kind};
use crate::workspace::WorkspaceId;
use crate::{ClientFile, Core, Ctx, Effect, FileOp, TerminalId, VisitIntent};

const MODE: &str = "server-edit";

/// A client waiting on its files.
pub(crate) struct Client {
    request: u64,
    terminal: TerminalId,
    workspace: Option<WorkspaceId>,
    /// The files not yet finished, in the client's order.
    edits: Vec<Edit>,
}

struct Edit {
    /// The file's buffer; `None` while visit `visit` reads it.
    buffer: Option<BufferId>,
    visit: u64,
    /// Where point goes, from 1.
    at: Option<(usize, Option<usize>)>,
    /// The edit made the buffer, so it is killed once the edit ends.
    opened: bool,
    /// Finishing waits on the buffer's save.
    saving: bool,
}

type Read = Result<Option<(Vec<u8>, u64)>, String>;

impl Core {
    /// Opens `files` for client `request` in terminal `terminal`'s workspace, reading those no
    /// buffer there visits.
    pub(crate) fn edit_request(
        &mut self,
        request: u64,
        terminal: TerminalId,
        files: Vec<ClientFile>,
    ) {
        let Some(&shell) = self.terminals.get(&terminal) else {
            return self.edit_reply(request, Err(format!("No terminal {}", terminal.0)));
        };
        if files.is_empty() {
            return self.edit_reply(request, Err("No file to edit".into()));
        }
        let workspace = self
            .buffers
            .get(shell)
            .expect("a live terminal's buffer")
            .workspace;
        let mut edits = Vec::new();
        for file in files {
            let at = file.line.map(|line| (line, file.col));
            let buffer = self.buffers.visiting_in(&file.path, workspace);
            let visit = match buffer {
                Some(id) => {
                    self.prepare(id, workspace, at);
                    0
                }
                None => {
                    self.visits += 1;
                    let intent = VisitIntent {
                        request: self.visits,
                        workspace,
                        window: self.windows.selected,
                    };
                    let path = file.path;
                    self.released
                        .push(Effect::File(FileOp::Open { intent, path }));
                    self.visits
                }
            };
            edits.push(Edit {
                buffer,
                visit,
                at,
                opened: false,
                saving: false,
            });
        }
        self.clients.push(Client {
            request,
            terminal,
            workspace,
            edits,
        });
        self.show_client(request);
    }

    /// Takes the file service's answer to visit `visit` when a client asked for it, giving
    /// `result` back otherwise.
    pub(crate) fn client_read(&mut self, visit: u64, path: &Path, result: Read) -> Option<Read> {
        let found = self.clients.iter().find_map(|c| {
            let at = c
                .edits
                .iter()
                .position(|e| e.buffer.is_none() && e.visit == visit)?;
            Some((c.request, c.workspace, at))
        });
        let Some((request, workspace, at)) = found else {
            return Some(result);
        };
        // A buffer made meanwhile, by `C-x C-f` or another client, is the one to edit.
        let made = match self.buffers.visiting_in(path, workspace) {
            Some(id) => Ok((id, false)),
            None => match result {
                Ok(file) => self.file_buffer(path, file, true).map(|id| (id, true)),
                Err(err) => {
                    self.echo(err.clone());
                    Err(err)
                }
            },
        };
        let (id, opened) = match made {
            Ok(made) => made,
            Err(err) => {
                self.abort_client(request, &err);
                return None;
            }
        };
        self.buffers.get_mut(id).expect("live").workspace = workspace;
        let client = (self.clients.iter_mut())
            .find(|c| c.request == request)
            .expect("found");
        let edit = &mut client.edits[at];
        (edit.buffer, edit.opened) = (Some(id), opened);
        let at = edit.at;
        self.prepare(id, workspace, at);
        self.show_client(request);
        None
    }

    /// Turns `server-edit` on in buffer `id` and puts its point at `at`.
    fn prepare(
        &mut self,
        id: BufferId,
        workspace: Option<WorkspaceId>,
        at: Option<(usize, Option<usize>)>,
    ) {
        let buffer = self.buffers.get_mut(id).expect("live");
        if !buffer.minor_modes.contains(&MODE) {
            buffer.minor_modes.insert(0, MODE);
        }
        if let Some((line, col)) = at {
            let (line, col) = (line.saturating_sub(1), col.unwrap_or(1).saturating_sub(1));
            // A line past the end is the end.
            let p = (buffer.char_idx(line, col)).unwrap_or(buffer.len_chars());
            _ = buffer.set_marker(buffer.point, p);
            // A window already showing it keeps its own point.
            let windows = match workspace == self.buffers.workspace {
                true => Some(&mut self.windows),
                false => (self.workspaces.iter_mut())
                    .find(|w| Some(w.id) == workspace)
                    .and_then(|w| w.windows.as_mut()),
            };
            for window in windows.into_iter().flat_map(|w| w.windows_mut()) {
                if window.buffer() == id {
                    _ = window.set_point(&mut self.buffers, p);
                }
            }
        }
    }

    /// Once every file of client `request` is open, shows the first in its workspace's window
    /// showing the terminal, else in its selected window.
    fn show_client(&mut self, request: u64) {
        let Some(client) = self.clients.iter().find(|c| c.request == request) else {
            return;
        };
        let buffers: Option<Vec<BufferId>> = client.edits.iter().map(|e| e.buffer).collect();
        let Some(&first) = buffers.as_deref().and_then(<[_]>::first) else {
            return;
        };
        let workspace = client.workspace;
        let shell = self.terminals.get(&client.terminal).copied();
        if workspace == self.buffers.workspace {
            self.close_minibuffer();
            let at = (self.windows.others.iter())
                .find(|(_, w)| Some(w.buffer()) == shell)
                .map(|(&id, _)| id);
            if let Some(at) = at {
                self.select(at);
            }
            return self.switch_to(first);
        }
        let Some(saved) = self.workspaces.iter_mut().find(|w| Some(w.id) == workspace) else {
            return;
        };
        let windows = saved
            .windows
            .as_mut()
            .expect("a workspace not current saves its windows");
        let window = match windows
            .others
            .values_mut()
            .find(|w| Some(w.buffer()) == shell)
        {
            Some(window) => window,
            None => &mut windows.window,
        };
        // shortcut: a workspace left in the diff view shows the file in the view's window, not its restored layout; quit saved views once that bites.
        window.show(&mut self.buffers, first);
        let name = saved.name.clone();
        let file = self.buffers.get(first).expect("live").name.clone();
        self.echo(format!("{file} waits for editing in workspace {name}"));
    }

    /// Ends every edit of buffer `id`: answers each client left with none, shows that client's
    /// next file, or its terminal, where `id` is shown, and kills `id` if an edit opened it.
    fn finish_edit(&mut self, id: BufferId) {
        let (mut opened, mut next, mut done) = (false, None, Vec::new());
        for client in &mut self.clients {
            let ended: Vec<Edit> = client
                .edits
                .extract_if(.., |e| e.buffer == Some(id))
                .collect();
            if ended.is_empty() {
                continue;
            }
            opened |= ended.iter().any(|e| e.opened);
            next = match client.edits.first() {
                Some(edit) => edit.buffer,
                None => {
                    done.push(client.request);
                    self.terminals.get(&client.terminal).copied()
                }
            };
        }
        self.clients.retain(|c| !c.edits.is_empty());
        for request in done {
            self.edit_reply(request, Ok(()));
        }
        self.leave(id, next, opened);
    }

    /// Gives client `request` code 1 with `err`, ending its edits as `finish_edit` does but
    /// showing the terminal.
    fn abort_client(&mut self, request: u64, err: &str) {
        let Some(at) = self.clients.iter().position(|c| c.request == request) else {
            return;
        };
        let client = self.clients.remove(at);
        self.edit_reply(request, Err(err.into()));
        let shell = self.terminals.get(&client.terminal).copied();
        for edit in client.edits {
            if let Some(id) = edit.buffer {
                self.leave(id, shell, edit.opened);
            }
        }
    }

    /// Buffer `id`'s edit has ended: it loses `server-edit` unless another client waits on it,
    /// the windows showing it show `next`, and with `kill` it is killed.
    fn leave(&mut self, id: BufferId, next: Option<BufferId>, kill: bool) {
        let waited = (self.clients.iter()).any(|c| c.edits.iter().any(|e| e.buffer == Some(id)));
        if waited || self.buffers.get(id).is_none() {
            return;
        }
        let buffer = self.buffers.get_mut(id).expect("live");
        buffer.minor_modes.retain(|&m| m != MODE);
        self.show_instead(id, next);
        if kill {
            self.kill_buffer(id);
        }
    }

    /// The current workspace's windows showing buffer `id` show `next` instead, if it is live.
    fn show_instead(&mut self, id: BufferId, next: Option<BufferId>) {
        let Some(next) = next.filter(|&b| self.buffers.get(b).is_some()) else {
            return;
        };
        if self.windows.window.buffer() == id {
            self.show_buffer(next);
        }
        for window in self
            .windows
            .others
            .values_mut()
            .filter(|w| w.buffer() == id)
        {
            window.show(&mut self.buffers, next);
        }
    }

    /// Before buffer `id` is killed: aborts the clients waiting on it, or on its terminal, and
    /// shows the terminal where `id` was shown.
    pub(crate) fn abort_for_kill(&mut self, id: BufferId) {
        let terminal = match self.buffers.get(id).map(|b| &b.kind) {
            Some(Kind::Terminal { id, .. }) => Some(*id),
            _ => None,
        };
        let (mut aborted, mut shell) = (Vec::new(), None);
        for client in &mut self.clients {
            let before = client.edits.len();
            client.edits.retain(|e| e.buffer != Some(id));
            if client.edits.len() < before {
                aborted.push((client.request, "Buffer killed"));
                shell = self.terminals.get(&client.terminal).copied();
            } else if Some(client.terminal) == terminal {
                aborted.push((client.request, "Terminal killed"));
            }
        }
        for (request, err) in aborted {
            self.abort_client(request, err);
        }
        self.show_instead(id, shell);
    }

    /// Client `request` went away: its buffers lose `server-edit`, and those it opened are
    /// killed, showing the terminal, unless they hold unsaved edits.
    pub(crate) fn edit_gone(&mut self, request: u64) {
        let Some(at) = self.clients.iter().position(|c| c.request == request) else {
            return;
        };
        let client = self.clients.remove(at);
        let shell = self.terminals.get(&client.terminal).copied();
        for edit in client.edits {
            let Some(id) = edit.buffer else { continue };
            let modified = self.buffers.get(id).is_some_and(|b| b.modified());
            if edit.opened && modified {
                let name = self.buffers.get(id).expect("live").name.clone();
                self.echo(format!(
                    "{name}: its client went away; the edits are unsaved"
                ));
            }
            self.leave(id, shell.filter(|_| !modified), edit.opened && !modified);
        }
    }

    /// Finishes the edits of buffer `id` once its save, which `ok` says succeeded, has ended.
    pub(crate) fn edit_saved(&mut self, id: BufferId, ok: bool) {
        let mut waiting = false;
        for client in &mut self.clients {
            for edit in client.edits.iter_mut().filter(|e| e.buffer == Some(id)) {
                waiting |= std::mem::take(&mut edit.saving);
            }
        }
        // Edits made while the save ran need another `C-c C-c`.
        if waiting && ok && !self.buffers.get(id).is_some_and(|b| b.modified()) {
            self.finish_edit(id);
        }
    }

    fn edit_reply(&mut self, request: u64, result: Result<(), String>) {
        self.released.push(Effect::EditReply { request, result });
    }

    fn waiting_on(&self, id: BufferId) -> Option<u64> {
        (self.clients.iter())
            .find(|c| c.edits.iter().any(|e| e.buffer == Some(id)))
            .map(|c| c.request)
    }
}

impl Ctx<'_> {
    /// `server-edit`: finishes the current buffer's edit, saving it first when modified.
    pub(crate) fn server_edit(&mut self) -> Result<(), String> {
        let id = self.core.windows.window.buffer();
        self.core.waiting_on(id).ok_or(NOT_WAITED)?;
        if !self.core.current().modified() {
            self.core.finish_edit(id);
            return Ok(());
        }
        self.save(id)?;
        for client in &mut self.core.clients {
            for edit in client.edits.iter_mut().filter(|e| e.buffer == Some(id)) {
                edit.saving = true;
            }
        }
        Ok(())
    }

    /// `server-edit-abort`: answers the current buffer's client with code 1, saving nothing.
    pub(crate) fn server_edit_abort(&mut self) -> Result<(), String> {
        let id = self.core.windows.window.buffer();
        let request = self.core.waiting_on(id).ok_or(NOT_WAITED)?;
        self.core.abort_client(request, "Edit aborted");
        Ok(())
    }
}

const NOT_WAITED: &str = "No client waits on this buffer";

#[cfg(test)]
mod tests {
    use crate::harness::{Harness, name};
    use crate::{ClientFile, Msg, TerminalId};

    const MSG: &str = "\n# Please enter the commit message\n";

    /// A shown terminal, with `COMMIT_EDITMSG`, `a` and `b` readable.
    fn git() -> Harness {
        let mut h = Harness::new(&[("COMMIT_EDITMSG", MSG), ("a", "ab\ncdef\n"), ("b", "b\n")]);
        h.keys("M-x vterm RET");
        h
    }

    fn edit(h: &mut Harness, request: u64, files: &[&str]) {
        let files = (files.iter())
            .map(|f| ClientFile {
                path: h.path(f),
                line: None,
                col: None,
            })
            .collect();
        h.send(Msg::EditRequest {
            request,
            terminal: TerminalId(0),
            files,
        });
    }

    fn editing(h: &Harness) -> bool {
        h.core.current().minor_modes.contains(&"server-edit")
    }

    fn exists(h: &Harness, name: &str) -> bool {
        h.core.buffers.find(name).is_some()
    }

    #[test]
    fn finishing_saves_then_replies_and_shows_the_terminal() {
        let mut h = git();
        let shell = name(&h).to_string();
        edit(&mut h, 1, &["COMMIT_EDITMSG"]);
        assert_eq!(name(&h), "COMMIT_EDITMSG");
        assert!(editing(&h));
        h.keys("F i x");
        h.save_reply(&format!("Fix{MSG}"));
        h.keys("C-c C-c");
        assert_eq!(h.edit_replies, [(1, Ok(()))]);
        assert_eq!(h.writes, [(h.path("COMMIT_EDITMSG"), format!("Fix{MSG}"))]);
        assert_eq!(name(&h), shell);
        assert!(
            !exists(&h, "COMMIT_EDITMSG"),
            "opened for the edit, so killed"
        );
    }

    #[test]
    fn a_failed_save_leaves_the_edit_waiting() {
        let mut h = git();
        edit(&mut h, 1, &["COMMIT_EDITMSG"]);
        h.keys("x");
        h.saves.push_back(Err("disk full".into()));
        h.keys("C-c C-c");
        assert!(h.edit_replies.is_empty());
        assert_eq!(name(&h), "COMMIT_EDITMSG");
        assert!(editing(&h));
    }

    #[test]
    fn aborting_saves_nothing_and_replies_failure() {
        let mut h = git();
        let shell = name(&h).to_string();
        edit(&mut h, 1, &["COMMIT_EDITMSG"]);
        h.keys("x C-c C-k");
        assert_eq!(h.edit_replies, [(1, Err("Edit aborted".into()))]);
        assert!(h.writes.is_empty());
        assert_eq!(name(&h), shell);
        assert!(!exists(&h, "COMMIT_EDITMSG"));
    }

    #[test]
    fn a_buffer_already_open_is_reused_and_stays() {
        let mut h = Harness::new(&[]);
        h.core.open(&h.path("a"), Some((b"a\n".to_vec(), 1)));
        h.keys("M-x vterm RET");
        let shell = name(&h).to_string();
        edit(&mut h, 1, &["a"]);
        assert_eq!(name(&h), "a");
        assert!(editing(&h));
        h.keys("C-x #");
        assert_eq!(h.edit_replies, [(1, Ok(()))]);
        assert_eq!(name(&h), shell);
        let a = h.core.buffers.find("a").expect("stays");
        assert!(h.core.buffers.get(a).unwrap().minor_modes.is_empty());
        assert!(!exists(&h, "a<2>"));
    }

    #[test]
    fn point_goes_to_the_position() {
        let mut h = git();
        h.send(Msg::EditRequest {
            request: 1,
            terminal: TerminalId(0),
            files: vec![ClientFile {
                path: h.path("a"),
                line: Some(2),
                col: Some(3),
            }],
        });
        assert_eq!(h.core.point(), 5, "line 2, after `cd`");
    }

    #[test]
    fn point_goes_to_the_position_in_a_window_already_showing_the_file() {
        let mut h = Harness::new(&[("a", "1\n2\n3\n4\n")]);
        h.keys("M-x vterm RET");
        h.open("a");
        assert_eq!(h.core.point(), 0);
        h.send(Msg::EditRequest {
            request: 1,
            terminal: TerminalId(0),
            files: vec![ClientFile {
                path: h.path("a"),
                line: Some(3),
                col: None,
            }],
        });
        assert_eq!(name(&h), "a");
        assert_eq!(h.core.point(), 4, "line 3");
    }

    #[test]
    fn an_unreadable_file_aborts_the_client() {
        let mut h = git();
        let shell = name(&h).to_string();
        h.reads
            .insert(h.path("c"), Err("c: permission denied".into()));
        edit(&mut h, 1, &["COMMIT_EDITMSG", "c"]);
        assert_eq!(h.edit_replies, [(1, Err("c: permission denied".into()))]);
        assert_eq!(name(&h), shell);
        assert!(!exists(&h, "COMMIT_EDITMSG"), "nothing is left open");
    }

    #[test]
    fn two_files_reply_once_both_are_finished() {
        let mut h = git();
        let shell = name(&h).to_string();
        edit(&mut h, 1, &["a", "b"]);
        assert_eq!(name(&h), "a");
        h.keys("C-c C-c");
        assert!(h.edit_replies.is_empty());
        assert_eq!(name(&h), "b");
        h.keys("C-c C-c");
        assert_eq!(h.edit_replies, [(1, Ok(()))]);
        assert_eq!(name(&h), shell);
    }

    #[test]
    fn killing_the_buffer_or_the_terminal_aborts() {
        let mut h = git();
        let shell = name(&h).to_string();
        edit(&mut h, 1, &["a"]);
        h.keys("C-x k RET");
        assert_eq!(h.edit_replies, [(1, Err("Buffer killed".into()))]);
        edit(&mut h, 2, &["b"]);
        h.keys(&format!("C-x b {shell} RET C-x k RET yes RET"));
        assert_eq!(h.edit_replies[1], (2, Err("Terminal killed".into())));
        assert!(!exists(&h, "b"));
    }

    #[test]
    fn the_terminal_window_is_used_and_shown_again_after_a_kill() {
        let mut h = git();
        let shell = name(&h).to_string();
        h.keys("C-x 2 C-x o C-x b *scratch*<main> RET");
        assert_ne!(name(&h), shell, "the selected window shows the scratch");
        edit(&mut h, 1, &["a"]);
        assert_eq!(name(&h), "a", "the terminal's window, now selected");
        h.keys("C-x o");
        let scratch = name(&h).to_string();
        h.keys("C-x o C-x k RET");
        assert_eq!(h.edit_replies, [(1, Err("Buffer killed".into()))]);
        assert_eq!(name(&h), shell);
        h.keys("C-x o");
        assert_eq!(name(&h), scratch);
    }

    #[test]
    fn quitting_asks_first() {
        let mut h = git();
        edit(&mut h, 1, &["a"]);
        h.keys("C-x C-c");
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Active processes exist; kill them and exit anyway? "
        );
        h.keys("yes RET");
        assert!(h.quit);
    }

    #[test]
    fn a_client_that_goes_away_kills_the_buffer_it_opened() {
        let mut h = git();
        let shell = name(&h).to_string();
        edit(&mut h, 1, &["a"]);
        h.send(Msg::EditGone { request: 1 });
        assert_eq!(name(&h), shell);
        assert!(!exists(&h, "a"));
        assert!(h.edit_replies.is_empty());
    }

    #[test]
    fn a_client_that_goes_away_leaves_unsaved_edits() {
        let mut h = git();
        edit(&mut h, 1, &["a"]);
        h.keys("x");
        h.send(Msg::EditGone { request: 1 });
        assert_eq!(name(&h), "a");
        assert!(!editing(&h));
        assert!(h.edit_replies.is_empty());
        assert_eq!(
            h.core.lookup(&crate::key::parse_keys("C-c C-k").unwrap()),
            None,
            "the keys are gone with the mode"
        );
    }

    #[test]
    fn an_unknown_terminal_is_refused() {
        let mut h = git();
        h.send(Msg::EditRequest {
            request: 1,
            terminal: TerminalId(9),
            files: Vec::new(),
        });
        assert_eq!(h.edit_replies, [(1, Err("No terminal 9".into()))]);
    }

    #[test]
    fn another_workspace_gets_the_file_and_says_so() {
        let mut h = git();
        let main = h.core.workspace().unwrap();
        h.new_workspace("other", "p");
        let here = name(&h).to_string();
        edit(&mut h, 1, &["a"]);
        assert_eq!(name(&h), here);
        assert_eq!(
            h.core.message(),
            Some("a waits for editing in workspace main")
        );
        h.core.switch_workspace(main).unwrap();
        assert_eq!(name(&h), "a");
        assert!(editing(&h));
    }
}
