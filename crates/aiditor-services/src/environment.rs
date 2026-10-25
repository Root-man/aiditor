//! A project's environment, captured from the user's interactive login shell.

use std::ffi::OsStr;
use std::io;
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
    use std::os::unix::fs::PermissionsExt;
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
}
