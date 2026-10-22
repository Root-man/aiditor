//! Reading and writing visited files. Blocking calls; the app runs saves on Tokio's blocking pool.

use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use aiditor_core::buffer::{encode, hash, revert};
use aiditor_core::{FileOp, Msg};
use ignore::WalkBuilder;

/// Carries out `op` and returns its reply for the core; the one place the app and headless share.
pub fn run(op: FileOp) -> Msg {
    match op {
        FileOp::Save {
            buffer,
            tick,
            path,
            text,
            crlf,
            bom,
            hash,
        } => {
            let result = write(&path, &encode(&text, crlf, bom), hash);
            // Not `error!`: the core echoes the failure, and an error log would show it twice.
            match &result {
                Ok(_) => tracing::info!(path = %path.display(), "saved"),
                Err(err) => tracing::info!(%err, "save failed"),
            }
            Msg::Saved {
                buffer,
                tick,
                result,
            }
        }
        FileOp::Revert {
            buffer,
            tick,
            path,
            text,
        } => {
            let file = read(&path).map_err(|err| format!("{}: {err}", path.display()));
            Msg::Reverted {
                buffer,
                tick,
                result: revert(&path, &text, file),
            }
        }
        FileOp::ListDir(dir) => {
            let result = list(&dir).map_err(|err| format!("{}: {err}", dir.display()));
            Msg::Listed { dir, result }
        }
        FileOp::Open(path) => {
            let result = read(&path).map_err(|err| format!("{}: {err}", path.display()));
            Msg::Opened { path, result }
        }
        FileOp::Preview(path) => {
            let result = read(&path).map_err(|err| format!("{}: {err}", path.display()));
            Msg::Previewed { path, result }
        }
        FileOp::Check(paths) => {
            let files = (paths.into_iter())
                .filter_map(|path| match read(&path) {
                    Ok(file) => Some((path, file.map(|(_, h)| h))),
                    Err(err) => {
                        tracing::info!(path = %path.display(), %err, "unreadable, not checked");
                        None
                    }
                })
                .collect();
            Msg::Checked(files)
        }
        FileOp::Index {
            project,
            walk,
            roots,
        } => {
            let roots = (roots.into_iter())
                .map(|root| {
                    let files = index(&root);
                    (root, files)
                })
                .collect();
            Msg::Indexed {
                project,
                walk,
                roots,
            }
        }
        FileOp::WriteProject { path, old, text } => {
            Msg::ProjectWritten(crate::projects::write(&path, old.as_deref(), text))
        }
    }
}

/// The file's bytes and their hash, or `None` when there is no file.
pub fn read(path: &Path) -> io::Result<Option<(Vec<u8>, u64)>> {
    match fs::read(path) {
        Ok(bytes) => {
            let h = hash(&bytes);
            Ok(Some((bytes, h)))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// The names in directory `dir`, sorted, each subdirectory's followed by `/`.
pub fn list(dir: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        // shortcut: non-UTF-8 names show lossily and can't be visited; keep OsStrings if one turns up.
        let mut name = entry.file_name().to_string_lossy().into_owned();
        // Stats only symlinks, following them so a link to a directory lists as one.
        let dir = entry.file_type().is_ok_and(|ft| {
            ft.is_dir() || ft.is_symlink() && fs::metadata(entry.path()).is_ok_and(|m| m.is_dir())
        });
        if dir {
            name.push('/');
        }
        names.push(name);
    }
    names.sort();
    Ok(names)
}

/// The files under `root`, relative to it and sorted: gitignored ones left out, hidden ones in,
/// `.git` never, symlinked directories not followed.
pub fn index(root: &Path) -> Result<Vec<PathBuf>, String> {
    fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))?;
    let files = (walk(root))
        .filter_map(|path| path.strip_prefix(root).ok().map(Path::to_path_buf))
        .collect();
    Ok(files)
}

/// The files under `root` that the index lists, as full paths in path order.
pub(crate) fn walk(root: &Path) -> impl Iterator<Item = PathBuf> {
    // Unreadable entries are skipped, as `rg` skips them.
    (walker(root).build().flatten())
        .filter(|e| {
            e.file_type()
                .is_some_and(|ft| ft.is_file() || ft.is_symlink() && e.path().is_file())
        })
        .map(ignore::DirEntry::into_path)
}

/// The index's walk of `root`: gitignored entries left out, hidden ones in, `.git` never,
/// symlinked directories not followed, in path order.
pub(crate) fn walker(root: &Path) -> WalkBuilder {
    // shortcut: one thread walks; switch to `build_parallel` if large repos index or grep slowly.
    let mut walk = WalkBuilder::new(root);
    walk.hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .sort_by_file_name(|a, b| a.cmp(b));
    walk
}

/// Writes `bytes` to `path` if its content still hashes to `expected` (`None`: no file) or the
/// file is gone, and returns the new hash.
pub fn write(path: &Path, bytes: &[u8], expected: Option<u64>) -> Result<u64, String> {
    let err = |e: io::Error| format!("{}: {e}", path.display());
    let on_disk = read(path).map_err(err)?.map(|(_, h)| h);
    // A gone file is written back: nothing on disk is lost.
    if on_disk.is_some() && on_disk != expected {
        // `projects::write` rewrites "not saved" in this message.
        return Err(format!("{} changed on disk; not saved", path.display()));
    }
    // A missing file or a dangling symlink: nothing to keep, and fs::write creates it through the link.
    let Ok(target) = fs::canonicalize(path) else {
        fs::write(path, bytes).map_err(err)?;
        return Ok(hash(bytes));
    };
    let meta = fs::metadata(&target).map_err(err)?;
    let in_place = meta.nlink() > 1
        || match replace(&target, bytes, meta.permissions()) {
            Ok(()) => false,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => true,
            Err(e) => return Err(err(e)),
        };
    if in_place {
        fs::write(&target, bytes).map_err(err)?;
    }
    Ok(hash(bytes))
}

/// Writes a temp file beside `target` with `perms` and renames it over `target`.
fn replace(target: &Path, bytes: &[u8], perms: Permissions) -> io::Result<()> {
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let tmp = target.with_file_name(format!(".{name}.aiditor-{}~", std::process::id()));
    _ = fs::remove_file(&tmp);
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.set_permissions(perms)?;
        file.sync_all()?;
        fs::rename(&tmp, target)
    })();
    if result.is_err() {
        _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn hash_of(path: &Path) -> Option<u64> {
        read(path).unwrap().map(|(_, h)| h)
    }

    #[test]
    fn run_diffs_a_revert_against_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        fs::write(&path, "a\r\nb\r\n").unwrap();
        let op = |path: &Path| FileOp::Revert {
            buffer: aiditor_core::buffer::Buffers::default().create("f"),
            tick: 3,
            path: path.to_path_buf(),
            text: "a\n".into(),
        };
        let Msg::Reverted { tick, result, .. } = run(op(&path)) else {
            panic!("not reverted");
        };
        let revert = result.unwrap();
        assert_eq!(
            (tick, revert.edits, revert.crlf),
            (3, vec![(2..2, "b\n".into())], true)
        );
        let Msg::Reverted { result, .. } = run(op(&dir.path().join("gone"))) else {
            panic!("not reverted");
        };
        assert!(result.unwrap_err().ends_with("gone no longer exists"));
    }

    #[test]
    fn run_replies_with_path_prefixed_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let Msg::Opened { result, .. } = run(FileOp::Open(missing)) else {
            panic!("not opened");
        };
        assert_eq!(result, Ok(None));
        let file = dir.path().join("f");
        fs::write(&file, "").unwrap();
        let Msg::Previewed { result, .. } = run(FileOp::Preview(file.clone())) else {
            panic!("not previewed");
        };
        assert_eq!(result.unwrap().unwrap().0, b"");
        let Msg::Listed { result, .. } = run(FileOp::ListDir(file.clone())) else {
            panic!("not listed");
        };
        assert!(
            result
                .unwrap_err()
                .starts_with(&format!("{}: ", file.display()))
        );
    }

    #[test]
    fn check_hashes_files_and_reports_gone_ones() {
        let dir = tempfile::tempdir().unwrap();
        let (a, gone) = (dir.path().join("a"), dir.path().join("gone"));
        fs::write(&a, "x").unwrap();
        let msg = run(FileOp::Check(vec![
            a.clone(),
            gone.clone(),
            dir.path().into(),
        ]));
        // The directory can't be read, so it is left out.
        assert_eq!(msg, Msg::Checked(vec![(a, Some(hash(b"x"))), (gone, None)]));
    }

    #[test]
    fn indexes_hidden_and_unignored_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (name, text) in [
            (".gitignore", "target/\n"),
            (".ignore", "*.log\n"),
            ("target/out", ""),
            (".github/ci.yml", ""),
            (".git/config", ""),
            ("sub/.git", "gitdir: elsewhere\n"),
            ("src/a.rs", ""),
            ("src/b.log", ""),
            ("elsewhere/c", ""),
        ] {
            fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
            fs::write(root.join(name), text).unwrap();
        }
        std::os::unix::fs::symlink(root.join("elsewhere"), root.join("dir-link")).unwrap();
        std::os::unix::fs::symlink(root.join("src/a.rs"), root.join("file-link")).unwrap();
        // Not a git repo, so this checks `.gitignore` applies without one.
        assert_eq!(
            index(root).unwrap(),
            [
                ".github/ci.yml",
                ".gitignore",
                ".ignore",
                "elsewhere/c",
                "file-link",
                "src/a.rs"
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn index_reports_a_missing_root_by_path() {
        let gone = tempfile::tempdir().unwrap().path().join("gone");
        let msg = run(FileOp::Index {
            project: "p".into(),
            walk: 3,
            roots: vec![gone.clone()],
        });
        let Msg::Indexed {
            project,
            walk,
            roots,
        } = msg
        else {
            panic!("not indexed");
        };
        assert_eq!((&*project, walk, roots.len()), ("p", 3, 1));
        let err = roots[0].1.clone().unwrap_err();
        assert!(err.starts_with(&format!("{}: ", gone.display())), "{err}");
    }

    #[test]
    fn lists_sorted_with_directories_marked() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b"), "").unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("a"), dir.path().join("c")).unwrap();
        assert_eq!(list(dir.path()).unwrap(), ["a/", "b", "c/"]);
        assert!(list(&dir.path().join("b")).is_err());
    }

    #[test]
    fn refuses_a_file_changed_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "old").unwrap();
        let loaded = hash_of(&path);
        fs::write(&path, "theirs").unwrap();
        let err = write(&path, b"mine", loaded).unwrap_err();
        assert!(err.ends_with("changed on disk; not saved"), "{err}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "theirs");
        // A file that appeared since it was opened as missing.
        assert!(write(&path, b"x", None).is_err());
        // A file gone since it was loaded is written back.
        fs::remove_file(&path).unwrap();
        assert_eq!(write(&path, b"back", loaded), Ok(hash(b"back")));
        assert_eq!(fs::read_to_string(&path).unwrap(), "back");
    }

    #[test]
    fn reads_a_missing_file_as_none_and_fails_on_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(&dir.path().join("none")).unwrap(), None);
        assert!(read(dir.path()).is_err());
    }

    #[test]
    fn writes_a_new_file_and_returns_its_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.txt");
        let h = write(&path, b"hi", None).unwrap();
        assert_eq!(hash_of(&path), Some(h));
        assert_eq!(write(&path, b"again", Some(h)), Ok(hash(b"again")));
    }

    #[test]
    fn keeps_the_mode_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.sh");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o754)).unwrap();
        write(&path, b"new", hash_of(&path)).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o754);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn saves_through_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let (target, link) = (dir.path().join("t.txt"), dir.path().join("l.txt"));
        fs::write(&target, "old").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        write(&link, b"new", hash_of(&link)).unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    }

    #[test]
    fn keeps_hard_links_shared() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
        fs::write(&a, "old").unwrap();
        fs::hard_link(&a, &b).unwrap();
        write(&a, b"new", hash_of(&a)).unwrap();
        assert_eq!(fs::read_to_string(&b).unwrap(), "new");
    }

    #[test]
    fn writes_in_place_when_the_directory_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o555)).unwrap();
        let result = write(&path, b"new", hash_of(&path));
        fs::set_permissions(dir.path(), Permissions::from_mode(0o755)).unwrap();
        result.unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }
}
