//! Reading and writing visited files. Blocking calls; the app runs saves on Tokio's blocking pool.

use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use aiditor_core::buffer::{encode, hash, prepare_preview, revert, unified_diff};
use aiditor_core::{FileOp, Head, Msg};
use ignore::WalkBuilder;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

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
            let result = write(
                &path,
                &encode(&text, crlf, bom),
                WriteExpectation::Update(hash),
            )
            .map_err(|err| err.describe(&path, "not saved"));
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
        FileOp::Diff {
            buffer,
            name,
            path,
            text,
        } => {
            let file = read(&path).map_err(|err| format!("{}: {err}", path.display()));
            Msg::Diffed {
                buffer,
                result: unified_diff(&path, &name, &text, file),
            }
        }
        FileOp::ListDir { dir, request } => {
            let result = list(&dir).map_err(|err| format!("{}: {err}", dir.display()));
            Msg::Listed { request, result }
        }
        FileOp::Open { intent, path } => {
            let result = read(&path).map_err(|err| format!("{}: {err}", path.display()));
            Msg::Opened {
                intent,
                path,
                result,
            }
        }
        FileOp::Preview(request) => {
            let result = match fs::read(&request.path) {
                Ok(bytes) => prepare_preview(&bytes).map(Some).map_err(str::to_string),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
                Err(err) => Err(err.to_string()),
            }
            .map_err(|err| format!("{}: {err}", request.path.display()));
            Msg::Previewed { request, result }
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
        FileOp::Head(paths) => Msg::Heads(
            (paths.into_iter())
                .map(|path| {
                    let head = head(&path);
                    (path, head)
                })
                .collect(),
        ),
        FileOp::Index {
            project,
            walk,
            roots,
            ignore,
        } => {
            let patterns = Patterns::lenient(&ignore);
            let mut directories = Vec::new();
            let roots = (roots.into_iter())
                .map(|root| {
                    let files = inventory(&root, &patterns).map(|(files, dirs)| {
                        directories.extend(dirs);
                        files
                    });
                    (root, files)
                })
                .collect();
            directories.sort();
            Msg::Indexed {
                project,
                walk,
                roots,
                directories,
            }
        }
        FileOp::WriteProject {
            intent,
            path,
            old,
            text,
        } => Msg::ProjectWritten {
            intent,
            result: crate::projects::write(&path, old.as_deref(), text),
        },
    }
}

/// The `HEAD` of the repository holding `path` and the branch it names, a detached head's being
/// its short hash; a `.git` file, as a linked worktree or submodule has, says where the repository
/// is. `None` outside a repository.
// shortcut: reftable repositories keep a placeholder in `HEAD`, shown as the branch `.invalid`; read their tables if one is used.
pub fn head(path: &Path) -> Option<Head> {
    let git = (path.ancestors().skip(1))
        .map(|dir| dir.join(".git"))
        .find(|git| git.exists())?;
    let dir = match git.is_file() {
        true => {
            let text = fs::read_to_string(&git).ok()?;
            git.parent()?.join(text.strip_prefix("gitdir:")?.trim())
        }
        false => git,
    };
    let head = dir.join("HEAD");
    let text = fs::read_to_string(&head).ok()?;
    let text = text.lines().next()?.trim();
    let branch = match text.strip_prefix("ref:") {
        Some(name) => {
            let name = name.trim();
            name.strip_prefix("refs/heads/").unwrap_or(name).to_string()
        }
        None => text.chars().take(7).collect(),
    };
    Some((head, branch))
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

/// The files under `root`, relative to it and sorted: gitignored ones and those `patterns`
/// match left out, hidden ones in, `.git` never, symlinked directories not followed.
pub fn index(root: &Path, patterns: &Patterns) -> Result<Vec<PathBuf>, String> {
    inventory(root, patterns).map(|(files, _)| files)
}

pub(crate) fn inventory(
    root: &Path,
    patterns: &Patterns,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))?;
    let mut files = Vec::new();
    let mut directories = Vec::new();
    for entry in walker(root, root, patterns).build().flatten() {
        let Some(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            directories.push(entry.path().to_path_buf());
        } else if kind.is_file() || kind.is_symlink() && entry.path().is_file() {
            files.push(entry.path().strip_prefix(root).unwrap().to_path_buf());
        }
    }
    files.sort();
    directories.sort();
    Ok((files, directories))
}

/// The files under `root` that the index lists, as full paths in path order.
pub(crate) fn walk(root: &Path, patterns: &Patterns) -> impl Iterator<Item = PathBuf> {
    // Unreadable entries are skipped, as `rg` skips them.
    (walker(root, root, patterns).build().flatten())
        .filter(|e| {
            e.file_type()
                .is_some_and(|ft| ft.is_file() || ft.is_symlink() && e.path().is_file())
        })
        .map(ignore::DirEntry::into_path)
}

/// The index's walk of `dir`, inside `root`: gitignored entries and those `patterns` match
/// left out, hidden ones in, `.git` never, symlinked directories not followed, in path order.
pub(crate) fn walker(dir: &Path, root: &Path, patterns: &Patterns) -> WalkBuilder {
    // shortcut: one thread walks; switch to `build_parallel` if large repos index or grep slowly.
    let mut walk = WalkBuilder::new(dir);
    let (root, patterns) = (root.to_path_buf(), patterns.clone());
    walk.hidden(false)
        .require_git(false)
        .filter_entry(move |e| {
            let ignored = (e.path().strip_prefix(&root)).is_ok_and(|rel| {
                let dir = e.file_type().is_some_and(|t| t.is_dir());
                patterns.ignored(rel, dir)
            });
            e.file_name() != ".git" && !ignored
        })
        .sort_by_file_name(|a, b| a.cmp(b));
    walk
}

/// One `ignore` pattern as its own matcher, so the patterns' order can be followed.
fn compile(line: &str) -> Result<Gitignore, String> {
    let mut builder = GitignoreBuilder::new("");
    let err = |e: ignore::Error| match e {
        // Its own message repeats the pattern.
        ignore::Error::Glob { err, .. } => format!("ignore pattern {line:?}: {err}"),
        e => format!("ignore pattern {line:?}: {e}"),
    };
    builder.add_line(None, line).map_err(err)?;
    builder.build().map_err(err)
}

/// A project's `ignore` patterns, compiled; they apply on top of the ignore files.
#[derive(Clone, Debug, Default)]
pub struct Patterns(Vec<Gitignore>);

impl Patterns {
    /// Compiles `lines`, failing on the first invalid one.
    pub fn new(lines: &[String]) -> Result<Self, String> {
        lines
            .iter()
            .map(|line| compile(line))
            .collect::<Result<_, _>>()
            .map(Patterns)
    }

    /// Compiles `lines`, which a project check has passed, logging and dropping any that fail.
    pub(crate) fn lenient(lines: &[String]) -> Self {
        let compiled = lines.iter().filter_map(|line| {
            compile(line)
                .inspect_err(|err| tracing::warn!(%err, "pattern dropped"))
                .ok()
        });
        Patterns(compiled.collect())
    }

    /// Whether `rel`, relative to a root, is left out: the last pattern matching it or a
    /// directory above it decides, so `*` then `!notes/` keeps everything under `notes/`.
    fn ignored(&self, rel: &Path, dir: bool) -> bool {
        (self.0.iter().rev())
            .map(|p| p.matched_path_or_any_parents(rel, dir))
            .find(|m| !m.is_none())
            .is_some_and(|m| m.is_ignore())
    }
}

/// The disk state a write may replace. Updates allow recreation after deletion.
#[derive(Clone, Copy, Debug)]
pub enum WriteExpectation {
    Create,
    Update(Option<u64>),
}

#[derive(Debug)]
pub enum WriteFailure {
    Conflict,
    AlreadyExists,
    Io(io::Error),
}

impl WriteFailure {
    pub(crate) fn describe(&self, path: &Path, conflict_action: &str) -> String {
        match self {
            Self::Conflict => format!("{} changed on disk; {conflict_action}", path.display()),
            Self::AlreadyExists => format!("{} already exists", path.display()),
            Self::Io(err) => format!("{}: {err}", path.display()),
        }
    }
}

static WRITE_GATE: Mutex<()> = Mutex::new(());
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Serializes the disk check and complete write, including aliases and project files.
pub fn write(path: &Path, bytes: &[u8], expected: WriteExpectation) -> Result<u64, WriteFailure> {
    let err = WriteFailure::Io;
    let _guard = WRITE_GATE
        .lock()
        .map_err(|_| err(io::Error::other("write gate poisoned")))?;
    match expected {
        WriteExpectation::Create => {
            match fs::symlink_metadata(path) {
                Ok(_) => return Err(WriteFailure::AlreadyExists),
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(err(e)),
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(err)?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| {
                    if e.kind() == ErrorKind::AlreadyExists {
                        WriteFailure::AlreadyExists
                    } else {
                        err(e)
                    }
                })?;
            if let Err(e) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                _ = fs::remove_file(path);
                return Err(err(e));
            }
            return Ok(hash(bytes));
        }
        WriteExpectation::Update(expected) => {
            let on_disk = read(path).map_err(err)?.map(|(_, h)| h);
            if on_disk.is_some() && on_disk != expected {
                return Err(WriteFailure::Conflict);
            }
        }
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
    let (tmp, mut file) = create_temp(target, &NEXT_TEMP)?;
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

fn create_temp(target: &Path, next: &AtomicU64) -> io::Result<(PathBuf, fs::File)> {
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    loop {
        let suffix = next.fetch_add(1, Ordering::Relaxed);
        let tmp =
            target.with_file_name(format!(".{name}.aiditor-{}-{suffix}~", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn hash_of(path: &Path) -> Option<u64> {
        read(path).unwrap().map(|(_, h)| h)
    }

    #[test]
    fn a_failed_create_can_be_retried() {
        const CHILD: &str = "AIDITOR_TEST_FILE_SIZE_LIMIT";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new("sh")
                .args(["-c", "trap '' XFSZ; ulimit -f 1; exec \"$@\"", "file-limit"])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "files::tests::a_failed_create_can_be_retried",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                // The file-size limit must not apply to redirected test logs.
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\nstdout: {}\nstderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        assert!(matches!(
            write(&path, &[b'x'; 64 * 1024], WriteExpectation::Create),
            Err(WriteFailure::Io(_))
        ));
        write(&path, b"retry", WriteExpectation::Create).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"retry");
    }

    #[test]
    fn write_expectations_distinguish_creation_conflict_recreation_and_io() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        write(&path, b"first", WriteExpectation::Create).unwrap();
        assert!(matches!(
            write(&path, b"second", WriteExpectation::Create),
            Err(WriteFailure::AlreadyExists)
        ));
        assert!(matches!(
            write(&path, b"second", WriteExpectation::Update(None)),
            Err(WriteFailure::Conflict)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"first");
        fs::remove_file(&path).unwrap();
        write(
            &path,
            b"recreated",
            WriteExpectation::Update(Some(hash(b"first"))),
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"recreated");
        fs::set_permissions(&path, Permissions::from_mode(0o000)).unwrap();
        let result = write(
            &path,
            b"unreadable",
            WriteExpectation::Update(Some(hash(b"recreated"))),
        );
        fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(result, Err(WriteFailure::Io(_))));
        assert_eq!(fs::read(&path).unwrap(), b"recreated");
    }

    #[test]
    fn concurrent_saves_check_the_winner_through_same_symlink_and_hard_link_paths() {
        for alias in ["same", "symlink", "hardlink"] {
            let dir = tempfile::tempdir().unwrap();
            let a = dir.path().join("a");
            let b = match alias {
                "same" => a.clone(),
                _ => dir.path().join("b"),
            };
            fs::write(&a, "base").unwrap();
            match alias {
                "symlink" => std::os::unix::fs::symlink(&a, &b).unwrap(),
                "hardlink" => fs::hard_link(&a, &b).unwrap(),
                _ => {}
            }
            let barrier = std::sync::Barrier::new(2);
            let save = |path: &Path, bytes: &[u8]| {
                barrier.wait();
                write(path, bytes, WriteExpectation::Update(Some(hash(b"base"))))
            };
            let results = std::thread::scope(|scope| {
                let first = scope.spawn(|| save(&a, b"first complete edit"));
                let second = scope.spawn(|| save(&b, b"second complete edit"));
                [first.join().unwrap(), second.join().unwrap()]
            });
            assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
            assert_eq!(
                results
                    .iter()
                    .filter(|r| matches!(r, Err(WriteFailure::Conflict)))
                    .count(),
                1
            );
            let winner = if results[0].is_ok() {
                b"first complete edit".as_slice()
            } else {
                b"second complete edit".as_slice()
            };
            assert_eq!(fs::read(&a).unwrap(), winner);
            assert_eq!(fs::read(&b).unwrap(), winner);
            if alias == "symlink" {
                assert!(fs::symlink_metadata(&b).unwrap().is_symlink());
            }
            if alias == "hardlink" {
                assert_eq!(
                    fs::metadata(&a).unwrap().ino(),
                    fs::metadata(&b).unwrap().ino()
                );
            }
        }
    }

    #[test]
    fn replacement_retries_collisions_and_cleans_only_its_owned_temp_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("file");
        let occupied = target.with_file_name(format!(".file.aiditor-{}-0~", std::process::id()));
        fs::write(&occupied, "other operation").unwrap();
        let (tmp, file) = create_temp(&target, &AtomicU64::new(0)).unwrap();
        assert_eq!(
            tmp.file_name().unwrap(),
            format!(".file.aiditor-{}-1~", std::process::id()).as_str()
        );
        drop(file);
        fs::remove_file(tmp).unwrap();
        fs::create_dir(&target).unwrap();
        assert!(
            replace(
                &target,
                b"cannot replace directory",
                Permissions::from_mode(0o600)
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(&occupied).unwrap(), "other operation");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        fs::remove_dir(&target).unwrap();
        fs::write(&target, "old").unwrap();
        write(
            &target,
            b"new",
            WriteExpectation::Update(Some(hash(b"old"))),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(fs::read_to_string(&occupied).unwrap(), "other operation");
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
    fn run_diffs_the_file_against_the_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        fs::write(&path, "a\r\n").unwrap();
        let op = FileOp::Diff {
            buffer: aiditor_core::buffer::Buffers::default().create("f"),
            name: "f".into(),
            path: path.clone(),
            text: "b\n".into(),
        };
        let Msg::Diffed { result, .. } = run(op) else {
            panic!("not diffed");
        };
        let header = format!("--- {}\n+++ f\n", path.display());
        assert_eq!(result.unwrap(), format!("{header}@@ -1 +1 @@\n-a\n+b\n"));
    }

    #[test]
    fn run_replies_with_path_prefixed_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let Msg::Opened { result, .. } = run(FileOp::Open {
            intent: aiditor_core::VisitIntent {
                request: 1,
                workspace: Default::default(),
                window: Default::default(),
            },
            path: missing,
        }) else {
            panic!("not opened");
        };
        assert_eq!(result, Ok(None));
        let file = dir.path().join("f");
        fs::write(&file, "").unwrap();
        let Msg::Previewed { result, .. } = run(FileOp::Preview(aiditor_core::PreviewRequest {
            session: 1,
            revision: 1,
            path: file.clone(),
        })) else {
            panic!("not previewed");
        };
        assert_eq!(result.unwrap().unwrap().to_string(), "");
        let Msg::Listed { result, .. } = run(FileOp::ListDir {
            dir: file.clone(),
            request: 1,
        }) else {
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
            index(root, &Patterns::default()).unwrap(),
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
    fn project_patterns_narrow_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for name in [
            ".gitignore",
            "a.rs",
            "b.log",
            "docs/x.md",
            "src/c.log",
            "notes/n.md",
            "notes/deep/d.md",
            "notes/x.tmp",
            "target/t",
        ] {
            fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
            fs::write(root.join(name), "").unwrap();
        }
        fs::write(root.join(".gitignore"), "target/\n").unwrap();
        let indexed = |lines: &[&str]| {
            let lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            let files = index(root, &Patterns::new(&lines).unwrap()).unwrap();
            files
                .iter()
                .map(|f| f.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            indexed(&["*.log", "docs/", "notes/", ".gitignore"]),
            ["a.rs"]
        );
        // An allow-list: `!notes/` keeps what is under it, until a later pattern excludes again.
        assert_eq!(
            indexed(&["*", "!notes/", "notes/*.tmp"]),
            ["notes/deep/d.md", "notes/n.md"]
        );
        // Patterns only add to the ignore files.
        assert!(!indexed(&["!target/"]).contains(&"target/t".into()));
    }

    #[test]
    fn inventory_counts_only_kept_files_and_directories_including_empty_ones() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for name in ["target/deep/x", "kept/a", "other/y", ".git/config"] {
            fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
            fs::write(root.join(name), "").unwrap();
        }
        fs::create_dir_all(root.join("kept/empty/nested")).unwrap();
        fs::write(root.join(".ignore"), "target/\n").unwrap();
        let patterns = Patterns::new(&["other/".into(), ".ignore".into()]).unwrap();
        assert_eq!(
            inventory(root, &patterns).unwrap(),
            (
                vec![PathBuf::from("kept/a")],
                ["", "kept", "kept/empty", "kept/empty/nested"]
                    .map(|p| root.join(p))
                    .to_vec()
            )
        );
        let patterns = Patterns::new(&["*".into(), "!kept/".into()]).unwrap();
        assert_eq!(
            inventory(root, &patterns).unwrap(),
            (
                vec![PathBuf::from("kept/a")],
                ["", "kept", "kept/empty", "kept/empty/nested"]
                    .map(|p| root.join(p))
                    .to_vec()
            )
        );
    }

    #[test]
    fn bad_patterns_are_named() {
        let err = Patterns::new(&["ok".into(), "src/[z-a]".into()]).unwrap_err();
        assert_eq!(
            err,
            "ignore pattern \"src/[z-a]\": invalid range; 'z' > 'a'"
        );
    }

    #[test]
    fn index_reports_a_missing_root_by_path() {
        let gone = tempfile::tempdir().unwrap().path().join("gone");
        let msg = run(FileOp::Index {
            project: aiditor_core::project::ProjectId::default(),
            walk: 3,
            roots: vec![gone.clone()],
            ignore: Vec::new(),
        });
        let Msg::Indexed {
            project,
            walk,
            roots,
            ..
        } = msg
        else {
            panic!("not indexed");
        };
        assert_eq!(
            (project, walk, roots.len()),
            (aiditor_core::project::ProjectId::default(), 3, 1)
        );
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
        let err = write(&path, b"mine", WriteExpectation::Update(loaded)).unwrap_err();
        assert!(matches!(err, WriteFailure::Conflict));
        assert_eq!(fs::read_to_string(&path).unwrap(), "theirs");
        // A file that appeared since it was opened as missing.
        assert!(write(&path, b"x", WriteExpectation::Update(None)).is_err());
        // A file gone since it was loaded is written back.
        fs::remove_file(&path).unwrap();
        assert_eq!(
            write(&path, b"back", WriteExpectation::Update(loaded)).unwrap(),
            hash(b"back")
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "back");
    }

    #[test]
    fn finds_the_branch_of_a_file_in_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join(".git");
        let file = dir.path().join("src/a.rs");
        fs::create_dir(&git).unwrap();
        let branch = |text: &str| {
            fs::write(git.join("HEAD"), text).unwrap();
            head(&file)
        };
        let found = |name: &str| Some((git.join("HEAD"), name.to_string()));
        assert_eq!(branch("ref: refs/heads/main\n"), found("main"));
        assert_eq!(
            branch("ref: refs/heads/fix/watcher\n"),
            found("fix/watcher")
        );
        assert_eq!(branch("ref: refs/tags/v1\n"), found("refs/tags/v1"));
        assert_eq!(
            branch(&format!("{}\n", "abcdef0".repeat(5))),
            found("abcdef0")
        );
    }

    #[test]
    fn follows_a_git_file_to_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("tree");
        let repo = dir.path().join("main/.git/worktrees/tree");
        fs::create_dir_all(&tree).unwrap();
        fs::create_dir_all(&repo).unwrap();
        fs::write(repo.join("HEAD"), "ref: refs/heads/side\n").unwrap();
        for gitdir in [
            repo.display().to_string(),
            "../main/.git/worktrees/tree".into(),
        ] {
            fs::write(tree.join(".git"), format!("gitdir: {gitdir}\n")).unwrap();
            let (path, branch) = head(&tree.join("a.rs")).unwrap();
            assert_eq!(branch, "side");
            assert!(path.ends_with("worktrees/tree/HEAD"));
        }
    }

    #[test]
    fn a_file_outside_a_repository_has_no_head() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        assert_eq!(head(&dir.path().join("a.rs")), None, "no HEAD");
        assert_eq!(head(Path::new("/nonexistent-root/a.rs")), None);
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
        let h = write(&path, b"hi", WriteExpectation::Update(None)).unwrap();
        assert_eq!(hash_of(&path), Some(h));
        assert_eq!(
            write(&path, b"again", WriteExpectation::Update(Some(h))).unwrap(),
            hash(b"again")
        );
    }

    #[test]
    fn keeps_the_mode_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.sh");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o754)).unwrap();
        write(&path, b"new", WriteExpectation::Update(hash_of(&path))).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o754);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn saves_through_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let (target, link) = (dir.path().join("t.txt"), dir.path().join("l.txt"));
        fs::write(&target, "old").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        write(&link, b"new", WriteExpectation::Update(hash_of(&link))).unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    }

    #[test]
    fn keeps_hard_links_shared() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
        fs::write(&a, "old").unwrap();
        fs::hard_link(&a, &b).unwrap();
        write(&a, b"new", WriteExpectation::Update(hash_of(&a))).unwrap();
        assert_eq!(fs::read_to_string(&b).unwrap(), "new");
    }

    #[test]
    fn writes_in_place_when_the_directory_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o555)).unwrap();
        let result = write(&path, b"new", WriteExpectation::Update(hash_of(&path)));
        fs::set_permissions(dir.path(), Permissions::from_mode(0o755)).unwrap();
        result.unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }
}
