//! Carries out the core's effects on the services, handing back what only the caller can schedule or do.

use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::thread::JoinHandle;

use aiditor_core::{Effect, Msg, RecoveryOp};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::{agents, configuration, environment, files, grep, terminal, watcher};

/// The services an effect can need. The caller builds the parts, so it can keep its own watcher.
pub struct Runner {
    /// Services send their results to the core on this.
    pub tx: Sender<Msg>,
    pub grep: grep::Grep,
    pub terminals: terminal::Terminals,
    pub agents: agents::Agents,
    pub watcher: watcher::Watcher,
    /// The recovery service, when there is a data dir.
    pub recovery: Option<(std_mpsc::Sender<RecoveryOp>, JoinHandle<()>)>,
    /// Where the user's settings and keymap files are, if known.
    pub config_dir: Option<PathBuf>,
}

/// What the caller must do after `Runner::run`.
pub enum Run {
    Done,
    /// A new terminal's or agent's channel, for the caller to drain into the core.
    Drain(Receiver<Msg>),
    /// Blocking work whose result is one message; the caller picks the thread.
    Blocking(Box<dyn FnOnce() -> Msg + Send>),
    /// A message for the core right away, such as a recovery failure.
    Reply(Msg),
    /// Clipboard, open and quit, which only the caller can do.
    Platform(Effect),
}

impl Runner {
    pub fn run(&mut self, effect: Effect) -> Run {
        match effect {
            Effect::Terminal(op) => self.terminals.run(op).map_or(Run::Done, Run::Drain),
            Effect::Agent(op) => self.agents.run(op).map_or(Run::Done, Run::Drain),
            Effect::File(op) => Run::Blocking(Box::new(move || files::run(op))),
            Effect::Grep(op) => {
                self.grep.run(op, self.tx.clone());
                Run::Done
            }
            Effect::Watch(set) => {
                self.watcher.set(set);
                Run::Done
            }
            Effect::Recovery(op) => {
                let failure = if let Some((tx, _)) = &self.recovery {
                    tx.send(op)
                        .err()
                        .and_then(|e| e.0.failed("Recovery service closed".into()))
                } else {
                    op.failed("Recovery service unavailable".into())
                };
                failure.map_or(Run::Done, Run::Reply)
            }
            Effect::CaptureEnvironment {
                project,
                request,
                dir,
            } => {
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let mut result = environment::run(&dir).await;
                    let direnv = match &mut result {
                        Ok(vars) => environment::direnv(vars, &dir, environment::DIRENV_TIMEOUT)
                            .await
                            .err(),
                        Err(_) => None,
                    };
                    _ = (tx.send(Msg::EnvironmentCaptured {
                        project,
                        request,
                        result,
                        direnv,
                    }))
                    .await;
                });
                Run::Done
            }
            Effect::LoadConfiguration { request } => {
                let dir = self.config_dir.clone();
                Run::Blocking(Box::new(move || {
                    configuration::load(dir.as_deref(), request)
                }))
            }
            effect @ (Effect::SetClipboard(_)
            | Effect::OpenWithSystem(_)
            | Effect::OpenUrl(_)
            | Effect::Quit) => Run::Platform(effect),
        }
    }

    /// Before the process exits: lets queued recovery deletes finish, then kills terminals and agents.
    pub fn shutdown(&mut self) {
        if let Some((tx, thread)) = self.recovery.take() {
            drop(tx);
            _ = thread.join();
        }
        self.terminals.kill_all();
        self.agents.kill_all();
    }
}

#[cfg(test)]
mod tests {
    use aiditor_core::FileOp;
    use aiditor_core::buffer::Buffers;

    use super::*;

    fn runner() -> Runner {
        Runner {
            tx: crate::channel().0,
            grep: grep::Grep::default(),
            terminals: terminal::Terminals::default(),
            agents: agents::Agents::default(),
            watcher: watcher::Watcher::spawn(|_| {}),
            recovery: None,
            config_dir: None,
        }
    }

    #[test]
    fn a_file_operation_is_blocking_work_returning_its_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "a").unwrap();
        let Run::Blocking(work) = runner().run(Effect::File(FileOp::Check(vec![path.clone()])))
        else {
            panic!("a file operation blocks");
        };
        let Msg::Checked(files) = work() else {
            panic!("a check answers with Checked");
        };
        assert_eq!(files, [(path, Some(aiditor_core::buffer::hash(b"a")))]);
    }

    #[test]
    fn recovery_without_its_service_replies_with_the_failure() {
        let op = RecoveryOp::AutoSave {
            buffer: Buffers::default().create("a"),
            request: 1,
            tick: 2,
            name: "a".into(),
            path: None,
            hash: None,
            text: "a".into(),
            replaces: Vec::new(),
        };
        let Run::Reply(Msg::AutoSaved {
            request, result, ..
        }) = runner().run(Effect::Recovery(op))
        else {
            panic!("the failure comes back as a reply");
        };
        assert_eq!(request, 1);
        assert_eq!(result, Err("Recovery service unavailable".into()));
    }

    #[test]
    fn the_clipboard_is_left_to_the_platform() {
        let run = runner().run(Effect::SetClipboard("x".into()));
        assert!(matches!(run, Run::Platform(Effect::SetClipboard(text)) if text == "x"));
    }
}
