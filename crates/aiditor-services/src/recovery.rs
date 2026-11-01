//! Recovery files for unsaved buffers, under the data dir so nothing lands in a project.

use std::fs::{self, DirBuilder, File};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

use aiditor_core::{Msg, Recovered, RecoveryOp};

use crate::files;

/// Starts the thread that carries out `RecoveryOp`s in `dir`,
/// in the order sent, so a delete never overtakes the write before it. It ends when the sender
/// is dropped.
pub fn spawn(
    dir: PathBuf,
    reply: impl Fn(Msg) + Send + 'static,
) -> (mpsc::Sender<RecoveryOp>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for op in rx {
            let result = run(&dir, &op);
            if let RecoveryOp::AutoSave {
                buffer,
                request,
                tick,
                replaces,
                ..
            } = op
            {
                let result = result
                    .map(|()| {
                        replaces
                            .into_iter()
                            .filter_map(|path| {
                                delete_recovery(&path)
                                    .err()
                                    .map(|error| (path, error.to_string()))
                            })
                            .collect()
                    })
                    .map_err(|e| e.to_string());
                reply(Msg::AutoSaved {
                    buffer,
                    request,
                    tick,
                    result,
                });
            } else if let Err(err) = result {
                tracing::warn!(%err, dir = %dir.display(), "auto-save cleanup failed");
            }
        }
    });
    (tx, handle)
}

fn run(dir: &Path, op: &RecoveryOp) -> io::Result<()> {
    match op {
        RecoveryOp::AutoSave {
            buffer,
            name,
            path,
            hash,
            text,
            ..
        } => {
            // Mode 0700: the files hold unsaved text.
            DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
            let time = (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
                .map_or(0, |d| d.as_secs());
            // shortcut: a non-UTF-8 path is stored as null; store bytes if one turns up.
            let path = path.as_deref().and_then(Path::to_str);
            let header =
                serde_json::json!({"name": name, "path": path, "hash": hash, "time": time});
            let file = dir.join(format!("{}-{}", std::process::id(), buffer.number()));
            let tmp = file.with_extension("tmp");
            let mut out = File::create(&tmp)?;
            writeln!(out, "{header}")?;
            for chunk in text.chunks() {
                out.write_all(chunk.as_bytes())?;
            }
            out.sync_all()?;
            fs::rename(&tmp, &file)?;
            Ok(())
        }
        RecoveryOp::DeleteAutoSave(buffer) => {
            match fs::remove_file(dir.join(format!("{}-{}", std::process::id(), buffer.number()))) {
                Err(err) if err.kind() != ErrorKind::NotFound => Err(err),
                _ => Ok(()),
            }
        }
        RecoveryOp::DeleteRecovery(file) => delete_recovery(file),
    }
}

/// Deletes an earlier session's recovery file.
fn delete_recovery(file: &Path) -> io::Result<()> {
    // A pid reused from the crashed session names this session's own files too.
    let own = format!("{}-", std::process::id());
    match file
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with(&own))
    {
        true => Ok(()),
        false => match fs::remove_file(file) {
            Err(err) if err.kind() != ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        },
    }
}

/// The recovery files in `dir`, oldest first, each with its visited file as it is now. An
/// unreadable one is logged and skipped.
// shortcut: files of another instance still running are offered too; check the pid if two instances become common.
pub fn list(dir: &Path) -> Vec<Recovered> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            tracing::warn!(%err, dir = %dir.display(), "can't list recovery files");
            return Vec::new();
        }
    };
    let mut found = Vec::new();
    // A `.tmp` is a write cut short; the file it would replace holds the previous text.
    for file in (entries.flatten().map(|e| e.path())).filter(|p| p.extension().is_none()) {
        match read(&file) {
            Ok(r) => found.push(r),
            Err(err) => tracing::warn!(%err, file = %file.display(), "bad recovery file"),
        }
    }
    found.sort_by_key(|(time, _)| *time);
    found.into_iter().map(|(_, r)| r).collect()
}

/// A recovery file and the time it was written.
fn read(file: &Path) -> io::Result<(u64, Recovered)> {
    #[derive(serde::Deserialize)]
    struct Header {
        name: String,
        path: Option<PathBuf>,
        hash: Option<u64>,
        time: u64,
    }
    let all = fs::read_to_string(file)?;
    let (header, text) = all.split_once('\n').ok_or(ErrorKind::InvalidData)?;
    let h: Header = serde_json::from_str(header)?;
    // An unreadable visited file opens as missing; its save then fails rather than overwrite it.
    let disk = (h.path.as_deref()).and_then(|p| files::read(p).ok().flatten());
    let recovered = Recovered {
        file: file.to_owned(),
        name: h.name,
        path: h.path,
        hash: h.hash,
        text: text.into(),
        disk,
    };
    Ok((h.time, recovered))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    use aiditor_core::{Core, Effect, Msg};

    use super::*;

    /// Sends the recovery ops among `effects` and waits for the thread to carry them out.
    fn run_all(dir: &Path, effects: Vec<Effect>) -> Vec<Msg> {
        let (replies, rx) = mpsc::channel();
        let (tx, handle) = spawn(dir.to_path_buf(), move |msg| {
            _ = replies.send(msg);
        });
        for effect in effects {
            if let Effect::Recovery(op) = effect {
                tx.send(op).unwrap();
            }
        }
        drop(tx);
        handle.join().unwrap();
        rx.into_iter().collect()
    }

    #[test]
    fn writes_with_a_header_then_deletes_leaving_no_temp_file() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("recovery");
        let (mut core, t0) = (Core::default(), Instant::now());
        core.open(Path::new("/nonexistent/a"), None);
        let insert = |text: &str| Msg::Insert {
            text: text.into(),
            replace: None,
        };
        core.handle(t0, insert("hi\nyou"));
        for msg in run_all(&dir, core.handle(t0 + Duration::from_secs(2), Msg::Tick)) {
            core.handle(t0 + Duration::from_secs(2), msg);
        }
        core.handle(t0 + Duration::from_secs(3), insert("!"));
        let effects = core.handle(t0 + Duration::from_secs(5), Msg::Tick);
        let Some(Effect::Recovery(RecoveryOp::AutoSave { buffer, .. })) = effects.first() else {
            panic!("no auto-save: {}", effects.len());
        };
        let buffer = *buffer;
        run_all(&dir, effects);
        let names: Vec<_> = (fs::read_dir(&dir).unwrap().flatten())
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        let text = fs::read_to_string(dir.join(&names[0])).unwrap();
        let (header, body) = text.split_once('\n').unwrap();
        let header: serde_json::Value = serde_json::from_str(header).unwrap();
        assert_eq!(header["name"], "a");
        assert_eq!(header["path"], "/nonexistent/a");
        assert!(header["hash"].is_null(), "a new file has no disk version");
        assert!(header["time"].as_u64().unwrap() > 0);
        assert_eq!(body, "hi\nyou!");
        let mode = fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // A second delete of a gone file is not an error.
        run_all(
            &dir,
            vec![Effect::Recovery(RecoveryOp::DeleteAutoSave(buffer))],
        );
        run_all(
            &dir,
            vec![Effect::Recovery(RecoveryOp::DeleteAutoSave(buffer))],
        );
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn a_replaced_recovery_file_goes_only_after_the_write() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("recovery");
        let old = root.path().join("1-1");
        fs::write(&old, "old").unwrap();
        let buffer = aiditor_core::buffer::Buffers::default().create("a");
        let save = || {
            Effect::Recovery(RecoveryOp::AutoSave {
                buffer,
                request: 1,
                tick: 1,
                name: "a".into(),
                path: None,
                hash: None,
                text: "new".into(),
                replaces: vec![old.clone()],
            })
        };
        let tmp = dir.join(format!("{}-{}.tmp", std::process::id(), buffer.number()));
        fs::create_dir_all(&tmp).unwrap();
        let replies = run_all(&dir, vec![save()]);
        assert!(matches!(
            &replies[..],
            [Msg::AutoSaved {
                request: 1,
                tick: 1,
                result: Err(_),
                ..
            }]
        ));
        assert!(old.exists(), "kept after a failed write");
        fs::remove_dir(&tmp).unwrap();
        let replies = run_all(&dir, vec![save()]);
        assert!(
            matches!(&replies[..], [Msg::AutoSaved { result: Ok(errors), .. }] if errors.is_empty())
        );
        assert!(!old.exists());
        assert_eq!(list(&dir)[0].text, "new");
    }

    #[test]
    fn persisted_replacements_report_cleanup_errors_separately() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("recovery");
        let old = root.path().join("old");
        fs::create_dir(&old).unwrap();
        fs::write(old.join("copy"), "old").unwrap();
        let buffer = aiditor_core::buffer::Buffers::default().create("a");
        let replies = run_all(
            &dir,
            vec![Effect::Recovery(RecoveryOp::AutoSave {
                buffer,
                request: 7,
                tick: 9,
                name: "a".into(),
                path: None,
                hash: None,
                text: "new".into(),
                replaces: vec![old.clone()],
            })],
        );
        assert!(
            matches!(&replies[..], [Msg::AutoSaved { request: 7, tick: 9, result: Ok(errors), .. }] if errors.len() == 1 && errors[0].0 == old)
        );
        assert_eq!(list(&dir)[0].text, "new");
        assert_eq!(fs::read_to_string(old.join("copy")).unwrap(), "old");
    }

    #[test]
    fn worker_failure_and_core_deadline_retry_persist_the_latest_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("recovery");
        let file = root.path().join("visited");
        fs::write(&file, "disk").unwrap();
        let mut core = Core::default();
        core.open(&file, files::read(&file).unwrap());
        let t0 = Instant::now();
        let insert = |s: &str| Msg::Insert {
            text: s.into(),
            replace: None,
        };
        core.handle(t0, insert("x"));
        let effects = core.handle(t0 + Duration::from_secs(2), Msg::Tick);
        let buffer = effects
            .iter()
            .find_map(|e| match e {
                Effect::Recovery(RecoveryOp::AutoSave { buffer, .. }) => Some(*buffer),
                _ => None,
            })
            .unwrap();
        let tmp = dir.join(format!("{}-{}.tmp", std::process::id(), buffer.number()));
        fs::create_dir_all(&tmp).unwrap();
        let (replies, rx) = mpsc::channel();
        let (tx, worker) = spawn(dir.clone(), move |msg| {
            _ = replies.send(msg);
        });
        for effect in effects {
            if let Effect::Recovery(op) = effect {
                tx.send(op).unwrap();
            }
        }
        let failure = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(&failure, Msg::AutoSaved { result: Err(_), .. }));
        core.handle(t0 + Duration::from_secs(2), failure);
        core.handle(t0 + Duration::from_secs(3), insert("y"));
        fs::remove_dir(&tmp).unwrap();
        for effect in core.handle(t0 + Duration::from_secs(4), Msg::Tick) {
            if let Effect::Recovery(op) = effect {
                tx.send(op).unwrap();
            }
        }
        let success = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(&success, Msg::AutoSaved { result: Ok(_), .. }));
        core.handle(t0 + Duration::from_secs(4), success);
        assert_eq!(list(&dir)[0].text, "xydisk");
        assert_eq!(fs::read_to_string(&file).unwrap(), "disk");
        drop(tx);
        worker.join().unwrap();
    }

    #[test]
    fn saturated_or_closed_gui_receivers_do_not_block_worker_cleanup() {
        for closed in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let dir = root.path().join("recovery");
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let handle = runtime.handle().clone();
            let (reports, mut rx) = crate::channel();
            if closed {
                rx.close();
            }
            let (tx, worker) = spawn(dir.clone(), move |msg| {
                let reports = reports.clone();
                handle.spawn(async move {
                    _ = reports.send(msg).await;
                });
            });
            let buffer = aiditor_core::buffer::Buffers::default().create("a");
            for request in 0..300 {
                tx.send(RecoveryOp::AutoSave {
                    buffer,
                    request,
                    tick: request,
                    name: "a".into(),
                    path: None,
                    hash: None,
                    text: "new".into(),
                    replaces: Vec::new(),
                })
                .unwrap();
            }
            tx.send(RecoveryOp::DeleteAutoSave(buffer)).unwrap();
            drop(tx);
            let start = Instant::now();
            while !worker.is_finished() {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "worker waited for GUI replies"
                );
                thread::sleep(Duration::from_millis(2));
            }
            worker.join().unwrap();
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
            if !closed {
                runtime.block_on(async {
                    for _ in 0..300 {
                        let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
                            .await
                            .unwrap()
                            .unwrap();
                        assert!(matches!(msg, Msg::AutoSaved { result: Ok(_), .. }));
                    }
                });
            }
        }
    }

    #[test]
    fn lists_files_oldest_first_and_deletes_by_path() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("recovery");
        fs::create_dir(&dir).unwrap();
        let visited = root.path().join("a");
        fs::write(&visited, "1\n").unwrap();
        let header = |time, path: &Path| {
            serde_json::json!({"name": "a", "path": path, "hash": 7, "time": time}).to_string()
        };
        fs::write(dir.join("1-2"), format!("{}\nnew", header(20, &visited))).unwrap();
        fs::write(dir.join("1-3"), format!("{}\nold", header(10, &visited))).unwrap();
        fs::write(dir.join("1-4.tmp"), "cut short").unwrap();
        fs::write(dir.join("1-5"), "not json\n").unwrap();
        let found = list(&dir);
        let texts: Vec<_> = found.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["old", "new"]);
        let r = &found[0];
        assert_eq!(
            (r.path.as_deref(), r.hash),
            (Some(visited.as_path()), Some(7))
        );
        assert_eq!(r.disk, files::read(&visited).unwrap());
        let own = dir.join(format!("{}-1", std::process::id()));
        fs::write(&own, "mine").unwrap();
        let delete = |p: &Path| Effect::Recovery(RecoveryOp::DeleteRecovery(p.to_owned()));
        run_all(&dir, vec![delete(&r.file), delete(&own)]);
        assert!(
            !r.file.exists() && own.exists(),
            "never this session's own file"
        );
        assert!(list(&root.path().join("none")).is_empty());
    }
}
