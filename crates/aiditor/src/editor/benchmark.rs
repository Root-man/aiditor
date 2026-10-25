//! Opt-in real-window measurements; the core, configuration and files belong only to this run.
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use aiditor_core::{Core, Msg, TerminalOp};
use gpui::{App, Application, WindowOptions, prelude::*};
use tokio::sync::{mpsc, oneshot};

use super::Editor;
use crate::Args;

const SAMPLES: usize = 120;
const SCENARIOS: [&str; 3] = ["baseline", "yes", "cat"];

pub(crate) struct Run {
    output: PathBuf,
    phase: usize,
    warm_until: Instant,
    pending: Option<(Instant, String, oneshot::Sender<()>)>,
    latency: Vec<Duration>,
    gaps: Vec<Duration>,
    last_paint: Option<Instant>,
    bytes: usize,
    report: String,
    terminal: Option<aiditor_core::TerminalId>,
}

impl Run {
    fn new(output: PathBuf, now: Instant) -> Self {
        Self {
            output,
            phase: 0,
            warm_until: now + Duration::from_secs(2),
            pending: None,
            latency: Vec::new(),
            gaps: Vec::new(),
            last_paint: None,
            bytes: 0,
            report: String::new(),
            terminal: None,
        }
    }

    pub(crate) fn painted(&mut self, now: Instant, text: Option<&str>) {
        if now < self.warm_until {
            return;
        }
        if let Some(last) = self.last_paint.replace(now) {
            self.gaps.push(now - last);
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, expected, _)| Some(expected.as_str()) == text)
        {
            let (sent, _, done) = self.pending.take().unwrap();
            self.latency.push(now - sent);
            _ = done.send(());
        }
    }

    pub(crate) fn output(&mut self, msg: &Msg, now: Instant) {
        if let Msg::TerminalOutput { id, bytes } = msg
            && Some(*id) == self.terminal
            && now >= self.warm_until
        {
            self.bytes += bytes.len();
        }
    }

    fn finish_phase(&mut self) {
        for (metric, samples) in [("input-to-paint", &self.latency), ("paint-gap", &self.gaps)] {
            for sample in samples {
                self.report.push_str(&format!(
                    "{}\t{metric}\t{}\n",
                    SCENARIOS[self.phase],
                    sample.as_nanos()
                ));
            }
        }
        self.report.push_str(&format!(
            "{}\toutput-bytes\t{}\n",
            SCENARIOS[self.phase], self.bytes
        ));
    }
}

fn input(
    editor: &mut Editor,
    sent: Instant,
    done: oneshot::Sender<()>,
    cx: &mut gpui::Context<Editor>,
) {
    let mut run = editor.benchmark.take().unwrap();
    if run.latency.len() == SAMPLES {
        run.finish_phase();
        editor.terminals.kill_all();
        if let Some(id) = run.terminal.take() {
            editor.handle(
                Msg::TerminalExited {
                    id,
                    code: 0,
                    signal: None,
                },
                cx,
            );
        }
        run.phase += 1;
        if run.phase == SCENARIOS.len() {
            if let Err(err) = std::fs::write(&run.output, &run.report) {
                eprintln!("GUI benchmark report: {err}");
            }
            cx.quit();
            return;
        }
        run.latency.clear();
        run.gaps.clear();
        run.last_paint = None;
        run.bytes = 0;
        run.warm_until = Instant::now() + Duration::from_secs(2);
        let (id, _) = editor.core.create_terminal("benchmark-output", 50, 120);
        run.terminal = Some(id);
        let command = if SCENARIOS[run.phase] == "yes" {
            vec!["/usr/bin/yes".into()]
        } else {
            vec![
                "/bin/sh".into(),
                "-c".into(),
                "while :; do cat \"$1\"; done".into(),
                "bench".into(),
                run.output
                    .with_extension("fixture")
                    .to_string_lossy()
                    .into_owned(),
            ]
        };
        let rx = editor
            .terminals
            .run(TerminalOp::Spawn {
                id,
                command: Some(command),
                cwd: std::env::temp_dir(),
                env: Vec::new(),
                rows: 50,
                cols: 120,
            })
            .expect("spawn returns a receiver");
        Editor::drain(rx, cx);
    }
    if Instant::now() < run.warm_until {
        _ = done.send(());
    } else {
        let value = format!("{:04}", run.latency.len() + 1);
        run.pending = Some((sent, format!("// probe {value}"), done));
        editor.handle(
            Msg::Insert {
                text: value,
                replace: Some(9..13),
            },
            cx,
        );
    }
    editor.benchmark = Some(run);
}

pub(crate) fn run(output: PathBuf) {
    let fixture = output.with_extension("fixture");
    let text = "alpha été terminal flood output\n".repeat(1_000_000);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&fixture)
        .expect("create temporary cat fixture")
        .write_all(text.as_bytes())
        .expect("write cat fixture");
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime");
    let _guard = runtime.enter();
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(90)).await;
        eprintln!("GUI benchmark timed out waiting for visible frame paints");
        std::process::exit(1);
    });
    let (tx, mut rx) = mpsc::channel(1);
    tokio::spawn(async move {
        loop {
            let due = Instant::now() + Duration::from_millis(50);
            tokio::time::sleep_until(due.into()).await;
            let (done, painted) = oneshot::channel();
            if tx.send((due, done)).await.is_err() {
                break;
            }
            if painted.await.is_err() {
                break;
            }
        }
    });
    Application::new().run(move |cx: &mut App| {
        crate::emoji::drop_undrawable(cx);
        let mut core = Core::default();
        let source = format!(
            "// probe 0000\n{}",
            "fn item() -> usize { 42 }\n".repeat(10_000)
        );
        core.open(&output.with_extension("rs"), Some((source.into_bytes(), 0)));
        let window = cx
            .open_window(WindowOptions::default(), |window, cx| {
                cx.new(|cx| {
                    let mut editor = Editor::new(
                        core,
                        Args::default(),
                        aiditor_services::channel(),
                        window,
                        cx,
                    );
                    editor.benchmark = Some(Run::new(output, Instant::now()));
                    cx.spawn(async move |this, cx| {
                        while let Some((sent, done)) = rx.recv().await {
                            if this.update(cx, |e, cx| input(e, sent, done, cx)).is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                    editor
                })
            })
            .expect("open benchmark window");
        window
            .update(cx, |editor, window, _| window.focus(&editor.focus))
            .unwrap();
        cx.activate(true);
    });
    std::fs::remove_file(fixture).expect("remove cat fixture");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_only_the_paint_containing_the_submitted_input() {
        let start = Instant::now();
        let mut run = Run::new("unused".into(), start);
        run.warm_until = start;
        let (done, mut received) = oneshot::channel();
        run.pending = Some((start, "// probe 0001".into(), done));
        run.painted(start + Duration::from_millis(5), Some("// probe 0000"));
        run.painted(start + Duration::from_millis(12), Some("// probe 0001"));
        assert_eq!(run.latency, [Duration::from_millis(12)]);
        assert_eq!(run.gaps, [Duration::from_millis(7)]);
        assert_eq!(received.try_recv(), Ok(()));
        run.finish_phase();
        assert!(run.report.contains("baseline\tinput-to-paint\t12000000\n"));
    }
}
