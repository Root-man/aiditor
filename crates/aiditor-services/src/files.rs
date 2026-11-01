//! Reading and writing visited files. Blocking calls; the app runs saves on Tokio's blocking pool.

use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use aiditor_core::buffer::{encode, hash, prepare_preview, revert, unified_diff};
use aiditor_core::{FileEntry, FileOp, Head, Msg};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{DirEntry, WalkBuilder};

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
        FileOp::Annotate { session, files } => {
            let now = SystemTime::now();
            let entries = files
                .into_iter()
                .map(|(name, path)| {
                    let metadata = fs::symlink_metadata(&path).ok();
                    let directory = metadata.as_ref().is_some_and(|m| {
                        m.is_dir()
                            || m.is_symlink() && fs::metadata(&path).is_ok_and(|m| m.is_dir())
                    });
                    FileEntry {
                        name,
                        annotation: file_annotation(metadata.as_ref(), directory, now),
                    }
                })
                .collect();
            Msg::Annotated { session, entries }
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
            let result = match preview(&request.path) {
                Ok(Some(bytes)) => prepare_preview(&bytes).map(Some).map_err(str::to_string),
                Ok(None) => Ok(None),
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
        FileOp::LanguageVersion {
            buffer,
            request,
            path,
            language,
        } => Msg::LanguageVersion {
            buffer,
            request,
            version: language_version(&path, language),
        },
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

/// The nearest declared version, read without invoking the tool or its version manager.
pub fn language_version(path: &Path, language: &str) -> Option<String> {
    aiditor_core::language_version::candidates(path, language)
        .into_iter()
        .find_map(|candidate| {
            let text = fs::read_to_string(&candidate).ok()?;
            aiditor_core::language_version::parse(&candidate, language, &text)
        })
        .filter(|version| !version.is_empty())
}

/// The repository holding `path`, with the branch `HEAD` names, a detached head's being its short
/// hash; a `.git` file, as a linked worktree or submodule has, says where the repository is.
/// `None` outside a repository.
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
    Some(Head {
        path: head,
        branch,
        changed: changed(path),
    })
}

/// Whether git reports `path` as changed or untracked; no if git can't say.
// shortcut: two processes per file, so a commit with many buffers open runs many; ask once per repository if that shows.
fn changed(path: &Path) -> bool {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return false;
    };
    let filters = Command::new("git")
        .args(["config", "--show-scope", "--get-regexp", "^filter\\."])
        .current_dir(dir)
        .stderr(Stdio::null())
        .output();
    match filters {
        Ok(output) if output.status.success() => {
            if String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| !matches!(line.split_whitespace().next(), Some("global" | "system")))
            {
                return false;
            }
        }
        Ok(output) if output.status.code() == Some(1) => {}
        _ => return false,
    }
    // Without `--no-optional-locks` status rewrites the index, whose report would run it again.
    // A repository's `core.fsmonitor` is a command, and opening a file mustn't run it.
    let status = Command::new("git")
        .args(["--no-optional-locks", "--literal-pathspecs"])
        .args(["-c", "core.fsmonitor=false", "status", "--porcelain", "--"])
        .arg(name)
        .current_dir(dir)
        .stderr(Stdio::null())
        .output();
    status.is_ok_and(|o| o.status.success() && !o.stdout.is_empty())
}

/// The file's bytes and their hash, or `None` when there is no file.
pub fn read(path: &Path) -> io::Result<Option<(Vec<u8>, u64)>> {
    match checked_metadata(path).and_then(|_| fs::read(path)) {
        Ok(bytes) => {
            let h = hash(&bytes);
            Ok(Some((bytes, h)))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

const PREVIEW_LIMIT: u64 = 10 * 1024 * 1024;

fn checked_metadata(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(io::Error::other("not a regular file"));
    }
    Ok(metadata)
}

fn preview(path: &Path) -> io::Result<Option<Vec<u8>>> {
    if checked_metadata(path)?.len() > PREVIEW_LIMIT {
        return Ok(None);
    }
    fs::read(path).map(Some)
}

/// Directory entries sorted by name, with subdirectories followed by `/`.
pub fn list(dir: &Path) -> io::Result<Vec<FileEntry>> {
    let now = SystemTime::now();
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        // shortcut: non-UTF-8 names show lossily and can't be visited; keep OsStrings if one turns up.
        let mut name = entry.file_name().to_string_lossy().into_owned();
        let metadata = fs::symlink_metadata(entry.path()).ok();
        let directory = entry.file_type().is_ok_and(|kind| {
            kind.is_dir()
                || kind.is_symlink() && fs::metadata(entry.path()).is_ok_and(|m| m.is_dir())
        });
        if directory {
            name.push('/');
        }
        entries.push(FileEntry {
            name,
            annotation: file_annotation(metadata.as_ref(), directory, now),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn file_annotation(metadata: Option<&fs::Metadata>, directory: bool, now: SystemTime) -> String {
    let mode = metadata.map_or_else(|| "—".into(), |m| mode_bits(m.mode()));
    let size = metadata
        .filter(|_| !directory)
        .map_or_else(|| "—".into(), |m| file_size(m.len()));
    let age = metadata.and_then(|m| m.modified().ok()).map_or_else(
        || "—".into(),
        |modified| file_age(now.duration_since(modified).unwrap_or_default().as_secs()),
    );
    format!("{mode:>10}  {size:>8}  {age:>12}")
}

fn mode_bits(mode: u32) -> String {
    let kind = match mode & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        0o010000 => 'p',
        0o140000 => 's',
        0o060000 => 'b',
        0o020000 => 'c',
        _ => '-',
    };
    let mut bits = String::from(kind);
    for (shift, special) in [(6, 0o4000), (3, 0o2000), (0, 0o1000)] {
        bits.push(if mode & (4 << shift) != 0 { 'r' } else { '-' });
        bits.push(if mode & (2 << shift) != 0 { 'w' } else { '-' });
        let executable = mode & (1 << shift) != 0;
        let special = mode & special != 0;
        bits.push(match (special, executable, shift) {
            (true, true, 0) => 't',
            (true, false, 0) => 'T',
            (true, true, _) => 's',
            (true, false, _) => 'S',
            (false, true, _) => 'x',
            (false, false, _) => '-',
        });
    }
    bits
}

fn file_size(bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = "B";
    for next in ["kB", "MB", "GB", "TB", "PB", "EB"] {
        if size < 999.95 {
            break;
        }
        size /= 1000.;
        unit = next;
    }
    let number = format!("{size:.1}");
    format!("{} {unit}", number.trim_end_matches(".0"))
}

fn file_age(seconds: u64) -> String {
    for (unit, suffix, next, next_suffix) in [
        (86400, "d", 3600, "h"),
        (3600, "h", 60, "m"),
        (60, "m", 1, "s"),
    ] {
        if seconds >= unit {
            let remainder = seconds % unit / next;
            return if remainder == 0 {
                format!("{}{suffix} ago", seconds / unit)
            } else {
                format!("{}{suffix}{remainder}{next_suffix} ago", seconds / unit)
            };
        }
    }
    format!("{seconds}s ago")
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
        } else if searchable_file(&entry) {
            files.push(entry.path().strip_prefix(root).unwrap().to_path_buf());
        }
    }
    files.sort();
    directories.sort();
    Ok((files, directories))
}

pub(crate) fn searchable_file(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .is_some_and(|ft| ft.is_file() || ft.is_symlink() && entry.path().is_file())
}

/// The index's walk of `dir`, inside `root`: gitignored entries and those `patterns` match
/// left out, hidden ones in, `.git` never, symlinked directories not followed, in path order.
pub(crate) fn walker(dir: &Path, root: &Path, patterns: &Patterns) -> WalkBuilder {
    let mut walk = WalkBuilder::new(dir);
    configure_walker(&mut walk, root, patterns);
    walk.sort_by_file_name(|a, b| a.cmp(b));
    walk
}

pub(crate) fn grep_walker(root: &Path, patterns: &Patterns, workers: usize) -> WalkBuilder {
    let mut walk = WalkBuilder::new(root);
    configure_walker(&mut walk, root, patterns);
    walk.threads(workers);
    walk
}

fn configure_walker(walk: &mut WalkBuilder, root: &Path, patterns: &Patterns) {
    let root = root.to_path_buf();
    let patterns = patterns.clone();
    walk.hidden(false)
        .require_git(false)
        .filter_entry(move |e| {
            e.file_name() != ".git"
                && !e.path().strip_prefix(&root).is_ok_and(|rel| {
                    let dir = e.file_type().is_some_and(|t| t.is_dir());
                    patterns.ignored(rel, dir)
                })
        });
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
    // A missing file or a dangling symlink: nothing to keep; create it through the link.
    let Ok(target) = fs::canonicalize(path) else {
        write_synced(path, bytes).map_err(err)?;
        return Ok(hash(bytes));
    };
    if let Err(e) = OpenOptions::new().write(true).open(&target)
        && e.kind() == ErrorKind::PermissionDenied
    {
        return Err(err(e));
    }
    let meta = fs::metadata(&target).map_err(err)?;
    let in_place = meta.nlink() > 1
        || match replace(&target, bytes, meta.permissions()) {
            Ok(()) => false,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => true,
            Err(e) => return Err(err(e)),
        };
    if in_place {
        write_synced(&target, bytes).map_err(err)?;
    }
    Ok(hash(bytes))
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
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
    #[test]
    fn language_versions_use_the_nearest_relevant_entry_in_filename_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let src = root.join("src");
        std::fs::create_dir(&src).unwrap();
        let file = src.join("a.rs");
        let version = || super::language_version(&file, "Rust");
        assert_eq!(version(), None);
        std::fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = '1.99.0'",
        )
        .unwrap();
        assert_eq!(version().as_deref(), Some("1.99.0"));
        std::fs::write(root.join("mise.toml"), "[tools]\nrust = '1.98.0'").unwrap();
        assert_eq!(version().as_deref(), Some("1.98.0"));
        std::fs::write(root.join(".tool-versions"), "rust 1.97.0").unwrap();
        assert_eq!(version().as_deref(), Some("1.97.0"));
        std::fs::write(src.join("mise.toml"), "[tools]\nrust = '1.96.0'").unwrap();
        assert_eq!(version().as_deref(), Some("1.96.0"));
        std::fs::write(src.join(".tool-versions"), "elixir 1.20.2-otp-26").unwrap();
        assert_eq!(version().as_deref(), Some("1.96.0"));
        assert_eq!(
            super::language_version(&src.join("a.ex"), "Elixir").as_deref(),
            Some("1.20.2-otp-26")
        );
        std::fs::write(src.join("mise.toml"), "[not valid").unwrap();
        assert_eq!(version().as_deref(), Some("1.97.0"));
        std::fs::write(src.join(".tool-versions"), "rust system").unwrap();
        assert_eq!(version(), None);
        std::fs::remove_file(src.join(".tool-versions")).unwrap();
        assert_eq!(version().as_deref(), Some("1.97.0"));
    }

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
    fn annotates_project_labels_and_preserves_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let small = dir.path().join("small");
        let large = dir.path().join("large");
        fs::write(&small, [b'x'; 41]).unwrap();
        fs::write(&large, [b'x'; 4100]).unwrap();
        fs::set_permissions(&small, Permissions::from_mode(0o640)).unwrap();
        let Msg::Annotated { session, entries } = run(FileOp::Annotate {
            session: 7,
            files: vec![
                ("lib/file".into(), small),
                ("lib<2>/file".into(), large),
                ("lib/gone".into(), dir.path().join("gone")),
            ],
        }) else {
            panic!("not annotated");
        };
        assert_eq!(session, 7);
        assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["lib/file", "lib<2>/file", "lib/gone"]
        );
        assert_eq!(
            entries[0]
                .annotation
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>(),
            ["-rw-r-----", "41", "B"]
        );
        assert!(entries[1].annotation.contains("4.1 kB"));
        assert_eq!(
            entries[2].annotation.split_whitespace().collect::<Vec<_>>(),
            ["—", "—", "—"]
        );
    }

    #[test]
    fn formats_filesystem_metadata() {
        for (mode, expected) in [
            (0o100644, "-rw-r--r--"),
            (0o040755, "drwxr-xr-x"),
            (0o120777, "lrwxrwxrwx"),
            (0o104754, "-rwsr-xr--"),
            (0o102644, "-rw-r-Sr--"),
            (0o041777, "drwxrwxrwt"),
            (0o041666, "drw-rw-rwT"),
            (0o010600, "prw-------"),
            (0o140600, "srw-------"),
            (0o060600, "brw-------"),
            (0o020600, "crw-------"),
        ] {
            assert_eq!(mode_bits(mode), expected);
        }
        for (bytes, expected) in [
            (0, "0 B"),
            (41, "41 B"),
            (999, "999 B"),
            (1000, "1 kB"),
            (4100, "4.1 kB"),
            (48_200_000, "48.2 MB"),
            (999_999, "1 MB"),
            (1_000_000_000, "1 GB"),
            (u64::MAX, "18.4 EB"),
        ] {
            assert_eq!(file_size(bytes), expected);
        }
        for (seconds, expected) in [
            (0, "0s ago"),
            (5, "5s ago"),
            (59, "59s ago"),
            (60, "1m ago"),
            (61, "1m1s ago"),
            (3600, "1h ago"),
            (18720, "5h12m ago"),
            (86400, "1d ago"),
            (295200, "3d10h ago"),
        ] {
            assert_eq!(file_age(seconds), expected);
        }
        assert_eq!(
            file_annotation(None, false, SystemTime::now())
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["—", "—", "—"]
        );
    }

    #[test]
    fn listing_includes_modes_sizes_ages_and_dangling_links() {
        use std::time::Duration;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        fs::write(&path, [b'x'; 41]).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o640)).unwrap();
        let modified = SystemTime::now() - Duration::from_secs(295200);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        std::os::unix::fs::symlink("missing", dir.path().join("dangling")).unwrap();
        let entries = list(dir.path()).unwrap();
        assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["dangling", "file", "sub/"]
        );
        assert!(entries[0].annotation.starts_with('l'));
        assert_eq!(
            entries[1].annotation.split_whitespace().collect::<Vec<_>>(),
            ["-rw-r-----", "41", "B", "3d10h", "ago"]
        );
        assert_eq!(entries[2].annotation.split_whitespace().nth(1), Some("—"));
        assert!(entries[2].annotation.starts_with('d'));
        let metadata = fs::metadata(path).unwrap();
        assert!(
            file_annotation(Some(&metadata), false, modified - Duration::from_secs(5))
                .ends_with("0s ago")
        );
    }

    #[test]
    fn lists_sorted_with_directories_marked() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b"), "").unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("a"), dir.path().join("c")).unwrap();
        assert_eq!(
            list(dir.path())
                .unwrap()
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["a/", "b", "c/"]
        );
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
        let found = |name: &str| {
            Some(Head {
                path: git.join("HEAD"),
                branch: name.to_string(),
                changed: false,
            })
        };
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
            let found = head(&tree.join("a.rs")).unwrap();
            assert_eq!(found.branch, "side");
            assert!(found.path.ends_with("worktrees/tree/HEAD"));
        }
    }

    #[test]
    fn says_whether_git_reports_a_file_as_changed() {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let mut command = Command::new("git");
            let identity = ["-c", "user.name=t", "-c", "user.email=t@t"];
            let status = command.args(identity).args(args).current_dir(dir.path());
            assert!(status.status().unwrap().success(), "{args:?}");
        };
        let file = dir.path().join("a b*.txt");
        let changed = |file: &Path| head(file).unwrap().changed;
        fs::write(&file, "one\n").unwrap();
        fs::write(dir.path().join("a b1.txt"), "other\n").unwrap();
        fs::write(dir.path().join(".gitignore"), "ignored\n").unwrap();
        git(&["init", "-q"]);
        assert!(changed(&file), "untracked");
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "x"]);
        assert!(!changed(&file), "committed");
        // Stale stat data with the same text is what status would refresh in the index.
        let index = fs::read(dir.path().join(".git/index")).unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
        (OpenOptions::new().write(true).open(&file).unwrap())
            .set_modified(later)
            .unwrap();
        assert!(!changed(&file), "touched");
        assert_eq!(fs::read(dir.path().join(".git/index")).unwrap(), index);
        fs::write(dir.path().join("ignored"), "").unwrap();
        assert!(!changed(&dir.path().join("ignored")));
        fs::write(dir.path().join("a b1.txt"), "edit\n").unwrap();
        assert!(
            !changed(&file),
            "another file, which the pattern's `*` would match"
        );
        fs::write(&file, "two\n").unwrap();
        assert!(changed(&file), "unstaged");
        git(&["add", "."]);
        assert!(changed(&file), "staged");
    }

    #[test]
    fn repository_filters_are_skipped_but_global_filters_work() {
        const CHILD: &str = "AIDITOR_TEST_GLOBAL_FILTER";
        if let Some(path) = std::env::var_os(CHILD) {
            assert!(head(Path::new(&path)).unwrap().changed);
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(dir.path())
                    .status()
                    .unwrap()
                    .success()
            );
        };
        git(&["init", "-q"]);
        fs::write(dir.path().join(".gitattributes"), "file filter=x\n").unwrap();
        let path = dir.path().join("file");
        fs::write(&path, "old\n").unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "initial",
        ]);
        git(&["config", "filter.x.clean", "touch marker; cat"]);
        fs::write(&path, "new\n").unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(10);
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(!head(&path).unwrap().changed);
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            3,
            "file, attributes and .git; no marker"
        );
        git(&["config", "--remove-section", "filter.x"]);
        let global = dir.path().join("global-config");
        fs::write(&global, "[filter \"x\"]\n clean = cat\n").unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "files::tests::repository_filters_are_skipped_but_global_filters_work",
            ])
            .env(CHILD, &path)
            .env("GIT_CONFIG_GLOBAL", global)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
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
    fn refuses_a_fifo_before_opening_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pipe");
        assert!(
            Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(read(&path).unwrap_err().to_string(), "not a regular file");
        assert_eq!(
            preview(&path).unwrap_err().to_string(),
            "not a regular file"
        );
        assert_eq!(
            read(dir.path()).unwrap_err().raw_os_error(),
            fs::read(dir.path()).unwrap_err().raw_os_error()
        );
    }

    #[test]
    fn previews_files_up_to_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large");
        let file = fs::File::create(&path).unwrap();
        file.set_len(PREVIEW_LIMIT + 1).unwrap();
        assert_eq!(preview(&path).unwrap(), None);
        fs::write(&path, vec![b'x'; PREVIEW_LIMIT as usize]).unwrap();
        let Msg::Previewed { result, .. } = run(FileOp::Preview(aiditor_core::PreviewRequest {
            session: 1,
            revision: 1,
            path,
        })) else {
            panic!("not previewed")
        };
        assert_eq!(result.unwrap().unwrap().len_bytes() as u64, PREVIEW_LIMIT);
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
    fn refuses_to_replace_a_read_only_file() {
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("readonly");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o444)).unwrap();
        let result = write(&path, b"new", WriteExpectation::Update(hash_of(&path)));
        assert!(
            matches!(result, Err(WriteFailure::Io(e)) if e.kind() == ErrorKind::PermissionDenied)
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "old");
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
