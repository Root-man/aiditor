use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use agent_client_protocol_schema::v1::{
    ContentChunk, NewSessionResponse, SessionNotification, SessionUpdate,
};
use aiditor_core::buffer::Buffers;
use aiditor_core::command::{Args, Prefix};
use aiditor_core::project::{Loaded, Project};
use aiditor_core::{AgentId, AgentOp, Core, Ctx, Effect, FileOp, Msg};

/// The markers `session()`'s transcript holds once a char is typed in its prompt.
const MARKERS: usize = 21_010;

#[path = "../../../benches/support.rs"]
mod support;
use support::{measure, prepared};

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

fn open(path: &str, text: &str) -> Core {
    let mut core = Core::default();
    core.open(Path::new(path), Some((text.as_bytes().to_vec(), 0)));
    let effects = core.handle(
        Instant::now(),
        Msg::Resize {
            width: 960.,
            height: core.settings().line_height() * 51.,
            cell_width: 8.,
        },
    );
    // A large file's first parse arrives before the cases act, as it would in the app.
    for effect in effects {
        if let Effect::Parse { buffer, parse } = effect {
            core.handle(Instant::now(), parse.run(buffer));
        }
    }
    core
}

fn chunk(core: &mut Core, id: AgentId, text: &str) {
    core.handle(
        Instant::now(),
        Msg::AgentUpdate {
            id,
            update: Box::new(SessionNotification::new(
                "s",
                SessionUpdate::AgentMessageChunk(ContentChunk::new(text.to_string().into())),
            )),
        },
    );
}

fn tool_output(core: &mut Core, id: AgentId, text: &str) {
    let update = serde_json::from_value(serde_json::json!({
        "sessionUpdate": "tool_call_update", "toolCallId": "tool",
        "_meta": {"terminal_output_delta": {"terminal_id": "t", "data": text}}
    }))
    .unwrap();
    core.handle(
        Instant::now(),
        Msg::AgentUpdate {
            id,
            update: Box::new(SessionNotification::new("s", update)),
        },
    );
}

fn update(core: &mut Core, id: AgentId, update: serde_json::Value) {
    core.handle(
        Instant::now(),
        Msg::AgentUpdate {
            id,
            update: Box::new(SessionNotification::new(
                "s",
                serde_json::from_value(update).unwrap(),
            )),
        },
    );
}

fn read(core: &mut Core, id: AgentId, call: &str) {
    update(
        core,
        id,
        serde_json::json!({
            "sessionUpdate": "tool_call", "toolCallId": call, "title": format!("Read {call}.rs"),
            "kind": "read", "status": "completed", "rawOutput": "text",
            "locations": [{"path": format!("{call}.rs")}]
        }),
    );
}

fn session() -> (Core, AgentId) {
    let mut core = Core::default();
    let project =
        Project::parse("name = 'bench'\nwatch = false\n[primary]\npath = '/bench/a'\n").unwrap();
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
    let effects = call(
        &mut core,
        "agent-start",
        Args {
            strings: vec!["claude".into()],
            ..Args::default()
        },
    );
    let id = effects
        .into_iter()
        .find_map(|e| match e {
            Effect::Agent(AgentOp::Spawn { id, .. }) => Some(id),
            _ => None,
        })
        .unwrap();
    core.handle(
        Instant::now(),
        Msg::AgentStarted {
            id,
            result: Ok(NewSessionResponse::new("s")),
        },
    );
    let group = "# Heading été\n\nSome **bold** and *italic* text with `code` and a [link](https://x.dev).\n\n```rust\nfn item() -> usize { 1 }\n```\n- a *list* item with `x`\n\nA closing **line**.\n| key | **value** |\n|---|--:|\n| a | 1 |\n| b | 22 |\n| `c` | 333 |\n| d | 4 |\n| e | 55 |\n| f | 666 |\n| [g](https://x.dev) | 7 |\n\n";
    let text = group.repeat(5_000 / group.matches('\n').count());
    assert_eq!(text.matches('\n').count(), 5_000);
    chunk(&mut core, id, &text);
    core.handle(
        Instant::now(),
        Msg::Resize {
            width: 960.,
            height: core.settings().line_height() * 51.,
            cell_width: 8.,
        },
    );
    black_box(core.frame());
    (core, id)
}

/// Thought rows inside a tool group add their properties out of order.
fn unsorted(output: &mut String) {
    let (mut core, id) = session();
    for call in ["a", "b", "c"] {
        if call != "a" {
            update(
                &mut core,
                id,
                serde_json::json!({
                    "sessionUpdate": "agent_thought_chunk", "content": {"type": "text", "text": "hmm"}
                }),
            );
        }
        read(&mut core, id, call);
    }
    let shows_thought = |core: &mut Core| {
        (core.frame().windows[0].display.lines.iter()).any(|line| {
            line.spans
                .iter()
                .any(|span| span.text.contains("Thought for"))
        })
    };
    // Opens the group from its line, two above the end, then shows its rows.
    command(&mut core, "end-of-buffer");
    command(&mut core, "previous-line");
    command(&mut core, "previous-line");
    command(&mut core, "agent-toggle-tool-call");
    command(&mut core, "end-of-buffer");
    assert!(shows_thought(&mut core));
    measure(
        "hot-stream/unsorted-props-clean-frame",
        || {
            black_box(core.frame());
        },
        output,
    );
    assert!(shows_thought(&mut core));
}

/// One byte of terminal output in a core visiting 200 files of two watched projects that index
/// 1,000 directories each, which is nearly all `handle`'s tail.
fn tail(output: &mut String) {
    let mut core = Core::default();
    let projects = ["a", "b"].map(|name| {
        let project = Project::parse(&format!(
            "name = '{name}'\n[primary]\npath = '/tail/{name}'\n"
        ));
        Ok(Loaded {
            project: project.unwrap(),
            file: None,
        })
    });
    core.handle(
        Instant::now(),
        Msg::ProjectsLoaded {
            dir: None,
            projects: projects.into(),
        },
    );
    for name in ["a", "b"] {
        let project = core.project_id(name).unwrap();
        core.new_workspace(name, project);
    }
    let walks: Vec<_> = (core.handle(Instant::now(), Msg::Tick).into_iter())
        .filter_map(|e| match e {
            Effect::File(FileOp::Index { project, walk, .. }) => Some((project, walk)),
            _ => None,
        })
        .collect();
    assert_eq!(walks.len(), 2, "both watched projects ask for a walk");
    for (project, walk) in walks {
        let root = PathBuf::from(format!("/tail/{}", core.project_name(project)));
        let files = (0..100).map(|i| format!("d{i}/f.rs").into()).collect();
        let directories = (0..1_000).map(|i| root.join(format!("d{i}"))).collect();
        core.handle(
            Instant::now(),
            Msg::Indexed {
                project,
                walk,
                roots: vec![(root, Ok(files))],
                directories,
            },
        );
    }
    for name in ["a", "b"] {
        for i in 0..100 {
            let path = format!("/tail/{name}/d{i}/f.rs");
            core.open(
                Path::new(&path),
                Some((format!("fn f{i}() {{}}\n").into(), 0)),
            );
        }
    }
    let (id, _) = core.create_terminal("*terminal*", 24, 80);
    core.handle(Instant::now(), Msg::Tick);
    let set = core.watch_set();
    assert_eq!(set.roots.len(), 2);
    assert!(set.files.len() >= 200, "{} watched files", set.files.len());
    assert_eq!(
        set.directories.iter().map(|d| d.len()).sum::<usize>(),
        2_000
    );
    let output_byte = Msg::TerminalOutput {
        id,
        bytes: b"x".to_vec(),
    };
    let effects = core.handle(Instant::now(), output_byte);
    assert!(effects.is_empty(), "{} effects", effects.len());
    measure(
        "handle-tail/terminal-output-200-file-buffers",
        || {
            black_box(core.handle(
                Instant::now(),
                Msg::TerminalOutput {
                    id,
                    bytes: b"x".to_vec(),
                },
            ));
        },
        output,
    );
}

fn main() {
    let mut output = String::new();
    tail(&mut output);
    // A self-insert in the prompt walks every marker the transcript holds.
    prepared(
        "hot-stream/prompt-typing-5000-line-transcript",
        || {
            let (mut core, _) = session();
            command(&mut core, "end-of-buffer");
            core
        },
        |core| {
            black_box(core.handle(
                Instant::now(),
                Msg::Insert {
                    text: "x".into(),
                    replace: None,
                },
            ));
        },
        &mut output,
    );
    let (mut core, _) = session();
    command(&mut core, "end-of-buffer");
    core.handle(
        Instant::now(),
        Msg::Insert {
            text: "x".into(),
            replace: None,
        },
    );
    assert!(core.text().ends_with("> x"));
    let markers = core.current().marker_count();
    println!("hot-stream/prompt-typing-5000-line-transcript: {markers} markers");
    assert_eq!(markers, MARKERS);
    for lines in [10_000, 100_000] {
        let source: String = (0..lines)
            .map(|i| {
                let mut line = format!("fn item_{i}() -> usize {{ let café = {i}; café + 1 }}\n");
                if i % 100 == 0 {
                    line.insert_str(0, &format!("mod group_{} {{ ", i / 100));
                }
                if i % 100 == 99 {
                    line.insert(line.len() - 1, '}');
                }
                line
            })
            .collect();
        let mut core = open("/scopes.rs", &source);
        let middle = core.current().text().line_to_char(lines / 2);
        if lines == 10_000 {
            prepared(
                "syntax-parse/rs/10000-lines-after-edit",
                || {
                    let mut buffers = Buffers::default();
                    let id = buffers.create_for_file(Path::new("/scopes.rs"), &source);
                    let buffer = buffers.get_mut(id).unwrap();
                    let Msg::Parsed { parse, .. } = buffer.ask_parse().unwrap().run(id) else {
                        unreachable!()
                    };
                    buffer.parsed(parse);
                    buffer.insert(middle, " ").unwrap();
                    // The buffers and the result live on, so no tree is freed while timed.
                    (buffer.ask_parse(), None, buffers, id)
                },
                |(parse, parsed, _, id)| {
                    let parse = parse.take().expect("the edit asks for a parse");
                    *parsed = Some(parse.run(*id));
                    assert!(matches!(parsed, Some(Msg::Parsed { .. })));
                },
                &mut output,
            );
        }
        call(
            &mut core,
            "forward-char",
            Args {
                prefix: Some(Prefix::Number(middle as i64)),
                ..Args::default()
            },
        );
        black_box(core.frame());
        prepared(
            &format!("hot-scopes/{lines}-lines-first-highlight-after-edit"),
            || {
                let mut core = open("/scopes.rs", &source);
                call(
                    &mut core,
                    "forward-char",
                    Args {
                        prefix: Some(Prefix::Number(middle as i64)),
                        ..Args::default()
                    },
                );
                black_box(core.frame());
                core.handle(
                    Instant::now(),
                    Msg::Insert {
                        text: " ".into(),
                        replace: None,
                    },
                );
                core
            },
            |core| {
                black_box(core.frame());
            },
            &mut output,
        );
        measure(
            &format!("hot-scopes/{lines}-lines-edit-highlight-undo-pair"),
            || {
                core.handle(
                    Instant::now(),
                    Msg::Insert {
                        text: " ".into(),
                        replace: None,
                    },
                );
                black_box(core.frame());
                core.handle(
                    Instant::now(),
                    Msg::Key(aiditor_core::key::parse_keys("C-/").unwrap()[0]),
                );
                black_box(core.frame());
            },
            &mut output,
        );
        assert_eq!(core.current().text().to_string(), source);
        measure(
            &format!("hot-scopes/{lines}-lines-cached-frame"),
            || {
                black_box(core.frame());
            },
            &mut output,
        );
    }
    for size in [100_000, 1_000_000] {
        let mut core = open("/dense.txt", &format!("{}\n", "x".repeat(size)));
        command(&mut core, "swiper");
        core.handle(
            Instant::now(),
            Msg::Insert {
                text: "x".into(),
                replace: None,
            },
        );
        let frame = core.frame();
        assert!(frame.minibuffer.as_ref().unwrap().count.contains("1/1"));
        assert!(
            frame.windows[0].display.lines[0]
                .spans
                .iter()
                .any(|s| s.text.contains('x'))
        );
        measure(
            &format!("hot-preview/{size}-matches-stationary"),
            || {
                black_box(core.frame());
            },
            &mut output,
        );
    }
    for size in [500_000, 5_000_000] {
        for state in ["shown-folded", "shown-open", "hidden"] {
            prepared(
                &format!("hot-stream/tool-output-{state}-{size}"),
                || {
                    let (mut core, id) = session();
                    let call = serde_json::from_value(serde_json::json!({
                        "sessionUpdate": "tool_call", "toolCallId": "tool", "title": "command",
                        "_meta": {"terminal_info": {"terminal_id": "t"}}
                    }))
                    .unwrap();
                    core.handle(
                        Instant::now(),
                        Msg::AgentUpdate {
                            id,
                            update: Box::new(SessionNotification::new("s", call)),
                        },
                    );
                    let line = format!("{}\n", "output line ".repeat(80));
                    tool_output(&mut core, id, &line.repeat(size / line.len()));
                    black_box(core.frame());
                    if state == "shown-open" {
                        command(&mut core, "end-of-buffer");
                        command(&mut core, "previous-line");
                        command(&mut core, "previous-line");
                        command(&mut core, "agent-toggle-tool-call");
                        assert!(
                            core.frame().windows[0]
                                .display
                                .lines
                                .iter()
                                .flat_map(|line| &line.spans)
                                .any(|span| span.text == "Output:")
                        );
                    } else if state == "hidden" {
                        core.open(Path::new("/other.txt"), Some((b"other".to_vec(), 0)));
                    }
                    tool_output(&mut core, id, "next line\n");
                    core
                },
                |core| {
                    black_box(core.frame());
                },
                &mut output,
            );
        }
    }
    for shown in [true, false] {
        let (mut core, _) = session();
        if !shown {
            core.open(Path::new("/other.txt"), Some((b"other".to_vec(), 0)));
        }
        let name = if shown { "shown" } else { "hidden" };
        measure(
            &format!("hot-stream/{name}-clean-frame"),
            || {
                black_box(core.frame());
            },
            &mut output,
        );
        prepared(
            &format!("hot-stream/{name}-dirty-frame"),
            || {
                let (mut core, id) = session();
                if !shown {
                    core.open(Path::new("/other.txt"), Some((b"other".to_vec(), 0)));
                }
                chunk(&mut core, id, " next");
                core
            },
            |core| {
                black_box(core.frame());
            },
            &mut output,
        );
    }
    unsorted(&mut output);
    prepared(
        "hot-stream/hidden-first-show-frame",
        || {
            let (mut core, id) = session();
            let name = core.current().name.clone();
            core.open(Path::new("/other.txt"), Some((b"other".to_vec(), 0)));
            chunk(&mut core, id, " next");
            black_box(core.frame());
            call(
                &mut core,
                "switch-to-buffer",
                Args {
                    strings: vec![name],
                    ..Args::default()
                },
            );
            core
        },
        |core| {
            black_box(core.frame());
        },
        &mut output,
    );

    let text = format!("//{}\n", "x".repeat(1_000_000)).repeat(10);
    prepared(
        "hot-swiper/ten-1mb-rows-first-frame",
        || {
            let mut core = open("/rows.rs", &text);
            command(&mut core, "swiper");
            core
        },
        |core| {
            let frame = core.frame();
            let mb = frame.minibuffer.unwrap();
            assert_eq!(mb.candidates.len(), 10);
            assert!(mb.candidates.iter().all(|s| s.contains("//")));
            assert!(mb.syntax.iter().take(10).all(|runs| !runs.is_empty()));
            black_box(mb);
        },
        &mut output,
    );
    if let Some(path) = std::env::var_os("BENCH_OUTPUT") {
        std::fs::write(path, output).expect("write benchmark results");
    }
}
