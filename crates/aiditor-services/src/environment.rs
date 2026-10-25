//! A project's environment, captured from the user's interactive login shell.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// How long a capture may take before its shell is killed.
pub const TIMEOUT: Duration = Duration::from_secs(5);

// `printf` turns `\036` into the record separator, so the markers are not in the command text,
// which bash also puts in `BASH_EXECUTION_STRING`.
const START: &[u8] = b"\x1eaiditor-env\x1e";
const END: &[u8] = b"\x1eaiditor-env-end\x1e";
const PRINT: &str = r"printf '\036aiditor-env\036'; env -0; printf '\036aiditor-env-end\036'";

/// Captures the environment of `SHELL` in `dir`, within `TIMEOUT`.
pub async fn run(dir: &Path) -> Result<Vec<(String, String)>, String> {
    // shortcut: no `SHELL` means `/bin/sh`; read the passwd entry if a launcher leaves it unset.
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
    capture(&shell, dir, TIMEOUT).await
}

/// Runs `shell -l -i -c` in `dir`, in a new session with stdin from `/dev/null`, and returns
/// the `NAME=value` entries it prints between the markers, without `SHLVL`, whatever its exit
/// status. After `timeout` the shell's process group is killed.
pub async fn capture(
    shell: &OsStr,
    dir: &Path,
    timeout: Duration,
) -> Result<Vec<(String, String)>, String> {
    let fish = Path::new(shell).file_name() == Some(OsStr::new("fish"));
    let command = if fish {
        format!("emit fish_prompt; {PRINT}")
    } else {
        PRINT.into()
    };
    let mut cmd = Command::new(shell);
    (cmd.args(["-l", "-i", "-c", &command]).current_dir(dir))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // A session of its own: no terminal to read from or be stopped by, and one group to kill.
    unsafe {
        cmd.pre_exec(|| match libc::setsid() {
            -1 => Err(io::Error::last_os_error()),
            _ => Ok(()),
        })
    };
    let shell = Path::new(shell).display();
    let mut child = cmd
        .spawn()
        .map_err(|err| format!("can't start {shell} in {}: {err}", dir.display()))?;
    let pid = child.id();
    let mut stdout = child.stdout.take().expect("stdout is piped");
    // Stops at the end marker, not at EOF, so a daemon the profile starts can't hold it open.
    let read = async {
        let mut out = Vec::new();
        let mut chunk = [0; 8192];
        loop {
            if let Some(vars) = between(&out) {
                return Ok(parse(vars));
            }
            match stdout.read(&mut chunk).await {
                Ok(0) => return Err(format!("{shell} printed no environment")),
                Ok(n) => out.extend_from_slice(&chunk[..n]),
                Err(err) => return Err(format!("reading {shell}'s output: {err}")),
            }
        }
    };
    let result = tokio::time::timeout(timeout, read).await;
    // Tokio reaps the dropped child; logout scripts run on without holding the result.
    result.unwrap_or_else(|_| {
        if let Some(pid) = pid {
            // The shell leads its session, so its pid is its group's id.
            unsafe { libc::killpg(pid as libc::pid_t, libc::SIGKILL) };
        }
        Err(format!("{shell} took over {} s", timeout.as_secs_f32()))
    })
}

/// How long `direnv export json` may take; `use flake` can build on first load.
pub const DIRENV_TIMEOUT: Duration = Duration::from_secs(30);

/// Applies `direnv export json` in `dir` to `vars` when `direnv` is on their `PATH`, within
/// `timeout`; on an error `vars` are left as they were.
pub async fn direnv(
    vars: &mut Vec<(String, String)>,
    dir: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let path = vars
        .iter()
        .find(|(n, _)| n == "PATH")
        .map(|(_, v)| v.as_str());
    let Some(direnv) = std::env::split_paths(path.unwrap_or_default())
        .map(|d| d.join("direnv"))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    else {
        return Ok(());
    };
    let mut cmd = Command::new(&direnv);
    (cmd.args(["export", "json"]).current_dir(dir))
        .env_clear()
        .envs(vars.iter().map(|(n, v)| (n, v)))
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        // shortcut: kills direnv alone, so a hung `.envrc`'s children outlive it; give it its
        // own group, as `capture` does, if that shows up.
        .kill_on_drop(true);
    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| format!("direnv took over {} s", timeout.as_secs_f32()))?
        .map_err(|err| format!("can't start {}: {err}", direnv.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("direnv {}: {}", output.status, stderr.trim()));
    }
    if output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(());
    }
    let changes: BTreeMap<String, Option<String>> = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("bad direnv output: {err}"))?;
    for (name, value) in changes {
        vars.retain(|(n, _)| *n != name);
        if let Some(value) = value {
            vars.push((name, value));
        }
    }
    Ok(())
}

/// What lies between the start marker and the first end marker after it.
fn between(out: &[u8]) -> Option<&[u8]> {
    let find = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).position(|w| w == needle);
    let start = find(out, START)? + START.len();
    let end = find(&out[start..], END)?;
    Some(&out[start..start + end])
}

/// `env -0`'s entries; ones without a name or not UTF-8 are skipped rather than mangled.
fn parse(vars: &[u8]) -> Vec<(String, String)> {
    (vars.split(|&b| b == 0))
        .filter_map(|entry| std::str::from_utf8(entry).ok()?.split_once('='))
        .filter(|(name, _)| !name.is_empty() && *name != "SHLVL")
        .map(|(name, value)| (name.into(), value.into()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Instant;

    use super::*;

    /// A fake `SHELL` named `name` in `dir` running `body`; `$4` is the command.
    fn shell(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn get<'a>(vars: &'a [(String, String)], name: &str) -> Option<&'a str> {
        vars.iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn parses_between_the_markers() {
        let out = b"noise\x1eaiditor-env\x1eA=1\0B=x=y\0SHLVL=2\0=bad\0\xff=1\0\x1eaiditor-env-end\x1elogout";
        let vars = parse(between(out).unwrap());
        assert_eq!(vars, [("A".into(), "1".into()), ("B".into(), "x=y".into())]);
        assert_eq!(between(b"\x1eaiditor-env\x1eA=1\0"), None, "no end marker");
    }

    #[tokio::test]
    async fn noise_and_a_failing_exit_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let body = r#"[ "$1 $2 $3" = "-l -i -c" ] || exit 2
echo profile noise
export FOO=bar HERE="$(pwd)"
sh -c "$4"
echo logout noise
exit 1"#;
        let sh = shell(tmp.path(), "sh", body);
        let vars = capture(sh.as_os_str(), tmp.path(), TIMEOUT).await.unwrap();
        assert_eq!(get(&vars, "FOO"), Some("bar"));
        assert_eq!(get(&vars, "SHLVL"), None);
        let noise = |(n, v): &(String, String)| format!("{n}={v}").contains("noise");
        assert!(!vars.iter().any(noise), "{vars:?}");
        let here = PathBuf::from(get(&vars, "HERE").unwrap());
        assert_eq!(
            here.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
    }

    #[tokio::test]
    async fn fish_emits_its_prompt_event_first() {
        let tmp = tempfile::tempdir().unwrap();
        let body = r#"[ "${4%%;*}" = "emit fish_prompt" ] || exit 2
FISH=1 sh -c "${4#*; }""#;
        let fish = shell(tmp.path(), "fish", body);
        let vars = capture(fish.as_os_str(), tmp.path(), TIMEOUT)
            .await
            .unwrap();
        assert_eq!(get(&vars, "FISH"), Some("1"));
    }

    #[tokio::test]
    async fn a_failing_or_missing_shell_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let sh = shell(tmp.path(), "sh", "echo oops; exit 1");
        let err = capture(sh.as_os_str(), tmp.path(), TIMEOUT).await;
        assert!(err.unwrap_err().ends_with("printed no environment"));
        let gone = tmp.path().join("gone");
        let err = capture(gone.as_os_str(), tmp.path(), TIMEOUT).await;
        assert!(err.unwrap_err().starts_with("can't start"));
    }

    #[tokio::test]
    async fn a_hanging_shell_is_killed_with_its_group() {
        let tmp = tempfile::tempdir().unwrap();
        let pid = tmp.path().join("pid");
        let body = format!("sleep 30 & echo $! > {}\nwait", pid.display());
        let sh = shell(tmp.path(), "sh", &body);
        let start = Instant::now();
        // Long enough for a loaded machine to start the shell and write the pid.
        let err = capture(sh.as_os_str(), tmp.path(), Duration::from_secs(1)).await;
        assert!(err.unwrap_err().contains("took over"));
        assert!(start.elapsed() < Duration::from_secs(3));
        let pid: libc::pid_t = std::fs::read_to_string(pid)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // The orphaned sleep is reaped by init once killed.
        let gone = (0..100).any(|_| {
            std::thread::sleep(Duration::from_millis(10));
            (unsafe { libc::kill(pid, 0) }) == -1
        });
        assert!(gone, "the shell's background job outlived it");
    }

    /// Runs `direnv` from a `PATH` holding a fake one with `body`, over `A=1` and `OLD=x`.
    async fn run_direnv(body: &str) -> (Result<(), String>, Vec<(String, String)>, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        shell(&bin, "direnv", body);
        let path = format!("/nowhere:{}", bin.display());
        let mut vars = vec![
            ("PATH".into(), path),
            ("A".into(), "1".into()),
            ("OLD".into(), "x".into()),
        ];
        let timeout = Duration::from_secs(1);
        let result = direnv(&mut vars, tmp.path(), timeout).await;
        (result, vars, tmp.path().canonicalize().unwrap())
    }

    #[tokio::test]
    async fn direnv_sets_and_unsets_with_the_captured_environment() {
        let body = r#"[ "$1 $2 $TERM $A" = "export json dumb 1" ] || exit 2
[ -z "$HOME" ] || exit 3
printf '{"HERE":"%s","OLD":null,"A":"2"}' "$(pwd -P)""#;
        let (result, vars, dir) = run_direnv(body).await;
        result.unwrap();
        assert_eq!(get(&vars, "HERE"), dir.to_str());
        assert_eq!(get(&vars, "A"), Some("2"));
        assert_eq!(get(&vars, "OLD"), None);
    }

    #[tokio::test]
    async fn direnv_without_changes_or_on_path_changes_nothing() {
        let (result, vars, _) = run_direnv("exit 0").await;
        result.unwrap();
        assert_eq!(get(&vars, "OLD"), Some("x"));
        let mut vars = vec![("PATH".to_string(), "/nowhere".to_string())];
        let tmp = tempfile::tempdir().unwrap();
        direnv(&mut vars, tmp.path(), TIMEOUT).await.unwrap();
        assert_eq!(vars.len(), 1);
    }

    #[tokio::test]
    async fn a_failing_direnv_keeps_the_environment() {
        let blocked = "echo 'direnv: error .envrc is blocked. Run `direnv allow` to approve its content' >&2; exit 1";
        let (result, vars, _) = run_direnv(blocked).await;
        let err = result.unwrap_err();
        assert!(err.contains(".envrc is blocked"), "{err}");
        assert_eq!(get(&vars, "OLD"), Some("x"));
        for bad in [r#"echo '{"A":2}'"#, "echo not json", "echo '[]'"] {
            let (result, vars, _) = run_direnv(bad).await;
            assert!(
                result.unwrap_err().starts_with("bad direnv output"),
                "{bad}"
            );
            assert_eq!(get(&vars, "A"), Some("1"));
        }
        // `exec` so the killed direnv is the sleep itself.
        let (result, vars, _) = run_direnv("exec /bin/sleep 30").await;
        assert_eq!(result.unwrap_err(), "direnv took over 1 s");
        assert_eq!(get(&vars, "A"), Some("1"));
    }
}
