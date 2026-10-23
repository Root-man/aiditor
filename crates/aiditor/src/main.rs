use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use aiditor_core::{Core, Msg};
use aiditor_services::configuration::{read_config, read_projects};
use aiditor_services::{files, recovery};
use gpui::{App, Application, WindowOptions, prelude::*};
use tokio::sync::mpsc;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer, filter::LevelFilter, fmt};

use editor::Editor;

mod editor;
mod element;
mod input;

const FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "Noto Sans Mono"
};

const USAGE: &str = "usage: aiditor [--config-dir DIR] [--debug] [FILE | FOLDER]";

/// The ad-hoc project root for `file`: the nearest directory above it holding `.git`, else its
/// folder.
fn root(file: &Path) -> &Path {
    let folder = file.parent().unwrap_or(file);
    (folder.ancestors())
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(folder)
}

#[derive(Debug, Default, PartialEq)]
struct Args {
    file: Option<PathBuf>,
    /// `--config-dir`, or the platform config dir.
    config_dir: Option<PathBuf>,
    /// `recovery` in the platform data dir, for auto-save files.
    recovery_dir: Option<PathBuf>,
    debug: bool,
}

fn parse_args(mut args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let mut parsed = Args::default();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--debug") => parsed.debug = true,
            Some("--config-dir") => {
                parsed.config_dir = Some(PathBuf::from(
                    args.next().ok_or("--config-dir needs a DIR")?,
                ))
            }
            Some("-h" | "--help") => return Err(USAGE.into()),
            Some(flag) if flag.starts_with('-') => return Err(format!("unknown flag {flag}")),
            _ if parsed.file.is_none() => parsed.file = Some(PathBuf::from(arg)),
            _ => return Err("more than one FILE".into()),
        }
    }
    let dirs = directories::ProjectDirs::from("", "", "aiditor");
    parsed.config_dir =
        (parsed.config_dir).or_else(|| dirs.as_ref().map(|d| d.config_dir().to_owned()));
    parsed.recovery_dir = dirs.map(|d| d.data_dir().join("recovery"));
    Ok(parsed)
}

/// Sends each event `tracing` formats to the core as a `*Messages*` line.
struct LogPipe(mpsc::Sender<Msg>);

impl std::io::Write for LogPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let line = String::from_utf8_lossy(buf).trim_end().to_string();
        // shortcut: a line is dropped while the channel is full; give logs their own channel if lines go missing.
        _ = self.0.try_send(Msg::Log(line));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The `tracing` layer that logs warnings and errors to `*Messages*`, formatted as stderr gets
/// them but without colour.
fn messages_layer<S>(tx: mpsc::Sender<Msg>) -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fmt::layer()
        .with_ansi(false)
        .with_writer(move || LogPipe(tx.clone()))
        .with_filter(LevelFilter::WARN)
}

fn main() {
    let channel = aiditor_services::channel();
    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(messages_layer(channel.0.clone()))
        .init();

    let mut args = parse_args(std::env::args_os().skip(1)).unwrap_or_else(|err| {
        if err == USAGE {
            println!("{USAGE}");
            std::process::exit(0);
        }
        eprintln!("aiditor: {err}\n{USAGE}");
        std::process::exit(2);
    });
    let mut core = Core::default();
    let dir = args.config_dir.as_deref();
    core.handle(
        Instant::now(),
        Msg::SettingsLoaded(read_config(dir, "settings.toml")),
    );
    core.handle(
        Instant::now(),
        Msg::KeymapLoaded(read_config(dir, "keymap.toml")),
    );
    core.handle(Instant::now(), read_projects(dir));
    if let Some(arg) = args.file.take() {
        let path = std::path::absolute(&arg).unwrap_or(arg);
        if path.is_dir() {
            core.launch(&path, &path);
        } else {
            let file = files::read(&path).unwrap_or_else(|err| {
                eprintln!("aiditor: {}: {err}", path.display());
                std::process::exit(1);
            });
            core.launch(&path, root(&path));
            core.open(&path, file);
        }
    }
    // Read before this session writes any, so all are an earlier session's.
    let recovered = args
        .recovery_dir
        .as_deref()
        .map(recovery::list)
        .unwrap_or_default();
    channel
        .0
        .try_send(Msg::Recover(recovered))
        .expect("the new service channel has room");

    // Services run here, on background threads; GPUI owns the main thread.
    let runtime = tokio::runtime::Runtime::new().expect("failed to start the Tokio runtime");
    let _guard = runtime.enter();
    tracing::info!("starting");

    Application::new().run(|cx: &mut App| {
        let window = cx
            .open_window(WindowOptions::default(), |window, cx| {
                cx.new(|cx| Editor::new(core, args, channel, window, cx))
            })
            .expect("failed to open a window");
        window
            .update(cx, |editor, window, _| window.focus(&editor.focus))
            .expect("the window was just opened");
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn warnings_reach_the_core_without_colour_and_info_does_not() {
        let (tx, mut rx) = aiditor_services::channel();
        let subscriber = tracing_subscriber::registry().with(messages_layer(tx));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("starting");
            tracing::warn!(n = 1, "careful");
        });
        let Ok(Msg::Log(line)) = rx.try_recv() else {
            panic!("no log line");
        };
        assert!(
            line.contains("WARN") && line.ends_with("careful n=1"),
            "{line}"
        );
        assert!(!line.contains('\x1b'), "{line}");
        assert!(rx.try_recv().is_err(), "info is not logged");
    }

    #[test]
    fn parses_flags_and_file() {
        let args = parse(&["--debug", "--config-dir", "/tmp/c", "a.txt"]).unwrap();
        assert_eq!(args.file, Some(PathBuf::from("a.txt")));
        assert_eq!(args.config_dir, Some(PathBuf::from("/tmp/c")));
        assert!(args.debug);
        assert!(!parse(&[]).unwrap().debug);
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
    fn rejects_bad_arguments() {
        for bad in [&["--config-dir"][..], &["--nope"], &["a", "b"]] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
