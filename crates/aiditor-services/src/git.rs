//! Git: statuses, staging, diffs and snapshots. Blocking calls, as `files::run` makes them.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, ErrorKind, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use aiditor_core::buffer::{decode, hash, hunks};
use aiditor_core::{
    ChangedFile, FileDiff, GitChange, GitFile, GitObjects, GitSide, GitStatus, GitStep, Head,
    Snapshot, TerminalGit,
};

use crate::files::{NEXT_TEMP, create_temp, read};

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
pub(crate) fn git_status(root: &Path) -> Result<Option<GitStatus>, String> {
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
pub(crate) fn git_update(
    repository: &Path,
    steps: &[(GitStep, Vec<PathBuf>)],
) -> Result<(), String> {
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
pub(crate) fn git_diff(
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
pub(crate) fn git_refs(root: &Path) -> Result<Option<(PathBuf, Vec<String>)>, String> {
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
pub(crate) fn git_diff_files(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::run;
    use aiditor_core::{FileOp, Hunk, Msg};
    use std::fs::OpenOptions;
    use std::ops::Range;
    use std::time::SystemTime;

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
