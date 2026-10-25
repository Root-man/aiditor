use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aiditor_core::buffer::{self, Buffers};
use aiditor_core::command::{Args, Prefix};
use aiditor_core::display;
use aiditor_core::project::{Loaded, Project};
use aiditor_core::settings::Settings;
use aiditor_core::terminal::Terminal;
use aiditor_core::{Core, Ctx, Effect, FileOp, Msg};

use super::support::{measure, prepared};

fn call(core: &mut Core, name: &str, args: Args) -> Vec<Effect> {
    let mut ctx = Ctx {
        core,
        now: Instant::now(),
        effects: Vec::new(),
    };
    ctx.call(name, args);
    ctx.effects
}

fn command(core: &mut Core, name: &str) {
    black_box(call(core, name, Args::default()));
}

fn count(core: &mut Core, name: &str, n: usize) {
    black_box(call(
        core,
        name,
        Args {
            prefix: Some(Prefix::Number(n as i64)),
            ..Args::default()
        },
    ));
}

fn open(path: &str, text: &str) -> Core {
    let mut core = Core::default();
    core.open(
        Path::new(path),
        Some((text.as_bytes().to_vec(), buffer::hash(text.as_bytes()))),
    );
    core.handle(
        Instant::now(),
        Msg::Resize {
            width: 960.,
            height: core.settings().line_height() * 51.,
            cell_width: 8.,
        },
    );
    core
}

/// `lines` lines of `ext` code, one item each; Rust and TSX items are nested 100 to a module or
/// component, as real files are, so the tree's root doesn't hold every item.
fn source(ext: &str, lines: usize) -> String {
    let group = |i: usize, open: String, close: &str| {
        let open = if i.is_multiple_of(100) {
            open
        } else {
            String::new()
        };
        let close = if i % 100 == 99 || i + 1 == lines {
            close
        } else {
            ""
        };
        (open, close.to_string())
    };
    (0..lines)
        .map(|i| match ext {
            "rs" => {
                let (open, close) = group(i, format!("mod m{} {{ ", i / 100), " }");
                format!("{open}fn item_{i}() -> usize {{ let café = {i}; café + 1 }}{close}\n")
            }
            "tsx" => {
                let (open, close) = group(
                    i,
                    format!("export function Group{}() {{ ", i / 100),
                    " return null; }",
                );
                format!(
                    "{open}const Item{i} = () => <div title=\"été\">{{{i} + 1}}</div>;{close}\n"
                )
            }
            "js" => format!("export function item{i}() {{ return \"été\" + {i}; }}\n"),
            "ts" => format!("export const item{i}: string = \"été {i}\";\n"),
            "ex" => format!("def item_{i}(), do: {{:ok, \"été\", {i}}}\n"),
            "heex" => format!("<div id=\"item{i}\" class={{@class}}>été <%= @name %></div>\n"),
            "yaml" => format!("item{i}: {{name: été, count: {i}}}\n"),
            "toml" => format!("item{i} = {{name = \"été\", count = {i}}}\n"),
            "md" => format!("# Heading {i} été\n"),
            _ => unreachable!(),
        })
        .collect()
}

fn colored(frame: &display::FrameDisplay) -> bool {
    frame.windows.iter().any(|w| {
        w.display
            .lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.face.fg.is_some()))
    })
}

fn syntax(output: &mut String) {
    let undo = aiditor_core::key::parse_keys("C-/").unwrap()[0];
    for ext in ["rs", "tsx", "js", "ts", "ex", "heex", "yaml", "toml", "md"] {
        for lines in [10_000, 100_000] {
            if lines == 100_000 && !["rs", "tsx"].contains(&ext) {
                continue;
            }
            let text = source(ext, lines);
            let path = format!("/benchmark.{ext}");
            let mut core = open(&path, &text);
            assert_eq!(core.current().len_lines(), lines + 1);
            assert!(colored(&core.frame()), "{ext} highlights");
            measure(
                &format!("syntax-load/{ext}/{lines}-lines-warm-grammar"),
                || {
                    let mut buffers = Buffers::default();
                    black_box(buffers.create_for_file(Path::new(&path), black_box(&text)));
                },
                output,
            );
            if !["rs", "tsx"].contains(&ext) {
                continue;
            }
            let middle = core.current().text().line_to_char(lines / 2);
            count(&mut core, "forward-char", middle);
            for (kind, inserted) in [("character", " "), ("comment-delimiter", "/*")] {
                measure(
                    &format!("syntax-edit-frame/{ext}/{lines}-lines-{kind}-undo-pair"),
                    || {
                        core.handle(
                            Instant::now(),
                            Msg::Insert {
                                text: inserted.into(),
                                replace: None,
                            },
                        );
                        black_box(core.frame());
                        core.handle(Instant::now(), Msg::Key(undo));
                        black_box(core.frame());
                    },
                    output,
                );
                assert!(core.text() == text, "text restored");
                assert_eq!(core.point(), middle);
                assert!(colored(&core.frame()));
            }
            measure(
                &format!("syntax-display/{ext}/{lines}-lines-stationary"),
                || {
                    black_box(core.frame());
                },
                output,
            );
            measure(
                &format!("syntax-display/{ext}/{lines}-lines-scroll-pair"),
                || {
                    command(&mut core, "scroll-up-command");
                    black_box(core.frame());
                    command(&mut core, "scroll-down-command");
                    black_box(core.frame());
                },
                output,
            );
            assert!(core.text() == text, "text restored");
        }
    }
    let text = format!("const data = \"{}\";\n", "x".repeat(100_000));
    let mut core = open("/long.js", &text);
    assert!(colored(&core.frame()));
    measure(
        "syntax-display/js/100kb-single-line",
        || {
            black_box(core.frame());
        },
        output,
    );
    let frame = core.frame();
    let shown: String = frame.windows[0].display.lines[0]
        .spans
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(shown.starts_with("const data = "));
    assert!(shown.len() <= 121);
    let text = format!("const DATA: &str = \"{}\";\n", "x".repeat(1_000_000));
    let mut core = open("/long.rs", &text);
    command(&mut core, "move-end-of-line");
    let frame = core.frame();
    let shown: String = (frame.windows[0].display.lines[0].spans.iter())
        .map(|s| s.text.as_str())
        .collect();
    assert!(
        shown.ends_with("xx\";") && colored(&frame),
        "the line's end, highlighted"
    );
    measure(
        "syntax-display/rs/1mb-single-line-at-end",
        || {
            black_box(core.frame());
        },
        output,
    );
}

fn settle(core: &mut Core, matches: usize) {
    let start = Instant::now();
    let expected = format!("1/{matches} ");
    loop {
        // Advance repoll timers without imposing their 16 ms UI cadence on matcher CPU work.
        core.handle(Instant::now() + Duration::from_secs(1), Msg::Tick);
        let frame = core.frame();
        if frame
            .minibuffer
            .as_ref()
            .is_some_and(|m| m.count == expected)
            && core.next_deadline().is_none()
        {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "completion did not settle to {expected}"
        );
        std::thread::yield_now();
    }
}

fn finder(output: &mut String) {
    for total in [10_000, 100_000] {
        let mut core = Core::default();
        let project = Project::parse("name = 'bench'\nwatch = false\n[primary]\npath = '/bench/a'\n[[roots]]\npath = '/bench/b'\n").unwrap();
        core.handle(
            Instant::now(),
            Msg::ProjectsLoaded {
                dir: None,
                projects: vec![Ok(Loaded {
                    project,
                    file: None,
                })],
            },
        );
        let project = core.project_id("bench").unwrap();
        core.new_workspace("bench", project);
        let effects = call(&mut core, "project-find-file", Args::default());
        let (project, walk) = effects
            .into_iter()
            .find_map(|e| match e {
                Effect::File(FileOp::Index { project, walk, .. }) => Some((project, walk)),
                _ => None,
            })
            .expect("finder requests index");
        let files: Vec<PathBuf> = (0..total / 2)
            .map(|i| format!("src/item{i:06}.rs").into())
            .collect();
        core.handle(
            Instant::now(),
            Msg::Indexed {
                project,
                walk,
                roots: vec![
                    ("/bench/a".into(), Ok(files.clone())),
                    ("/bench/b".into(), Ok(files)),
                ],
                directories: Vec::new(),
            },
        );
        settle(&mut core, total);
        command(&mut core, "keyboard-quit");
        measure(
            &format!("finder/{total}-paths-open-close"),
            || {
                command(&mut core, "project-find-file");
                settle(&mut core, total);
                command(&mut core, "keyboard-quit");
            },
            output,
        );
        command(&mut core, "project-find-file");
        settle(&mut core, total);
        let mut previous = 0;
        measure(
            &format!("finder/{total}-paths-narrow-broaden-pair"),
            || {
                for (query, expected) in [("item000001", 2), ("item", total)] {
                    core.handle(
                        Instant::now(),
                        Msg::Insert {
                            text: query.into(),
                            replace: Some(0..previous),
                        },
                    );
                    previous = query.len();
                    settle(&mut core, expected);
                }
            },
            output,
        );
        assert_eq!(
            core.frame().minibuffer.unwrap().count,
            format!("1/{total} ")
        );
    }
}

fn reverts(output: &mut String) {
    for ext in ["txt", "rs"] {
        let old = source("rs", 100_000);
        let path = format!("/revert.{ext}");
        for (kind, every) in [
            ("one-line", 100_000),
            ("scattered", 100),
            ("replace-1000-line-block", 1),
        ] {
            let new: String = old
                .lines()
                .enumerate()
                .map(|(i, l)| {
                    if (every == 1 && (49_500..50_500).contains(&i))
                        || (every != 1 && i % every == 0)
                    {
                        format!("{l} // changed {i}\n")
                    } else {
                        format!("{l}\n")
                    }
                })
                .collect();
            let forward = buffer::diff(&old, &new);
            let new_len = new.chars().count();
            let first_line = new.lines().next().unwrap().to_string() + "\n";
            measure(
                &format!("revert-diff/{ext}/100k-lines-{kind}"),
                || {
                    black_box(buffer::diff(black_box(&old), black_box(&new)));
                },
                output,
            );
            prepared(
                &format!("revert-apply-frame/{ext}/100k-lines-{kind}"),
                || {
                    let mut core = open(&path, &old);
                    let effects = core.handle(
                        Instant::now(),
                        Msg::Checked(vec![(path.clone().into(), Some(1))]),
                    );
                    let (buffer, tick) = effects
                        .into_iter()
                        .find_map(|e| match e {
                            Effect::File(FileOp::Revert { buffer, tick, .. }) => {
                                Some((buffer, tick))
                            }
                            _ => None,
                        })
                        .expect("external change requests revert");
                    let result = Ok(buffer::Revert {
                        edits: forward.clone(),
                        crlf: false,
                        bom: false,
                        hash: 1,
                    });
                    (
                        core,
                        Some(Msg::Reverted {
                            buffer,
                            tick,
                            result,
                        }),
                    )
                },
                |(core, msg)| {
                    core.handle(Instant::now(), msg.take().unwrap());
                    black_box(core.frame());
                    assert_eq!(core.current().len_chars(), new_len);
                    assert_eq!(core.current().text().line(0).to_string(), first_line);
                },
                output,
            );
            let mut buffers = Buffers::default();
            let id = buffers.create_for_file(Path::new(&path), &old);
            buffers.get_mut(id).unwrap().replace(&forward).unwrap();
            assert!(
                *buffers.get(id).unwrap().text() == new,
                "revert matches disk text"
            );
        }
    }
}

fn terminals(output: &mut String) {
    for (kind, line) in [
        ("plain", "alpha été 日本 output\r\n"),
        (
            "colored",
            "\x1b[31malpha \x1b[32mété \x1b[1;34m日本 \x1b[0moutput\r\n",
        ),
    ] {
        let bytes = line.repeat(10_100).into_bytes();
        let mut core = open("/terminal-host.txt", "");
        let (id, _) = core.create_terminal("bench-term", 50, 120);
        core.handle(Instant::now(), Msg::TerminalOutput { id, bytes });
        black_box(call(
            &mut core,
            "switch-to-buffer",
            Args {
                strings: vec!["bench-term".into()],
                ..Args::default()
            },
        ));
        assert_eq!(core.current().terminal().unwrap().history(), 10_000);
        measure(
            &format!("terminal-copy/{kind}/10k-lines-enter-exit-pair"),
            || {
                command(&mut core, "vterm-copy-mode");
                command(&mut core, "vterm-copy-mode");
            },
            output,
        );
        command(&mut core, "vterm-copy-mode");
        assert!(core.text().contains("alpha été 日本 output"));
        assert!(core.current().len_lines() >= 10_000);
        command(&mut core, "beginning-of-buffer");
        measure(
            &format!("terminal-copy/{kind}/10k-lines-display"),
            || {
                black_box(core.frame());
            },
            output,
        );
        measure(
            &format!("terminal-copy/{kind}/10k-lines-scroll-pair"),
            || {
                command(&mut core, "scroll-up-command");
                black_box(core.frame());
                command(&mut core, "scroll-down-command");
                black_box(core.frame());
            },
            output,
        );
        if kind == "colored" {
            assert!(colored(&core.frame()));
        }
    }
    let theme = Settings::default().theme;
    let mut terminal = Terminal::new(50, 120);
    let trace = String::from("\x1b[3J\x1b[2J\x1b[H")
        + &concat!(
            "\x1b[?1049h\x1b[2J\x1b[H",
            "\x1b[38;2;100;150;200mété 日本 🙂\x1b[0m",
            "\x1b[2;5H\x1b[1mstatus\x1b[0m\x1b[K",
            "\x1b[?1049l",
            "\x1b]7;file://localhost/tmp\x07\x1b]133;A\x07$ ",
            "\x1b]133;B\x07echo rich\r\n\x1b]133;C\x07",
            "\x1b[32mrich output\x1b[0m\r\n\x1b]133;D;0\x07"
        )
        .repeat(256);
    measure(
        "terminal-output/ansi-tui-osc-trace",
        || {
            black_box(terminal.feed(trace.as_bytes()));
        },
        output,
    );
    assert!(terminal.screen().contains("rich output"));
    assert_eq!(terminal.cwd(), Some(Path::new("/tmp")));
    assert!(terminal.marks().len() >= 4);
    terminal.feed(b"\x1b[?1049h\x1b[2J");
    for row in 1..=50 {
        terminal.feed(
            format!(
                "\x1b[{row};1H{}\x1b[0m",
                "\x1b[31mr\x1b[32mg\x1b[34mb \x1b[0m日本🙂 ".repeat(8)
            )
            .as_bytes(),
        );
    }
    assert!(
        terminal
            .display(&theme)
            .rows
            .iter()
            .map(Vec::len)
            .sum::<usize>()
            > 100
    );
    measure(
        "terminal-display/ansi-unicode-50x120",
        || {
            black_box(terminal.display(&theme));
        },
        output,
    );
    terminal.feed(b"\x1b[?1049l");
    prepared(
        "terminal-resize/10k-lines-120-to-80-columns",
        || {
            let mut t = Terminal::new(50, 120);
            t.feed("long wrapped output ".repeat(100_000).as_bytes());
            assert_eq!(t.history(), 10_000);
            t
        },
        |t| {
            t.resize(50, 80);
            black_box(t.display(&theme));
            assert_eq!(t.size(), (50, 80));
        },
        output,
    );
}

fn splits(output: &mut String) {
    let text = source("rs", 10_000);
    for shared in [true, false] {
        for panes in [1, 2, 4] {
            let mut core = open("/pane0.rs", &text);
            for i in 1..panes {
                command(&mut core, "split-window-right");
                command(&mut core, "other-window");
                if !shared {
                    core.open(
                        Path::new(&format!("/pane{i}.rs")),
                        Some((text.as_bytes().to_vec(), 0)),
                    );
                }
            }
            core.handle(
                Instant::now(),
                Msg::Resize {
                    width: 960. * panes as f32,
                    height: 1020.,
                    cell_width: 8.,
                },
            );
            let frame = core.frame();
            assert_eq!(frame.windows.len(), panes);
            assert!(colored(&frame));
            let kind = if shared { "shared" } else { "distinct" };
            measure(
                &format!("frame/{panes}-panes-{kind}-rust"),
                || {
                    black_box(core.frame());
                },
                output,
            );
        }
    }
}

pub fn run(output: &mut String) {
    syntax(output);
    finder(output);
    reverts(output);
    terminals(output);
    splits(output);
}
