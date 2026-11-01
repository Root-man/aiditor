use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use agent_client_protocol_schema::v1::{
    ContentChunk, NewSessionResponse, SessionNotification, SessionUpdate,
};
use aiditor_core::command::{Args, Prefix};
use aiditor_core::project::{Loaded, Project};
use aiditor_core::{AgentId, AgentOp, Core, Ctx, Effect, Msg};

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
    let group = "# Heading été\n\nSome **bold** and *italic* text with `code` and a [link](https://x.dev).\n\n```rust\nfn item() -> usize { 1 }\n```\n- a *list* item with `x`\n\nA closing **line**.\n";
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

fn main() {
    let mut output = String::new();
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
