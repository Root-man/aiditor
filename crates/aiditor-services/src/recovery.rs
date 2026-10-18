//! Recovery files for unsaved buffers, under the data dir so nothing lands in a project.

use std::fs::{self, DirBuilder, File};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

use aiditor_core::Effect;

/// Starts the thread that carries out `Effect::AutoSave` and `Effect::DeleteAutoSave` in `dir`,
/// in the order sent, so a delete never overtakes the write before it. It ends when the sender
/// is dropped.
pub fn spawn(dir: PathBuf) -> (mpsc::Sender<Effect>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for effect in rx {
            if let Err(err) = run(&dir, effect) {
                tracing::warn!(%err, dir = %dir.display(), "auto-save failed");
            }
        }
    });
    (tx, handle)
}

fn run(dir: &Path, effect: Effect) -> io::Result<()> {
    match effect {
        Effect::AutoSave {
            buffer,
            name,
            path,
            hash,
            text,
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
            fs::rename(&tmp, &file)
        }
        Effect::DeleteAutoSave(buffer) => {
            match fs::remove_file(dir.join(format!("{}-{}", std::process::id(), buffer.number()))) {
                Err(err) if err.kind() != ErrorKind::NotFound => Err(err),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    use aiditor_core::{Core, Msg};

    use super::*;

    /// Sends `effects` and waits for the thread to carry them out.
    fn run_all(dir: &Path, effects: Vec<Effect>) {
        let (tx, handle) = spawn(dir.to_path_buf());
        effects.into_iter().for_each(|e| tx.send(e).unwrap());
        drop(tx);
        handle.join().unwrap();
    }

    #[test]
    fn writes_with_a_header_then_deletes_leaving_no_temp_file() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("recovery");
        let (mut core, t0) = (Core::default(), Instant::now());
        let insert = |text: &str| Msg::Insert {
            text: text.into(),
            replace: None,
        };
        core.handle(t0, insert("hi\nyou"));
        run_all(&dir, core.handle(t0 + Duration::from_secs(2), Msg::Tick));
        core.handle(t0, insert("!"));
        let effects = core.handle(t0 + Duration::from_secs(4), Msg::Tick);
        let Some(Effect::AutoSave { buffer, .. }) = effects.first() else {
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
        assert_eq!(header["name"], "*scratch*");
        assert!(header["path"].is_null() && header["hash"].is_null());
        assert!(header["time"].as_u64().unwrap() > 0);
        assert_eq!(body, "hi\nyou!");
        let mode = fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // A second delete of a gone file is not an error.
        run_all(&dir, vec![Effect::DeleteAutoSave(buffer)]);
        run_all(&dir, vec![Effect::DeleteAutoSave(buffer)]);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    }
}
