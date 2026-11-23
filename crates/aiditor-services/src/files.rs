//! Reading and writing visited files. Blocking calls; the app runs saves on Tokio's blocking pool.

use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use aiditor_core::buffer::{decode, encode, hash, hunks, prepare_preview, revert, unified_diff};
use aiditor_core::{
    ChangedFile, FileDiff, FileEntry, FileOp, GitChange, GitFile, GitObjects, GitSide, GitStatus,
    GitStep, Head, Msg, Snapshot, TerminalGit,
};
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
            let root = intent
                .workspace
                .is_none()
                .then(|| root(&path).to_path_buf());
            Msg::Opened {
                intent,
                path,
                result,
                root,
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
        FileOp::TerminalGit { id, dir } => Msg::TerminalGit {
            id,
            git: terminal_git(&dir),
            dir,
        },
        FileOp::GitStatus(roots) => Msg::GitStatus(std::thread::scope(|scope| {
            let threads: Vec<_> = (roots.iter())
                .map(|root| scope.spawn(|| git_status(root)))
                .collect();
            (threads.into_iter())
                .zip(roots.clone())
                .map(|(thread, root)| {
                    let status = thread
                        .join()
                        .unwrap_or_else(|_| Err("git status panicked".into()));
                    (root, status)
                })
                .collect()
        })),
        FileOp::GitDiff {
            request,
            repository,
            path,
            old,
            new,
            objects,
        } => Msg::GitDiffed {
            request,
            result: git_diff(&repository, &path, [&old, &new], objects.as_ref()),
        },
        FileOp::DiffLines { request, old, new } => Msg::LinesDiffed {
            request,
            hunks: hunks(&old.to_string(), &new.to_string()),
        },
        FileOp::GitRefs { request, roots } => Msg::GitRefs {
            request,
            roots: (roots.into_iter())
                .map(|root| {
                    let refs = git_refs(&root);
                    (root, refs)
                })
                .collect(),
        },
        FileOp::GitDiffFiles {
            request,
            root,
            old,
            new,
            objects,
        } => Msg::GitDiffFiles {
            request,
            result: git_diff_files(&root, &old, new.as_deref(), objects.as_ref()),
        },
        FileOp::GitUpdate {
            list,
            repository,
            steps,
        } => Msg::GitUpdated {
            list,
            result: git_update(&repository, &steps),
        },
        FileOp::GitSnapshot { request, root, dir } => Msg::GitSnapshot {
            request,
            result: snapshot(&root, &dir),
        },
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
    let (root, head, branch) = repository(path.parent()?)?;
    Some(Head {
        path: head,
        root,
        branch,
        changed: changed(path),
    })
}

/// The repository holding directory `dir`, with its lines changed against `HEAD` and its
/// commits ahead of and behind the upstream; `None` outside a repository.
pub fn terminal_git(dir: &Path) -> Option<TerminalGit> {
    let (root, head, branch) = repository(dir)?;
    let run = |args: &[&str]| {
        let output = git(dir).args(args).stderr(Stdio::null()).output().ok()?;
        output.status.success().then_some(output.stdout)
    };
    // A filter is a command git would run on the working tree's files; an unborn `HEAD` fails.
    let lines = (own_filter(dir) == Some(false))
        .then(|| run(&["diff", "--numstat", "HEAD"]))
        .flatten()
        .map(|out| {
            // Binary files count `-`, which doesn't parse.
            let text = String::from_utf8_lossy(&out);
            let counts = text.lines().filter_map(|line| {
                let mut fields = line.split('\t');
                Some((fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
            });
            counts.fold((0, 0), |(a, r), (x, y): (usize, usize)| (a + x, r + y))
        });
    let upstream =
        run(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]).and_then(|out| {
            let text = String::from_utf8_lossy(&out);
            let mut counts = text.split_whitespace().map(str::parse);
            Some((counts.next()?.ok()?, counts.next()?.ok()?))
        });
    Some(TerminalGit {
        head,
        root,
        branch,
        lines,
        upstream,
    })
}

/// The directory holding the nearest `.git` at or above `dir`, its `HEAD` file and the branch.
fn repository(dir: &Path) -> Option<(PathBuf, PathBuf, String)> {
    let git = (dir.ancestors())
        .map(|dir| dir.join(".git"))
        .find(|git| git.exists())?;
    let root = git.parent()?.to_path_buf();
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
    Some((root, head, branch))
}

/// `root`'s repository, branch and the changes git reports under it; `None` outside a repository.
// shortcut: a submodule's own filter config isn't checked; check each submodule if one turns up.
fn git_status(root: &Path) -> Result<Option<GitStatus>, String> {
    let Some((repository, _, branch)) = repository(root) else {
        return Ok(None);
    };
    no_filter(root)?;
    let output = git(root)
        .args([
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
        ])
        .output()
        .map_err(|err| format!("git: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let files = porcelain_v2(&output.stdout);
    Ok(Some(GitStatus {
        repository,
        branch,
        files,
    }))
}

/// Runs `steps` on paths relative to `repository`, stopping at the first failure.
fn git_update(repository: &Path, steps: &[(GitStep, Vec<PathBuf>)]) -> Result<(), String> {
    no_filter(repository)?;
    let steps = steps
        .iter()
        .filter(|(step, paths)| matches!(step, GitStep::Apply(_)) || !paths.is_empty());
    for (step, paths) in steps {
        let args: &[&str] = match step {
            GitStep::Stage => &["add", "-A"],
            // `reset` over `restore --staged`, which fails on an unborn branch.
            GitStep::Unstage => &["reset", "-q"],
            GitStep::Restore => &["checkout", "-q"],
            GitStep::Delete => {
                for path in paths {
                    fs::remove_file(repository.join(path))
                        .map_err(|err| format!("{}: {err}", path.display()))?;
                }
                continue;
            }
            GitStep::Apply(patch) => {
                let mut child = (git(repository).args(["apply", "--cached", "-"]))
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|err| format!("git: {err}"))?;
                let written = (child.stdin.take().expect("piped")).write_all(patch.as_bytes());
                let output = child.wait_with_output();
                written.map_err(|err| format!("git: {err}"))?;
                check(output.map_err(|err| format!("git: {err}"))?)?;
                continue;
            }
        };
        let output = git(repository)
            .args(args)
            .arg("--")
            .args(paths)
            .output()
            .map_err(|err| format!("git: {err}"))?;
        check(output)?;
    }
    Ok(())
}

/// Git's message when `output` failed.
fn check(output: std::process::Output) -> Result<(), String> {
    match output.status.success() {
        true => Ok(()),
        false => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

/// The files in `git status --porcelain=v2 -z` output.
fn porcelain_v2(out: &[u8]) -> Vec<GitFile> {
    let path = |bytes: &[u8]| PathBuf::from(OsStr::from_bytes(bytes));
    let change = git_change;
    let mut records = out.split(|&b| b == 0).filter(|r| !r.is_empty());
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        // Ordinary entries have 8 space-separated fields before the path, renames 9, unmerged 10.
        let fields = match record.first() {
            Some(b'1') => 8,
            Some(b'2') => 9,
            Some(b'u') => 10,
            Some(b'?') => {
                files.push(GitFile {
                    path: path(record.get(2..).unwrap_or_default()),
                    staged: None,
                    unstaged: Some(GitChange::Untracked),
                    old_path: None,
                });
                continue;
            }
            _ => continue,
        };
        let mut parts = record.splitn(fields + 1, |&b| b == b' ');
        let (Some(&[x, y]), Some(name)) = (parts.nth(1), parts.nth(fields - 2)) else {
            continue;
        };
        files.push(GitFile {
            path: path(name),
            staged: change(x),
            unstaged: change(y),
            old_path: (record[0] == b'2')
                .then(|| records.next().map(path))
                .flatten(),
        });
    }
    files
}

/// A change from git's status letter.
fn git_change(letter: u8) -> Option<GitChange> {
    match letter {
        b'M' => Some(GitChange::Modified),
        b'T' => Some(GitChange::TypeChanged),
        b'A' => Some(GitChange::Added),
        b'D' => Some(GitChange::Deleted),
        b'R' => Some(GitChange::Renamed),
        b'C' => Some(GitChange::Copied),
        b'U' => Some(GitChange::Unmerged),
        _ => None,
    }
}

/// `path`'s texts on two sides of the repository at `repository`, decoded as a buffer would be,
/// and the lines that differ.
fn git_diff(
    repository: &Path,
    path: &Path,
    sides: [&GitSide; 2],
    objects: Option<&GitObjects>,
) -> Result<FileDiff, String> {
    let [old, new] = sides.map(|side| {
        let bytes = side_bytes(repository, path, side, objects)?;
        decode(&bytes)
            .map(|(text, ..)| text)
            .map_err(|reason| format!("{}: {reason}", path.display()))
    });
    let (old, new) = (old?, new?);
    Ok(FileDiff {
        hunks: hunks(&old, &new),
        old: old.into(),
        new: new.into(),
    })
}

/// `path`'s bytes on `side`, empty where it is missing.
// shortcut: a blob is read whole with no size limit; check `cat-file -s` first if a huge one turns up.
fn side_bytes(
    repository: &Path,
    path: &Path,
    side: &GitSide,
    objects: Option<&GitObjects>,
) -> Result<Vec<u8>, String> {
    let tree = match side {
        // shortcut: a symlink reads as its target and a submodule as an error, unlike a tree side; read links and gitlinks if the view shows them.
        GitSide::WorkingTree => {
            let file = repository.join(path);
            let read = read(&file).map_err(|err| format!("{}: {err}", file.display()))?;
            return Ok(read.map(|(bytes, _)| bytes).unwrap_or_default());
        }
        GitSide::Index => None,
        GitSide::Tree(tree) => Some(tree_ish(tree)?),
    };
    // Stage 0, since `:/…` would search commit messages.
    let mut spec = OsString::from(format!("{}:", tree.unwrap_or(":0")));
    spec.push(path);
    let output = with_objects(git(repository), objects)?
        .args(["cat-file", "blob"])
        .arg(&spec)
        .output()
        .map_err(|err| format!("git: {err}"))?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    // A missing path and an unknown tree-ish fail alike; a tree-ish that names a tree means the former.
    let missing = tree.is_none_or(|tree| {
        (with_objects(git(repository), objects).ok())
            .unwrap_or_else(|| git(repository))
            .args(["rev-parse", "--verify", "--quiet"])
            .arg(format!("{tree}^{{tree}}"))
            .output()
            .is_ok_and(|o| o.status.success())
    });
    match missing {
        true => Ok(Vec::new()),
        false => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

/// `root`'s repository's top directory and the short names of its local branches and tags;
/// `None` outside a repository.
fn git_refs(root: &Path) -> Result<Option<(PathBuf, Vec<String>)>, String> {
    let Some((repository, ..)) = repository(root) else {
        return Ok(None);
    };
    let output = git(root)
        .args([
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/tags",
        ])
        .output()
        .map_err(|err| format!("git: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let refs = String::from_utf8_lossy(&output.stdout);
    Ok(Some((
        repository,
        refs.lines().map(str::to_string).collect(),
    )))
}

/// The files under `root` that differ between `old` and `new`, or the working tree, as
/// `git diff --name-status` reports them.
fn git_diff_files(
    root: &Path,
    old: &str,
    new: Option<&str>,
    objects: Option<&GitObjects>,
) -> Result<Vec<ChangedFile>, String> {
    let trees = [Some(old), new].into_iter().flatten();
    let trees = trees.map(tree_ish).collect::<Result<Vec<_>, _>>()?;
    if new.is_none() {
        no_filter(root)?;
    }
    let output = with_objects(git(root), objects)?
        .args([
            "diff",
            "--name-status",
            "-z",
            "--no-ext-diff",
            "--no-relative",
        ])
        .args(trees)
        .args(["--", "."])
        .output()
        .map_err(|err| format!("git: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(name_status(&output.stdout))
}

/// A tree of `root`'s repository's working tree as it is on disk, untracked files included and
/// ignored ones not, written with an index and objects kept in `dir`, so the repository is
/// untouched. The kept index seeds the next snapshot, so a file whose stat is unchanged isn't
/// read again. `None` outside a repository.
pub fn snapshot(root: &Path, dir: &Path) -> Result<Option<Snapshot>, String> {
    let Some((repository, ..)) = repository(root) else {
        return Ok(None);
    };
    // `git add` runs clean filters.
    no_filter(&repository)?;
    let run = |git: &mut Command| {
        let output = git.output().map_err(|err| format!("git: {err}"))?;
        match output.status.success() {
            true => Ok(output.stdout),
            false => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        }
    };
    // A linked worktree has its own index and the main repository's objects.
    let paths = run(git(&repository).args([
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "index",
        "--git-path",
        "objects",
    ]))?;
    let mut paths = (paths.split(|&b| b == b'\n')).map(|p| PathBuf::from(OsStr::from_bytes(p)));
    let (Some(index), Some(alternate)) = (paths.next(), paths.next()) else {
        return Err("git rev-parse named no index".into());
    };
    let objects = GitObjects {
        directory: dir.join("objects"),
        alternates: vec![alternate],
    };
    let indexes = dir.join("indexes");
    let kept = indexes.join(format!("{:016x}", hash(repository.as_os_str().as_bytes())));
    let io = |err: io::Error| format!("{}: {err}", dir.display());
    fs::create_dir_all(&indexes).map_err(io)?;
    fs::create_dir_all(&objects.directory).map_err(io)?;
    // Each snapshot writes its own copy, so two at once don't share a lock; the later one is kept.
    let (copy, _) = create_temp(&kept, &NEXT_TEMP).map_err(io)?;
    let tree = (|| {
        let seed = if kept.exists() { &kept } else { &index };
        match fs::copy(seed, &copy) {
            Ok(_) => {}
            // A repository with nothing added has no index; git starts one.
            Err(err) if err.kind() == ErrorKind::NotFound => fs::remove_file(&copy).map_err(io)?,
            Err(err) => return Err(io(err)),
        }
        let snap = |args: &[&str]| {
            let mut git = with_objects(git(&repository), Some(&objects))?;
            // A split index would write its shared part next to the repository's index.
            git.args(["-c", "core.splitIndex=false"])
                .env("GIT_INDEX_FILE", &copy);
            run(git.args(args))
        };
        snap(&["add", "-A"])?;
        let tree = snap(&["write-tree"])?;
        fs::rename(&copy, &kept).map_err(io)?;
        Ok(String::from_utf8_lossy(&tree).trim().to_string())
    })();
    if tree.is_err() {
        _ = fs::remove_file(&copy);
    }
    Ok(Some(Snapshot {
        repository,
        tree: tree?,
        objects,
    }))
}

/// Removes the snapshot directories under `parent`, each named by its process id, whose process
/// has exited.
pub fn clear_snapshots(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let pid = (entry.file_name().to_str()).and_then(|name| name.parse::<libc::pid_t>().ok());
        // Signal 0 is only checked; ESRCH means no such process.
        let gone = pid.is_some_and(|pid| {
            pid > 0
                && unsafe { libc::kill(pid, 0) } != 0
                && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        });
        if gone {
            _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// The files in `git diff --name-status -z` output: a status, then the path, or a rename's or
/// copy's old and new paths.
fn name_status(out: &[u8]) -> Vec<ChangedFile> {
    let path = |bytes: &[u8]| PathBuf::from(OsStr::from_bytes(bytes));
    let mut fields = out.split(|&b| b == 0);
    let mut files = Vec::new();
    while let Some(status) = fields.next() {
        let Some(change) = status.first().copied().and_then(git_change) else {
            continue;
        };
        let Some(first) = fields.next() else { break };
        let (name, old_path) = match change {
            GitChange::Renamed | GitChange::Copied => (fields.next(), Some(path(first))),
            _ => (Some(first), None),
        };
        // The output ends in NUL, so a cut record's last field is empty.
        let Some(name) = name.filter(|name| !name.is_empty()) else {
            break;
        };
        files.push(ChangedFile {
            path: path(name),
            change,
            old_path,
        });
    }
    files
}

/// `tree`, unless git would read it as an option.
fn tree_ish(tree: &str) -> Result<&str, String> {
    match tree.starts_with('-') {
        true => Err(format!("{tree}: not a tree-ish")),
        false => Ok(tree),
    }
}

/// git in `dir` with no optional locks, literal pathspecs and fsmonitor off, so it neither
/// rewrites the index nor runs the repository's fsmonitor command.
fn git(dir: &Path) -> Command {
    let mut git = Command::new("git");
    git.args(["--no-optional-locks", "--literal-pathspecs"])
        .args(["-c", "core.fsmonitor=false"])
        .current_dir(dir);
    git
}

/// `git` reading objects from `objects` instead of the repository's.
fn with_objects(mut git: Command, objects: Option<&GitObjects>) -> Result<Command, String> {
    if let Some(objects) = objects {
        let alternates = std::env::join_paths(&objects.alternates)
            .map_err(|err| format!("object alternates: {err}"))?;
        git.env("GIT_OBJECT_DIRECTORY", &objects.directory);
        git.env("GIT_ALTERNATE_OBJECT_DIRECTORIES", alternates);
    }
    Ok(git)
}

/// Err when the repository at `dir` defines a content filter, which git would run, or git
/// can't say.
fn no_filter(dir: &Path) -> Result<(), String> {
    match own_filter(dir) {
        Some(false) => Ok(()),
        Some(true) => Err("the repository's config defines a filter, which git would run".into()),
        None => Err("git can't read the repository's config".into()),
    }
}

/// Whether git reports `path` as changed or untracked; no if git can't say.
// shortcut: two processes per file, so a commit with many buffers open runs many; ask once per repository if that shows.
fn changed(path: &Path) -> bool {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return false;
    };
    if own_filter(dir) != Some(false) {
        return false;
    }
    // Without `--no-optional-locks` status rewrites the index, whose report would run it again.
    // A repository's `core.fsmonitor` is a command, and opening a file mustn't run it.
    let status = git(dir)
        .args(["status", "--porcelain", "--"])
        .arg(name)
        .stderr(Stdio::null())
        .output();
    status.is_ok_and(|o| o.status.success() && !o.stdout.is_empty())
}

/// Whether the repository at `dir` defines a content filter in its own config, which git would
/// run; `None` when git can't say.
fn own_filter(dir: &Path) -> Option<bool> {
    let output = Command::new("git")
        .args(["config", "--show-scope", "--get-regexp", "^filter\\."])
        .current_dir(dir)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    match output.status.code() {
        Some(0) => Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| !matches!(line.split_whitespace().next(), Some("global" | "system"))),
        ),
        Some(1) => Some(false),
        _ => None,
    }
}

/// The ad-hoc project root for `file`: the nearest directory above it holding `.git`, else its
/// folder.
pub fn root(file: &Path) -> &Path {
    let folder = file.parent().unwrap_or(file);
    (folder.ancestors())
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(folder)
}

/// The file's bytes and their hash, or `None` when there is no file. Files over
/// `VISIT_LIMIT` are refused unread.
pub fn read(path: &Path) -> io::Result<Option<(Vec<u8>, u64)>> {
    let checked = checked_metadata(path).and_then(|m| match m.len() {
        len if len > VISIT_LIMIT => Err(io::Error::other(format!(
            "file too large ({} MiB)",
            len / (1024 * 1024)
        ))),
        _ => fs::read(path),
    });
    match checked {
        Ok(bytes) => {
            let h = hash(&bytes);
            Ok(Some((bytes, h)))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

const PREVIEW_LIMIT: u64 = 10 * 1024 * 1024;
const VISIT_LIMIT: u64 = 256 * 1024 * 1024;

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
    /// Whatever the file holds, for the editor's own files.
    Overwrite,
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
            // A second save sent before the first answered expects what the first replaced.
            if on_disk == Some(hash(bytes)) {
                return Ok(hash(bytes));
            }
            if on_disk.is_some() && on_disk != expected {
                return Err(WriteFailure::Conflict);
            }
        }
        WriteExpectation::Overwrite => {}
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
    use aiditor_core::Hunk;
    use std::ops::Range;

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
    fn a_second_save_of_the_same_bytes_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        fs::write(&path, "old").unwrap();
        let old = WriteExpectation::Update(Some(hash(b"old")));
        assert_eq!(write(&path, b"new", old).unwrap(), hash(b"new"));
        assert_eq!(write(&path, b"new", old).unwrap(), hash(b"new"));
        assert!(matches!(
            write(&path, b"newer", old),
            Err(WriteFailure::Conflict)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"new");
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
                root: dir.path().to_path_buf(),
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
            assert_eq!(found.root, tree, "the worktree, not the main repository");
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
    fn refuses_to_visit_files_over_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.log");
        let file = fs::File::create(&path).unwrap();
        file.set_len(300 * 1024 * 1024).unwrap();
        let Msg::Opened { result, .. } = run(FileOp::Open {
            intent: aiditor_core::VisitIntent {
                request: 1,
                workspace: Default::default(),
                window: Default::default(),
            },
            path: path.clone(),
        }) else {
            panic!("not opened");
        };
        let too_large = format!("{}: file too large (300 MiB)", path.display());
        assert_eq!(result, Err(too_large));
        // A save over it fails on the disk check and leaves the file alone.
        let err = write(&path, b"x", WriteExpectation::Update(None)).unwrap_err();
        assert!(matches!(&err, WriteFailure::Io(e) if e.to_string().contains("too large")));
        assert_eq!(fs::metadata(&path).unwrap().len(), 300 * 1024 * 1024);
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

    #[test]
    fn a_file_s_root_is_its_git_repo_or_folder() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("src")).unwrap();
        assert_eq!(root(&repo.join("src/x.rs")), repo);
        assert_eq!(root(&repo.join("x.rs")), repo);
        let notes = dir.path().join("notes");
        assert_eq!(root(&notes.join("a.txt")), notes, "outside any repo");
        let worktree = dir.path().join("worktree");
        std::fs::create_dir(&worktree).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: ../repo/.git").unwrap();
        assert_eq!(
            root(&worktree.join("x.rs")),
            worktree,
            "a worktree's .git file"
        );
    }

    #[test]
    fn a_lobby_visit_reply_carries_the_file_s_root() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let open = |workspace| {
            let intent = aiditor_core::VisitIntent {
                request: 1,
                workspace,
                window: Default::default(),
            };
            let path = repo.join("src/x.rs");
            match run(FileOp::Open { intent, path }) {
                Msg::Opened { root, .. } => root,
                _ => panic!("not opened"),
            }
        };
        assert_eq!(open(None), Some(repo.clone()));
        assert_eq!(open(Some(Default::default())), None, "only the lobby's");
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let identity = ["-c", "user.name=t", "-c", "user.email=t@t"];
        let mut git = Command::new("git");
        let status = git.args(identity).args(args).current_dir(dir).status();
        assert!(status.unwrap().success(), "{args:?}");
    }

    fn statuses(roots: &[&Path]) -> Vec<(PathBuf, Result<Option<GitStatus>, String>)> {
        match run(FileOp::GitStatus(
            roots.iter().map(|r| r.to_path_buf()).collect(),
        )) {
            Msg::GitStatus(roots) => roots,
            other => panic!("{other:?}"),
        }
    }

    fn file(path: &str, staged: Option<GitChange>, unstaged: Option<GitChange>) -> GitFile {
        GitFile {
            path: path.into(),
            staged,
            unstaged,
            old_path: None,
        }
    }

    #[test]
    fn terminal_git_counts_changed_lines_and_the_upstream() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let git = |args: &[&str]| git_in(repo, args);
        git(&["init", "-q", "-b", "main"]);
        fs::write(repo.join("a"), "1\n2\n3\n").unwrap();
        fs::write(repo.join("bin"), b"\0\x01").unwrap();
        fs::create_dir(repo.join("sub")).unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "one"]);
        let state = terminal_git(&repo.join("sub")).unwrap();
        assert_eq!((state.root.as_path(), &*state.branch), (repo, "main"));
        assert_eq!(state.head, repo.join(".git/HEAD"));
        assert_eq!((state.lines, state.upstream), (Some((0, 0)), None));

        // Staged and unstaged count together; binary and untracked files don't.
        fs::write(repo.join("a"), "1\nx\n3\n").unwrap();
        git(&["add", "a"]);
        fs::write(repo.join("a"), "1\nx\n3\n4\n").unwrap();
        fs::write(repo.join("bin"), b"\0\x02").unwrap();
        fs::write(repo.join("new"), "untracked\n").unwrap();
        assert_eq!(terminal_git(repo).unwrap().lines, Some((2, 1)));

        git(&["commit", "-q", "-am", "two"]);
        git(&["branch", "other", "HEAD~1"]);
        git(&["branch", "-q", "-u", "other"]);
        git(&["commit", "-q", "--allow-empty", "-m", "three"]);
        assert_eq!(terminal_git(repo).unwrap().upstream, Some((2, 0)));
        git(&["switch", "-q", "other"]);
        git(&["commit", "-q", "--allow-empty", "-m", "elsewhere"]);
        git(&["switch", "-q", "main"]);
        assert_eq!(terminal_git(repo).unwrap().upstream, Some((2, 1)));

        git(&["config", "filter.x.clean", "cat"]);
        let state = terminal_git(repo).unwrap();
        assert_eq!(
            (state.lines, &*state.branch),
            (None, "main"),
            "no filter runs"
        );

        let outside = tempfile::tempdir().unwrap();
        assert_eq!(terminal_git(outside.path()), None);
    }

    #[test]
    fn terminal_git_on_an_unborn_branch_has_no_counts() {
        let dir = tempfile::tempdir().unwrap();
        git_in(dir.path(), &["init", "-q", "-b", "main"]);
        let state = terminal_git(dir.path()).unwrap();
        assert_eq!(
            (&*state.branch, state.lines, state.upstream),
            ("main", None, None)
        );
    }

    #[test]
    fn git_status_reports_each_kind_of_change_on_each_side() {
        use GitChange::*;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let git = |args: &[&str]| git_in(repo, args);
        git(&["init", "-q", "-b", "main"]);
        for name in ["modified", "deleted", "renamed", "both", "kept"] {
            fs::write(repo.join(name), format!("{name}\n")).unwrap();
        }
        fs::write(repo.join(".gitignore"), "ignored\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "x"]);
        fs::write(repo.join("modified"), "edit\n").unwrap();
        fs::remove_file(repo.join("deleted")).unwrap();
        git(&["mv", "renamed", "new name"]);
        fs::write(repo.join("added"), "added\n").unwrap();
        git(&["add", "added"]);
        fs::write(repo.join("both"), "staged\n").unwrap();
        git(&["add", "both"]);
        fs::write(repo.join("both"), "and again\n").unwrap();
        fs::write(repo.join("ignored"), "").unwrap();
        fs::write(repo.join("ü x.txt"), "").unwrap();
        let index = fs::read(repo.join(".git/index")).unwrap();
        let [(root, status)] = &statuses(&[repo])[..] else {
            panic!()
        };
        assert_eq!(root, repo);
        let status = status.clone().unwrap().unwrap();
        assert_eq!(
            (status.repository.as_path(), &*status.branch),
            (repo, "main")
        );
        let mut files = status.files;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let renamed = GitFile {
            old_path: Some("renamed".into()),
            ..file("new name", Some(Renamed), None)
        };
        assert_eq!(
            files,
            [
                file("added", Some(Added), None),
                file("both", Some(Modified), Some(Modified)),
                file("deleted", None, Some(Deleted)),
                file("modified", None, Some(Modified)),
                renamed,
                file("ü x.txt", None, Some(Untracked)),
            ]
        );
        assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    }

    #[test]
    fn git_status_answers_each_root_on_its_own() {
        let repo = tempfile::tempdir().unwrap();
        let plain = tempfile::tempdir().unwrap();
        git_in(repo.path(), &["init", "-q", "-b", "trunk"]);
        fs::create_dir(repo.path().join("sub")).unwrap();
        fs::write(repo.path().join("outside"), "").unwrap();
        fs::write(repo.path().join("sub/inside"), "").unwrap();
        let sub = repo.path().join("sub");
        let answers = statuses(&[plain.path(), &sub]);
        assert_eq!(answers[0], (plain.path().to_path_buf(), Ok(None)));
        let status = answers[1].1.clone().unwrap().unwrap();
        assert_eq!(answers[1].0, sub);
        assert_eq!(status.branch, "trunk");
        assert_eq!(
            status.files,
            [file("sub/inside", None, Some(GitChange::Untracked))]
        );
    }

    /// A repository with `names` committed, each holding its name.
    fn committed(names: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git_in(dir.path(), &["init", "-q", "-b", "main"]);
        for name in names {
            fs::write(dir.path().join(name), format!("{name}\n")).unwrap();
        }
        git_in(dir.path(), &["add", "."]);
        git_in(dir.path(), &["commit", "-q", "-m", "x"]);
        dir
    }

    fn short_status(repo: &Path) -> String {
        let out = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo)
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap()
    }

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn git_update_stages_modified_deleted_and_untracked_files() {
        let dir = committed(&["modified", "deleted"]);
        let repo = dir.path();
        fs::write(repo.join("modified"), "edit\n").unwrap();
        fs::remove_file(repo.join("deleted")).unwrap();
        fs::write(repo.join("new [x]"), "").unwrap();
        let steps = [(GitStep::Stage, paths(&["modified", "deleted", "new [x]"]))];
        assert_eq!(git_update(repo, &steps), Ok(()));
        assert_eq!(
            short_status(repo),
            "D  deleted\nM  modified\nA  \"new [x]\"\n"
        );
    }

    #[test]
    fn git_update_unstages_a_rename_by_both_paths() {
        let dir = committed(&["old"]);
        let repo = dir.path();
        git_in(repo, &["mv", "old", "new"]);
        assert_eq!(
            git_update(repo, &[(GitStep::Unstage, paths(&["new", "old"]))]),
            Ok(())
        );
        assert_eq!(short_status(repo), " D old\n?? new\n");
    }

    #[test]
    fn git_update_unstages_on_an_unborn_branch() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git_in(repo, &["init", "-q"]);
        fs::write(repo.join("a"), "").unwrap();
        git_in(repo, &["add", "a"]);
        assert_eq!(
            git_update(repo, &[(GitStep::Unstage, paths(&["a"]))]),
            Ok(())
        );
        assert_eq!(short_status(repo), "?? a\n");
    }

    #[test]
    fn git_update_restores_and_deletes() {
        let dir = committed(&["modified", "deleted"]);
        let repo = dir.path();
        fs::write(repo.join("modified"), "edit\n").unwrap();
        fs::remove_file(repo.join("deleted")).unwrap();
        fs::write(repo.join("untracked"), "").unwrap();
        let steps = [
            (GitStep::Restore, paths(&["modified", "deleted"])),
            (GitStep::Delete, paths(&["untracked"])),
        ];
        assert_eq!(git_update(repo, &steps), Ok(()));
        assert_eq!(short_status(repo), "");
        assert_eq!(
            fs::read_to_string(repo.join("deleted")).unwrap(),
            "deleted\n"
        );
    }

    #[test]
    fn git_update_stops_at_the_first_failure() {
        let dir = committed(&["a"]);
        let repo = dir.path();
        fs::write(repo.join("untracked"), "").unwrap();
        let steps = [
            (GitStep::Restore, paths(&["unknown"])),
            (GitStep::Delete, paths(&["untracked"])),
        ];
        let err = git_update(repo, &steps).unwrap_err();
        assert!(err.contains("unknown"), "{err}");
        assert!(repo.join("untracked").exists());
    }

    /// A repository with `my file` committed as `text`, and the index's text of it.
    fn committed_as(text: &str) -> tempfile::TempDir {
        let dir = committed(&["my file"]);
        fs::write(dir.path().join("my file"), text).unwrap();
        git_in(dir.path(), &["commit", "-q", "-am", "y"]);
        dir
    }

    fn staged_text(repo: &Path) -> String {
        let out = git(repo).args(["show", ":my file"]).output().unwrap();
        String::from_utf8(out.stdout).unwrap()
    }

    fn apply(base: &str, edited: &str) -> (GitStep, Vec<PathBuf>) {
        let patch = aiditor_core::buffer::patch(Path::new("my file"), base, edited);
        (GitStep::Apply(patch), Vec::new())
    }

    #[test]
    fn git_update_applies_one_change_of_several_to_the_index() {
        let dir = committed_as("a\nb\nc\nd\ne\nf\ng\nh\n");
        let repo = dir.path();
        fs::write(repo.join("my file"), "A\nb\nc\nd\ne\nf\ng\nH\n").unwrap();
        let step = apply("a\nb\nc\nd\ne\nf\ng\nh\n", "A\nb\nc\nd\ne\nf\ng\nh\n");
        assert_eq!(git_update(repo, &[step]), Ok(()));
        assert_eq!(staged_text(repo), "A\nb\nc\nd\ne\nf\ng\nh\n");
        let diff = git(repo).args(["diff", "-U0"]).output().unwrap();
        let diff = String::from_utf8(diff.stdout).unwrap();
        assert!(diff.ends_with("-h\n+H\n"), "{diff}");
    }

    #[test]
    fn git_update_applies_at_the_start_and_end_without_a_final_newline() {
        let dir = committed_as("a\nb\nc");
        let repo = dir.path();
        let steps = [apply("a\nb\nc", "A\nb\nc"), apply("A\nb\nc", "A\nb\nC")];
        assert_eq!(git_update(repo, &steps), Ok(()));
        assert_eq!(staged_text(repo), "A\nb\nC");
        let step = apply("A\nb\nC", "A\nb\nC\nd\n");
        assert_eq!(git_update(repo, &[step]), Ok(()));
        assert_eq!(staged_text(repo), "A\nb\nC\nd\n");
        assert_eq!(fs::read_to_string(repo.join("my file")).unwrap(), "a\nb\nc");
    }

    #[test]
    fn git_update_answers_a_patch_that_does_not_apply() {
        let dir = committed_as("a\nb\n");
        let repo = dir.path();
        let err = git_update(repo, &[apply("x\nb\n", "y\nb\n")]).unwrap_err();
        assert!(err.contains("patch does not apply"), "{err}");
        assert_eq!(staged_text(repo), "a\nb\n");
    }

    #[test]
    fn git_update_refuses_a_repository_filter() {
        let dir = committed(&["a"]);
        let repo = dir.path();
        git_in(repo, &["config", "filter.x.clean", "touch marker; cat"]);
        fs::write(repo.join(".gitattributes"), "* filter=x\n").unwrap();
        fs::write(repo.join("a"), "edit\n").unwrap();
        let index = fs::read(repo.join(".git/index")).unwrap();
        assert!(git_update(repo, &[(GitStep::Stage, paths(&["a"]))]).is_err());
        assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
        assert!(!repo.join("marker").exists());
    }

    #[test]
    fn git_status_refuses_a_repository_filter() {
        let dir = tempfile::tempdir().unwrap();
        git_in(dir.path(), &["init", "-q"]);
        git_in(
            dir.path(),
            &["config", "filter.x.clean", "touch marker; cat"],
        );
        fs::write(dir.path().join(".gitattributes"), "* filter=x\n").unwrap();
        let answers = statuses(&[dir.path()]);
        assert!(answers[0].1.is_err(), "{answers:?}");
        assert!(!dir.path().join("marker").exists());
    }

    #[test]
    fn git_status_answers_git_s_failure() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        let answers = statuses(&[dir.path()]);
        assert!(answers[0].1.is_err(), "{answers:?}");
    }

    #[test]
    fn porcelain_v2_reads_copies_conflicts_and_raw_names() {
        use GitChange::*;
        let fields = "N... 100644 100644 100644 h1 h2";
        let out = [
            format!("2 C. {fields} C100 copy\0source\0").into_bytes(),
            b"u UU N... 100644 100644 100644 100644 h1 h2 h3 conflict\0".to_vec(),
            b"? raw\xff name\0?\0".to_vec(),
        ]
        .concat();
        let copy = GitFile {
            old_path: Some("source".into()),
            ..file("copy", Some(Copied), None)
        };
        let raw = GitFile {
            path: PathBuf::from(OsStr::from_bytes(b"raw\xff name")),
            ..file("", None, Some(Untracked))
        };
        assert_eq!(
            porcelain_v2(&out),
            [
                copy,
                file("conflict", Some(Unmerged), Some(Unmerged)),
                raw,
                file("", None, Some(Untracked)),
            ]
        );
    }

    fn diff_of(
        repo: &Path,
        path: &str,
        old: GitSide,
        new: GitSide,
        objects: Option<GitObjects>,
    ) -> Result<(String, String, Vec<Hunk>), String> {
        let op = FileOp::GitDiff {
            request: 7,
            repository: repo.into(),
            path: path.into(),
            old,
            new,
            objects,
        };
        match run(op) {
            Msg::GitDiffed { request: 7, result } => {
                result.map(|d| (d.old.to_string(), d.new.to_string(), d.hunks))
            }
            other => panic!("{other:?}"),
        }
    }

    fn listed(
        root: &Path,
        old: &str,
        new: Option<&str>,
        objects: Option<GitObjects>,
    ) -> Result<Vec<ChangedFile>, String> {
        let op = FileOp::GitDiffFiles {
            request: 8,
            root: root.into(),
            old: old.into(),
            new: new.map(Into::into),
            objects,
        };
        match run(op) {
            Msg::GitDiffFiles { request: 8, result } => result,
            other => panic!("{other:?}"),
        }
    }

    fn hunk(old: Range<usize>, new: Range<usize>) -> Hunk {
        Hunk { old, new }
    }

    fn changed_file(path: &str, change: GitChange, old_path: Option<&str>) -> ChangedFile {
        ChangedFile {
            path: path.into(),
            change,
            old_path: old_path.map(Into::into),
        }
    }

    #[test]
    fn git_diff_reads_each_pair_of_sides() {
        use GitSide::*;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let git = |args: &[&str]| git_in(repo, args);
        let write = |text: &str| fs::write(repo.join("a.txt"), text).unwrap();
        git(&["init", "-q"]);
        write("x\none\n");
        git(&["add", "."]);
        git(&["commit", "-qm", "1"]);
        write("x\ntwo\n");
        git(&["commit", "-qam", "2"]);
        write("x\nthree\n");
        git(&["add", "."]);
        write("x\r\nfour\r\n");
        let tree = |t: &str| Tree(t.into());
        let pairs = [
            (tree("HEAD~"), tree("HEAD"), "one", "two"),
            (tree("HEAD"), Index, "two", "three"),
            (Index, WorkingTree, "three", "four"),
            (tree("HEAD"), WorkingTree, "two", "four"),
        ];
        for (old, new, before, after) in pairs {
            assert_eq!(
                diff_of(repo, "a.txt", old.clone(), new.clone(), None),
                Ok((
                    format!("x\n{before}\n"),
                    format!("x\n{after}\n"),
                    vec![hunk(1..2, 1..2)]
                )),
                "{old:?} → {new:?}"
            );
        }
    }

    #[test]
    fn git_diff_reads_a_missing_side_as_empty() {
        use GitSide::*;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let git = |args: &[&str]| git_in(repo, args);
        git(&["init", "-q"]);
        fs::write(repo.join("gone"), "a\nb\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "1"]);
        git(&["rm", "-q", "gone"]);
        fs::write(repo.join("added"), "a\nb\n").unwrap();
        git(&["add", "added"]);
        fs::write(repo.join("untracked"), "a\nb\n").unwrap();
        let both = |path| diff_of(repo, path, Tree("HEAD".into()), Index, None);
        let all = vec![hunk(0..0, 0..2)];
        assert_eq!(both("added"), Ok(("".into(), "a\nb\n".into(), all.clone())));
        assert_eq!(
            both("gone"),
            Ok(("a\nb\n".into(), "".into(), vec![hunk(0..2, 0..0)]))
        );
        assert_eq!(
            diff_of(repo, "untracked", Index, WorkingTree, None),
            Ok(("".into(), "a\nb\n".into(), all))
        );
        let unknown = diff_of(repo, "added", Tree("nope".into()), Index, None);
        assert!(unknown.is_err(), "{unknown:?}");
    }

    #[test]
    fn git_diff_answers_a_binary_file_s_reason() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git_in(repo, &["init", "-q"]);
        fs::write(repo.join("bin"), b"a\0b").unwrap();
        git_in(repo, &["add", "."]);
        let result = diff_of(repo, "bin", GitSide::Index, GitSide::WorkingTree, None);
        assert_eq!(result, Err("bin: binary file".into()));
    }

    #[test]
    fn diff_lines_matches_the_diff_of_the_saved_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git_in(repo, &["init", "-q"]);
        let old = "a\nb\nc\nd\ne\n";
        fs::write(repo.join("f"), old).unwrap();
        git_in(repo, &["add", "."]);
        let edited = "a\nB\nc\ne\nf\n";
        let op = FileOp::DiffLines {
            request: 3,
            old: old.into(),
            new: edited.into(),
        };
        let Msg::LinesDiffed { request: 3, hunks } = run(op) else {
            panic!("not diffed")
        };
        assert!(!hunks.is_empty());
        fs::write(repo.join("f"), edited).unwrap();
        let saved = diff_of(repo, "f", GitSide::Index, GitSide::WorkingTree, None);
        assert_eq!(saved.unwrap().2, hunks);
    }

    #[test]
    fn git_refs_lists_branches_and_tags_and_no_repository() {
        let (dir, outside) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let repo = dir.path();
        let git = |args: &[&str]| git_in(repo, args);
        git(&["init", "-q", "-b", "main"]);
        fs::create_dir(repo.join("sub")).unwrap();
        fs::write(repo.join("sub/f"), "f\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "1"]);
        git(&["branch", "topic"]);
        git(&["tag", "v1"]);
        let roots = vec![repo.join("sub"), outside.path().to_path_buf()];
        let refs = ["main", "topic", "v1"].map(String::from).to_vec();
        match run(FileOp::GitRefs {
            request: 3,
            roots: roots.clone(),
        }) {
            Msg::GitRefs {
                request: 3,
                roots: answer,
            } => assert_eq!(
                answer,
                vec![
                    (roots[0].clone(), Ok(Some((repo.to_path_buf(), refs)))),
                    (roots[1].clone(), Ok(None)),
                ]
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn git_diff_files_lists_what_git_diff_name_status_does() {
        use GitChange::*;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let git = |args: &[&str]| git_in(repo, args);
        git(&["init", "-q"]);
        for name in ["mod", "gone", "old", "ä b"] {
            fs::write(repo.join(name), format!("{name}\nsame\nlines\nhere\n")).unwrap();
        }
        git(&["add", "."]);
        git(&["commit", "-qm", "1"]);
        fs::write(repo.join("mod"), "changed\n").unwrap();
        fs::write(repo.join("new"), "new\n").unwrap();
        git(&["rm", "-q", "gone"]);
        git(&["mv", "old", "renamed"]);
        git(&["add", "."]);
        git(&["commit", "-qm", "2"]);
        let names = Command::new("git")
            .args(["diff", "--name-status", "HEAD~", "HEAD"])
            .current_dir(repo)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&names.stdout).lines().count(), 4);
        assert_eq!(
            listed(repo, "HEAD~", Some("HEAD"), None),
            Ok(vec![
                changed_file("gone", Deleted, None),
                changed_file("mod", Modified, None),
                changed_file("new", Added, None),
                changed_file("renamed", Renamed, Some("old")),
            ])
        );
        fs::write(repo.join("ä b"), "edited\n").unwrap();
        fs::write(repo.join("untracked"), "u\n").unwrap();
        assert_eq!(
            listed(repo, "HEAD", None, None),
            Ok(vec![changed_file("ä b", Modified, None)])
        );
    }

    #[test]
    fn git_diff_reads_a_snapshot_tree_from_its_object_directory() {
        let dir = tempfile::tempdir().unwrap();
        let repo = &dir.path().join("repo");
        let snapshot = dir.path().join("snapshot");
        fs::create_dir_all(repo).unwrap();
        fs::create_dir_all(snapshot.join("objects")).unwrap();
        git_in(repo, &["init", "-q"]);
        fs::write(repo.join("a"), "one\n").unwrap();
        git_in(repo, &["add", "."]);
        git_in(repo, &["commit", "-qm", "1"]);
        fs::write(repo.join("a"), "two\n").unwrap();
        let objects = GitObjects {
            directory: snapshot.join("objects"),
            alternates: vec![repo.join(".git/objects")],
        };
        fs::copy(repo.join(".git/index"), snapshot.join("index")).unwrap();
        let snap = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .env("GIT_INDEX_FILE", snapshot.join("index"))
                .env("GIT_OBJECT_DIRECTORY", &objects.directory)
                .env(
                    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                    repo.join(".git/objects"),
                )
                .current_dir(repo)
                .output()
                .unwrap()
        };
        assert!(snap(&["add", "-A"]).status.success());
        let tree = String::from_utf8(snap(&["write-tree"]).stdout).unwrap();
        let tree = tree.trim();
        fs::write(repo.join("a"), "three\n").unwrap();
        let side = || GitSide::Tree(tree.into());
        assert_eq!(
            diff_of(
                repo,
                "a",
                side(),
                GitSide::WorkingTree,
                Some(objects.clone())
            ),
            Ok(("two\n".into(), "three\n".into(), vec![hunk(0..1, 0..1)]))
        );
        assert_eq!(
            listed(repo, tree, None, Some(objects)),
            Ok(vec![changed_file("a", GitChange::Modified, None)])
        );
        let without = diff_of(repo, "a", side(), GitSide::WorkingTree, None);
        assert!(without.is_err(), "{without:?}");
    }

    #[test]
    fn git_diff_runs_no_option_or_repository_filter() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git_in(repo, &["init", "-q"]);
        fs::write(repo.join("a"), "a\n").unwrap();
        git_in(repo, &["add", "."]);
        git_in(repo, &["commit", "-qm", "1"]);
        let option = GitSide::Tree("--output=x".into());
        assert!(diff_of(repo, "a", option, GitSide::Index, None).is_err());
        assert!(listed(repo, "HEAD", Some("--output=x"), None).is_err());
        assert!(!repo.join("x").exists());
        git_in(repo, &["config", "filter.x.clean", "touch marker; cat"]);
        fs::write(repo.join(".gitattributes"), "* filter=x\n").unwrap();
        fs::write(repo.join("a"), "b\n").unwrap();
        assert!(listed(repo, "HEAD", None, None).is_err());
        assert_eq!(listed(repo, "HEAD", Some("HEAD"), None), Ok(vec![]));
        assert!(!repo.join("marker").exists());
    }

    #[test]
    fn name_status_reads_copies_raw_names_and_stops_at_a_cut_record() {
        use GitChange::*;
        let out = b"C075\0src\0copy\0M\0raw\xff\0R100\0old\0".to_vec();
        let raw = ChangedFile {
            path: PathBuf::from(OsStr::from_bytes(b"raw\xff")),
            ..changed_file("", Modified, None)
        };
        assert_eq!(
            name_status(&out),
            [changed_file("copy", Copied, Some("src")), raw]
        );
    }

    #[test]
    fn git_diff_strips_a_byte_order_mark() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git_in(repo, &["init", "-q"]);
        fs::write(repo.join("a"), "\u{feff}a\n").unwrap();
        let result = diff_of(repo, "a", GitSide::Index, GitSide::WorkingTree, None);
        assert_eq!(
            result,
            Ok(("".into(), "a\n".into(), vec![hunk(0..0, 0..1)]))
        );
    }

    /// Each file in a snapshot's tree with its text.
    fn snapshot_files(snap: &Snapshot) -> Vec<(String, String)> {
        let read = |args: &[&str]| {
            let mut git = with_objects(git(&snap.repository), Some(&snap.objects)).unwrap();
            let output = git.args(args).output().unwrap();
            assert!(output.status.success(), "{args:?}");
            String::from_utf8(output.stdout).unwrap()
        };
        (read(&["ls-tree", "-r", "--name-only", &snap.tree]).lines())
            .map(|path| {
                let blob = format!("{}:{path}", snap.tree);
                (path.to_string(), read(&["cat-file", "blob", &blob]))
            })
            .collect()
    }

    /// Every file under `dir` with its bytes.
    fn all_bytes(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir).unwrap().flatten() {
            match entry.file_type().unwrap().is_dir() {
                true => files.extend(all_bytes(&entry.path())),
                false => files.push((entry.path(), fs::read(entry.path()).unwrap())),
            }
        }
        files.sort();
        files
    }

    /// A repository with a modified, a staged-then-changed, a deleted, an untracked and an
    /// ignored file.
    fn dirty_repo(repo: &Path) {
        fs::create_dir_all(repo).unwrap();
        git_in(repo, &["init", "-q"]);
        fs::write(repo.join(".gitignore"), "ignored\n").unwrap();
        for name in ["a", "b", "c"] {
            fs::write(repo.join(name), format!("{name}1\n")).unwrap();
        }
        git_in(repo, &["add", "."]);
        git_in(repo, &["commit", "-qm", "1"]);
        fs::write(repo.join("a"), "a2\n").unwrap();
        fs::write(repo.join("b"), "b2\n").unwrap();
        git_in(repo, &["add", "b"]);
        fs::write(repo.join("b"), "b3\n").unwrap();
        fs::remove_file(repo.join("c")).unwrap();
        fs::create_dir(repo.join("sub")).unwrap();
        fs::write(repo.join("sub/d"), "d\n").unwrap();
        fs::write(repo.join("ignored"), "x\n").unwrap();
    }

    fn snapshot_of(root: &Path, dir: &Path) -> Result<Option<Snapshot>, String> {
        let op = FileOp::GitSnapshot {
            request: 7,
            root: root.into(),
            dir: dir.into(),
        };
        match run(op) {
            Msg::GitSnapshot { request: 7, result } => result,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn git_snapshot_holds_the_working_tree_as_it_is_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let repo = &dir.path().join("repo");
        dirty_repo(repo);
        let snap = snapshot_of(&repo.join("sub"), &dir.path().join("snapshots"));
        let snap = snap.unwrap().unwrap();
        assert_eq!(&snap.repository, repo);
        let expected = [(".gitignore", "ignored\n"), ("a", "a2\n"), ("b", "b3\n")];
        let mut expected: Vec<_> = (expected.iter())
            .map(|(path, text)| (path.to_string(), text.to_string()))
            .collect();
        expected.push(("sub/d".into(), "d\n".into()));
        assert_eq!(snapshot_files(&snap), expected);
        let outside = tempfile::tempdir().unwrap();
        assert_eq!(snapshot_of(outside.path(), dir.path()), Ok(None));
    }

    #[test]
    fn git_snapshot_leaves_the_repository_byte_for_byte_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let repo = &dir.path().join("repo");
        dirty_repo(repo);
        let before = all_bytes(repo);
        let snapshots = tempfile::tempdir().unwrap();
        for _ in 0..2 {
            snapshot_of(repo, snapshots.path()).unwrap().unwrap();
        }
        assert_eq!(all_bytes(repo), before);
    }

    #[test]
    fn git_snapshot_of_a_linked_worktree_reads_its_own_files() {
        let dir = tempfile::tempdir().unwrap();
        let repo = &dir.path().join("repo");
        dirty_repo(repo);
        git_in(repo, &["worktree", "add", "-q", "../wt"]);
        let worktree = dir.path().join("wt");
        fs::write(worktree.join("new"), "n\n").unwrap();
        let snap = snapshot_of(&worktree, &dir.path().join("snapshots"));
        let paths: Vec<_> = (snapshot_files(&snap.unwrap().unwrap()).into_iter())
            .map(|(path, _)| path)
            .collect();
        assert_eq!(paths, [".gitignore", "a", "b", "c", "new"]);
    }

    #[test]
    fn git_snapshot_refuses_a_repository_filter() {
        let dir = tempfile::tempdir().unwrap();
        let repo = &dir.path().join("repo");
        dirty_repo(repo);
        git_in(repo, &["config", "filter.x.clean", "touch marker; cat"]);
        fs::write(repo.join(".gitattributes"), "* filter=x\n").unwrap();
        let answer = snapshot_of(repo, &dir.path().join("snapshots"));
        assert!(answer.is_err(), "{answer:?}");
        assert!(!repo.join("marker").exists());
    }

    #[test]
    fn git_snapshot_inside_a_submodule_reads_the_submodule() {
        let dir = tempfile::tempdir().unwrap();
        let (inner, outer) = (&dir.path().join("inner"), &dir.path().join("outer"));
        dirty_repo(inner);
        git_in(inner, &["add", "-A"]);
        git_in(inner, &["commit", "-qm", "2"]);
        fs::create_dir(outer).unwrap();
        git_in(outer, &["init", "-q"]);
        let file = ["-c", "protocol.file.allow=always"];
        let add = [&file[..], &["submodule", "add", "-q", "../inner", "sub"]].concat();
        git_in(outer, &add);
        let sub = outer.join("sub");
        fs::write(sub.join("a"), "a3\n").unwrap();
        let snap = snapshot_of(&sub, &dir.path().join("snapshots"))
            .unwrap()
            .unwrap();
        assert_eq!(snap.repository, sub);
        let files = snapshot_files(&snap);
        assert!(files.contains(&("a".into(), "a3\n".into())), "{files:?}");
        assert!(files.iter().all(|(path, _)| path != ".gitmodules"));
    }

    #[test]
    fn git_snapshot_again_reads_no_unchanged_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = &dir.path().join("repo");
        dirty_repo(repo);
        // An mtime older than the index, so git doesn't take the entries as racily clean.
        let past = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        for name in [".gitignore", "a", "b", "sub/d"] {
            let file = fs::File::options().write(true).open(repo.join(name));
            file.unwrap().set_modified(past).unwrap();
        }
        // diff-files lists every entry whose stat differs from the disk; those are the files
        // the next `git add -A` would read. The repository's index has `.gitignore`'s old stat.
        let stale = |index: &Path| {
            let output = git(repo)
                .args(["diff-files", "--name-only"])
                .env("GIT_INDEX_FILE", index)
                .output()
                .unwrap();
            assert!(output.status.success());
            String::from_utf8(output.stdout).unwrap()
        };
        assert!(stale(&repo.join(".git/index")).contains(".gitignore"));
        let snapshots = dir.path().join("snapshots");
        let first = snapshot_of(repo, &snapshots).unwrap().unwrap();
        let kept = fs::read_dir(snapshots.join("indexes")).unwrap();
        let kept: Vec<_> = kept.flatten().map(|entry| entry.path()).collect();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert_eq!(stale(&kept[0]), "");
        let again = snapshot_of(repo, &snapshots).unwrap().unwrap();
        assert_eq!(again.tree, first.tree);
    }

    #[test]
    fn clear_snapshots_removes_only_exited_processes_directories() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = Command::new("true").spawn().unwrap();
        let exited = child.id().to_string();
        child.wait().unwrap();
        let alive = std::process::id().to_string();
        for name in [&exited, &alive, "notes"] {
            fs::create_dir_all(dir.path().join(name).join("objects")).unwrap();
        }
        clear_snapshots(dir.path());
        let mut left: Vec<_> = (fs::read_dir(dir.path()).unwrap().flatten())
            .map(|entry| entry.file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, [alive, "notes".to_string()]);
    }
}
