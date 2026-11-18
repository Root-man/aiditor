use agent_client_protocol_schema::v1::{
    AvailableCommandsUpdate, ConfigOptionUpdate, ContentChunk, ImageContent, PlanEntryPriority,
    PlanEntryStatus, SessionConfigSelectGroup, SessionConfigSelectOption, SessionUpdate,
    ToolCallLocation, ToolCallUpdateFields, ToolKind, UnstructuredCommandInput,
};

use super::*;
use crate::buffer::{Face, Icon, Prop};
use crate::harness::{Harness, listed, name};
use crate::project::Root;
use crate::settings::Agent;

/// A harness in workspace `work` on a project at `/aiditor-test/app`, with agents `claude`
/// (`env` `A=2`) and `codex`.
fn work() -> Harness {
    let mut h = Harness::new(&[("app/src/main.rs", "fn main() {}\n")]);
    h.core.rename_workspace("work").unwrap();
    h.core.projects[0].project.primary.path = h.path("app");
    let agent = |command: &str, env: &[(&str, &str)]| Agent {
        command: command.into(),
        args: vec!["--acp".into()],
        env: (env.iter()).map(|&(k, v)| (k.into(), v.into())).collect(),
        default_mode: None,
    };
    let agents = &mut h.core.settings.agents;
    agents.insert("claude".into(), agent("claude-agent-acp", &[("A", "2")]));
    agents.insert("codex".into(), agent("codex-acp", &[]));
    h
}

/// The agent ops since the last call, as text.
fn ops(h: &mut Harness) -> Vec<String> {
    (h.agent_ops.drain(..))
        .map(|op| match op {
            AgentOp::Spawn {
                id,
                command,
                cwd,
                env,
                ..
            } => {
                let env: Vec<_> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
                format!(
                    "spawn {} {command} {} {}",
                    id.0,
                    cwd.display(),
                    env.join(",")
                )
            }
            AgentOp::Reply {
                id,
                request,
                result,
            } => {
                let answer = match result {
                    Ok(ClientResponse::RequestPermissionResponse(r)) => match r.outcome {
                        RequestPermissionOutcome::Selected(s) => s.option_id.to_string(),
                        _ => "cancelled".into(),
                    },
                    Ok(ClientResponse::CreateElicitationResponse(r)) => {
                        serde_json::to_string(&r).unwrap()
                    }
                    Ok(other) => format!("{other:?}"),
                    Err(err) => err.message,
                };
                format!("reply {} {request} {answer}", id.0)
            }
            AgentOp::Prompt { id, request } => {
                let texts: Vec<_> = (request.prompt.iter())
                    .map(|block| match block {
                        ContentBlock::Text(text) => format!("{:?}", text.text),
                        ContentBlock::ResourceLink(l) => format!("link {} {}", l.name, l.uri),
                        ContentBlock::Image(i) => {
                            format!("image {} {}", i.mime_type, i.uri.as_deref().unwrap())
                        }
                        _ => panic!("text, links and images only: {request:?}"),
                    })
                    .collect();
                format!("prompt {} {} {}", id.0, request.session_id, texts.join(" "))
            }
            AgentOp::SetConfigOption { id, request } => {
                let value = request.value.as_value_id().unwrap();
                let (session, config) = (request.session_id, request.config_id);
                format!("set {} {session} {config}={value}", id.0)
            }
            AgentOp::Cancel { id, session } => format!("cancel {} {session}", id.0),
            AgentOp::Kill { id } => format!("kill {}", id.0),
            AgentOp::Delete {
                id,
                source,
                session,
            } => format!("delete {} {} {session}", id.0, source.0),
            AgentOp::List {
                request, delete, ..
            } => match delete {
                Some((session, stop)) => {
                    format!(
                        "list {request} delete {session} stop {:?}",
                        stop.map(|s| s.0)
                    )
                }
                None => format!("list {request}"),
            },
        })
        .collect()
}

/// The request number of the last capture started.
fn capture(h: &mut Harness) -> u64 {
    (h.effects.iter().rev())
        .find_map(|effect| match effect {
            Effect::CaptureEnvironment { request, .. } => Some(*request),
            _ => None,
        })
        .expect("a capture started")
}

fn messages(h: &Harness) -> String {
    let id = h.core.buffers.find("*Messages*").unwrap();
    h.core.buffers.get(id).unwrap().text().to_string()
}

fn log_text(h: &Harness, name: &str) -> String {
    let id = h.core.buffers.find(name).unwrap();
    h.core.buffers.get(id).unwrap().text().to_string()
}

/// Config options whose `mode` select is `current`, with values `auto` and `plan`.
fn options(current: &str) -> Vec<SessionConfigOption> {
    let values = ["auto", "plan"].map(|v| SessionConfigSelectOption::new(v, v));
    let mode = SessionConfigOption::select("mode", "Mode", current.to_owned(), values.to_vec());
    let model = SessionConfigOption::select("model", "Model", "m", values[..1].to_vec());
    vec![model, mode.category(SessionConfigOptionCategory::Mode)]
}

fn ready(h: &mut Harness, id: u64) {
    let response = NewSessionResponse::new("s").config_options(options("auto"));
    h.send(Msg::AgentStarted {
        id: AgentId(id),
        result: Ok(response),
    });
}

fn exited(h: &mut Harness, id: u64, code: u32) {
    h.send(Msg::AgentExited {
        id: AgentId(id),
        code,
        signal: None,
    });
}

#[test]
fn the_default_agent_is_preselected_and_spawned_in_the_primary_repo() {
    let mut h = work();
    h.core.projects[0].project.default_agent = Some("codex".into());
    h.keys("M-x agent-start RET");
    assert_eq!(h.core.minibuffer().unwrap().prompt, "Agent: ");
    assert_eq!(
        listed(&mut h),
        (vec!["claude".into(), "codex".into()], Some(1))
    );
    h.keys("RET");
    let app = h.path("app");
    assert_eq!(
        ops(&mut h),
        [format!("spawn 0 codex-acp {} ", app.display())]
    );
    assert_eq!(h.core.message(), Some("Starting codex..."));
    // The session buffer is shown, the log isn't.
    assert_eq!(name(&h), "*codex*<work>");
    assert_eq!((h.core.text(), h.core.point()), ("> ".into(), 2));
    let session = h.core.current();
    assert!(!session.read_only && session.workspace == h.core.workspace());
    let log = h.core.buffers.find("*codex stderr*<work>").unwrap();
    let log = h.core.buffers.get(log).unwrap();
    assert!(log.read_only && log.workspace == h.core.workspace());
}

#[test]
fn enabled_agents_take_the_project_list_over_the_settings_list() {
    let mut h = work();
    let mut p = h.core.projects[0].project.clone();
    assert_eq!(h.core.enabled_agents(&p), ["claude", "codex"]);
    h.core.settings.enabled_agents = Some(vec!["codex".into()]);
    assert_eq!(h.core.enabled_agents(&p), ["codex"]);
    p.enabled_agents = Some(vec!["codex".into(), "claude".into(), "gemini".into()]);
    assert_eq!(
        h.core.enabled_agents(&p),
        ["claude", "codex"],
        "registry order"
    );
    p.enabled_agents = Some(Vec::new());
    assert!(h.core.enabled_agents(&p).is_empty());
}

#[test]
fn agent_start_offers_only_enabled_agents_and_takes_a_single_one_without_asking() {
    let mut h = work();
    let gemini = h.core.settings.agents["codex"].clone();
    h.core.settings.agents.insert("gemini".into(), gemini);
    h.core.projects[0].project.enabled_agents = Some(vec!["claude".into(), "codex".into()]);
    h.core.projects[0].project.default_agent = Some("gemini".into());
    h.keys("M-x agent-start RET");
    let preselected_first = (vec!["claude".into(), "codex".into()], Some(0));
    assert_eq!(
        listed(&mut h),
        preselected_first,
        "a disabled default isn't preselected"
    );
    h.keys("C-g");
    h.core.projects[0].project.enabled_agents = Some(vec!["codex".into()]);
    h.keys("M-x agent-start RET");
    assert!(h.core.minibuffer().is_none());
    let app = h.path("app");
    assert_eq!(
        ops(&mut h),
        [format!("spawn 0 codex-acp {} ", app.display())]
    );
    assert_eq!(h.core.message(), Some("Starting codex..."));
    let args = crate::command::Args {
        strings: vec!["claude".into()],
        ..Default::default()
    };
    h.call("agent-start", args);
    let project = h.core.projects[0].project.name.clone();
    let refused = format!("\"claude\" is not enabled in {project}");
    assert_eq!(h.core.message(), Some(refused.as_str()));
    assert!(ops(&mut h).is_empty());
}

#[test]
fn the_environment_layers_in_order() {
    let mut h = work();
    h.core.capture_environments();
    h.send(Msg::Tick);
    let request = capture(&mut h);
    h.send(Msg::EnvironmentCaptured {
        project: ProjectId(0),
        request,
        result: Ok(vec![("PATH".into(), "/a".into()), ("A".into(), "1".into())]),
        direnv: None,
    });
    h.core.projects[0]
        .project
        .env
        .insert("B".into(), "3".into());
    h.keys("M-x agent-start RET claude RET");
    let app = h.path("app");
    assert_eq!(
        ops(&mut h),
        [format!(
            "spawn 0 claude-agent-acp {} PATH=/a,A=1,A=2,B=3",
            app.display()
        )]
    );
}

#[test]
fn an_unknown_agent_starts_nothing() {
    let mut h = work();
    h.call(
        "agent-start",
        Args {
            strings: vec!["gemini".into()],
            ..Args::default()
        },
    );
    assert_eq!(
        h.core.message(),
        Some("\"gemini\" is not a registered agent")
    );
    assert!(ops(&mut h).is_empty());
    assert!(h.core.agents.is_empty());
}

#[test]
fn the_spawn_waits_for_the_capture() {
    let mut h = work();
    h.core.capture_environments();
    h.keys("M-x agent-start RET claude RET");
    assert!(ops(&mut h).is_empty());
    let request = capture(&mut h);
    h.send(Msg::EnvironmentCaptured {
        project: ProjectId(0),
        request,
        result: Ok(vec![("PATH".into(), "/opt/bin".into())]),
        direnv: None,
    });
    let app = h.path("app");
    assert_eq!(
        ops(&mut h),
        [format!(
            "spawn 0 claude-agent-acp {} PATH=/opt/bin,A=2",
            app.display()
        )]
    );
}

#[test]
fn a_failed_capture_spawns_without_its_variables() {
    let mut h = work();
    h.core.capture_environments();
    h.keys("M-x agent-start RET claude RET");
    let request = capture(&mut h);
    h.send(Msg::EnvironmentCaptured {
        project: ProjectId(0),
        request,
        result: Err("timed out".into()),
        direnv: None,
    });
    let app = h.path("app");
    assert_eq!(
        ops(&mut h),
        [format!("spawn 0 claude-agent-acp {} A=2", app.display())]
    );
}

#[test]
fn a_session_killed_while_waiting_never_spawns() {
    let mut h = work();
    h.core.capture_environments();
    h.keys("M-x agent-start RET claude RET");
    let session = h.core.buffers.find("*claude*<work>").unwrap();
    h.core.kill_buffer(session);
    h.send(Msg::Tick);
    let request = capture(&mut h);
    h.send(Msg::EnvironmentCaptured {
        project: ProjectId(0),
        request,
        result: Ok(Vec::new()),
        direnv: None,
    });
    // The kill of a never-spawned agent is ignored by the service.
    assert_eq!(ops(&mut h), ["kill 0"]);
}

#[test]
fn stderr_goes_to_the_log_and_ready_to_messages() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    for line in ["hello", "world"] {
        h.send(Msg::AgentStderr {
            id: AgentId(0),
            line: line.into(),
        });
    }
    assert_eq!(log_text(&h, "*claude stderr*<work>"), "hello\nworld\n");
    ready(&mut h, 0);
    assert!(messages(&h).contains("claude session ready (modes: auto, plan)\n"));
    // It replaces `Starting claude...` in the echo area.
    assert_eq!(
        h.core.message(),
        Some("claude session ready (modes: auto, plan)")
    );
    h.keys("M-x agent-start RET claude RET");
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Ok(NewSessionResponse::new("t")),
    });
    assert!(messages(&h).contains("claude session ready (modes: none)\n"));
}

#[test]
fn a_failed_start_is_noted_and_keeps_the_log() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    h.send(Msg::AgentStarted {
        id: AgentId(0),
        result: Err("claude-agent-acp: No such file or directory".into()),
    });
    let note = "claude session failed to start: claude-agent-acp: No such file or directory";
    assert_eq!(h.core.message(), Some(note));
    assert!(messages(&h).contains(note));
    assert_eq!(ops(&mut h), ["kill 0"]);
    assert!(!h.core.live_agents());
    // Late stderr still reaches the log, and the exit adds no crash note.
    h.send(Msg::AgentStderr {
        id: AgentId(0),
        line: "auth failed".into(),
    });
    exited(&mut h, 0, 1);
    assert_eq!(log_text(&h, "*claude stderr*<work>"), "auth failed\n");
    assert!(!messages(&h).contains("exited"));
}

#[test]
fn a_crash_ends_only_its_own_session() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET M-x agent-start RET claude RET");
    ready(&mut h, 0);
    ready(&mut h, 1);
    ops(&mut h);
    exited(&mut h, 0, 3);
    assert!(messages(&h).contains("claude session exited with code 3\n"));
    // Only the crashed agent's handle is released.
    assert_eq!(ops(&mut h), ["kill 0"]);
    assert!(h.core.live_agents());
    let second = h.core.buffers.find("*claude*<work><2>").unwrap();
    assert!(h.core.live_agent(second));
    let first = h.core.buffers.find("*claude*<work>").unwrap();
    assert!(!h.core.live_agent(first) && h.core.buffers.get(first).is_some());
    h.send(Msg::AgentExited {
        id: AgentId(1),
        code: 1,
        signal: Some("Killed: 9".into()),
    });
    assert!(messages(&h).contains("claude session killed by Killed: 9\n"));
}

fn update(h: &mut Harness, id: u64, update: SessionUpdate) {
    h.send(Msg::AgentUpdate {
        id: AgentId(id),
        update: Box::new(SessionNotification::new("s", update)),
    });
}

fn chunk(h: &mut Harness, text: &str) {
    update(
        h,
        0,
        SessionUpdate::AgentMessageChunk(ContentChunk::new(text.into())),
    );
}

fn prompted(h: &mut Harness, result: Result<StopReason, &str>) {
    h.send(Msg::AgentPrompted {
        id: AgentId(0),
        result: result.map_err(String::from),
    });
}

/// A ready `claude` session in `*claude*<work>`, shown.
fn session() -> Harness {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ready(&mut h, 0);
    ops(&mut h);
    h
}

fn paste_text(lines: usize) -> String {
    (1..=lines).map(|n| format!("line {n} λ\n")).collect()
}

fn session_rows(h: &mut Harness) -> Vec<String> {
    // Tall enough that a third of it docks a 20-line prompt whole.
    h.joined_frame(1000., 2000.).windows[0]
        .display
        .lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
        .collect()
}

#[test]
fn a_long_yank_previews_four_lines_and_sends_every_line() {
    let mut h = session();
    h.clock("12:34:00");
    h.send(Msg::Insert {
        text: "before ".into(),
        replace: None,
    });
    let text = paste_text(21);
    h.clipboard = Some(text.clone());
    h.keys("C-y");
    h.send(Msg::Insert {
        text: " after".into(),
        replace: None,
    });
    assert_eq!(h.text(), format!("> before {text} after"));
    assert_eq!(session_rows(&mut h), ["❯ before  ≡ Pasted 21 lines  after"]);
    h.keys("C-x C-x TAB");
    let rows = session_rows(&mut h);
    assert_eq!(
        &rows[..6],
        [
            "❯ before  ≡ Pasted 21 lines ",
            "line 1 λ",
            "line 2 λ",
            "line 3 λ",
            "line 4 λ",
            "… 17 more lines after"
        ]
    );
    h.keys("RET");
    assert_eq!(
        ops(&mut h),
        [format!("prompt 0 s {:?}", format!("before {text} after"))]
    );
    let rows = session_rows(&mut h);
    assert!(rows[0].contains("≡ Pasted 21 lines"));
    assert!(rows.contains(&"12:34:00".to_string()), "{rows:?}");
    h.keys("M-< C-u 9 C-f TAB");
    let rows = session_rows(&mut h);
    assert_eq!(
        &rows[1..5],
        ["line 1 λ", "line 2 λ", "line 3 λ", "line 4 λ"]
    );
    h.keys("C-n TAB");
    assert_eq!(h.point(), 9);
    assert!(session_rows(&mut h)[0].contains("≡ Pasted 21 lines"));
}

#[test]
fn a_huge_paste_sends_and_its_echo_marks_every_line() {
    let mut h = session();
    let text = paste_text(20_000);
    h.clipboard = Some(text.clone());
    h.keys("C-y RET");
    assert_eq!(ops(&mut h), [format!("prompt 0 s {text:?}")]);
    assert!(session_rows(&mut h)[0].contains("≡ Pasted 20000 lines"));
    // The folded echo hides its lines; marked as plain text, each gets the band.
    let mut h = Harness::new(&[]);
    let b = crate::harness::buffer(&mut h);
    b.insert(0, &text).unwrap();
    user_lines(b, 0..b.len_chars());
    let user = Prop::Line(Decoration::Background(FaceName::SessionUser));
    assert_eq!(b.properties().filter(|(_, p)| **p == user).count(), 20_000);
}

#[test]
fn a_stale_top_row_clamps_when_the_window_widens() {
    let mut h = session();
    h.send(Msg::Insert {
        text: "q".into(),
        replace: None,
    });
    h.keys("RET");
    let long = "word ".repeat(400);
    chunk(&mut h, &format!("{long}\n{}", "x\n".repeat(60)));
    prompted(&mut h, Ok(StopReason::EndTurn));
    // Point at the long line's end, in a short narrow window, scrolls to one of its last rows.
    let text = h.text();
    let at = text[..text.find(&long).unwrap() + long.len() - 1]
        .chars()
        .count();
    let core = &mut h.core;
    (core.windows.window.set_point(&mut core.buffers, at)).unwrap();
    h.joined_frame(300., 100.);
    assert!(h.core.windows.window.top_row() > 2);
    h.keys("M->");
    let rows: Vec<String> = (h.joined_frame(1000., 400.).windows[0].display.lines.iter())
        .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
        .collect();
    assert!(rows[0].starts_with("word "), "{rows:?}");
}

#[test]
fn short_yanks_and_ordinary_buffers_stay_plain() {
    for lines in [1, 19, 20] {
        let mut h = session();
        let text = paste_text(lines);
        h.clipboard = Some(text.clone());
        h.keys("C-y");
        assert_eq!(h.text(), format!("> {text}"));
        assert_eq!(session_rows(&mut h)[0], "❯ line 1 λ");
    }
    let mut h = Harness::new(&[]);
    let text = paste_text(21);
    h.clipboard = Some(text.clone());
    h.keys("C-y");
    assert_eq!(h.text(), text);
    assert_eq!(session_rows(&mut h)[0], "line 1 λ");
}

#[test]
fn paste_folds_follow_streaming_and_yank_pop() {
    let mut h = session();
    h.core.settings.paste_fold_lines = 4;
    h.send(Msg::Insert {
        text: "question".into(),
        replace: None,
    });
    h.keys("RET");
    ops(&mut h);
    let text = paste_text(5);
    h.clipboard = Some(text.clone());
    h.keys("C-u C-y");
    assert_eq!(h.point(), h.core.current().prompt_start());
    chunk(&mut h, "answer\n");
    assert!(
        session_rows(&mut h)
            .iter()
            .any(|r| r.contains("≡ Pasted 5 lines"))
    );
    h.keys("TAB TAB");
    assert!(
        session_rows(&mut h)
            .iter()
            .any(|r| r.contains("≡ Pasted 5 lines"))
    );
    prompted(&mut h, Ok(StopReason::EndTurn));
    h.keys("RET");
    assert_eq!(ops(&mut h), [format!("prompt 0 s {text:?}")]);

    let mut h = session();
    h.clipboard = Some("short".into());
    h.keys("C-y");
    h.clipboard = Some(paste_text(21));
    h.keys("C-y M-y");
    assert_eq!(h.text(), "> shortshort");
    h.keys("M-y");
    assert!(session_rows(&mut h)[0].contains("≡ Pasted 21 lines"));
    h.keys("C-/");
    assert_eq!(h.text(), "> shortshort");
}

#[test]
fn a_custom_threshold_previews_short_pastes_and_reload_applies_to_the_next_yank() {
    let mut h = session();
    h.send(Msg::SettingsLoaded(Ok(Some("paste_fold_lines = 0".into()))));
    h.clipboard = Some("one\ntwo".into());
    h.keys("C-y C-x C-x TAB");
    assert_eq!(session_rows(&mut h), ["❯  ≡ Pasted 2 lines ", "one", "two"]);
    assert_eq!(h.text(), "> one\ntwo");
    h.keys("TAB M->");
    h.send(Msg::SettingsLoaded(Ok(Some("paste_fold_lines = 2".into()))));
    h.keys("C-y");
    assert_eq!(session_rows(&mut h), ["❯  ≡ Pasted 2 lines one", "two"]);
    assert_eq!(h.text(), "> one\ntwoone\ntwo");
    h.send(Msg::SettingsLoaded(Ok(Some(
        "paste_fold_lines = -1".into(),
    ))));
    assert_eq!(h.core.settings.paste_fold_lines, 2);
}

#[test]
fn several_pastes_and_a_file_chip_send_in_order() {
    let mut h = session();
    let text = paste_text(21);
    h.clipboard = Some(text.clone());
    h.keys("C-y C-y");
    let chip = h.core.chips.intern(Some("/outside/a.rs".into()), false);
    h.send(Msg::Insert {
        text: format!(" see {chip}"),
        replace: None,
    });
    assert_eq!(
        session_rows(&mut h),
        ["❯  ≡ Pasted 21 lines  ≡ Pasted 21 lines  see  ▤ a.rs "]
    );
    h.keys("RET");
    assert_eq!(
        ops(&mut h),
        [format!(
            "prompt 0 s {:?} link a.rs file:///outside/a.rs",
            format!("{text}{text} see ")
        )]
    );
    h.keys("M-< C-f C-f TAB");
    let rows = session_rows(&mut h);
    assert!(rows[5].contains("… 17 more lines ≡ Pasted 21 lines"));
}

#[test]
fn deleting_and_undoing_a_paste_preserves_the_original_text() {
    let mut h = session();
    let text = paste_text(21);
    h.clipboard = Some(text.clone());
    h.keys("C-y C-w");
    assert_eq!(h.text(), "> ");
    session_rows(&mut h);
    h.keys("C-/");
    assert_eq!(h.text(), format!("> {text}"));
    h.keys("RET");
    assert_eq!(ops(&mut h), [format!("prompt 0 s {text:?}")]);
}

fn info(h: &mut Harness, id: u64, info: Value) {
    update(
        h,
        id,
        SessionUpdate::SessionInfoUpdate(serde_json::from_value(info).unwrap()),
    );
}

#[test]
fn a_session_title_names_the_shown_buffer() {
    let mut h = session();
    info(
        &mut h,
        0,
        serde_json::json!({"title": " Fruit list \nMore"}),
    );
    assert_eq!(name(&h), "*claude: Fruit list*<work>");
}

#[test]
fn each_title_renames_the_session_buffer() {
    let mut h = work();
    h.keys("M-x agent-start RET codex RET");
    ready(&mut h, 0);
    for title in [
        "Reply with a list",
        "Two-fruit markdown list",
        "Two-fruit markdown list",
    ] {
        info(&mut h, 0, serde_json::json!({"title": title}));
        assert_eq!(name(&h), format!("*codex: {title}*<work>"));
    }
}

#[test]
fn long_titles_use_forty_chars_of_the_first_line() {
    let mut h = session();
    for (title, shown) in [
        (
            format!(" {} \nSecond line", "é".repeat(60)),
            format!("{}…", "é".repeat(40)),
        ),
        ("🦀".repeat(40), "🦀".repeat(40)),
    ] {
        info(&mut h, 0, serde_json::json!({"title": title}));
        assert_eq!(name(&h), format!("*claude: {shown}*<work>"));
    }
}

#[test]
fn a_meta_only_update_keeps_the_session_title() {
    let mut h = session();
    info(&mut h, 0, serde_json::json!({"title": "Fruit list"}));
    info(&mut h, 0, serde_json::json!({"_meta": {"status": "idle"}}));
    assert_eq!(name(&h), "*claude: Fruit list*<work>");
}

#[test]
fn null_and_empty_titles_restore_the_base_name() {
    let mut h = session();
    for title in [
        Value::Null,
        serde_json::json!(""),
        serde_json::json!(" \t\nSecond line"),
    ] {
        info(&mut h, 0, serde_json::json!({"title": "Fruit list"}));
        info(&mut h, 0, serde_json::json!({"title": title}));
        assert_eq!(name(&h), "*claude*<work>");
    }
}

#[test]
fn sessions_with_the_same_title_get_unique_names() {
    let mut h = session();
    let first = h.core.agents[&AgentId(0)].buffer;
    h.keys("M-x agent-start RET claude RET");
    ready(&mut h, 1);
    info(&mut h, 0, serde_json::json!({"title": "Fruit list"}));
    info(&mut h, 1, serde_json::json!({"title": "Fruit list"}));
    assert_eq!(name(&h), "*claude: Fruit list*<work><2>");
    h.core.switch_to(first);
    assert_eq!(name(&h), "*claude: Fruit list*<work>");
}

#[test]
fn titles_keep_the_workspace_name_the_session_started_with() {
    let mut h = session();
    info(
        &mut h,
        0,
        serde_json::json!({"title": "Fruit *list* <today>"}),
    );
    h.core.rename_workspace("renamed").unwrap();
    assert_eq!(name(&h), "*claude: Fruit *list* <today>*<work>");
    info(&mut h, 0, serde_json::json!({"title": "New title"}));
    assert_eq!(name(&h), "*claude: New title*<work>");
    info(&mut h, 0, serde_json::json!({"title": null}));
    assert_eq!(name(&h), "*claude*<work>");
}

#[test]
fn an_ended_session_keeps_its_last_title() {
    let mut h = session();
    info(&mut h, 0, serde_json::json!({"title": "Fruit list"}));
    exited(&mut h, 0, 0);
    info(&mut h, 0, serde_json::json!({"title": "Late title"}));
    assert_eq!(name(&h), "*claude: Fruit list*<work>");
}

#[test]
fn message_chunks_stream_above_the_prompt() {
    let mut h = session();
    let before = log_text(&h, "*claude stderr*<work>");
    chunk(&mut h, "Hi");
    let thought = SessionUpdate::AgentThoughtChunk(ContentChunk::new("hmm".into()));
    update(&mut h, 0, thought);
    chunk(&mut h, " there");
    // Unknown agents' updates are dropped.
    update(
        &mut h,
        9,
        SessionUpdate::AgentMessageChunk(ContentChunk::new("x".into())),
    );
    // The thought is a block of its own, between two message blocks.
    assert_eq!(h.core.text(), "Hi\n\nThought for 0s\nhmm\n\n there\n\n> ");
    assert_eq!(h.core.point(), h.core.text().chars().count());
    assert_eq!(log_text(&h, "*claude stderr*<work>"), before);
    exited(&mut h, 0, 0);
    let text = h.core.text().to_string();
    chunk(&mut h, "late");
    assert_eq!(h.core.text(), text);
}

#[test]
fn an_unknown_agents_update_preserves_every_buffer() {
    let mut h = session();
    let before: Vec<_> = h
        .core
        .buffers
        .iter_mut()
        .map(|(id, b)| (id, b.text().to_string()))
        .collect();
    update(
        &mut h,
        9,
        SessionUpdate::AgentMessageChunk(ContentChunk::new("late".into())),
    );
    let after: Vec<_> = h
        .core
        .buffers
        .iter_mut()
        .map(|(id, b)| (id, b.text().to_string()))
        .collect();
    assert_eq!(after, before);
}

#[test]
fn a_prompt_is_sent_and_its_reply_streams_above_the_prompt() {
    let mut h = session();
    h.keys("h e l l o RET");
    assert_eq!(ops(&mut h), [r#"prompt 0 s "hello""#]);
    assert_eq!(h.core.text(), "> hello\n\nWorking for 0s\n\n> ");
    assert_eq!(h.core.point(), 27);
    chunk(&mut h, "Hi");
    chunk(&mut h, " there");
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert_eq!(h.core.text(), "> hello\n\nHi there\n\nWorked 0s\n\n> ");
    // The turn ended, so the next prompt goes out.
    h.keys("a g a i n RET");
    assert_eq!(ops(&mut h), [r#"prompt 0 s "again""#]);
    chunk(&mut h, "Again");
    assert_eq!(
        h.core.text(),
        "> hello\n\nHi there\n\nWorked 0s\n\n> again\n\nAgain\n\nWorking for 0s\n\n> "
    );
}

#[test]
fn attachments_insert_external_files_and_images_as_chips_and_send_them() {
    for key in ["RET", "C-RET"] {
        let mut h = session();
        h.listing("/outside", &["a.rs", "view.png"]);
        h.keys("compare SPC SPC please");
        h.ctx().goto(10).unwrap();
        h.keys("M-x agent-attach-file RET C-a C-k /outside/a.rs");
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(mb.prompt, "Attach: ");
        let buffer = h.core.origin().unwrap().buffer();
        assert_eq!(
            h.core.buffers.get(buffer).unwrap().text().to_string(),
            "> compare  please"
        );
        chunk(&mut h, "Output");
        h.keys(key);
        let chip = h.core.chips.intern(Some("/outside/a.rs".into()), false);
        assert_eq!(h.text(), format!("Output\n\n> compare {chip} please"));
        assert_eq!(h.point(), "Output\n\n> compare ".chars().count() + 1);
        h.keys("M-> SPC M-x agent-attach-file RET C-a C-k /outside/view.png RET");
        drawn_line(&mut h, "❯ compare  ▤ a.rs  please  ▣ view.png ");
        h.keys("RET");
        assert_eq!(
            ops(&mut h),
            [
                "prompt 0 s \"compare \" link a.rs file:///outside/a.rs \" please \" image image/png file:///outside/view.png"
            ]
        );
    }
}

#[test]
fn attachments_enter_directories_or_attach_them_with_control_return() {
    let mut h = session();
    h.listing("/outside", &["empty/", "screens/"]);
    h.listing("/outside/screens", &["view.png"]);
    h.keys("M-x agent-attach-file RET C-a C-k /outside/screen RET");
    assert_eq!(h.text(), "/outside/screens/");
    let mb = h.core.minibuffer().unwrap();
    assert_eq!(mb.candidates, ["view.png"]);
    assert_eq!(mb.selected, None);
    h.keys("C-RET SPC M-x agent-attach-file RET C-a C-k /outside/empt C-RET");
    drawn_line(&mut h, "❯  ▦ screens   ▦ empty ");
    h.keys("RET");
    assert_eq!(
        ops(&mut h),
        ["prompt 0 s link screens file:///outside/screens/ link empty file:///outside/empty/"]
    );
}

#[test]
fn attachments_cancel_restores_point_and_transcript_invocation_appends() {
    let mut h = session();
    h.listing("/outside", &["a.rs"]);
    h.keys("before SPC SPC after");
    h.ctx().goto(9).unwrap();
    h.keys("M-x agent-attach-file RET C-a C-k /outside/a");
    chunk(&mut h, "Output");
    h.keys("C-g x");
    assert_eq!(h.text(), "Output\n\n> before x after");
    h.keys("M-< M-x agent-attach-file RET C-a C-k /outside/a RET");
    let chip = h.core.chips.intern(Some("/outside/a.rs".into()), false);
    assert_eq!(h.text(), format!("Output\n\n> before x after{chip}"));
    assert_eq!(h.point(), h.core.current().len_chars());
    h.keys("C-/");
    assert_eq!(h.text(), "Output\n\n> before x after");
}

#[test]
fn attachments_use_filesystem_annotations_and_directory_editing() {
    let mut h = session();
    let base = h.core.directory();
    h.listing(base.to_str().unwrap(), &["folder/", "note.txt"]);
    h.keys("M-x agent-attach-file RET");
    assert_eq!(h.text(), base.join("").to_string_lossy());
    let mb = h.core.minibuffer().unwrap();
    assert_eq!((mb.count.trim(), mb.selected), ("*/2", None));
    h.keys("folder TAB DEL");
    assert_eq!(h.text(), base.join("").to_string_lossy());
    h.keys("C-g");
    let note = "-rw-r--r--      41 B       5s ago";
    h.listings.insert(
        base,
        Ok(vec![crate::FileEntry {
            name: "note.txt".into(),
            annotation: note.into(),
        }]),
    );
    h.keys("M-x agent-attach-file RET");
    assert_eq!(h.core.minibuffer().unwrap().annotations, [note]);
}

#[test]
fn attachments_require_a_session_before_opening_the_picker() {
    let mut h = Harness::new(&[]);
    h.keys("M-x agent-attach-file RET");
    assert_eq!(h.core.message(), Some("Not in an agent session buffer"));
}

#[test]
fn references_offer_files_and_directories_in_every_root_without_previewing() {
    let mut h = session();
    let buffer = h.core.windows.window.buffer();
    let other = h.path("other/app");
    h.core.projects[0].project.roots.push(Root { path: other });
    h.index_reply("app", &["src/main.rs"], &["app", "app/src"]);
    h.index_reply("other/app", &["lib.rs"], &["other/app", "other/app/empty"]);
    h.keys("@");
    let mb = h.core.minibuffer().unwrap();
    assert_eq!(mb.prompt, "Reference in home: ");
    assert_eq!(
        mb.candidates,
        [
            "app/",
            "app/src/",
            "app/src/main.rs",
            "app<2>/",
            "app<2>/empty/",
            "app<2>/lib.rs"
        ]
    );
    assert_eq!(h.core.origin().unwrap().buffer(), buffer);
    let origin = h.core.buffers.get(buffer).unwrap();
    assert_eq!(origin.text().to_string(), "> @");
    h.keys("lib.rs RET");
    let chip = h.core.chips.intern(Some(h.path("other/app/lib.rs")), false);
    assert_eq!(h.text(), format!("> {chip}"));
    drawn_line(&mut h, "❯  ▤ lib.rs ");
    h.keys("RET");
    assert_eq!(
        ops(&mut h),
        ["prompt 0 s link lib.rs file:///aiditor-test/other/app/lib.rs"]
    );
}

#[test]
fn references_annotate_files_with_age_and_directories_as_dir() {
    let mut h = indexed_session();
    h.collect_effects = true;
    h.keys("@ main");
    let session = h
        .effects
        .iter()
        .rev()
        .find_map(|e| match e {
            Effect::File(crate::FileOp::Annotate { session, .. }) => Some(*session),
            _ => None,
        })
        .unwrap();
    h.send(Msg::Annotated {
        session,
        entries: vec![crate::FileEntry {
            name: "app/src/main.rs".into(),
            annotation: "-rw-r--r--      41 B       5s ago".into(),
        }],
    });
    assert_eq!(h.core.minibuffer().unwrap().annotations, ["5s ago"]);
    h.keys("C-a C-k");
    let mb = h.core.minibuffer().unwrap();
    let dir = mb.candidates.iter().position(|c| c == "app/src/").unwrap();
    assert_eq!(mb.annotations[dir], "dir");
}

#[test]
fn references_replace_only_the_at_sign_amid_text_after_output_arrives() {
    let mut h = indexed_session();
    h.keys("compare SPC SPC please");
    h.ctx().goto(10).unwrap();
    h.keys("@ main");
    chunk(&mut h, "An answer");
    h.keys("RET");
    let chip = h.core.chips.intern(Some(h.path("app/src/main.rs")), false);
    assert_eq!(h.text(), format!("An answer\n\n> compare {chip} please"));
    assert_eq!(h.point(), "An answer\n\n> compare ".chars().count() + 1);
    h.keys("RET");
    assert_eq!(
        ops(&mut h),
        ["prompt 0 s \"compare \" link main.rs file:///aiditor-test/app/src/main.rs \" please\""]
    );
}

#[test]
fn references_cancel_keeps_the_literal_at_sign_and_restores_point() {
    let mut h = indexed_session();
    h.keys("before SPC SPC after");
    h.ctx().goto(9).unwrap();
    h.keys("@ main");
    chunk(&mut h, "Output");
    h.keys("C-g x");
    assert_eq!(h.text(), "Output\n\n> before @x after");
}

#[test]
fn references_attach_empty_directories_and_images_as_their_chip_kinds() {
    let mut h = session();
    h.index_reply("app", &["cat.png"], &["app", "app/empty"]);
    h.keys("@ empty RET SPC @ cat RET");
    drawn_line(&mut h, "❯  ▦ empty   ▣ cat.png ");
    h.keys("RET");
    assert_eq!(
        ops(&mut h),
        [
            "prompt 0 s link empty file:///aiditor-test/app/empty/ image image/png file:///aiditor-test/app/cat.png"
        ]
    );
}

#[test]
fn references_fill_when_first_use_indexing_answers() {
    let mut h = session();
    h.collect_effects = true;
    h.keys("@");
    let (project, walk) = h
        .effects
        .iter()
        .find_map(|e| match e {
            Effect::File(crate::FileOp::Index { project, walk, .. }) => Some((*project, *walk)),
            _ => None,
        })
        .unwrap();
    h.send(Msg::Indexed {
        project,
        walk,
        roots: vec![(h.path("app"), Ok(vec!["src/main.rs".into()]))],
        directories: vec![h.path("app"), h.path("app/src")],
    });
    assert_eq!(listed(&mut h).0, ["app/", "app/src/", "app/src/main.rs"]);
    h.keys("main RET");
    let chip = h.core.chips.intern(Some(h.path("app/src/main.rs")), false);
    assert_eq!(h.text(), format!("> {chip}"));
}

#[test]
fn references_keep_the_captured_mapping_after_roots_change() {
    let mut h = indexed_session();
    h.keys("@ main");
    h.core.projects[0].project.primary.path = h.path("new/app");
    h.keys("RET");
    h.keys("RET");
    assert_eq!(
        ops(&mut h),
        ["prompt 0 s link main.rs file:///aiditor-test/app/src/main.rs"]
    );
}

#[test]
fn references_require_a_match_and_preserve_read_only_text() {
    let mut h = indexed_session();
    h.keys("@ nonexistent RET");
    assert_eq!(h.core.message(), Some("Match required"));
    assert_eq!(h.core.minibuffer().unwrap().prompt, "Reference in home: ");
    h.keys("C-g");
    assert_eq!(h.text(), "> @");
    chunk(&mut h, "Answer");
    h.keys("M-< @");
    assert_eq!(h.core.message(), Some("Text is read-only"));
    assert_eq!(h.text(), "Answer\n\n> @");
}

fn commands(h: &mut Harness, names: &[&str]) {
    let commands = (names.iter())
        .map(|&name| {
            let description = match name {
                "review" => "Review changes".to_string(),
                _ => format!("Run {name}"),
            };
            let command = AvailableCommand::new(name, description);
            match name {
                "review" => command.input(AvailableCommandInput::Unstructured(
                    UnstructuredCommandInput::new("[path]"),
                )),
                _ => command,
            }
        })
        .collect();
    let update = AvailableCommandsUpdate::new(commands);
    self::update(h, 0, SessionUpdate::AvailableCommandsUpdate(update));
}

#[test]
fn slash_at_the_prompt_start_picks_a_command_with_its_hint() {
    let mut h = session();
    commands(&mut h, &["review", "mcp"]);
    h.keys("/");
    let mb = h.core.minibuffer().unwrap();
    assert_eq!(mb.candidates, ["review", "mcp"]);
    assert_eq!(mb.annotations, ["[path]  Review changes", "Run mcp"]);
    h.keys("RET");
    let review = h.core.chips.command("review");
    assert_eq!(h.core.text(), format!("> {review} "));
    assert_eq!(h.core.point(), h.core.text().chars().count());
    drawn_line(&mut h, "❯  ▷ /review  ");
    // `RET` on the chip sends, as text, as ACP expects.
    h.keys("x C-b C-b C-b RET");
    assert_eq!(ops(&mut h), [r#"prompt 0 s "/review x""#]);
}

#[test]
fn the_command_list_follows_updates() {
    let mut h = session();
    commands(&mut h, &["review"]);
    commands(&mut h, &["compact"]);
    h.keys("/");
    assert_eq!(listed(&mut h).0, ["compact"]);
}

#[test]
fn long_descriptions_show_their_first_line_cut_to_sixty_chars() {
    let mut h = session();
    let long = format!("\n{}\nmore", "x".repeat(70));
    let list = AvailableCommandsUpdate::new(vec![AvailableCommand::new("skill", long)]);
    update(&mut h, 0, SessionUpdate::AvailableCommandsUpdate(list));
    let note = h.core.command_note("skill").unwrap();
    assert_eq!(note, format!("{}…", "x".repeat(59)));
}

#[test]
fn slash_types_itself_elsewhere_and_without_commands() {
    let mut h = session();
    h.keys("/");
    assert!(h.core.minibuffer().is_none());
    assert_eq!(h.core.text(), "> /");
    commands(&mut h, &["review"]);
    // Inside a word, as in a path.
    h.keys("/");
    assert!(h.core.minibuffer().is_none());
    assert_eq!(h.core.text(), "> //");
    let review = h.core.chips.command("review");
    // After a space, mid-message.
    h.keys("SPC / RET a");
    assert_eq!(h.core.text(), format!("> // {review} a"));
    // At the start of a prompt that holds text, the command goes before it.
    h.ctx().goto(2).unwrap();
    h.keys("/ RET");
    assert_eq!(h.core.text(), format!("> {review} // {review} a"));
}

#[test]
fn agent_command_from_the_transcript_appends_to_the_prompt() {
    let mut h = session();
    commands(&mut h, &["compact"]);
    chunk(&mut h, "Hi");
    h.keys("w o r k M-<");
    h.keys("M-x agent-command RET RET");
    let compact = h.core.chips.command("compact");
    assert!(h.core.text().ends_with(&format!("> work {compact} ")));
    assert_eq!(h.core.point(), h.core.text().chars().count());
    // An answer that names no command goes in as typed.
    h.keys("M-x agent-command RET f o o RET");
    assert!(h.core.text().ends_with(&format!("> work {compact} /foo ")));
}

#[test]
fn replayed_commands_the_session_knows_are_chips() {
    let mut h = session();
    commands(&mut h, &["review"]);
    let user = SessionUpdate::UserMessageChunk(ContentChunk::new("/review it, not /x".into()));
    update(&mut h, 0, user);
    let review = h.core.chips.command("review");
    assert!(h.core.text().starts_with(&format!("> {review} it, not /x")));
}

#[test]
fn project_roots_reach_the_spawn_and_prompts_carry_the_user_input() {
    let mut h = work();
    let lib = Root {
        path: h.path("lib"),
    };
    let docs = Root {
        path: h.path("docs"),
    };
    h.core.projects[0].project.roots = vec![lib, docs];
    h.keys("M-x agent-start RET claude RET");
    let roots: Vec<_> = (h.agent_ops.iter())
        .filter_map(|op| match op {
            AgentOp::Spawn { roots, .. } => Some(roots.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(roots, [vec![h.path("lib"), h.path("docs")]]);
    ready(&mut h, 0);
    // Taken at start: root changes wait for the next start.
    h.core.projects[0].project.roots.clear();
    ops(&mut h);
    h.keys("h i RET");
    assert_eq!(ops(&mut h), [r#"prompt 0 s "hi""#]);
    assert_eq!(h.core.text(), "> hi\n\nWorking for 0s\n\n> ");
    prompted(&mut h, Ok(StopReason::EndTurn));
    h.keys("a g a i n RET");
    assert_eq!(ops(&mut h), [r#"prompt 0 s "again""#]);
}

#[test]
fn an_agent_that_cant_take_the_roots_is_noted() {
    let mut h = session();
    h.send(Msg::AgentUnscoped { id: AgentId(0) });
    assert!(
        messages(&h).ends_with("claude can't take additional directories\n"),
        "{}",
        messages(&h)
    );
}

#[test]
fn replayed_user_messages_show_as_sent_prompts() {
    let mut h = session();
    chunk(&mut h, "a");
    let user = |text: &str| SessionUpdate::UserMessageChunk(ContentChunk::new(text.into()));
    update(&mut h, 0, user("see `b`"));
    let image = ContentBlock::Image(ImageContent::new("", "image/png"));
    update(
        &mut h,
        0,
        SessionUpdate::UserMessageChunk(ContentChunk::new(image)),
    );
    chunk(&mut h, "c");
    let image = h.core.chips.intern(None, false);
    assert_eq!(
        h.core.text(),
        format!("a\n\n> see `b`\n\n> {image}\n\nc\n\n> ")
    );
    let b = props(&h).into_iter().find(|(t, _)| t == "b");
    let code = vec![FaceRef::Face(FaceName::SessionCode)];
    assert_eq!(b, Some(("b".into(), Prop::Named(code))));
    h.keys("M-<");
    let user = Decoration::Background(FaceName::SessionUser);
    let line = drawn_line(&mut h, "❯ see b");
    assert!(line.decorations.contains(&user));
    let border = |top| Decoration::Border {
        top,
        face: FaceName::SessionUserRule,
    };
    let borders = (line.decorations.iter()).filter(|d| matches!(d, Decoration::Border { .. }));
    assert_eq!(borders.collect::<Vec<_>>(), [&border(true), &border(false)]);
    assert!(!drawn_line(&mut h, "c").decorations.contains(&user));
}

/// Session 0's messages as their ids and text.
fn message_list(h: &Harness) -> Vec<(Option<String>, String)> {
    let session = &h.core.agents[&AgentId(0)];
    let buffer = h.core.buffers.get(session.buffer).unwrap();
    (session.messages())
        .map(|(block, m)| {
            let text = buffer.text().slice(block.range(buffer)).to_string();
            (m.id.as_ref().map(|id| id.0.to_string()), text)
        })
        .collect()
}

fn agent_chunk(h: &mut Harness, id: &str, text: &str) {
    let chunk = ContentChunk::new(text.into()).message_id(id);
    update(h, 0, SessionUpdate::AgentMessageChunk(chunk));
}

#[test]
fn message_ids_are_kept_and_a_new_id_starts_a_block() {
    let mut h = session();
    agent_chunk(&mut h, "m1", "a");
    agent_chunk(&mut h, "m1", "b");
    agent_chunk(&mut h, "m2", "c");
    assert_eq!(h.core.text(), "ab\n\nc\n\n> ");
    assert_eq!(
        message_list(&h),
        [
            (Some("m1".into()), "ab".into()),
            (Some("m2".into()), "c".into())
        ]
    );
}

#[test]
fn replayed_user_messages_keep_their_ids_in_order() {
    let mut h = session();
    let user = ContentChunk::new("hi".into()).message_id("u1");
    update(&mut h, 0, SessionUpdate::UserMessageChunk(user));
    agent_chunk(&mut h, "m1", "yo");
    assert_eq!(
        message_list(&h),
        [
            (Some("u1".into()), "> hi".into()),
            (Some("m1".into()), "yo".into())
        ]
    );
}

#[test]
fn a_sent_prompt_is_a_message_after_the_last_agent_message() {
    let mut h = session();
    agent_chunk(&mut h, "m1", "yo");
    h.keys("h i RET");
    assert_eq!(
        message_list(&h),
        [(Some("m1".into()), "yo".into()), (None, "> hi".into())]
    );
}

/// The forks spawned since the last call: the source session and the message id.
fn forks(h: &mut Harness) -> Vec<(String, Option<String>)> {
    (h.agent_ops.drain(..))
        .filter_map(|op| match op {
            AgentOp::Spawn {
                session: Open::Fork(source, at),
                ..
            } => Some((source.to_string(), at.map(|m| m.0.to_string()))),
            _ => None,
        })
        .collect()
}

#[test]
fn a_fork_from_the_prompt_takes_the_whole_session_and_shows_once_loaded() {
    let mut h = session();
    info(&mut h, 0, serde_json::json!({"title": "Fix it"}));
    agent_chunk(&mut h, "m1", "yo");
    let source = h.core.windows.window.buffer();
    // Point on the prompt line's `> ` marker counts as the prompt.
    let marker = h.core.current().prompt_start() - PROMPT.len();
    h.ctx().goto(marker).unwrap();
    h.keys("M-x agent-fork-session RET");
    assert_eq!(forks(&mut h), [("s".into(), None)]);
    assert_eq!(h.core.message(), Some("Forking claude session..."));
    let fork = h.core.buffers.find("*claude: Fix it*<work><2>").unwrap();
    assert_eq!(h.core.windows.window.buffer(), source);
    let replayed = ContentChunk::new("yo".into()).message_id("m1");
    update(&mut h, 1, SessionUpdate::AgentMessageChunk(replayed));
    let response = NewSessionResponse::new("f").config_options(options("plan"));
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Ok(response),
    });
    assert_eq!(h.core.windows.window.buffer(), fork);
    assert_eq!(h.core.text(), "yo\n\n> ");
    // The fork keeps its mode, and the source is untouched and still sends.
    assert_eq!(ops(&mut h), Vec::<String>::new());
    h.core.switch_to(source);
    assert_eq!(h.core.text(), "yo\n\n> ");
    h.keys("M-> h i RET");
    assert_eq!(ops(&mut h), ["prompt 0 s \"hi\""]);
}

#[test]
fn a_fork_in_the_transcript_is_at_the_marked_message() {
    let mut h = session();
    agent_chunk(&mut h, "m1", "yo");
    tool(&mut h, ls());
    point_in(&mut h, "ls", 0);
    h.keys("M-x agent-fork-session RET");
    assert_eq!(forks(&mut h), [("s".into(), Some("m1".into()))]);
}

#[test]
fn a_fork_needs_a_ready_session_and_a_message_with_an_id() {
    let mut h = work();
    h.keys("M-x agent-fork-session RET");
    assert_eq!(h.core.message(), Some("Not in an agent session buffer"));
    h.keys("M-x agent-start RET claude RET");
    h.keys("M-x agent-fork-session RET");
    assert_eq!(h.core.message(), Some("claude is not ready"));
    ready(&mut h, 0);
    h.keys("h i RET");
    point_in(&mut h, "> hi", 2);
    h.keys("M-x agent-fork-session RET");
    assert_eq!(h.core.message(), Some("Fork at an agent message"));
    // A replayed prompt has an id but is the user's all the same.
    let user = ContentChunk::new("yo".into()).message_id("u1");
    update(&mut h, 0, SessionUpdate::UserMessageChunk(user));
    point_in(&mut h, "> yo", 2);
    h.keys("M-x agent-fork-session RET");
    assert_eq!(h.core.message(), Some("Fork at an agent message"));
    assert_eq!(forks(&mut h), []);
}

#[test]
fn a_failed_fork_opens_nothing() {
    let mut h = session();
    let source = h.core.windows.window.buffer();
    let names = h.core.buffer_names(true);
    h.keys("M-x agent-fork-session RET");
    ops(&mut h);
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Err("agent can't fork sessions".into()),
    });
    assert_eq!(
        h.core.message(),
        Some("claude fork failed: agent can't fork sessions")
    );
    assert_eq!(h.core.buffer_names(true), names);
    assert_eq!(h.core.windows.window.buffer(), source);
    assert_eq!(ops(&mut h), ["kill 1"]);
    assert!(!h.core.agents.contains_key(&AgentId(1)));
}

/// The rollbacks spawned since the last call: the source session and the message id.
fn rollbacks(h: &mut Harness) -> Vec<(String, String)> {
    (h.agent_ops.drain(..))
        .filter_map(|op| match op {
            AgentOp::Spawn {
                session: Open::Rollback(source, at),
                ..
            } => Some((source.to_string(), at.0.to_string())),
            _ => None,
        })
        .collect()
}

/// A session with agent messages `m1` and `m2`, point on `m1`, rolled back after `answer`.
fn rolled_back(answer: &str) -> Harness {
    let mut h = session();
    agent_chunk(&mut h, "m1", "yo");
    agent_chunk(&mut h, "m2", "later");
    point_in(&mut h, "yo", 1);
    h.keys("M-x agent-rollback-session RET");
    assert_eq!(
        h.core.minibuffer().unwrap().prompt,
        "Roll back to this message? Files stay as they are "
    );
    h.keys(&format!("{answer} RET"));
    h
}

#[test]
fn a_rollback_takes_the_source_buffers_place() {
    let mut h = rolled_back("yes");
    assert_eq!(rollbacks(&mut h), [("s".into(), "m1".into())]);
    assert_eq!(h.core.message(), Some("Rolling back claude session..."));
    let source = h.core.windows.window.buffer();
    h.keys("C-x 2");
    let replayed = ContentChunk::new("yo".into()).message_id("m1");
    update(&mut h, 1, SessionUpdate::AgentMessageChunk(replayed));
    let response = NewSessionResponse::new("f").config_options(options("auto"));
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Ok(response),
    });
    let fork = h.core.agents[&AgentId(1)].buffer;
    assert_ne!(fork, source);
    assert!(h.core.windows.windows_mut().all(|w| w.buffer() == fork));
    assert_eq!(h.core.text(), "yo\n\n> ");
    let names = [
        "*claude*<work>",
        "*scratch*<work>",
        "*Messages*",
        "*claude stderr*<work>",
    ];
    assert_eq!(h.core.buffer_names(true), names);
    assert_eq!(ops(&mut h), ["delete 1 0 s"]);
    assert!(!h.core.agents.contains_key(&AgentId(0)));
    h.keys("h i RET");
    assert_eq!(ops(&mut h), ["prompt 1 f \"hi\""]);
}

#[test]
fn a_rollback_moves_the_sources_pin_to_the_fork() {
    let mut h = rolled_back("yes");
    let cwd = h.core.agents[&AgentId(0)].root.display().to_string();
    let pin = |s| format!("[[pin]]\nagent = \"claude\"\ncwd = \"{cwd}\"\nsession = \"{s}\"\n");
    h.send(Msg::PinsLoaded(Ok(Some(pin("s")))));
    let response = NewSessionResponse::new("f").config_options(options("auto"));
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Ok(response),
    });
    let written = crate::sessions::parse_pins(h.pins.last().map(String::as_str)).unwrap();
    assert_eq!(
        written,
        crate::sessions::parse_pins(Some(&pin("f"))).unwrap()
    );
    assert_eq!(h.core.pins, written);
}

#[test]
fn every_kill_of_a_waiting_session_cancels_its_ask_then_stops_it() {
    let mut h = session();
    ask(&mut h, 4, "c", Some("Edit a.txt"));
    let strings = vec!["*claude*<work>".into(), "yes".into()];
    h.call(
        "kill-buffer",
        Args {
            strings,
            ..Args::default()
        },
    );
    h.send(Msg::Tick);
    assert_eq!(ops(&mut h), ["reply 0 4 cancelled", "kill 0"]);

    let mut h = session();
    ask(&mut h, 4, "c", Some("Edit a.txt"));
    let work = h.core.workspace().unwrap();
    h.new_workspace("other", "home");
    h.core.kill_workspace(work).unwrap();
    h.send(Msg::Tick);
    assert_eq!(ops(&mut h), ["reply 0 4 cancelled", "kill 0"]);

    let mut h = rolled_back("yes");
    ask(&mut h, 4, "c", Some("Edit a.txt"));
    rollbacks(&mut h);
    let response = NewSessionResponse::new("f").config_options(options("auto"));
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Ok(response),
    });
    assert_eq!(ops(&mut h), ["reply 0 4 cancelled", "delete 1 0 s"]);
}

#[test]
fn a_message_the_agent_cant_cut_at_says_so() {
    let not_found = "Invalid params: Fork point message m1 was not found in session s";
    for (command, echo) in [
        (
            "agent-rollback-session RET yes",
            "Can't roll back to this message",
        ),
        ("agent-fork-session", "Can't fork at this message"),
    ] {
        let mut h = session();
        agent_chunk(&mut h, "m1", "yo");
        point_in(&mut h, "yo", 1);
        h.keys(&format!("M-x {command} RET"));
        h.send(Msg::AgentStarted {
            id: AgentId(1),
            result: Err(not_found.into()),
        });
        assert_eq!(h.core.message(), Some(echo));
        let messages = h.core.buffers.find("*Messages*").unwrap();
        let logged = h.core.buffers.get(messages).unwrap().text().to_string();
        assert!(logged.contains(not_found), "{logged}");
    }
}

#[test]
fn a_declined_rollback_does_nothing() {
    let mut h = rolled_back("no");
    assert_eq!(rollbacks(&mut h), []);
    assert_eq!(h.core.agents.len(), 1);
}

#[test]
fn a_failed_rollback_leaves_the_source() {
    let mut h = rolled_back("yes");
    let source = h.core.windows.window.buffer();
    ops(&mut h);
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Err("agent can't fork sessions".into()),
    });
    assert_eq!(
        h.core.message(),
        Some("claude rollback failed: agent can't fork sessions")
    );
    assert_eq!(h.core.windows.window.buffer(), source);
    assert_eq!(ops(&mut h), ["kill 1"]);
    h.keys("M-> h i RET");
    assert_eq!(ops(&mut h), ["prompt 0 s \"hi\""]);
}

#[test]
fn a_rollback_needs_an_agent_message_at_point() {
    let mut h = session();
    h.keys("M-x agent-rollback-session RET");
    assert_eq!(h.core.message(), Some("No message at point"));
    h.keys("h i RET");
    point_in(&mut h, "> hi", 2);
    h.keys("M-x agent-rollback-session RET");
    assert_eq!(h.core.message(), Some("Roll back to an agent message"));
    assert!(h.core.minibuffer().is_none());
    chunk(&mut h, "no id");
    point_in(&mut h, "no id", 0);
    h.keys("M-x agent-rollback-session RET");
    assert_eq!(h.core.message(), Some("No message id to roll back to"));
    assert_eq!(rollbacks(&mut h), []);
}

/// The text of each transcript row and each prompt pane row of a frame `rows` text rows high,
/// and which part holds the cursor.
fn docked(h: &mut Harness, rows: usize) -> (Vec<String>, Vec<String>, &'static str) {
    let settings = h.core.settings();
    let height = rows as f32 * settings.line_height() + settings.mode_line_height();
    let frame = h.frame(800., height);
    let display = &frame.windows[0].display;
    let text = |lines: &[crate::display::Line]| {
        let line = |l: &crate::display::Line| l.spans.iter().map(|s| s.text.clone()).collect();
        lines.iter().map(line).collect::<Vec<String>>()
    };
    let pane = display
        .prompt
        .as_ref()
        .expect("a session window docks its prompt");
    let cursor = match (display.cursor, pane.cursor) {
        (Some(_), None) => "transcript",
        (None, Some(_)) => "pane",
        _ => "?",
    };
    (text(&display.lines), text(&pane.lines), cursor)
}

#[test]
fn the_prompt_area_docks_at_the_window_s_bottom() {
    let mut h = session();
    chunk(&mut h, "one\ntwo\nthree");
    let (transcript, pane, cursor) = docked(&mut h, 30);
    assert_eq!(transcript[..4], ["one", "two", "three", ""]);
    assert_eq!((pane, cursor), (vec!["❯ ".to_string()], "pane"));
    // The transcript gets the rows the pane and its rule and padding leave.
    let line = h.core.settings().line_height();
    let left = (29. * line - crate::display::PANE_CHROME) / line;
    assert_eq!(h.core.windows.window.rows(), left as usize);
    // Output arriving while typing keeps the transcript at its end and the pane as it was.
    h.keys("h i");
    chunk(
        &mut h,
        &(0..40).map(|i| format!("\nline {i}")).collect::<String>(),
    );
    let (transcript, pane, cursor) = docked(&mut h, 30);
    assert_eq!(transcript[transcript.len() - 2..], ["line 39", ""]);
    assert_eq!((pane, cursor), (vec!["❯ hi".to_string()], "pane"));
    // Reading the transcript scrolls it alone.
    h.keys("M-<");
    let (transcript, pane, cursor) = docked(&mut h, 30);
    assert_eq!((transcript[0].as_str(), cursor), ("one", "transcript"));
    assert_eq!(pane, ["❯ hi"]);
    // A long prompt docks a third of the rows, point's line among them.
    h.keys("M->");
    for _ in 0..19 {
        h.keys("C-j x");
    }
    let (_, pane, cursor) = docked(&mut h, 30);
    assert_eq!(
        (pane.len(), pane.last().unwrap().as_str(), cursor),
        (10, "x", "pane")
    );
    h.keys("M-<");
    h.keys("C-s h i RET");
    let (_, pane, cursor) = docked(&mut h, 30);
    assert_eq!((pane[0].as_str(), cursor), ("❯ hi", "pane"));
}

#[test]
fn paging_and_recentering_move_the_transcript_rows() {
    let mut h = session();
    chunk(
        &mut h,
        &(0..100).map(|i| format!("line {i}\n")).collect::<String>(),
    );
    h.keys("M-<");
    let (transcript, ..) = docked(&mut h, 30);
    let rows = h.core.windows.window.rows();
    assert_eq!(transcript.len(), rows, "the rows above the pane");
    h.keys("C-v");
    let (transcript, pane, cursor) = docked(&mut h, 30);
    assert_eq!(
        (transcript[0].clone(), cursor),
        (format!("line {}", rows - 2), "transcript")
    );
    assert_eq!(pane, ["❯ "]);
    h.keys("M-v");
    assert_eq!(docked(&mut h, 30).0[0], "line 0");
    h.keys("M-< C-u 4 9 C-n C-l");
    let (transcript, ..) = docked(&mut h, 30);
    assert_eq!(transcript[rows / 2], "line 49");
}

/// A session with 100 transcript lines, `line 0` to `line 99`, drawn in 30 rows.
fn long_session() -> Harness {
    let mut h = session();
    chunk(
        &mut h,
        &(0..100).map(|i| format!("line {i}\n")).collect::<String>(),
    );
    docked(&mut h, 30);
    h
}

#[test]
fn the_transcript_keeps_its_rows_while_typing_scrolled_back() {
    let mut h = long_session();
    h.keys("M-< C-v M->");
    let (before, ..) = docked(&mut h, 30);
    assert_ne!(before[0], "line 0", "C-v scrolled it");
    h.keys("h i");
    chunk(&mut h, "more\nand more\n");
    let (after, pane, cursor) = docked(&mut h, 30);
    assert_eq!(after, before);
    assert_eq!((pane, cursor), (vec!["❯ hi".to_string()], "pane"));
    // Paging to the end follows output again.
    h.keys("C-v C-v C-v C-v C-v");
    chunk(&mut h, "last\n");
    let (transcript, ..) = docked(&mut h, 30);
    let tail: Vec<&str> = transcript
        .iter()
        .rev()
        .take(3)
        .map(String::as_str)
        .collect();
    assert!(tail.contains(&"last"), "{tail:?}");
}

#[test]
fn sending_shows_the_transcript_s_end() {
    let mut h = long_session();
    h.keys("M-v M-v");
    assert!(!docked(&mut h, 30).0.contains(&"line 99".to_string()));
    h.keys("h i RET");
    let (transcript, ..) = docked(&mut h, 30);
    assert!(
        transcript
            .iter()
            .rev()
            .take(3)
            .any(|l| l.starts_with("Working for")),
        "{transcript:?}"
    );
}

#[test]
fn paging_from_the_prompt_moves_the_transcript_not_point() {
    let mut h = long_session();
    h.keys("h i");
    let point = h.point();
    let (end, ..) = docked(&mut h, 30);
    h.keys("M-v");
    let (paged, ..) = docked(&mut h, 30);
    assert_ne!(paged, end);
    assert_eq!(h.point(), point);
    h.keys("C-v");
    assert_eq!(docked(&mut h, 30).0, end);
    h.keys("C-v");
    assert_eq!(h.core.message(), Some("End of buffer"));
    h.keys("M-v M-v M-v M-v M-v M-v");
    assert_eq!(docked(&mut h, 30).0[0], "line 0");
    h.keys("M-v");
    assert_eq!(h.core.message(), Some("Beginning of buffer"));
    // C-l recenters on the transcript point, kept within the rows shown.
    h.keys("C-l");
    let (transcript, _, cursor) = docked(&mut h, 30);
    assert_eq!((h.point(), cursor), (point, "pane"));
    let rows = h.core.windows.window.rows();
    assert_eq!(transcript[rows / 2], format!("line {}", rows - 1));
}

#[test]
fn c_t_moves_between_the_prompt_and_the_transcript() {
    let mut h = long_session();
    h.keys("h i M-< C-u 4 0 C-n C-f C-f");
    let (rows, ..) = docked(&mut h, 30);
    let reading = h.point();
    h.keys("C-t");
    assert_eq!(
        h.core
            .current()
            .text()
            .slice(..h.point())
            .to_string()
            .rsplit('\n')
            .next(),
        Some("> hi")
    );
    assert_eq!(
        docked(&mut h, 30),
        (rows.clone(), vec!["❯ hi".to_string()], "pane")
    );
    h.keys("C-t");
    assert_eq!(h.point(), reading);
    assert_eq!(docked(&mut h, 30).0, rows);
    // A transcript point paged out of view lands in the rows shown.
    h.keys("C-t M-v M-v");
    let (paged, ..) = docked(&mut h, 30);
    h.keys("C-t");
    let (transcript, _, cursor) = docked(&mut h, 30);
    assert_eq!((transcript, cursor), (paged, "transcript"));
    assert_ne!(h.point(), reading);
    h.keys("C-t");
    h.keys("M-x agent-toggle-prompt RET");
    assert_eq!(docked(&mut h, 30).2, "transcript");
}

#[test]
fn typing_in_the_transcript_goes_to_the_prompt() {
    let mut h = long_session();
    h.keys("x M-< C-n");
    let (rows, ..) = docked(&mut h, 30);
    h.keys("a");
    assert!(h.core.text().ends_with("> xa"));
    assert_eq!(h.point(), h.core.text().chars().count());
    assert_eq!(docked(&mut h, 30), (rows, vec!["❯ xa".to_string()], "pane"));
    // Bound keys keep their meaning: TAB on a tool call's header opens it.
    let call = ls().status(ToolCallStatus::Completed);
    tool(&mut h, call.raw_output(serde_json::json!("out\n")));
    point_in(&mut h, "Ran ls", 0);
    h.keys("TAB");
    assert!(
        h.core.text().ends_with("> xa"),
        "nothing typed in the prompt"
    );
    assert!(h.core.text().contains("$ ls"), "{}", h.core.text());
}

#[test]
fn other_buffers_dock_no_prompt() {
    let mut h = Harness::new(&[("a.txt", "a\n")]);
    h.open("a.txt");
    assert!(h.frame(800., 600.).windows[0].display.prompt.is_none());
}

#[test]
fn a_session_window_insets_its_text_as_frame_6() {
    let mut h = session();
    chunk(&mut h, &"x".repeat(60));
    let frame = h.joined_frame(211., 600.);
    let w = &frame.windows[0];
    assert_eq!(w.inset, (27., 24.));
    let text: String = w.display.lines[0]
        .spans
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(text.chars().count(), 19, "20 columns less the wrap's");
    assert_eq!(h.core.windows.window.cols(), 20);
    let mut h = Harness::new(&[("a.txt", "a\n")]);
    h.open("a.txt");
    assert_eq!(
        h.frame(211., 600.).windows[0].inset.0,
        crate::display::TEXT_INSET
    );
}

#[test]
fn prompts_draw_a_glyph_and_echoes_the_user_background() {
    let mut h = session();
    h.keys("a C-j b RET");
    assert!(h.core.text().starts_with("> a\nb"));
    h.keys("M-<");
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    let text = |l: &crate::display::Line| l.spans.iter().map(|s| s.text.clone()).collect();
    let lines: Vec<(String, _)> = (lines.iter())
        .map(|l| (text(l), l.decorations.clone()))
        .collect();
    let user = Decoration::Background(FaceName::SessionUser);
    assert!(lines[0].0.starts_with("❯ a"), "{:?}", lines[0].0);
    assert_eq!(lines[1].0, "b");
    assert!(lines[0].1.contains(&user) && lines[1].1.contains(&user));
    assert!(!lines[2].1.contains(&user), "the blank line after it");
    let border = |top| Decoration::Border {
        top,
        face: FaceName::SessionUserRule,
    };
    assert!(lines[0].1.contains(&border(true)) && !lines[0].1.contains(&border(false)));
    assert!(lines[1].1.contains(&border(false)) && !lines[1].1.contains(&border(true)));
    let theme = &h.core.settings().theme;
    let user_face = theme.face(FaceName::SessionUser);
    let b = &frame.windows[0].display.lines[1].spans[0];
    assert_eq!(
        (b.face.fg, b.face.bold),
        (user_face.fg, true),
        "the text in session-user"
    );
    let prompt = lines
        .iter()
        .find(|(t, _)| t.starts_with("❯ ") && t.len() == "❯ ".len());
    let prompt = prompt.expect("the live prompt");
    assert!(!prompt.1.contains(&user) && !prompt.1.contains(&border(true)));
    let glyph = &frame.windows[0].display.lines[0].spans[0];
    let info = h.core.settings().theme.face(FaceName::StatusInfo);
    assert_eq!(
        (glyph.text.as_str(), glyph.face.fg, glyph.face.bold),
        ("❯", info.fg, true)
    );
}

#[test]
fn message_keys_move_past_tool_calls_and_thoughts() {
    let mut h = session();
    agent_chunk(&mut h, "m1", "one\ntwo");
    let call = ToolCall::new("t1", "Read");
    update(&mut h, 0, SessionUpdate::ToolCall(call));
    thought(&mut h, "hmm");
    agent_chunk(&mut h, "m2", "three");
    let text = h.core.text();
    let second = text[..text.find("three").unwrap()].chars().count();
    h.keys("C-c C-p");
    assert_eq!(h.core.point(), second);
    h.keys("C-c C-p");
    assert_eq!(h.core.point(), 0);
    h.keys("C-c C-p");
    assert_eq!(h.core.message(), Some("No message found"));
    assert_eq!(h.core.point(), 0);
    h.keys("C-c C-n");
    assert_eq!(h.core.point(), second);
    h.keys("C-c C-n");
    assert_eq!(h.core.message(), Some("No message found"));
    assert_eq!(h.core.point(), second);
    // From inside a message it goes to that message's start.
    h.keys("C-f C-c C-p");
    assert_eq!(h.core.point(), second);
    // A count stops at the farthest, and a negative one turns back.
    h.keys("C-u 5 C-c C-p");
    assert_eq!(h.core.point(), 0);
    h.keys("C-u - C-c C-p");
    assert_eq!(h.core.point(), second);
}

#[test]
fn the_bar_follows_point_and_each_window_has_its_own() {
    let bar = Decoration::Bar(FaceName::MessageAtPoint);
    let barred = |h: &mut Harness, window: usize| -> Vec<usize> {
        let frame = h.joined_frame(800., 600.);
        let display = &frame.windows[window].display;
        (display.lines.iter())
            .filter(|l| l.decorations.contains(&bar))
            .map(|l| l.number)
            .collect()
    };
    let mut h = session();
    assert!(barred(&mut h, 0).is_empty());
    agent_chunk(&mut h, "m1", "one\ntwo");
    let call = ToolCall::new("t1", "Read");
    update(&mut h, 0, SessionUpdate::ToolCall(call.clone()));
    // Point in the prompt keeps it on the last message, past the tool call.
    assert_eq!(barred(&mut h, 0), [0, 1]);
    agent_chunk(&mut h, "m2", "three");
    assert_eq!(barred(&mut h, 0), [5]);
    h.keys("C-x 2 C-c C-p C-c C-p");
    assert_eq!(barred(&mut h, 0), [0, 1]);
    assert_eq!(barred(&mut h, 1), [5]);
}

/// Each property of the current buffer, with the text it covers.
/// The properties, without the prompt's glyph and the echo's background every session has.
fn props(h: &Harness) -> Vec<(String, Prop)> {
    let b = h.core.current();
    let prompt = |p: &Prop| match p {
        Prop::Display(pieces) => pieces[0].0 == "❯",
        Prop::Line(Decoration::Border { face, .. }) => *face == FaceName::SessionUserRule,
        _ => *p == Prop::Line(Decoration::Background(FaceName::SessionUser)),
    };
    (b.properties())
        .filter(|(_, p)| !prompt(p))
        .map(|(r, p)| (b.text().slice(r).to_string(), p.clone()))
        .collect()
}

fn thought(h: &mut Harness, text: &str) {
    update(
        h,
        0,
        SessionUpdate::AgentThoughtChunk(ContentChunk::new(text.into())),
    );
}

/// Reloads settings that switch to a theme giving `session-thought` and `session-block`
/// these colours, as a user's `theme` change does.
fn retheme(h: &mut Harness) -> (u32, u32) {
    let settings = "theme = \"other\"\n[themes.other.faces]\nsession-thought = { fg = 0x010203 }\nsession-block = { bg = 0x040506 }";
    h.send(Msg::SettingsLoaded(Ok(Some(settings.into()))));
    (0x010203, 0x040506)
}

/// The face the display gives `text`, a span of the frame.
fn drawn(h: &mut Harness, text: &str) -> Face {
    h.keys("M-<");
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    let mut spans = lines.iter().flat_map(|l| &l.spans);
    spans.find(|s| s.text == text).expect("drawn").face
}

#[test]
fn a_theme_change_redraws_completed_replies() {
    let mut h = session();
    h.keys("a RET");
    thought(&mut h, "hmm");
    chunk(&mut h, "see `x`\n\n```\nblock\n```");
    prompted(&mut h, Ok(StopReason::EndTurn));
    h.core.frame();
    let before = props(&h);
    let (dim, code) = retheme(&mut h);
    assert_eq!(props(&h), before, "only the display changes");
    assert_eq!(drawn(&mut h, "hmm").fg, Some(dim));
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    let block = (lines.iter())
        .find(|l| l.spans.iter().any(|s| s.text == "block"))
        .expect("drawn");
    let face = Decoration::Background(FaceName::SessionBlock);
    assert_eq!(block.decorations, [face]);
    assert_eq!(
        h.core.settings().theme.face(FaceName::SessionBlock).bg,
        Some(code)
    );
}

#[test]
fn a_tool_calls_rule_names_its_face() {
    let mut h = session();
    tool(&mut h, ls());
    let rules: Vec<_> = (props(&h).into_iter())
        .filter_map(|(_, p)| match p {
            Prop::Line(Decoration::Rule(name)) => Some(name),
            _ => None,
        })
        .collect();
    assert!(
        !rules.is_empty() && rules.iter().all(|&n| n == FaceName::SessionThought),
        "{rules:?}"
    );
}

#[test]
fn faces_sharing_a_colour_part_on_a_theme_change() {
    let mut h = session();
    let same = "[themes.default.faces]\nsession-thought = { fg = 5 }\nsession-code = { fg = 5 }";
    h.send(Msg::SettingsLoaded(Ok(Some(same.into()))));
    thought(&mut h, "hmm");
    chunk(&mut h, "`x`");
    h.core.frame();
    let apart = format!(
        "{same}\n[themes.other.faces]\nsession-thought = {{ fg = 6 }}\nsession-code = {{ fg = 7 }}\n"
    );
    let apart = format!("theme = \"other\"\n{apart}");
    h.send(Msg::SettingsLoaded(Ok(Some(apart))));
    assert_eq!(drawn(&mut h, "hmm").fg, Some(6));
    assert_eq!(drawn(&mut h, "x").fg, Some(7));
}

#[test]
fn a_streamed_block_renders_when_the_frame_is_built() {
    let rendered = |h: &Harness| {
        let own = |(t, p): &(String, Prop)| {
            t.starts_with("Work")
                || matches!(p, Prop::Line(Decoration::Indent(2) | Decoration::HRule(_)))
        };
        (props(h).into_iter().filter(|p| !own(p))).collect::<Vec<_>>()
    };
    let mut h = session();
    h.keys("a RET");
    chunk(&mut h, "**a");
    chunk(&mut h, "b**");
    assert_eq!(h.core.text(), "> a\n\n**ab**\n\nWorking for 0s\n\n> ");
    assert!(rendered(&h).is_empty(), "nothing rendered before the frame");
    h.core.frame();
    let bold = Prop::Named(vec![FaceRef::Bold]);
    let hidden = Prop::Invisible(String::new());
    let first = vec![
        ("**".into(), hidden.clone()),
        ("ab".into(), bold),
        ("**".into(), hidden.clone()),
    ];
    assert_eq!(rendered(&h), first);
    // The next reply renders on its own; the first keeps its properties.
    prompted(&mut h, Ok(StopReason::EndTurn));
    h.keys("b RET");
    chunk(&mut h, "*c*");
    assert_eq!(rendered(&h), first);
    h.core.frame();
    let italic = Prop::Named(vec![FaceRef::Italic]);
    let all = [
        first.clone(),
        vec![
            ("*".into(), hidden.clone()),
            ("c".into(), italic),
            ("*".into(), hidden.clone()),
        ],
    ]
    .concat();
    assert_eq!(rendered(&h), all);
    h.core.frame();
    assert_eq!(rendered(&h), all, "a frame without chunks changes nothing");
}

#[test]
fn a_finished_block_renders_before_the_next_starts() {
    let mut h = session();
    chunk(&mut h, "`x`");
    thought(&mut h, "hmm");
    // The message rendered when the thought began; the thought waits for the frame.
    assert_eq!(props(&h).len(), 3);
    h.core.frame();
    let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
    assert_eq!(props(&h).last(), Some(&("hmm".into(), dim)));
}

#[test]
fn hidden_streams_keep_pending_formatting_until_shown() {
    let mut h = session();
    let buffer = h.core.current().name.clone();
    let id = h.core.agents[&AgentId(0)].buffer;
    h.open("app/src/main.rs");
    chunk(&mut h, "**a");
    chunk(&mut h, "b**");
    h.joined_frame(800., 600.);
    let b = h.core.buffers.get(id).unwrap();
    assert!(b.text().to_string().contains("**ab**"));
    assert!(h.core.agents[&AgentId(0)].stream.as_ref().unwrap().dirty);
    h.call(
        "switch-to-buffer",
        Args {
            strings: vec![buffer],
            ..Args::default()
        },
    );
    h.joined_frame(800., 600.);
    assert!(props(&h).contains(&("ab".into(), Prop::Named(vec![FaceRef::Bold]))));
    assert!(!h.core.agents[&AgentId(0)].stream.as_ref().unwrap().dirty);
}

#[test]
fn inactive_panes_duplicate_panes_and_prompt_origins_flush_streams() {
    let mut h = session();
    h.keys("C-x 3");
    chunk(&mut h, "**a**");
    h.joined_frame(800., 600.);
    assert!(props(&h).contains(&("a".into(), Prop::Named(vec![FaceRef::Bold]))));
    h.open("app/src/main.rs");
    chunk(&mut h, " *b*");
    h.joined_frame(800., 600.);
    let id = h.core.agents[&AgentId(0)].buffer;
    assert!(
        h.core
            .buffers
            .get(id)
            .unwrap()
            .properties()
            .any(|(_, p)| *p == Prop::Named(vec![FaceRef::Italic]))
    );
    h.keys("C-x o M-x");
    chunk(&mut h, " `c`");
    h.joined_frame(800., 600.);
    assert!(!h.core.agents[&AgentId(0)].stream.as_ref().unwrap().dirty);
    assert!(
        h.core
            .buffers
            .get(id)
            .unwrap()
            .properties()
            .any(|(_, p)| *p == Prop::Named(vec![FaceRef::Face(FaceName::SessionCode)]))
    );
}

#[test]
fn inactive_workspace_streams_finalize_hidden_blocks_and_kill_safely() {
    let mut h = session();
    let workspace = h.core.current_workspace().unwrap().id;
    let project = h.core.current_workspace().unwrap().project;
    h.core.new_workspace("other", project);
    chunk(&mut h, "**a**");
    h.joined_frame(800., 600.);
    assert!(h.core.agents[&AgentId(0)].stream.as_ref().unwrap().dirty);
    thought(&mut h, "hmm");
    let id = h.core.agents[&AgentId(0)].buffer;
    assert!(
        h.core
            .buffers
            .get(id)
            .unwrap()
            .properties()
            .any(|(_, p)| *p == Prop::Named(vec![FaceRef::Bold]))
    );
    h.core.switch_workspace(workspace).unwrap();
    h.joined_frame(800., 600.);
    assert!(props(&h).contains(&(
        "hmm".into(),
        Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)])
    )));
    h.keys("C-x k RET yes RET");
    h.joined_frame(800., 600.);
    assert!(h.core.buffers.get(id).is_none());
}

#[test]
fn the_echoed_prompt_renders_as_markdown() {
    let mut h = session();
    h.keys("* * h i * * RET");
    assert_eq!(h.core.text(), "> **hi**\n\nWorking for 0s\n\n> ");
    let bold = Prop::Named(vec![FaceRef::Bold]);
    assert!(props(&h).contains(&("hi".into(), bold)));
}

#[test]
fn rendered_replies_show_without_markup_and_keep_it_in_the_text() {
    let mut h = session();
    chunk(
        &mut h,
        "# Title\n\nSome **bold** [docs](https://x.dev).\n\n```rust\nfn main() {}\n```",
    );
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    let shown: Vec<String> = (lines.iter())
        .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
        .collect();
    assert_eq!(
        shown[..7],
        ["Title", "", "Some bold docs.", "", "", "fn main() {}", ""]
    );
    let code = Decoration::Background(FaceName::SessionBlock);
    assert!((4..7).all(|i| lines[i].decorations.contains(&code)));
    assert!(!lines[3].decorations.contains(&code));
    // Swiper matches the hidden markup, and copying yields it.
    h.keys("C-s * * b");
    assert_eq!(listed(&mut h).0, ["3 Some **bold** [docs](https://x.dev)."]);
    h.keys("C-g M-< C-n C-n C-SPC C-e M-w");
    assert_eq!(
        h.clipboard.as_deref(),
        Some("Some **bold** [docs](https://x.dev).")
    );
}

fn tool(h: &mut Harness, call: ToolCall) {
    update(h, 0, SessionUpdate::ToolCall(call));
}

fn tool_update(h: &mut Harness, id: &'static str, fields: ToolCallUpdateFields) {
    update(
        h,
        0,
        SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(id, fields)),
    );
}

/// The text of each display line of a frame `rows` lines high.
fn shown(h: &mut Harness, rows: usize) -> Vec<String> {
    let settings = h.core.settings();
    let height = rows as f32 * settings.line_height() + settings.mode_line_height();
    let frame = h.joined_frame(800., height);
    (frame.windows[0].display.lines.iter())
        .map(|l| {
            // Right-aligning padding reads as the one space it is in the text.
            let padding = |s: &str| s.len() > 1 && s.trim().is_empty();
            let text = |(i, s): (usize, &crate::display::Span)| match i > 0 && padding(&s.text) {
                true => " ".to_string(),
                false => s.text.clone(),
            };
            l.spans.iter().enumerate().map(text).collect()
        })
        .collect()
}

/// An `execute` call `t` titled `` `ls` `` with the input `{"command":"ls"}`.
fn ls() -> ToolCall {
    ToolCall::new("t", "`ls`")
        .kind(ToolKind::Execute)
        .raw_input(serde_json::json!({"command": "ls"}))
}

const LS_INPUT: &str = "```\n$ ls\n```";

#[test]
fn a_message_ending_in_newlines_keeps_one_blank_line_before_a_call() {
    let mut h = session();
    h.keys("a RET");
    chunk(&mut h, "Let me look.\n\n");
    tool(&mut h, ls());
    assert_eq!(
        shown(&mut h, 10)[2..5],
        ["Let me look.", "", "▸ ●   Running ls"]
    );
}

#[test]
fn a_tool_call_starts_folded_and_updates_in_place() {
    let mut h = session();
    h.keys("a RET");
    chunk(&mut h, "Let me look.");
    tool(&mut h, ls());
    assert_eq!(
        h.core.text(),
        format!("> a\n\nLet me look.\n\n▸ ●    Running ls\n{LS_INPUT}\n\nWorking for 0s\n\n> ")
    );
    assert_eq!(
        shown(&mut h, 10)[..5],
        ["❯ a", "", "Let me look.", "", "▸ ●   Running ls"]
    );
    // A message after the call, then updates: the call's block changes where it is.
    chunk(&mut h, "Done.");
    let status = |s| ToolCallUpdateFields::new().status(s);
    tool_update(&mut h, "t", status(ToolCallStatus::InProgress));
    tool_update(
        &mut h,
        "t",
        status(ToolCallStatus::Completed).raw_output(serde_json::json!("a.rs\nb.rs\n")),
    );
    assert_eq!(
        h.core.text(),
        "> a\n\nLet me look.\n\n▸ ✓    Ran ls\n```\n$ ls\na.rs\nb.rs\n```\n0.0s\n\
         Done.\n\nWorking for 0s\n\n> "
    );
    assert_eq!(shown(&mut h, 12)[4..7], ["▸ ✓   Ran ls", "0.0s", "Done."]);
    // Props stay in order, so the display can bisect them.
    let props: Vec<_> = h.core.current().properties().map(|(r, _)| r).collect();
    assert!(
        props
            .windows(2)
            .all(|w| w[0].start <= w[1].start && w[0].end <= w[1].end)
    );
    // The header carries the rule and its icon, the verb is dim and the target isn't.
    let frame = h.joined_frame(800., 600.);
    let header = &frame.windows[0].display.lines[4];
    let dim = FaceName::SessionThought;
    assert_eq!(header.decorations, [Decoration::Rule(dim)]);
    assert_eq!(header.icons, [(4, Icon::Terminal)]);
    let span = |text: &str| header.spans.iter().find(|s| s.text.contains(text)).unwrap();
    let theme = &h.core.settings().theme;
    assert_eq!(span("Ran").face.fg, theme.face(dim).fg);
    assert_ne!(span("ls").face.fg, theme.face(dim).fg);
}

#[test]
fn a_tool_call_without_content_is_its_header() {
    let mut h = session();
    tool(&mut h, ToolCall::new("t", "Think"));
    assert_eq!(h.core.text(), "▸ ●    Using Think\n\n> ");
    assert_eq!(shown(&mut h, 5)[0], "▸ ●   Using Think");
    // An update for an unknown call without a title shows nothing.
    let fields = ToolCallUpdateFields::new().status(ToolCallStatus::Failed);
    tool_update(&mut h, "u", fields.clone());
    assert_eq!(h.core.text(), "▸ ●    Using Think\n\n> ");
    // With a title it starts a block.
    tool_update(&mut h, "u", fields.title("Grep".to_string()));
    assert!(
        h.core
            .text()
            .ends_with("▸ ✗    Used Grep 0.0s\n1 failed\n> ")
    );
}

#[test]
fn tab_on_a_header_shows_and_hides_its_content() {
    let mut h = session();
    tool(&mut h, ls());
    h.keys("TAB");
    assert_eq!(h.core.message(), Some("No tool call here"));
    h.keys("M-< C-e TAB");
    let open = ["▾ ●   Running ls", "$ ls", "", "❯ "];
    assert_eq!(shown(&mut h, 10)[..4], open);
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    let rule = Decoration::Rule(FaceName::SessionThought);
    assert!(
        (0..2).all(|i| lines[i].decorations.contains(&rule)),
        "{lines:?}"
    );
    assert!(!lines[2].decorations.contains(&rule));
    // An update keeps it open.
    let output = ToolCallUpdateFields::new().raw_output(serde_json::json!("a.rs"));
    tool_update(&mut h, "t", output);
    assert_eq!(shown(&mut h, 12)[1..4], ["$ ls", "a.rs", ""]);
    // TAB inside the open content folds it, point going to the header.
    h.keys("C-n C-n C-n TAB");
    assert_eq!(shown(&mut h, 5)[..3], ["▸ ●   Running ls", "", "❯ "]);
    assert_eq!(h.core.point(), 0);
}

#[test]
fn line_motion_passes_hidden_fences() {
    let mut h = session();
    tool(&mut h, ls().raw_output(serde_json::json!("a.rs\nb.rs")));
    h.keys("M-< TAB");
    let row = |h: &mut Harness| {
        h.joined_frame(800., 600.).windows[0]
            .display
            .cursor
            .unwrap()
            .0
    };
    let mut rows = Vec::new();
    for _ in 0..3 {
        h.keys("C-n");
        rows.push(row(&mut h));
    }
    assert_eq!(rows, [1, 2, 3], "$ ls, a.rs, b.rs");
    h.keys("C-p C-p");
    assert_eq!(row(&mut h), 1);
    h.keys("C-p");
    assert_eq!(row(&mut h), 0);
}

#[test]
fn line_motion_and_scrolling_pass_a_folded_call() {
    let mut h = session();
    let output: String = (0..50).map(|i| format!("line {i}\n")).collect();
    tool(&mut h, ls().raw_output(serde_json::json!(output)));
    // The window keeps the header in view above the prompt line.
    assert_eq!(shown(&mut h, 5)[..3], ["▸ ●   Running ls", "", "❯ "]);
    h.keys("M-< C-n");
    assert_eq!(h.core.current().line_col(h.core.point()), Ok((54, 0)));
    h.keys("C-p");
    assert_eq!(h.core.point(), 0);
    h.keys("TAB C-n");
    // Past the hidden fence, onto `$ ls`.
    assert_eq!(h.core.current().line_col(h.core.point()), Ok((2, 0)));
    // Open, its first lines show and the rest is counted.
    let lines = shown(&mut h, 70);
    assert!((0..20).all(|i| lines.contains(&format!("line {i}"))));
    assert!(lines.contains(&"… 30 more lines".to_string()));
}

#[test]
fn a_repeated_tool_call_replaces_it_and_point_stays_on_its_header() {
    let mut h = session();
    tool(&mut h, ls());
    h.keys("M-< C-e TAB");
    tool(&mut h, ls().status(ToolCallStatus::Completed));
    assert_eq!(h.core.text().matches("Ran ls").count(), 1);
    assert_eq!(shown(&mut h, 6)[..2], ["▾ ✓   Ran ls", "$ ls"]);
    assert_eq!(h.core.current().line_col(h.core.point()).unwrap().0, 0);
    // TAB on a call without content leaves it folded.
    chunk(&mut h, "Next.");
    tool(&mut h, ToolCall::new("e", "Empty"));
    h.keys("M-> C-p C-p TAB");
    let content = ToolCallUpdateFields::new().raw_output(serde_json::json!("x"));
    tool_update(&mut h, "e", content);
    assert!(shown(&mut h, 20).contains(&"▸ ●   Using Empty".to_string()));
}

/// `ls` finished with `lines` lines of output, opened, point on its header.
fn long_ls(lines: usize) -> Harness {
    let mut h = session();
    let output: String = (1..=lines).map(|i| format!("line {i}\n")).collect();
    let call = ls().status(ToolCallStatus::Completed);
    tool(&mut h, call.raw_output(serde_json::json!(output)));
    h.keys("M-< C-e TAB");
    h
}

#[test]
fn headers_name_the_kind_and_its_target() {
    let header = |call: ToolCall| {
        let mut h = session();
        tool(&mut h, call);
        let lines = h.core.text();
        lines.lines().next().unwrap().to_string()
    };
    let at = |path: &str| vec![ToolCallLocation::new(path)];
    let read = ToolCall::new("t", "Read File").kind(ToolKind::Read);
    let path = read.clone().locations(at("/aiditor-test/app/src/a.rs"));
    assert_eq!(header(path.clone()), "▸ ●    Reading src/a.rs");
    let done = path.status(ToolCallStatus::Completed);
    assert_eq!(header(done), "▸ ✓    Read src/a.rs");
    // Outside the primary root the path stays whole, and without one the title shows.
    assert_eq!(
        header(read.clone().locations(at("/etc/hosts"))),
        "▸ ●    Reading /etc/hosts"
    );
    assert_eq!(header(read), "▸ ●    Reading Read File");
    // A diff's path stands in for a location.
    let diff = agent_client_protocol_schema::v1::Diff::new("/aiditor-test/app/a.rs", "b\n");
    let edit = ToolCall::new("t", "Editing files").kind(ToolKind::Edit);
    let edit = edit.content(vec![ToolCallContent::Diff(diff)]);
    assert_eq!(header(edit), "▸ ●    Editing a.rs +1 −0");
    // A search keeps its pattern, whatever it found.
    let search = ToolCall::new("t", "grep x").kind(ToolKind::Search);
    assert_eq!(
        header(search.locations(at("/x"))),
        "▸ ●    Searching grep x"
    );
    let fetch = ToolCall::new("t", "Fetch").kind(ToolKind::Fetch);
    let fetch = fetch.raw_input(serde_json::json!({"url": "https://x.dev"}));
    assert_eq!(header(fetch), "▸ ●    Fetching https://x.dev");
    let two = ToolCall::new("t", "`a`").kind(ToolKind::Execute);
    let two = two.raw_input(serde_json::json!({"command": "a\nb"}));
    assert_eq!(header(two.clone()), "▸ ●    Running a …");
    // Backticks in a title go; a pending delete still has its verb.
    assert_eq!(header(ToolCall::new("t", "`rm`")), "▸ ●    Using rm");
    let gone = ToolCall::new("t", "x").kind(ToolKind::Delete);
    assert_eq!(header(gone.locations(at("/y"))), "▸ ●    Deleting /y");
    let moved = ToolCall::new("t", "x").kind(ToolKind::Move);
    let moved = moved.status(ToolCallStatus::Failed).locations(at("/y"));
    assert_eq!(header(moved), "▸ ✗    Moved /y");
}

#[test]
fn each_kind_has_its_icon() {
    for (kind, icon) in [
        (ToolKind::Read, Icon::Book),
        (ToolKind::Search, Icon::Magnifier),
        (ToolKind::Edit, Icon::Pencil),
        (ToolKind::Delete, Icon::Pencil),
        (ToolKind::Move, Icon::Pencil),
        (ToolKind::Execute, Icon::Terminal),
        (ToolKind::Fetch, Icon::Globe),
        (ToolKind::Other, Icon::Wrench),
    ] {
        let mut h = session();
        tool(&mut h, ToolCall::new("t", "x").kind(kind));
        let frame = h.joined_frame(800., 600.);
        assert_eq!(frame.windows[0].display.lines[0].icons, [(4, icon)]);
    }
}

#[test]
fn a_failed_header_marks_its_status_and_exit() {
    let mut h = session();
    tool(&mut h, command());
    tool_update(
        &mut h,
        "t",
        ToolCallUpdateFields::new().status(ToolCallStatus::Failed),
    );
    let exit = serde_json::json!({
        "terminal_exit": {"terminal_id": "t", "exit_code": 2, "signal": null}
    });
    meta_update(&mut h, ToolCallUpdateFields::new(), exit);
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    let span = |text: &str| {
        let mut spans = lines.iter().flat_map(|l| &l.spans);
        spans.find(|s| s.text.contains(text)).unwrap()
    };
    let failed = h.core.settings().theme.face(FaceName::StatusFailed).fg;
    assert_eq!(span("✗").face.fg, failed);
    assert_eq!(span("exit 2").face.fg, failed);
    // Running and finished calls share the green status face.
    let mut h = session();
    tool(&mut h, ls());
    let face = |h: &mut Harness, mark: &str| {
        let frame = h.joined_frame(800., 600.);
        let line = &frame.windows[0].display.lines[0];
        (line.spans.iter())
            .find(|s| s.text.contains(mark))
            .unwrap()
            .face
            .fg
    };
    let theme = h.core.settings().theme.clone();
    assert_eq!(face(&mut h, "●"), theme.face(FaceName::StatusRunning).fg);
    tool(&mut h, ls().status(ToolCallStatus::Completed));
    assert_eq!(face(&mut h, "✓"), theme.face(FaceName::StatusRunning).fg);
}

#[test]
fn tab_changes_the_fold_marker_and_point_stays() {
    let mut h = session();
    tool(&mut h, ls());
    h.keys("M-<");
    h.keys("TAB");
    assert!(h.core.text().starts_with("▾ ●"));
    assert_eq!(h.core.point(), 0);
    h.keys("C-e TAB");
    assert!(h.core.text().starts_with("▸ ●"));
    assert_eq!(h.core.current().line_col(h.core.point()).unwrap().0, 0);
    assert!(h.core.point() > 0);
}

#[test]
fn an_open_command_shows_twenty_lines_and_counts_the_rest() {
    let mut h = long_ls(50);
    let lines = shown(&mut h, 30);
    assert_eq!(
        lines[..3],
        ["▾ ✓   Ran ls".to_string(), "$ ls".into(), "line 1".into()]
    );
    assert_eq!(
        (lines[2].as_str(), lines[21].as_str()),
        ("line 1", "line 20")
    );
    assert_eq!(lines[22], "… 30 more lines");
    assert_eq!(
        lines[23..25],
        ["0.0s", "❯ "],
        "no row for the closing fence"
    );
    // The rest stays in the text, and props stay in order for bisecting.
    assert!(h.core.text().contains("line 50\n"));
    assert!(h.core.text().contains("```\n$ ls\n") && h.core.text().contains("line 50\n```"));
    let props: Vec<_> = h.core.current().properties().map(|(r, _)| r).collect();
    assert!(
        props
            .windows(2)
            .all(|w| w[0].start <= w[1].start && w[0].end <= w[1].end)
    );
    // The count's line keeps the rule and the block's background.
    let frame = h.joined_frame(800., 700.);
    let more = &frame.windows[0].display.lines[22];
    assert!(
        more.decorations
            .contains(&Decoration::Rule(FaceName::SessionThought))
    );
    assert!(
        more.decorations
            .contains(&Decoration::Background(FaceName::SessionBlock))
    );
    let count = more.spans.iter().find(|s| s.text.starts_with('…')).unwrap();
    let annotation = h.core.settings().theme.face(FaceName::Annotation);
    assert_eq!(count.face.fg, annotation.fg);
    // Motion passes the count, and one line more reads singular.
    let mut h = long_ls(21);
    assert_eq!(shown(&mut h, 30)[22], "… 1 more line");
    h.keys("M-> C-p C-p C-p C-p");
    assert_eq!(h.core.current().line_col(h.core.point()).unwrap().0, 22);
}

#[test]
fn twenty_lines_show_whole() {
    let mut h = long_ls(20);
    let lines = shown(&mut h, 30);
    assert_eq!(lines[21], "line 20");
    assert!(!lines.iter().any(|l| l.contains("more line")));
}

#[test]
fn output_streams_past_the_cut_of_an_open_call() {
    let mut h = session();
    tool(
        &mut h,
        command().raw_input(serde_json::json!({"command": "make"})),
    );
    output(&mut h, "terminal_output", "t", "first\n");
    h.joined_frame(800., 600.);
    h.keys("M-< TAB");
    let more: String = (2..=25)
        .map(|i| format!("\u{1b}[31mline {i}\u{1b}[0m\n"))
        .collect();
    output(&mut h, "terminal_output", "t", &more);
    let lines = shown(&mut h, 30);
    assert_eq!(
        (lines[2].as_str(), lines[21].as_str()),
        ("first", "line 20")
    );
    assert_eq!(lines[22], "… 5 more lines");
    output(&mut h, "terminal_output", "t", "line 26\n");
    assert_eq!(shown(&mut h, 30)[22], "… 6 more lines");
    // A folded call's tail goes the short way and opens the same.
    h.keys("TAB");
    output(&mut h, "terminal_output", "t", "line 27\n");
    h.joined_frame(800., 600.);
    h.keys("TAB");
    assert_eq!(shown(&mut h, 30)[22], "… 7 more lines");
}

#[test]
fn appending_past_the_cut_matches_rendering_the_call_afresh() {
    let more: String = (2..=40)
        .map(|i| format!("\u{1b}[3{}mline {i}\u{1b}[0m plain\n", i % 7 + 1))
        .collect();
    for size in [1, 7, 64, 1000] {
        let (mut fast, mut fresh) = (session(), session());
        for h in [&mut fast, &mut fresh] {
            tool(h, command());
            output(h, "terminal_output", "t", "first\n");
            h.joined_frame(800., 600.);
            h.keys("M-< TAB");
        }
        for chunk in more.as_bytes().chunks(size) {
            let chunk = std::str::from_utf8(chunk).unwrap();
            for h in [&mut fast, &mut fresh] {
                output(h, "terminal_output_delta", "t", chunk);
                h.joined_frame(800., 600.);
            }
            let call = fresh.core.agents[&AgentId(0)].tools.values().next();
            fresh.core.show_tool(AgentId(0), call.unwrap().call.clone());
            assert_eq!(fast.core.text(), fresh.core.text(), "chunks of {size}");
            // A fold first rendered one char long sorts before the rule and indent at its
            // start, which it doesn't layer with, so only the set of properties is compared.
            let sorted = |h: &Harness| {
                let mut props: Vec<_> = props(h).iter().map(|p| format!("{p:?}")).collect();
                props.sort();
                props
            };
            assert_eq!(sorted(&fast), sorted(&fresh), "chunks of {size}");
        }
        assert_eq!(shown(&mut fast, 30)[22], "… 20 more lines");
    }
}

#[test]
fn backticks_in_a_command_and_its_output_stay_fenced() {
    let mut h = session();
    let call = command().raw_input(serde_json::json!({"command": "echo ````"}));
    tool(&mut h, call);
    output(&mut h, "terminal_output", "t", "ok\n");
    h.joined_frame(800., 600.);
    output(&mut h, "terminal_output", "t", "more\n");
    h.joined_frame(800., 600.);
    let text = h.core.text();
    assert!(
        text.contains("\n`````\n$ echo ````\nok\nmore\n`````\n"),
        "{text}"
    );
}

#[test]
fn other_kinds_stay_uncut_and_edits_show_their_diff() {
    let mut h = session();
    let output: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    let read = ToolCall::new("t", "x").kind(ToolKind::Read);
    tool(&mut h, read.raw_output(serde_json::json!(output)));
    h.keys("M-< TAB");
    let lines = shown(&mut h, 60);
    assert!(lines.contains(&"Output:".to_string()) && lines.contains(&"line 50".to_string()));
    let mut h = session();
    let diff = agent_client_protocol_schema::v1::Diff::new("a.rs", "b\nc\n");
    let edit = ToolCall::new("t", "Edit")
        .kind(ToolKind::Edit)
        .content(vec![ToolCallContent::Diff(
            diff.old_text("b\n".to_string()),
        )]);
    tool(
        &mut h,
        edit.raw_input(serde_json::json!({"file_path": "a.rs"})),
    );
    h.keys("M-< TAB");
    assert!(!h.core.text().contains("Input:"));
    let body = &shown(&mut h, 10)[1..];
    assert!(body.iter().any(|l| l.starts_with("@@")), "{body:?}");
    let header = |l: &&String| l.contains("a.rs") || l.starts_with("---") || l.starts_with("+++");
    assert!(!body.iter().any(|l| header(&l)), "{body:?}");
    let added = drawn(&mut h, "+c").fg;
    let theme = &h.core.settings().theme;
    assert_eq!(added, theme.face(FaceName::DiffAdded).fg);
    assert_eq!(
        drawn(&mut h, "@@ -1 +1,2 @@").fg,
        h.core.settings().theme.face(FaceName::DiffHunk).fg
    );
}

#[test]
fn edits_count_their_lines_and_commands_show_as_code() {
    let mut h = session();
    let diff = agent_client_protocol_schema::v1::Diff::new("a.rs", "1\n2\n3\n4\n5\n6\n");
    let edit = ToolCall::new("t", "Edit").kind(ToolKind::Edit);
    let edit = edit.content(vec![ToolCallContent::Diff(
        diff.old_text("x\n".to_string()),
    )]);
    tool(&mut h, edit);
    assert!(h.core.text().starts_with("▸ ●    Editing a.rs +6 −1\n"));
    let theme = h.core.settings().theme.clone();
    assert_eq!(
        drawn(&mut h, "+6").fg,
        theme.face(FaceName::StatusRunning).fg
    );
    assert_eq!(
        drawn(&mut h, "−1").fg,
        theme.face(FaceName::StatusFailed).fg
    );
    assert_eq!(drawn(&mut h, "+6").bg, None);
    let mut h = session();
    let ls = ToolCall::new("l", "`ls`").kind(ToolKind::Execute);
    tool(&mut h, ls.raw_input(serde_json::json!({"command": "ls"})));
    assert_eq!(drawn(&mut h, "ls").fg, theme.face(FaceName::SessionCode).fg);
}

/// A session whose project indexes `src/a.rs` and `src/main.rs`, `a.rs` three lines long, and
/// `lib/b.rs` in a second root.
fn indexed_session() -> Harness {
    let mut h = session();
    h.index_reply("app", &["src/a.rs", "src/main.rs"], &["app", "app/src"]);
    let other = crate::project::Root {
        path: h.path("other"),
    };
    h.core.projects[0].project.roots.push(other);
    h.index_reply("other", &["lib/b.rs"], &["other", "other/lib"]);
    h.read_reply("app/src/a.rs", Some(b"one\ntwo\nthree\n"));
    h.keys("M-x project-index RET home RET");
    h
}

/// Whether `text` draws as a link.
fn linked(h: &mut Harness, text: &str) -> bool {
    let link = h.core.settings().theme.face(FaceName::Link).fg;
    let face = drawn(h, text);
    face.underline && face.fg == link
}

/// Puts point `n` chars into the first `text` in the buffer.
fn point_in(h: &mut Harness, text: &str, n: usize) {
    let all = h.core.text();
    let at = all[..all.find(text).expect("in the buffer")]
        .chars()
        .count();
    h.ctx().goto(at + n).unwrap();
}

#[test]
fn existing_paths_in_messages_are_links() {
    let mut h = indexed_session();
    h.keys("a RET");
    chunk(&mut h, "see `src/a.rs:2`, `lib/b.rs`, not `src/gone.rs`");
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert!(linked(&mut h, "src/a.rs:2"));
    assert!(linked(&mut h, "lib/b.rs"), "a second root's file");
    assert!(!linked(&mut h, "src/gone.rs"));
}

/// A chip for `path`, a directory when `dir`, typed at point.
fn put_chip(h: &mut Harness, path: &str, dir: bool) -> char {
    let chip = h.core.chips.intern(Some(path.into()), dir);
    h.send(Msg::Insert {
        text: chip.to_string(),
        replace: None,
    });
    chip
}

/// The display line holding `text`.
fn drawn_line(h: &mut Harness, text: &str) -> crate::display::Line {
    let mut frame = h.joined_frame(800., 600.);
    let lines = std::mem::take(&mut frame.windows[0].display.lines);
    let line = lines.into_iter().find(|l| {
        let drawn: String = l.spans.iter().map(|s| s.text.as_str()).collect();
        drawn.contains(text)
    });
    line.unwrap_or_else(|| panic!("{text:?} drawn"))
}

#[test]
fn a_settled_turn_is_drawn_as_a_rule_however_it_ends() {
    let rule = Decoration::HRule(FaceName::Separator);
    for end in 0..4 {
        let mut h = session();
        h.keys("a RET");
        let working = drawn_line(&mut h, "Working for");
        assert!(!working.decorations.contains(&rule), "running: no rule");
        match end {
            0 => prompted(&mut h, Ok(StopReason::EndTurn)),
            1 => prompted(&mut h, Ok(StopReason::Cancelled)),
            2 => prompted(&mut h, Err("Internal error")),
            _ => exited(&mut h, 0, 1),
        }
        let worked = drawn_line(&mut h, "Worked");
        assert!(worked.decorations.contains(&rule), "ending {end}");
        assert_eq!(worked.indent(), 2);
        let face = h.core.settings().theme.face(FaceName::Annotation);
        assert_eq!(worked.spans[0].face.fg, face.fg);
    }
}

#[test]
fn a_chip_draws_as_its_icon_and_name_and_edits_as_one_char() {
    let mut h = session();
    h.keys("x");
    let chip = put_chip(&mut h, "/p/src/main.rs", false);
    h.keys("y");
    let line = drawn_line(&mut h, "x ▤ main.rs y");
    let attachment = h.core.settings().theme.face(FaceName::Attachment);
    let name = line.spans.iter().find(|s| s.text == " main.rs ").unwrap();
    assert_eq!(name.face.boxed, attachment.boxed);
    h.keys("C-b C-b C-f");
    assert_eq!(h.core.point(), 4, "past the chip");
    h.keys("DEL");
    assert_eq!(h.core.text(), "> xy");
    h.keys("C-/");
    assert_eq!(h.core.text(), format!("> x{chip}y"));
    drawn_line(&mut h, "x ▤ main.rs y");
    h.keys("C-e C-b C-b C-k C-y C-y");
    drawn_line(&mut h, "x ▤ main.rs y ▤ main.rs y");
}

#[test]
fn chips_go_as_resource_links_and_images_and_show_in_the_echo() {
    let mut h = session();
    h.keys("l o o k SPC");
    put_chip(&mut h, "/p/a.rs", false);
    put_chip(&mut h, "/p/src", true);
    put_chip(&mut h, "/p/Cat.JPG", false);
    h.keys("SPC o k RET");
    assert_eq!(
        ops(&mut h),
        [concat!(
            r#"prompt 0 s "look " link a.rs file:///p/a.rs link src file:///p/src/ "#,
            r#"image image/jpeg file:///p/Cat.JPG " ok""#
        )]
    );
    drawn_line(&mut h, "❯ look  ▤ a.rs  ▦ src  ▣ Cat.JPG  ok");
    // The agent service answers an unreadable image as the prompt's error.
    prompted(&mut h, Err("can't read /p/Cat.JPG: not found"));
    assert!(
        h.core
            .text()
            .contains("Error: can't read /p/Cat.JPG: not found\n\nWorked")
    );
}

#[test]
fn replayed_links_and_images_are_chips_in_one_message() {
    let mut h = session();
    let user = |block: ContentBlock| {
        let chunk = ContentChunk::new(block).message_id(Some("u1".into()));
        SessionUpdate::UserMessageChunk(chunk)
    };
    for text in ["look at ", "[@a.rs](file:///p/a.rs)", " please"] {
        update(&mut h, 0, user(text.into()));
    }
    update(
        &mut h,
        0,
        user(ContentBlock::Image(ImageContent::new("Zm9v", "image/png"))),
    );
    chunk(&mut h, "ok");
    let a = h.core.chips.intern(Some("/p/a.rs".into()), false);
    let image = h.core.chips.intern(None, false);
    assert_eq!(
        h.core.text(),
        format!("> look at {a} please{image}\n\nok\n\n> ")
    );
    assert_eq!(message_list(&h).len(), 2);
    drawn_line(&mut h, "❯ look at  ▤ a.rs  please ▣ image ");
}

#[test]
fn replayed_cli_mentions_of_indexed_files_are_chips() {
    let mut h = indexed_session();
    let user = |text: &str| SessionUpdate::UserMessageChunk(ContentChunk::new(text.into()));
    update(&mut h, 0, user("@src/a.rs vs @src/gone.rs"));
    let a = h.core.chips.intern(Some(h.path("app/src/a.rs")), false);
    assert!(h.core.text().starts_with(&format!("> {a} vs @src/gone.rs")));
    drawn_line(&mut h, "❯  ▤ a.rs  vs @src/gone.rs");
}

#[test]
fn word_motion_takes_a_chip_as_a_word_of_its_own() {
    let mut h = session();
    h.keys("a b SPC");
    let chip = put_chip(&mut h, "/p/a.rs", false);
    h.keys("c d");
    // `> ab <chip>cd`: the chip is at 5.
    h.keys("C-a M-f M-f");
    assert_eq!(h.core.point(), 6, "after the chip");
    h.keys("M-f");
    assert_eq!(h.core.point(), 8, "the word after it");
    h.keys("M-b M-b");
    assert_eq!(h.core.point(), 5, "back before the chip");
    h.keys("C-f M-DEL");
    assert_eq!(h.core.text(), "> ab cd", "it kills alone");
    h.keys("C-y");
    assert_eq!(h.core.text(), format!("> ab {chip}cd"), "and yanks back");
}

#[test]
fn ret_on_an_image_chip_opens_it_with_the_system_and_a_gone_file_echoes() {
    let mut h = session();
    put_chip(&mut h, "/p/cat.png", false);
    h.keys("C-b RET");
    let opened = (h.effects.iter())
        .any(|e| matches!(e, Effect::OpenWithSystem(p) if p == std::path::Path::new("/p/cat.png")));
    assert!(opened);
    assert_eq!(h.core.windows.tree.leaves().len(), 1, "no split");
    h.checks.insert("/p/gone.rs".into(), None);
    h.keys("C-e");
    put_chip(&mut h, "/p/gone.rs", false);
    h.keys("C-b RET");
    assert_eq!(h.core.message.as_deref(), Some("/p/gone.rs: no such file"));
    assert_eq!(h.core.windows.tree.leaves().len(), 1, "no split");
    assert!(ops(&mut h).is_empty(), "nothing is sent");
}

#[test]
fn ret_on_a_chip_visits_it_and_the_echo_shows_its_path() {
    let mut h = indexed_session();
    let a = h.path("app/src/a.rs");
    put_chip(&mut h, &a.to_string_lossy(), false);
    h.keys("C-b");
    assert_eq!(h.core.message.as_deref(), Some(&*a.to_string_lossy()));
    h.keys("RET");
    assert!(ops(&mut h).is_empty(), "nothing is sent");
    assert_eq!(h.core.windows.tree.leaves().len(), 2);
    assert_eq!(
        h.core.current().path().map(std::path::PathBuf::from),
        Some(a)
    );
}

#[test]
fn ret_on_a_directory_chip_opens_find_file_there_and_a_pathless_one_echoes() {
    let mut h = session();
    put_chip(&mut h, "/p/src", true);
    h.keys("C-b RET");
    assert_eq!(h.core.current().text().to_string(), "/p/src/");
    h.keys("C-g");
    let image = h.core.chips.intern(None, false);
    h.send(Msg::Insert {
        text: image.to_string(),
        replace: None,
    });
    h.keys("C-b RET");
    assert_eq!(h.core.message.as_deref(), Some("No file"));
    assert!(ops(&mut h).is_empty(), "nothing is sent");
}

#[test]
fn a_gone_file_is_struck_through_wherever_its_chip_shows() {
    let mut h = session();
    h.checks.insert("/p/gone.rs".into(), None);
    put_chip(&mut h, "/p/gone.rs", false);
    h.keys("RET");
    let theme = h.core.settings().theme.clone();
    let line = drawn_line(&mut h, "❯  ▤ gone.rs");
    let icon = line.spans.iter().find(|s| s.text == "▤").unwrap();
    assert_eq!(icon.face.fg, theme.face(FaceName::StatusFailed).fg);
    assert!(
        line.spans
            .iter()
            .find(|s| s.text == " gone.rs ")
            .unwrap()
            .face
            .strike
    );
    // Back on disk, as the watcher reports.
    h.checks.insert("/p/gone.rs".into(), Some(1));
    h.send(Msg::FilesChanged {
        paths: vec![("/p".into(), crate::Disk::Dir)],
        rescan: false,
    });
    let line = drawn_line(&mut h, "❯  ▤ gone.rs");
    assert!(
        !line
            .spans
            .iter()
            .find(|s| s.text == " gone.rs ")
            .unwrap()
            .face
            .strike
    );
}

#[test]
fn ret_on_a_message_link_visits_its_line_in_another_window() {
    let mut h = indexed_session();
    h.keys("a RET");
    chunk(&mut h, "see src/a.rs:3 here");
    prompted(&mut h, Ok(StopReason::EndTurn));
    h.joined_frame(800., 600.);
    ops(&mut h);
    point_in(&mut h, "src/a.rs:3", 2);
    h.keys("RET");
    assert!(ops(&mut h).is_empty(), "nothing is sent");
    assert_eq!(h.core.windows.tree.leaves().len(), 2);
    assert_eq!(
        h.core.current().path().map(std::path::PathBuf::from),
        Some(h.path("app/src/a.rs"))
    );
    assert_eq!(h.core.current().line_col(h.point()).unwrap(), (2, 0));
}

#[test]
fn ret_anywhere_on_a_file_link_label_visits_its_destination() {
    // Each message, the label text point goes to and where its char `n` is.
    let cases = [
        ("See [the spec](ABS/src/a.rs:3).", "the spec", 5),
        ("See [src/main.rs, line 2](src/a.rs:3).", ", line", 0),
        ("See [src/main.rs:2](./src/a.rs:3).", "main", 1),
        ("| l |\n|---|\n| [the spec](src/a.rs:3) |\n", "the spec", 6),
    ];
    for (message, label, n) in cases {
        let mut h = indexed_session();
        let message = message.replace("ABS", &h.path("app").to_string_lossy());
        h.keys("a RET");
        chunk(&mut h, &message);
        prompted(&mut h, Ok(StopReason::EndTurn));
        h.joined_frame(800., 600.);
        ops(&mut h);
        point_in(&mut h, label, n);
        h.keys("RET");
        assert!(ops(&mut h).is_empty(), "{message}: nothing is sent");
        assert_eq!(h.core.windows.tree.leaves().len(), 2, "{message}");
        assert_eq!(
            h.core.current().path().map(std::path::PathBuf::from),
            Some(h.path("app/src/a.rs")),
            "{message}"
        );
        let line = h.core.current().line_col(h.point()).unwrap();
        assert_eq!(line, (2, 0), "{message}");
    }
}

#[test]
fn ret_on_a_web_link_opens_it_in_the_browser() {
    let mut h = indexed_session();
    h.keys("a RET");
    chunk(
        &mut h,
        "See [docs](https://x.dev), <https://x.dev/a>, [run](file:///bin/sh) and src/a.rs:3.",
    );
    h.joined_frame(800., 600.);
    ops(&mut h);
    let opened = |h: &mut Harness| {
        let urls: Vec<_> = (h.effects.drain(..))
            .filter_map(|e| match e {
                Effect::OpenUrl(url) => Some(url),
                _ => None,
            })
            .collect();
        urls
    };
    point_in(&mut h, "docs", 1);
    h.keys("RET");
    assert_eq!(opened(&mut h), ["https://x.dev"]);
    point_in(&mut h, "x.dev/a", 1);
    h.keys("RET");
    assert_eq!(opened(&mut h), ["https://x.dev/a"]);
    point_in(&mut h, "run", 1);
    h.keys("RET");
    assert!(opened(&mut h).is_empty());
    assert_eq!(
        h.core.message.as_deref(),
        Some("Not a web link: file:///bin/sh")
    );
    assert!(ops(&mut h).is_empty(), "nothing is sent, and no busy echo");
    prompted(&mut h, Ok(StopReason::EndTurn));
    chunk(&mut h, "now https://y.dev/b.");
    h.joined_frame(800., 600.);
    assert!(linked(&mut h, "https://y.dev/b"));
    point_in(&mut h, "y.dev/b", 1);
    h.keys("RET");
    assert_eq!(opened(&mut h), ["https://y.dev/b"]);
    assert_eq!(h.core.windows.tree.leaves().len(), 1, "no split");
    point_in(&mut h, "src/a.rs:3", 2);
    h.keys("RET");
    assert_eq!(
        h.core.current().path().map(std::path::PathBuf::from),
        Some(h.path("app/src/a.rs")),
        "files still visit"
    );
}

fn blocks(h: &mut Harness) -> Vec<String> {
    h.core.frame();
    let session = &h.core.agents[&AgentId(0)];
    session.block_texts(h.core.buffers.get(session.buffer).unwrap())
}

#[test]
fn block_ends_follow_streaming_rewrites_and_meta_lines() {
    let mut h = session();
    h.clock("14:01:05");
    h.keys("a RET");
    agent_chunk(&mut h, "m", "Hello");
    assert_eq!(blocks(&mut h), ["> a", "Hello", "Working for 0s"]);
    agent_chunk(&mut h, "m", " **world**\n\n");
    thought(&mut h, "hmm");
    assert_eq!(
        blocks(&mut h),
        ["> a", "Hello **world**", "hmm", "Working for 0s"]
    );
    tool(&mut h, ls());
    h.advance(secs(3));
    assert_eq!(
        blocks(&mut h),
        [
            "> a",
            "Hello **world**",
            "Thought for 0s\nhmm",
            "▸ ●    Running ls\n```\n$ ls\n```",
            "Working for 3s"
        ]
    );
    let done = ToolCallUpdateFields::new()
        .status(ToolCallStatus::Completed)
        .raw_output(serde_json::json!("a.txt\nb.txt"));
    tool_update(&mut h, "t", done);
    let ran = "▸ ✓    Ran ls\n```\n$ ls\na.txt\nb.txt\n```";
    assert!(
        h.core.text().contains(&format!("{ran}\n3.0s\n")),
        "its time under it"
    );
    let mut want = [
        "> a",
        "Hello **world**",
        "Thought for 0s\nhmm",
        ran,
        "Working for 3s",
    ];
    assert_eq!(blocks(&mut h), want, "rewritten in place");
    on_line(&mut h, "▸ ✓");
    h.keys("TAB");
    let open = ran.replace('▸', "▾");
    want[3] = &open;
    assert_eq!(blocks(&mut h), want, "opened");
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert!(h.core.text().contains("\nWorked 3s 14:01:08\n"));
    assert_eq!(blocks(&mut h), want[..4], "the turn ended");
}

#[test]
fn block_ends_follow_asks_answers_waiting_counters_and_plans() {
    use PlanEntryPriority::*;
    use PlanEntryStatus::*;
    let mut h = session();
    h.keys("a RET");
    tool(&mut h, ls());
    ask(&mut h, 1, "t", None);
    h.advance(secs(2));
    h.core.frame();
    let running = "▸ ●    Running ls\n```\n$ ls\n```";
    assert!(
        h.core
            .text()
            .contains(&format!("{running}\nwaiting 0:02\n"))
    );
    let permission = asked("`ls`");
    assert_eq!(
        blocks(&mut h),
        ["> a", running, &permission, "Working for 2s"],
        "the waiting counter goes under the call"
    );
    h.keys("C-c C-y");
    assert!(
        h.core
            .text()
            .contains(&format!("{running}\n\n✓ allowed once\n\n"))
    );
    assert_eq!(
        blocks(&mut h),
        ["> a", running, "Working for 2s"],
        "the counter went"
    );
    question(&mut h, 2, None);
    let asking = " Question  Which?\n   1   A  First\n   2   B\n   o   Own answer\n  1-2 answer   RET answer with the row at point   C-c C-k skip";
    assert_eq!(
        blocks(&mut h).last().map(String::as_str),
        Some("Working for 2s")
    );
    assert_eq!(blocks(&mut h).iter().rev().nth(1).unwrap(), asking);
    on_line(&mut h, "Question");
    h.keys("2");
    let answered = "▸ ✓ Which?  B\n question_0  B";
    assert_eq!(
        blocks(&mut h).iter().rev().nth(1).unwrap(),
        answered,
        "settled with its rows"
    );
    on_line(&mut h, "▸ ✓ Which?");
    h.keys("TAB");
    let open = answered.replace('▸', "▾");
    assert_eq!(blocks(&mut h).iter().rev().nth(1).unwrap(), &open, "opened");
    plan(&mut h, &[("Read the code", InProgress, High)]);
    plan(
        &mut h,
        &[("Read", Completed, High), ("Write tests", Pending, Medium)],
    );
    let plan = "Plan\n  ✓ Read\n  ○ Write tests";
    assert_eq!(
        blocks(&mut h).iter().rev().nth(1).unwrap(),
        plan,
        "replaced in place"
    );

    let mut h = grouped();
    ask(&mut h, 3, "a", None);
    h.advance(secs(2));
    let read = |file, time| format!("  ▸ ✓    Read {file}.rs {time}\nOutput:\n```\ntext\n```");
    let (a, b) = (read("a", "waiting 0:02"), read("b", "0.0s"));
    assert_eq!(
        blocks(&mut h),
        ["> a", &a, &b, &asked("Read a.rs"), "Working for 2s"],
        "a grouped call's counter is in its header, the group's time under its rows"
    );
}

#[test]
fn block_at_matches_a_scan_of_block_ranges_for_every_kind() {
    use PlanEntryPriority::*;
    use PlanEntryStatus::*;
    let mut h = session();
    h.keys("a RET");
    agent_chunk(&mut h, "m", "Hello");
    prompted(&mut h, Ok(StopReason::MaxTokens));
    h.keys("b RET");
    plan(&mut h, &[("Read the code", InProgress, High)]);
    tool(&mut h, read("a", "a.rs"));
    thought(&mut h, "hmm\nmore");
    tool(&mut h, read("b", "b.rs"));
    question(&mut h, 1, None);
    on_line(&mut h, "Question");
    h.keys("2");
    ask(&mut h, 2, "a", None);
    several(&mut h, 3);
    on_line(&mut h, "▸    Read");
    h.keys("TAB");
    on_line(&mut h, "▸ Thought");
    h.keys("TAB");
    h.core.frame();
    let session = &h.core.agents[&AgentId(0)];
    let b = h.core.buffers.get(session.buffer).unwrap();
    let kinds: HashSet<_> = (session.all())
        .map(|(_, block)| std::mem::discriminant(&block.kind))
        .collect();
    assert_eq!(kinds.len(), 9, "a block of every kind");
    let mut found = HashSet::new();
    for point in 0..=b.len_chars() {
        // The innermost block holding `point`, a group's rows coming after it.
        let scanned = (session.all())
            .filter(|(_, block)| {
                let range = block.range(b);
                (range.start..=range.end).contains(&point)
            })
            .map(|(path, _)| path)
            .last();
        assert_eq!(session.path_at(b, point), scanned, "at {point}");
        let want = scanned.and_then(|path| match path {
            (g, None) if matches!(session.blocks[g].kind, BlockKind::Group(_)) => {
                Some(At::Group(g))
            }
            (g, Some(_)) => session.target(b, path, point).or(Some(At::Group(g))),
            _ => session.target(b, path, point),
        });
        let at = session.block_at(b, point);
        assert_eq!(at, want, "at {point}");
        found.extend(at.map(|at| std::mem::discriminant(&at)));
    }
    assert_eq!(
        found.len(),
        5,
        "an ask, an answer, a call, a thought and a group"
    );
}

#[test]
fn a_message_streamed_in_any_split_renders_as_a_whole() {
    let source = "# Title é\n\n- one\n- two\n  more\n- three\n\n```rust\nfn a() {}\n\nb\n```\n| k | v |\n|---|--:|\n| a | 1 |\n| bb | 22 |\n\nSetext\n===\n\nlast *words*";
    let chars: Vec<char> = source.chars().collect();
    for size in [1, 2, 5, 13] {
        for first in 1..=size {
            let mut h = session();
            let mut sent = String::new();
            let mut from = 0;
            while from < chars.len() {
                let to = (if from == 0 { first } else { from + size }).min(chars.len());
                let text: String = chars[from..to].iter().collect();
                chunk(&mut h, &text);
                sent.push_str(&text);
                h.joined_frame(800., 600.);
                let (message, _) = h.core.agents[&AgentId(0)].messages().next().unwrap();
                let b = h.core.current();
                check_message(b, message.range(b), &sent, None, 100);
                from = to;
            }
            let stream = h.core.agents[&AgentId(0)].stream.as_ref().unwrap();
            assert_eq!(
                &source[stream.stable.0..],
                "last *words*",
                "only the tail renders"
            );
        }
    }
}

#[test]
fn tables_are_drawn_into_the_text() {
    let mut h = session();
    chunk(&mut h, "| a | b |\n|---|--:|\n");
    let frame = h.joined_frame(800., 600.);
    let line = &frame.windows[0].display.lines[0];
    let row = Decoration::Table {
        edges: vec![0, 3, 6],
        first: true,
        last: true,
    };
    assert!(line.decorations.contains(&row), "{:?}", line.decorations);
    assert!(h.core.text().starts_with(" a  b \n"), "the header alone");
    chunk(&mut h, "| x | 10 |\n");
    h.joined_frame(800., 600.);
    assert!(h.core.text().starts_with(" a   b \n x  10 \n"), "widened");
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert!(
        h.core.text().starts_with(" a   b \n x  10 \n"),
        "{:?}",
        h.core.text()
    );
    let (message, _) = h.core.agents[&AgentId(0)].messages().next().unwrap();
    let b = h.core.current();
    let text = b.text().slice(message.range(b)).to_string();
    assert!(
        text.starts_with(" a   b \n x  10"),
        "its length follows: {text:?}"
    );
    h.keys("M-< C-SPC C-n C-n M-w");
    assert_eq!(h.clipboard.as_deref(), Some(" a   b \n x  10 \n"));
    h.keys("C-g C-s - - -");
    let found = listed(&mut h).0;
    assert!(
        found.is_empty(),
        "no markdown to find: {found:?} in {:?}",
        h.core.text()
    );
    h.keys("C-g C-s x SPC SPC 1 0");
    assert_eq!(listed(&mut h).0, ["2  x  10 "]);
}

#[test]
fn tables_wrap_at_the_table_width_setting() {
    let mut h = session();
    h.send(Msg::SettingsLoaded(Ok(Some("table_width = 10".into()))));
    chunk(&mut h, "| a | b |\n|---|---|\n| one two | x |\n");
    h.joined_frame(800., 600.);
    assert!(
        (h.core.text()).starts_with(" a      b \n one    x \n two      \n"),
        "{:?}",
        h.core.text()
    );
}

#[test]
fn a_message_time_goes_under_its_table_rows() {
    let mut h = session();
    h.clock("14:01:00");
    h.keys("a RET");
    chunk(&mut h, "| a |\n|---|\n| b |\n\nAfter.");
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert!(
        h.core.text().contains(" a \n b \n\nAfter.\n14:01:00\n"),
        "{:?}",
        h.core.text()
    );
    h.keys("a RET");
    chunk(&mut h, "| c |\n|---|\n| d |\n");
    prompted(&mut h, Ok(StopReason::EndTurn));
    let text = h.core.text();
    assert!(
        text.contains("\n c \n d \n14:01:00\nWorked"),
        "a table alone has its time too: {text:?}"
    );
}

#[test]
fn wide_tables_are_cut_and_scroll_as_one() {
    let mut h = session();
    // Under the cap, so only the window cuts it.
    h.core.settings.table_width = 1000;
    let wide = "w".repeat(150);
    let prose = "word ".repeat(40);
    chunk(
        &mut h,
        &format!("| a | {wide} |\n|---|---|\n| b | x |\n| c | y |\n\n{prose}"),
    );
    h.joined_frame(800., 600.);
    let cols = h.core.windows.window.cols();
    let text =
        |l: &crate::display::Line| -> String { l.spans.iter().map(|s| s.text.as_str()).collect() };
    let edges = |l: &crate::display::Line| {
        (l.decorations.iter()).find_map(|d| match d {
            Decoration::Table { edges, .. } => Some(edges.clone()),
            _ => None,
        })
    };
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    assert_eq!(
        text(&lines[0]).chars().count(),
        cols + 1,
        "cut, one more for a part column"
    );
    assert!(
        text(&lines[1]).starts_with(" b "),
        "no wrapped rows: {:?}",
        text(&lines[1])
    );
    assert!(text(&lines[2]).starts_with(" c "));
    assert!(text(&lines[4]).starts_with("word") && text(&lines[5]).starts_with("word"));
    assert_eq!(edges(&lines[0]), Some(vec![0, 3, 155]));
    h.keys("M-< C-e");
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    let left = 156 - cols as isize;
    assert_eq!(frame.windows[0].display.cursor, Some((0, cols - 1)));
    for line in &lines[..3] {
        assert_eq!(
            edges(line),
            Some(vec![-left, 3 - left, 155 - left]),
            "scrolled as one"
        );
    }
    assert!(
        text(&lines[4]).starts_with("word"),
        "prose stays at column 0"
    );
    h.keys("C-n C-n C-n");
    let frame = h.joined_frame(800., 600.);
    let lines = &frame.windows[0].display.lines;
    assert_eq!(frame.windows[0].display.cursor.map(|(l, _)| l), Some(3));
    assert_eq!(edges(&lines[0]), Some(vec![0, 3, 155]), "back to column 0");
}

#[test]
fn ret_on_a_link_in_a_table_opens_its_destination() {
    let mut h = indexed_session();
    h.keys("a RET");
    chunk(
        &mut h,
        "| l |\n|---|\n| [docs](https://x.dev) |\n| [sh](file:///bin/sh) |\n",
    );
    h.joined_frame(800., 600.);
    ops(&mut h);
    point_in(&mut h, " docs ", 2);
    h.keys("RET");
    let opened: Vec<_> = (h.effects.drain(..))
        .filter_map(|e| match e {
            Effect::OpenUrl(url) => Some(url),
            _ => None,
        })
        .collect();
    assert_eq!(opened, ["https://x.dev"]);
    point_in(&mut h, " sh ", 1);
    h.keys("RET");
    assert!(!h.effects.iter().any(|e| matches!(e, Effect::OpenUrl(_))));
    assert_eq!(
        h.core.message.as_deref(),
        Some("Not a web link: file:///bin/sh")
    );
    assert!(ops(&mut h).is_empty(), "nothing is sent");
}

#[test]
fn ret_on_a_url_in_the_prompt_sends_it() {
    let mut h = session();
    h.keys("https://x.dev RET");
    assert!(!h.effects.iter().any(|e| matches!(e, Effect::OpenUrl(_))));
    assert_eq!(ops(&mut h).len(), 1, "the prompt is sent");
}

#[test]
fn tool_headers_link_indexed_files_and_ret_visits_them() {
    let mut h = indexed_session();
    let read = |id: &str, path: PathBuf| {
        let location = ToolCallLocation::new(path).line(2);
        ToolCall::new(id.to_string(), "Read")
            .kind(ToolKind::Read)
            .locations(vec![location])
    };
    let a = h.path("app/src/a.rs");
    // Two calls would fold into a group.
    let mut outside = indexed_session();
    tool(&mut outside, read("u", "/etc/hosts".into()));
    // Plain, it runs on from the space before it.
    assert!(!linked(&mut outside, " /etc/hosts"));
    tool(&mut h, read("t", a));
    assert!(linked(&mut h, "src/a.rs"));
    // Off the link, the header still opens the output.
    point_in(&mut h, "Reading src/a.rs", 1);
    h.keys("RET");
    assert_eq!(h.core.message.as_deref(), Some("No output"));
    point_in(&mut h, "src/a.rs", 1);
    h.keys("RET");
    assert_eq!(
        h.core.current().path().map(std::path::PathBuf::from),
        Some(h.path("app/src/a.rs"))
    );
    assert_eq!(h.core.current().line_col(h.point()).unwrap(), (1, 0));
}

#[test]
fn ret_on_a_header_opens_the_full_output() {
    let other = |h: &Harness| h.core.windows.others.values().next().map(|w| w.buffer());
    let mut h = session();
    let call = command().raw_input(serde_json::json!({"command": "make"}));
    tool(&mut h, call);
    output(&mut h, "terminal_output", "t", "\u{1b}[31mred\u{1b}[0m\n");
    h.joined_frame(800., 600.);
    ops(&mut h);
    h.keys("x M-< RET");
    assert!(ops(&mut h).is_empty(), "nothing is sent");
    let id = other(&h).expect("shown in another window");
    let buffer = h.core.buffers.get(id).unwrap();
    assert!(buffer.read_only);
    assert_eq!(buffer.name, "*claude output: make*<work>");
    assert_eq!(buffer.text().to_string(), "red\n");
    assert_eq!(buffer.properties().count(), 1);
    // RET again refreshes that buffer.
    output(&mut h, "terminal_output", "t", "more\n");
    h.joined_frame(800., 600.);
    h.keys("RET");
    let text = h.core.buffers.get(id).unwrap().text().to_string();
    assert_eq!((other(&h), text.as_str()), (Some(id), "red\nmore\n"));
    // A killed one comes back.
    h.keys("C-x o C-x k RET C-x o");
    assert!(h.core.buffers.get(id).is_none());
    h.keys("M-< RET");
    let again = other(&h).unwrap();
    assert_ne!(again, id);
    assert_eq!(
        h.core.buffers.get(again).unwrap().text().to_string(),
        "red\nmore\n"
    );
}

#[test]
fn ret_on_a_header_works_during_a_turn_and_after_the_session_ended() {
    let mut h = session();
    h.keys("a RET");
    tool(&mut h, ls().raw_output(serde_json::json!("x")));
    h.keys("M-< C-n C-n RET");
    assert_eq!(h.core.message(), None);
    let other = |h: &Harness| h.core.windows.others.values().next().map(|w| w.buffer());
    let id = other(&h).expect("shown during the turn");
    h.keys("C-x 1");
    exited(&mut h, 0, 0);
    h.keys("RET");
    assert_eq!(other(&h), Some(id));
}

#[test]
fn ret_on_a_call_without_output_echoes() {
    let mut h = session();
    tool(&mut h, ToolCall::new("t", "Think"));
    h.keys("M-< RET");
    assert_eq!(h.core.message(), Some("No output"));
}

#[test]
fn full_output_prefers_diffs_then_raw_output_then_text() {
    let diff = agent_client_protocol_schema::v1::Diff::new("a", "b\n");
    let edit = ToolCall::new("t", "Edit").content(vec![ToolCallContent::Diff(diff)]);
    let (text, faces) = tool_output(&edit, None).unwrap();
    assert!(
        text.starts_with("--- a\n+++ a\n@@") && text.contains("+b"),
        "{text}"
    );
    assert!(!faces.is_empty());
    let raw = ToolCall::new("t", "x")
        .content(vec!["seen".into()])
        .raw_output(serde_json::json!("all"));
    assert_eq!(tool_output(&raw, None).unwrap().0, "all");
    let item = ToolCall::new("t", "x").content(vec!["seen".into()]);
    assert_eq!(tool_output(&item, None).unwrap().0, "seen\n");
    assert!(tool_output(&ToolCall::new("t", "x"), None).is_none());
}

#[test]
fn tool_content_is_uncut_in_long_enough_fences() {
    let mut call = ToolCall::new("t", "Run\nmore");
    assert_eq!(
        tool_header(&call, None, false, "/x".as_ref()),
        "▸ ●    Using Run …"
    );
    // A list of text blocks shows as their text; other output as JSON.
    let content = |call: &ToolCall| tool_content(call, None).text;
    call.raw_output = Some(serde_json::json!([{"type": "text", "text": "x ``` y"}]));
    assert_eq!(content(&call), "Output:\n````\nx ``` y\n````");
    call.raw_output = Some(serde_json::json!({"code": 0}));
    assert_eq!(content(&call), "Output:\n```\n{\n  \"code\": 0\n}\n```");
    // Content items replace the raw output.
    call.content = vec![
        "**hi**\u{1b}[0m".into(),
        ToolCallContent::Diff(
            agent_client_protocol_schema::v1::Diff::new("a.rs", "b\n").old_text("a\n".to_string()),
        ),
    ];
    assert_eq!(
        content(&call),
        "**hi**\n\n```diff\n@@ -1 +1 @@\n-a\n+b\n```"
    );
    // A coloured text item's faces land on its chars after the parts before it.
    call.raw_input = Some(serde_json::json!(1));
    call.content = vec!["\u{1b}[34mfile\u{1b}[0m".into()];
    let blue = vec![FaceRef::Fg(4)];
    let content = tool_content(&call, None);
    assert_eq!(content.text, "Input:\n```json\n1\n```\n\nfile");
    assert_eq!(content.faces, vec![(22..26, blue)]);
}

#[test]
fn sgr_escapes_become_faces() {
    use FaceRef::{Bg, Bold, Fg, Underline};
    assert_eq!(
        coloured("\u{1b}[1;4;92mok\u{1b}[22mx\u{1b}[0my"),
        (
            "okxy".into(),
            vec![
                (0..2, vec![Bold, Underline, Fg(10)]),
                (2..3, vec![Underline, Fg(10)])
            ]
        )
    );
    // Colours beyond the 16 show in the default colour.
    let beyond = "\u{1b}[38;5;196ma\u{1b}[38;2;1;2;3mb\u{1b}[38;5;4mc";
    assert_eq!(coloured(beyond), ("abc".into(), vec![(2..3, vec![Fg(4)])]));
    // A face stops at each line's end; `39` and `49` reset the colours.
    assert_eq!(
        coloured("\u{1b}[48;5;4ma\u{1b}[101mb"),
        ("ab".into(), vec![(0..1, vec![Bg(4)]), (1..2, vec![Bg(9)])])
    );
    assert_eq!(
        coloured("\u{1b}[32;41ma\r\nb\u{1b}[39;49mc\u{1b}]0;title\u{7}"),
        (
            "a\nbc".into(),
            vec![(0..1, vec![Fg(2), Bg(1)]), (2..3, vec![Fg(2), Bg(1)])]
        )
    );
}

/// An `execute` call `t` titled `make`, announcing terminal `t` in its content and `_meta`.
fn command() -> ToolCall {
    serde_json::from_value(serde_json::json!({
        "toolCallId": "t", "title": "make", "kind": "execute", "status": "in_progress",
        "content": [{"type": "terminal", "terminalId": "t"}],
        "_meta": {"terminal_info": {"terminal_id": "t"}},
    }))
    .unwrap()
}

/// Sends call `t` an update with `fields` and `meta`.
fn meta_update(h: &mut Harness, fields: ToolCallUpdateFields, meta: Value) {
    let update = ToolCallUpdate::new("t", fields).meta(meta.as_object().cloned());
    self::update(h, 0, SessionUpdate::ToolCallUpdate(update));
}

fn output(h: &mut Harness, key: &str, terminal: &str, data: &str) {
    let meta = serde_json::json!({key: {"terminal_id": terminal, "data": data}});
    meta_update(h, ToolCallUpdateFields::new(), meta);
}

#[test]
fn terminal_output_streams_into_its_call_once_per_frame() {
    let mut h = session();
    tool(&mut h, command());
    assert_eq!(h.core.text(), "▸ ●    Running make\n\n> ");
    output(&mut h, "terminal_output_delta", "t", "\u{1b}[31mre");
    output(&mut h, "terminal_output", "t", "d\u{1b}[0m\r\n");
    output(&mut h, "terminal_output", "other", "lost");
    assert_eq!(h.core.text(), "▸ ●    Running make\n\n> ");
    h.joined_frame(800., 600.);
    assert_eq!(
        h.core.text(),
        "▸ ●    Running make\nOutput:\n```\nred\n```\n\n> "
    );
    // Its colour shows once the call is open.
    h.keys("M-< C-e TAB");
    let red = h.core.settings().theme.ansi[1];
    assert_eq!(drawn(&mut h, "red").fg, Some(red));
}

#[test]
fn folded_and_open_output_agree_across_chunk_boundaries() {
    let (mut folded, mut open) = (session(), session());
    for h in [&mut folded, &mut open] {
        tool(h, command());
        output(h, "terminal_output", "t", "initial\n");
        h.joined_frame(800., 600.);
    }
    open.keys("M-< C-e TAB");
    for text in [
        "\u{1b}[3",
        "1mred",
        "der\u{1b}[0m\n",
        "```",
        "`été\n",
        "tail\n",
    ] {
        for h in [&mut folded, &mut open] {
            output(h, "terminal_output_delta", "t", text);
            h.joined_frame(800., 600.);
        }
        // Past the fold marker, the one difference.
        let text = |h: &Harness| h.core.text().chars().skip(1).collect::<String>();
        assert_eq!(text(&folded), text(&open));
        let faces = |h: &Harness| {
            h.core.agents[&AgentId(0)]
                .tools
                .values()
                .next()
                .unwrap()
                .faces
                .clone()
        };
        assert_eq!(faces(&folded), faces(&open));
    }
    folded.keys("M-< C-e TAB");
    assert_eq!(shown(&mut folded, 20), shown(&mut open, 20));
    let red = folded.core.settings.theme.ansi[1];
    assert_eq!(drawn(&mut folded, "redder").fg, Some(red));
}

#[test]
fn hidden_tool_output_waits_for_its_session_to_be_shown() {
    for workspace in [false, true] {
        let mut h = session();
        let work = h.core.workspace().unwrap();
        let buffer = h.core.windows.window.buffer();
        tool(&mut h, command());
        output(&mut h, "terminal_output", "t", "initial\n");
        h.joined_frame(800., 600.);
        let before = h.core.buffers.get(buffer).unwrap().text().to_string();
        if workspace {
            h.new_workspace("other", "home");
        } else {
            h.open("other.txt");
        }
        output(&mut h, "terminal_output_delta", "t", "later\n");
        h.joined_frame(800., 600.);
        assert_eq!(
            h.core.buffers.get(buffer).unwrap().text().to_string(),
            before
        );
        h.core.switch_workspace(work).unwrap();
        h.core.switch_to(buffer);
        h.joined_frame(800., 600.);
        assert!(h.core.text().contains("initial\nlater\n```"));
    }
}

#[test]
fn a_coloured_text_item_shows_its_colour_when_open() {
    let mut h = session();
    let call = ToolCall::new("x", "ls").content(vec!["\u{1b}[34mfile\u{1b}[0m".into()]);
    tool(&mut h, call);
    h.keys("M-< C-e TAB");
    assert_eq!(h.core.text(), "▾ ●    Using ls\nfile\n\n> ");
    let blue = h.core.settings().theme.ansi[4];
    assert_eq!(drawn(&mut h, "file").fg, Some(blue));
}

#[test]
fn a_theme_change_redraws_command_output() {
    let mut h = session();
    tool(&mut h, command());
    output(&mut h, "terminal_output", "t", "\u{1b}[31mred");
    h.joined_frame(800., 600.);
    h.keys("M-< C-e TAB");
    let ansi = (1..=16)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",");
    h.send(Msg::SettingsLoaded(Ok(Some(format!(
        "[themes.default]\nansi = [{ansi}]"
    )))));
    assert_eq!(drawn(&mut h, "red").fg, Some(2));
    // Folding and opening the call again keeps the new colour.
    h.keys("TAB M-< C-e TAB");
    assert_eq!(drawn(&mut h, "red").fg, Some(2));
}

#[test]
fn a_failed_command_shows_its_exit_in_the_header() {
    let mut h = session();
    tool(&mut h, command());
    // The header, and the time on the line under the block, above the prompt.
    let header = |h: &mut Harness| {
        h.joined_frame(800., 600.);
        let text = h.core.text();
        let first = text.lines().next().unwrap();
        format!("{first} | {}", text.lines().rev().nth(1).unwrap())
    };
    let exit = |code: Value, signal: Value| {
        serde_json::json!({"terminal_exit":
            {"terminal_id": "t", "exit_code": code, "signal": signal}})
    };
    let failed = ToolCallUpdateFields::new().status(ToolCallStatus::Failed);
    meta_update(&mut h, failed, exit(2.into(), Value::Null));
    assert_eq!(header(&mut h), "▸ ✗    Ran make | exit 2");
    meta_update(
        &mut h,
        ToolCallUpdateFields::new(),
        exit(Value::Null, "SIGTERM".into()),
    );
    assert_eq!(header(&mut h), "▸ ✗    Ran make | signal SIGTERM");
    let completed = ToolCallUpdateFields::new().status(ToolCallStatus::Completed);
    meta_update(&mut h, completed, exit(0.into(), Value::Null));
    assert_eq!(header(&mut h), "▸ ✓    Ran make | 0.0s");
}

#[test]
fn a_call_started_from_an_update_opens_its_terminal() {
    let mut h = session();
    let meta = serde_json::json!({"terminal_info": {"terminal_id": "t"}});
    meta_update(&mut h, ToolCallUpdateFields::new().title("make"), meta);
    output(&mut h, "terminal_output", "t", "ok\n");
    h.joined_frame(800., 600.);
    assert_eq!(
        h.core.text(),
        "▸ ●    Using make\nOutput:\n```\nok\n```\n\n> "
    );
}

#[test]
fn terminal_info_alone_opens_a_terminal() {
    let mut h = session();
    let mut call = command();
    call.content.clear();
    tool(&mut h, call);
    output(&mut h, "terminal_output", "t", "ok\n");
    h.joined_frame(800., 600.);
    assert_eq!(
        h.core.text(),
        "▸ ●    Running make\nOutput:\n```\nok\n```\n\n> "
    );
}

fn plan(h: &mut Harness, entries: &[(&str, PlanEntryStatus, PlanEntryPriority)]) {
    let entries = (entries.iter())
        .map(|(content, status, priority)| {
            PlanEntry::new(*content, priority.clone(), status.clone())
        })
        .collect();
    update(h, 0, SessionUpdate::Plan(Plan::new(entries)));
}

#[test]
fn a_new_plan_replaces_the_old_one_in_place() {
    use PlanEntryPriority::*;
    use PlanEntryStatus::*;
    let mut h = session();
    chunk(&mut h, "Planning.");
    plan(
        &mut h,
        &[
            ("Read the code", InProgress, High),
            ("Write tests", Pending, Medium),
        ],
    );
    let first = "Plan\n  ● Read the code\n  ○ Write tests";
    assert_eq!(h.core.text(), format!("Planning.\n\n{first}\n\n> "));
    let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
    let waiting = Prop::Named(vec![FaceRef::Face(FaceName::StatusWaiting)]);
    let faces = |h: &Harness| {
        (props(h).into_iter())
            .filter(|(t, _)| t != "Planning." && t != "Done.")
            .collect::<Vec<_>>()
    };
    assert_eq!(
        faces(&h),
        [
            ("Plan".into(), dim.clone()),
            ("●".into(), waiting.clone()),
            ("○ Write tests".into(), dim.clone()),
        ],
        "no rule, and the entry in progress plain"
    );
    // Point on the plan's first entry stays there while a later entry changes.
    h.keys("M-< C-n C-n C-n");
    let point = h.core.point();
    chunk(&mut h, "Done.");
    plan(
        &mut h,
        &[
            ("Read the code", Completed, High),
            ("Write tests", Pending, Medium),
            ("Ship\nit", Pending, Low),
        ],
    );
    assert_eq!(h.core.point(), point);
    let second = "Plan\n  ✓ Read the code\n  ○ Write tests\n  ○ Ship …";
    assert_eq!(
        h.core.text(),
        format!("Planning.\n\n{second}\n\nDone.\n\n> ")
    );
    assert!(faces(&h).contains(&("✓ Read the code".into(), dim.clone())));
    plan(&mut h, &[]);
    assert_eq!(h.core.text(), "Planning.\n\nPlan\n\nDone.\n\n> ");
    assert_eq!(faces(&h), [("Plan".into(), dim)]);
}

#[test]
fn session_model_effort_and_title_follow_configuration() {
    use agent_client_protocol_schema::v1::SessionConfigSelectGroup;
    let mut h = session();
    let model = SessionConfigOption::select(
        "model",
        "Model",
        "opus",
        vec![SessionConfigSelectOption::new("opus", "Opus 5.5")],
    )
    .category(SessionConfigOptionCategory::Model);
    let effort = SessionConfigOption::select(
        "effort",
        "Effort",
        "high",
        vec![SessionConfigSelectGroup::new(
            "levels",
            "Levels",
            vec![SessionConfigSelectOption::new("high", "High")],
        )],
    )
    .category(SessionConfigOptionCategory::ThoughtLevel);
    let mut config = options("plan");
    config.push(
        SessionConfigOption::boolean("model-flag", "Flag", true)
            .category(SessionConfigOptionCategory::Model),
    );
    config.extend([model.clone(), effort.clone()]);
    update(
        &mut h,
        0,
        SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(config)),
    );
    info(&mut h, 0, serde_json::json!({"title":" Fix it \nmore"}));
    assert_eq!(
        h.display(5).mode_line.text(),
        "Opus 5.5 · High │ [plan] │ Fix it  "
    );
    let unknown = SessionConfigOption::select(
        "model",
        "Model",
        "new-model",
        Vec::<SessionConfigSelectOption>::new(),
    )
    .category(SessionConfigOptionCategory::Model);
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(vec![unknown, effort.clone()]),
    });
    assert_eq!(h.display(5).mode_line.text(), "new-model · High │ Fix it  ");
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(vec![effort]),
    });
    assert_eq!(h.display(5).mode_line.text(), "High │ Fix it  ");
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(vec![model]),
    });
    assert_eq!(h.display(5).mode_line.text(), "Opus 5.5 │ Fix it  ");
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(vec![]),
    });
    info(&mut h, 0, serde_json::json!({"title":null}));
    assert_eq!(h.display(5).mode_line.text(), "claude  ");
    info(&mut h, 0, serde_json::json!({"title":"é".repeat(41)}));
    assert_eq!(
        h.display(5).mode_line.text(),
        format!("{}…  ", "é".repeat(40))
    );
}

#[test]
fn every_adapter_mode_has_its_risk_face() {
    let mut h = session();
    for (mode, face) in [
        ("plan", FaceName::StatusInfo),
        ("read-only", FaceName::StatusInfo),
        ("default", FaceName::ModeLine),
        ("acceptEdits", FaceName::StatusWaiting),
        ("workspace-write", FaceName::StatusWaiting),
        ("auto", FaceName::StatusRunning),
        ("agent", FaceName::StatusRunning),
        ("bypassPermissions", FaceName::StatusFailed),
        ("agent-full-access", FaceName::StatusFailed),
        ("future-mode", FaceName::ModeLine),
    ] {
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(options(mode)),
        });
        assert_eq!(
            h.display(5).mode_line.left[0],
            vec![(format!("[{mode}]"), face)]
        );
    }
}

#[test]
fn workspace_dots_follow_owned_sessions_even_while_another_workspace_is_current() {
    let mut h = session();
    let work = h.core.workspace().unwrap();
    let working = h.core.windows.window.buffer();
    let dots = |h: &mut Harness| {
        h.joined_frame(1600., 600.)
            .workspaces
            .into_iter()
            .filter(|s| s.text == " ●")
            .map(|s| s.face.fg)
            .collect::<Vec<_>>()
    };
    let face = |h: &Harness, name| h.core.settings.theme.face(name).fg;
    assert_eq!(dots(&mut h), vec![]);
    h.keys("a RET");
    assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusRunning)]);
    h.keys("M-x agent-start RET claude RET");
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Err("failed".into()),
    });
    let failed = h.core.windows.window.buffer();
    assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusFailed)]);
    ask(&mut h, 1, "one", Some("First"));
    h.core.rename_workspace("renamed").unwrap();
    let other = h.new_workspace("other", "home");
    assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusWaiting)]);
    h.core.switch_workspace(work).unwrap();
    h.core.switch_to(working);
    h.keys("C-c C-y");
    assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusFailed)]);
    h.core.kill_buffer(failed);
    assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusRunning)]);
    h.core.switch_workspace(other).unwrap();
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert_eq!(dots(&mut h), vec![]);
    exited(&mut h, 0, 0);
    assert_eq!(dots(&mut h), vec![face(&h, FaceName::StatusFailed)]);
    h.core.kill_workspace(work).unwrap();
    assert_eq!(dots(&mut h), vec![]);
}

#[test]
fn folded_workspace_list_project_uses_urgent_state_across_workspaces() {
    let mut h = session();
    h.keys("running RET");
    let first = h.core.workspace().unwrap();
    let project = h.core.current_workspace().unwrap().project;
    let second = h.core.new_workspace("other", project);
    h.core.switch_workspace(second).unwrap();
    h.keys("M-x agent-start RET claude RET");
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Err("failed".into()),
    });
    h.core.switch_workspace(first).unwrap();
    for (waiting, text, face) in [
        (false, "● exited", FaceName::StatusFailed),
        (true, "● awaiting permission", FaceName::StatusWaiting),
    ] {
        if waiting {
            ask(&mut h, 1, "one", Some("First"));
        }
        h.keys("C-x t l p TAB");
        let frame = h.frame(1440., 900.);
        let line = frame.windows[0]
            .display
            .lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.text.contains("2 workspaces")))
            .unwrap();
        assert!(
            line.spans
                .iter()
                .any(|s| s.text == text && s.face.fg == h.core.settings.theme.face(face).fg)
        );
        h.keys("q");
    }
}

#[test]
fn workspace_list_counts_only_files_terminals_and_registered_sessions() {
    let mut h = session();
    h.open("app/ internal.rs");
    h.keys("changed");
    h.core.create_terminal(" hidden terminal", 2, 10);
    let fake = h.core.buffers.create("*codex*");
    h.core.buffers.get_mut(fake).unwrap().major_mode = MODE;
    h.core.buffers.create("ordinary fileless");
    h.send(Msg::AgentStderr {
        id: AgentId(0),
        line: "stderr".into(),
    });
    h.keys("C-x t l");
    let rows: Vec<_> = h
        .text()
        .lines()
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[1], "▾ * work 3 buffers · 1 unsaved");
    assert!(rows.iter().any(|s| s.starts_with("internal.rs ")));
    assert!(rows.iter().any(|s| s == "hidden terminal"));
    assert!(rows.iter().any(|s| s == "*claude*"));
    let empty = h
        .core
        .new_workspace("empty", h.core.current_workspace().unwrap().project);
    h.core.switch_workspace(empty).unwrap();
    h.keys("C-x t l");
    let row = h
        .text()
        .lines()
        .find(|s| s.contains("* empty "))
        .unwrap()
        .to_string();
    assert!(row.starts_with("    * empty "));
    assert_eq!(
        row.split_whitespace().collect::<Vec<_>>().join(" "),
        "* empty 0 buffers"
    );
}

#[test]
fn workspace_list_retains_session_quality_faces_and_safe_labels_across_resize() {
    let mut h = session();
    let config = |model: &'static str, effort: &'static str| {
        vec![
            SessionConfigOption::select(
                "model",
                "Model",
                model,
                vec![SessionConfigSelectOption::new(
                    model,
                    format!("{model}\nlabel"),
                )],
            )
            .category(SessionConfigOptionCategory::Model),
            SessionConfigOption::select(
                "effort",
                "Effort",
                effort,
                vec![SessionConfigSelectOption::new(
                    effort,
                    format!("{effort}\tlabel"),
                )],
            )
            .category(SessionConfigOptionCategory::ThoughtLevel),
        ]
    };
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(config("opus", "high")),
    });
    let labels = h.core.agents[&AgentId(0)].model_effort();
    h.keys("C-x t l");
    let snapshot = h.text();
    assert_eq!(snapshot.lines().count(), 3);
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(config("haiku", "low")),
    });
    let dark = h.core.settings.theme.clone();
    for light in [false, true] {
        h.core.settings.theme = dark.clone();
        if light {
            h.core
                .settings
                .theme
                .faces
                .get_mut(&FaceName::QualityLegendary)
                .unwrap()
                .fg = Some(0x8f3f71);
            h.core
                .settings
                .theme
                .faces
                .get_mut(&FaceName::QualityRare)
                .unwrap()
                .fg = Some(0x076678);
        }
        let unbounded = h.display(30);
        for display in [unbounded, h.frame(1440., 900.).windows.remove(0).display] {
            for (label, face) in &labels {
                assert!(
                    display
                        .lines
                        .iter()
                        .flat_map(|l| &l.spans)
                        .any(|s| s.text == *label
                            && s.face.fg == h.core.settings.theme.face(*face).fg),
                    "{label}: {display:?}"
                );
            }
        }
        let narrow = h.frame(450., 900.).windows.remove(0).display;
        assert!(
            narrow
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.text.contains("*claude*")))
        );
        let restored = h.frame(1440., 900.).windows.remove(0).display;
        for (label, face) in &labels {
            assert!(
                restored
                    .lines
                    .iter()
                    .flat_map(|l| &l.spans)
                    .any(|s| s.text == *label && s.face.fg == h.core.settings.theme.face(*face).fg)
            );
        }
        assert_eq!(h.text(), snapshot);
    }
    h.keys("g");
    assert!(h.text().contains("haiku label · low label"));
}

#[test]
fn workspace_list_sessions_show_model_effort_and_the_bars_urgent_state() {
    let mut h = session();
    let config = [
        SessionConfigOption::select(
            "model",
            "Model",
            "opus",
            vec![SessionConfigSelectOption::new("opus", "Opus 5.5")],
        )
        .category(SessionConfigOptionCategory::Model),
        SessionConfigOption::select(
            "effort",
            "Effort",
            "high",
            vec![SessionConfigSelectOption::new("high", "High")],
        )
        .category(SessionConfigOptionCategory::ThoughtLevel),
    ];
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(config.to_vec()),
    });
    h.keys("a RET");
    for (step, word, face) in [
        (0, "● working", FaceName::StatusRunning),
        (1, "● exited", FaceName::StatusFailed),
        (2, "● awaiting permission", FaceName::StatusWaiting),
    ] {
        if step == 1 {
            h.keys("M-x agent-start RET claude RET");
            h.send(Msg::AgentStarted {
                id: AgentId(1),
                result: Err("failed".into()),
            });
        } else if step == 2 {
            ask(&mut h, 1, "one", Some("First"));
        }
        h.keys("C-x t l");
        let snapshot = h.text();
        assert!(
            snapshot
                .lines()
                .find(|s| s.starts_with("  ▾ * work "))
                .unwrap()
                .ends_with(word)
        );
        assert!(snapshot.lines().any(|s| s.starts_with("      *claude* ")
            && s.contains("Opus 5.5 · High")
            && s.ends_with(if step == 2 { word } else { "● working" })));
        h.keys("p TAB");
        assert!(
            h.text()
                .lines()
                .any(|s| s.starts_with("▸ ") && s.contains(" · 1 workspace") && s.ends_with(word))
        );
        h.keys("TAB n");
        let dark = h.core.settings.theme.clone();
        for light in [false, true] {
            h.core.settings.theme = dark.clone();
            if light {
                for (face, fg) in [
                    (FaceName::Annotation, 0x7c6f64),
                    (FaceName::StatusRunning, 0x79740e),
                    (FaceName::StatusFailed, 0x9d0006),
                    (FaceName::StatusWaiting, 0xb57614),
                ] {
                    h.core.settings.theme.faces.get_mut(&face).unwrap().fg = Some(fg);
                }
            }
            let frame = h.frame(1440., 900.);
            let fg = h.core.settings.theme.face(face).fg;
            assert!(
                frame
                    .workspaces
                    .iter()
                    .any(|s| s.text == " ●" && s.face.fg == fg)
            );
            let row = frame.windows[0]
                .display
                .lines
                .iter()
                .find(|l| l.spans.iter().any(|s| s.text.contains("* work")))
                .unwrap();
            assert!(row.spans.iter().any(|s| s.text == word && s.face.fg == fg));
            for (label, face) in h.core.agents[&AgentId(0)].model_effort() {
                assert!(
                    frame.windows[0]
                        .display
                        .lines
                        .iter()
                        .flat_map(|l| &l.spans)
                        .any(
                            |s| s.text == label && s.face.fg == h.core.settings.theme.face(face).fg
                        )
                );
            }
        }
        h.core.settings.theme = dark;
        if step == 2 {
            exited(&mut h, 0, 1);
            assert_eq!(h.text(), snapshot);
            h.keys("C-x t l");
            assert!(
                h.text()
                    .lines()
                    .find(|s| s.starts_with("  ▾ * work "))
                    .unwrap()
                    .ends_with("● exited")
            );
        }
        h.keys("q");
    }
}

#[test]
fn session_status_follows_turns_permissions_and_exit() {
    let mut h = session();
    assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ");
    h.keys("a RET");
    assert_eq!(
        h.display(5).mode_line.right[0],
        vec![("● working".into(), FaceName::StatusRunning)]
    );
    ask(&mut h, 1, "one", Some("First"));
    ask(&mut h, 2, "two", Some("Second"));
    let waiting = vec![("● awaiting permission".into(), FaceName::StatusWaiting)];
    assert_eq!(h.display(5).mode_line.right[0], waiting);
    h.keys("C-c C-y");
    assert_eq!(h.display(5).mode_line.right[0], waiting);
    h.keys("C-c C-k");
    assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ● working");
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ");
    h.keys("b RET");
    ask(&mut h, 3, "three", Some("Third"));
    h.keys("C-c C-c");
    assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ● working");
    prompted(&mut h, Ok(StopReason::Cancelled));
    assert_eq!(h.display(5).mode_line.text(), "[auto] │ claude  ");
    h.keys("c RET");
    ask(&mut h, 4, "four", Some("Fourth"));
    exited(&mut h, 0, 1);
    assert_eq!(
        h.display(5).mode_line.right[0],
        vec![("● exited".into(), FaceName::StatusFailed)]
    );
    h.keys("M-x agent-start RET claude RET");
    assert_eq!(h.display(5).mode_line.text(), "claude  ");
    h.send(Msg::AgentStarted {
        id: AgentId(1),
        result: Err("failed".into()),
    });
    assert_eq!(h.display(5).mode_line.text(), "claude  ● exited");
}

#[test]
fn context_usage_is_bounded_and_keeps_the_latest_update() {
    use agent_client_protocol_schema::v1::UsageUpdate;
    let mut h = session();
    for (used, size, expected) in [
        (23613, 475000, "ctx 4%"),
        (25499, 475000, "ctx 5%"),
        (0, 100, "ctx 0%"),
        (u64::MAX, u64::MAX, "ctx 100%"),
        (u64::MAX, 1, "ctx 100%"),
        (1, 0, ""),
        (1, 100, "ctx 1%"),
    ] {
        update(
            &mut h,
            0,
            SessionUpdate::UsageUpdate(UsageUpdate::new(used, size)),
        );
        assert_eq!(
            h.display(5).mode_line.text(),
            format!("[auto] │ claude  {expected}")
        );
    }
    exited(&mut h, 0, 0);
    update(
        &mut h,
        0,
        SessionUpdate::UsageUpdate(UsageUpdate::new(99, 100)),
    );
    assert_eq!(
        h.display(5).mode_line.text(),
        "[auto] │ claude  ● exited │ ctx 1%"
    );
}

#[test]
fn context_usage_is_coloured_by_how_full_it_is() {
    use agent_client_protocol_schema::v1::UsageUpdate;
    let mut h = session();
    for (used, face) in [
        (49, FaceName::StatusRunning),
        (50, FaceName::StatusWaiting),
        (79, FaceName::StatusWaiting),
        (80, FaceName::StatusFailed),
    ] {
        let usage = SessionUpdate::UsageUpdate(UsageUpdate::new(used, 100));
        update(&mut h, 0, usage);
        let right = h.display(5).mode_line.right;
        assert_eq!(right[0], vec![(format!("ctx {used}%"), face)]);
    }
}

#[test]
fn model_and_effort_are_coloured_by_item_quality() {
    use FaceName::*;
    let mut h = session();
    for (value, name, effort, faces) in [
        ("opus", "Opus 5.5", "high", (QualityEpic, QualityRare)),
        (
            "gpt-6-astra",
            "6 Astra",
            "ultra",
            (QualityLegendary, QualityArtifact),
        ),
        (
            "gpt-6-luna",
            "6 Luna",
            "low",
            (QualityUncommon, QualityCommon),
        ),
        (
            "fable",
            "Fable 5.1",
            "max",
            (QualityLegendary, QualityLegendary),
        ),
        (
            "sonnet",
            "Sonnet 5.5",
            "medium",
            (QualityRare, QualityUncommon),
        ),
        (
            "haiku",
            "Haiku 5.5",
            "xhigh",
            (QualityUncommon, QualityEpic),
        ),
        (
            "default",
            "Default (recommended)",
            "default",
            (ModeLineDim, QualityPoor),
        ),
        (
            "claude-opus-4-8",
            "Opus 4.8",
            "turbo",
            (QualityEpic, ModeLineDim),
        ),
        ("gpt-5.6-sol", "5.6 Sol", "high", (QualityEpic, QualityRare)),
        (
            "gpt-5.6-terra",
            "5.6 Terra",
            "high",
            (QualityRare, QualityRare),
        ),
        ("solo", "Solo", "high", (ModeLineDim, QualityRare)),
    ] {
        let model = SessionConfigOption::select(
            "model",
            "Model",
            value,
            vec![SessionConfigSelectOption::new(value, name)],
        );
        let level = SessionConfigOption::select(
            "effort",
            "Effort",
            effort,
            vec![SessionConfigSelectOption::new(
                effort,
                effort.to_uppercase(),
            )],
        );
        let mut options = options("auto");
        options.push(model.category(SessionConfigOptionCategory::Model));
        options.push(level.category(SessionConfigOptionCategory::ThoughtLevel));
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result: Ok(options),
        });
        assert_eq!(
            h.display(5).mode_line.left[0],
            vec![
                (name.to_string(), faces.0),
                (" · ".to_string(), ModeLineDim),
                (effort.to_uppercase(), faces.1),
            ],
            "{name} · {effort}"
        );
    }
}

#[test]
fn session_frame_spans_follow_active_inactive_and_narrow_windows() {
    let mut h = session();
    info(&mut h, 0, serde_json::json!({"title":"A session title"}));
    h.keys("a RET C-x 3");
    let frame = h.joined_frame(1200., 300.);
    let theme = &h.core.settings.theme;
    for window in &frame.windows {
        let bar = &window.mode_line;
        let text = |spans: &[crate::display::Span]| {
            spans.iter().map(|s| s.text.as_str()).collect::<String>()
        };
        assert_eq!(text(&bar.left), "[auto] │ A session title");
        assert_eq!(text(&bar.right), "● working");
        if window.selected {
            let base = theme.face(FaceName::ModeLine);
            assert_eq!(
                bar.left[0].face,
                theme.face(FaceName::StatusRunning).over(base)
            );
            assert_eq!(bar.left[1].face, theme.face(FaceName::Separator).over(base));
            assert_eq!(
                bar.left[2].face,
                theme.face(FaceName::ModeLineBufferId).over(base)
            );
        } else {
            assert!(
                bar.left
                    .iter()
                    .chain(&bar.right)
                    .all(|s| s.face == theme.face(FaceName::ModeLineInactive))
            );
        }
    }
    let frame = h.joined_frame(400., 300.);
    for window in frame.windows {
        let left = window
            .mode_line
            .left
            .iter()
            .map(|s| s.text.as_str())
            .collect::<String>();
        assert!(left.ends_with('…'), "{left}");
        assert_eq!(window.mode_line.right[0].text, "● working");
    }
}

#[test]
fn the_mode_line_shows_the_current_mode() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    assert!(!h.display(5).mode_line.text().contains('['));
    ready(&mut h, 0);
    assert!(h.display(5).mode_line.text().contains("[auto]"));
    let options = ConfigOptionUpdate::new(options("plan"));
    update(&mut h, 0, SessionUpdate::ConfigOptionUpdate(options));
    let line = h.display(5).mode_line.text();
    assert!(line.contains("[plan]") && !line.contains("auto"), "{line}");
}

#[test]
fn a_session_starts_in_the_default_mode() {
    let mut h = work();
    h.core
        .settings
        .agents
        .get_mut("claude")
        .unwrap()
        .default_mode = Some("plan".into());
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    ready(&mut h, 0);
    assert_eq!(ops(&mut h), ["set 0 s mode=plan"]);
    assert!(h.display(5).mode_line.text().contains("[auto]"));
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(options("plan")),
    });
    assert!(h.display(5).mode_line.text().contains("[plan]"));
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Err("no".into()),
    });
    assert!(messages(&h).contains("claude could not set mode plan: no\n"));
    assert!(h.display(5).mode_line.text().contains("[plan]"));
}

#[test]
fn options_without_a_mode_select_are_no_mode() {
    let mut h = work();
    h.core
        .settings
        .agents
        .get_mut("claude")
        .unwrap()
        .default_mode = Some("plan".into());
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    // A mode-category boolean is not a mode.
    let flag = SessionConfigOption::boolean("fast", "Fast", true)
        .category(SessionConfigOptionCategory::Mode);
    let mut options = options("auto");
    options.pop();
    options.push(flag);
    let response = NewSessionResponse::new("s").config_options(options.clone());
    h.send(Msg::AgentStarted {
        id: AgentId(0),
        result: Ok(response),
    });
    assert!(ops(&mut h).is_empty());
    let note = "claude session ready (modes: none)\nclaude has no mode plan\n";
    assert!(messages(&h).contains(note));
    assert!(!h.display(5).mode_line.text().contains('['));
    // An answer or update without a mode option clears the mode.
    ready(&mut h, 0);
    assert!(h.display(5).mode_line.text().contains("[auto]"));
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(options),
    });
    assert!(!h.display(5).mode_line.text().contains('['));
}

#[test]
fn a_current_or_unknown_default_mode_sends_nothing() {
    let mut h = work();
    h.core
        .settings
        .agents
        .get_mut("claude")
        .unwrap()
        .default_mode = Some("auto".into());
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    ready(&mut h, 0);
    assert!(ops(&mut h).is_empty());
    h.core
        .settings
        .agents
        .get_mut("claude")
        .unwrap()
        .default_mode = Some("nope".into());
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    ready(&mut h, 1);
    assert!(ops(&mut h).is_empty());
    assert!(
        messages(&h)
            .contains("claude session ready (modes: auto, plan)\nclaude has no mode nope\n")
    );
    assert!(h.display(5).mode_line.text().contains("[auto]"));
    assert_eq!(messages(&h).matches("has no mode").count(), 1);
}

#[test]
fn s_tab_cycles_through_every_mode_and_back() {
    let mut h = session();
    let configured = |h: &mut Harness, result| {
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result,
        });
    };
    // Mid-turn too.
    h.keys("a RET");
    ops(&mut h);
    h.keys("S-TAB");
    assert_eq!(ops(&mut h), ["set 0 s mode=plan"]);
    configured(&mut h, Ok(options("plan")));
    assert!(h.display(5).mode_line.text().contains("[plan]"));
    h.keys("S-TAB");
    assert_eq!(ops(&mut h), ["set 0 s mode=auto"]);
    configured(&mut h, Ok(options("auto")));
    assert!(h.display(5).mode_line.text().contains("[auto]"));
    // The agent's own change is where the next cycle starts.
    let plan = ConfigOptionUpdate::new(options("plan"));
    update(&mut h, 0, SessionUpdate::ConfigOptionUpdate(plan));
    h.keys("S-TAB");
    assert_eq!(ops(&mut h), ["set 0 s mode=auto"]);
    configured(&mut h, Err("no".into()));
    assert!(messages(&h).contains("claude could not set mode auto: no\n"));
    assert!(h.display(5).mode_line.text().contains("[plan]"));
}

#[test]
fn s_tab_needs_a_ready_session_with_modes() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    h.keys("S-TAB");
    assert_eq!(h.core.message(), Some("claude is not ready"));
    h.send(Msg::AgentStarted {
        id: AgentId(0),
        result: Ok(NewSessionResponse::new("s")),
    });
    h.keys("S-TAB");
    assert_eq!(h.core.message(), Some("claude has no modes"));
    assert!(ops(&mut h).is_empty());
    exited(&mut h, 0, 1);
    ops(&mut h);
    h.keys("S-TAB");
    assert_eq!(h.core.message(), Some("claude session has ended"));
    assert!(ops(&mut h).is_empty());
}

fn models(current: &'static str, grouped: bool) -> Vec<SessionConfigOption> {
    let opus = SessionConfigSelectOption::new("opus", "Opus").description("Most capable\nmore");
    let sonnet = SessionConfigSelectOption::new("sonnet", "Sonnet");
    let model = match grouped {
        false => SessionConfigOption::select("model", "Model", current, vec![opus, sonnet]),
        true => SessionConfigOption::select(
            "model",
            "Model",
            current,
            vec![
                SessionConfigSelectGroup::new("a", "A", vec![opus]),
                SessionConfigSelectGroup::new("b", "B", vec![sonnet]),
            ],
        ),
    };
    let mut options = options("auto");
    options.push(model.category(SessionConfigOptionCategory::Model));
    options
}

#[test]
fn agent_select_model_sets_the_picked_model() {
    for grouped in [false, true] {
        let mut h = session();
        let configured = |h: &mut Harness, result| {
            h.send(Msg::AgentConfigured {
                id: AgentId(0),
                result,
            });
        };
        configured(&mut h, Ok(models("sonnet", grouped)));
        h.keys("M-x agent-select-model RET");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Model: ");
        assert_eq!(
            listed(&mut h),
            (vec!["Opus".into(), "Sonnet".into()], Some(1))
        );
        assert_eq!(
            h.core.minibuffer().unwrap().annotations,
            ["Most capable", ""]
        );
        h.keys("C-p RET");
        assert_eq!(ops(&mut h), ["set 0 s model=opus"]);
        configured(&mut h, Ok(models("opus", grouped)));
        assert!(h.display(5).mode_line.text().starts_with("Opus │ [auto]"));
        h.keys("M-x agent-select-model RET Sonnet RET");
        assert_eq!(ops(&mut h), ["set 0 s model=sonnet"]);
        configured(&mut h, Err("no".into()));
        assert!(messages(&h).contains("claude could not set model sonnet: no\n"));
        assert!(h.display(5).mode_line.text().starts_with("Opus │"));
    }
}

#[test]
fn agent_select_model_needs_a_ready_session_with_a_model() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    h.keys("M-x agent-select-model RET");
    assert_eq!(h.core.message(), Some("claude is not ready"));
    assert!(h.core.minibuffer().is_none());
    ready(&mut h, 0);
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(vec![]),
    });
    h.keys("M-x agent-select-model RET");
    assert_eq!(h.core.message(), Some("claude has no model choice"));
    assert!(h.core.minibuffer().is_none());
    h.send(Msg::AgentConfigured {
        id: AgentId(0),
        result: Ok(models("opus", false)),
    });
    h.keys("M-x agent-select-model RET Haiku RET");
    assert_eq!(h.core.message(), Some("claude has no model Haiku"));
    assert!(ops(&mut h).is_empty());
    exited(&mut h, 0, 1);
    ops(&mut h);
    h.keys("M-x agent-select-model RET");
    assert_eq!(h.core.message(), Some("claude session has ended"));
    assert!(ops(&mut h).is_empty());
}

/// The model options with an effort option over `values`, the last one current.
fn efforts(values: &[(&'static str, &'static str)]) -> Vec<SessionConfigOption> {
    let current = values.last().unwrap().0;
    let values = (values.iter())
        .map(|&(id, name)| SessionConfigSelectOption::new(id, name).description(id))
        .collect::<Vec<_>>();
    let mut options = models("opus", false);
    let effort = SessionConfigOption::select("effort", "Effort", current, values);
    options.push(effort.category(SessionConfigOptionCategory::ThoughtLevel));
    options
}

#[test]
fn agent_select_effort_sets_the_picked_effort() {
    let mut h = session();
    let configured = |h: &mut Harness, result| {
        h.send(Msg::AgentConfigured {
            id: AgentId(0),
            result,
        });
    };
    configured(&mut h, Ok(efforts(&[("low", "Low"), ("high", "High")])));
    h.keys("M-x agent-select-effort RET");
    assert_eq!(h.core.minibuffer().unwrap().prompt, "Effort: ");
    assert_eq!(listed(&mut h), (vec!["Low".into(), "High".into()], Some(1)));
    assert_eq!(h.core.minibuffer().unwrap().annotations, ["low", "high"]);
    h.keys("C-p RET");
    assert_eq!(ops(&mut h), ["set 0 s effort=low"]);
    configured(&mut h, Ok(efforts(&[("high", "High"), ("low", "Low")])));
    assert!(h.display(5).mode_line.text().starts_with("Opus · Low │"));
    // A model change reports other efforts; the list follows.
    configured(&mut h, Ok(efforts(&[("low", "Low"), ("max", "Max")])));
    h.keys("M-x agent-select-effort RET");
    assert_eq!(listed(&mut h), (vec!["Low".into(), "Max".into()], Some(1)));
    h.keys("C-g");
    h.keys("M-x agent-select-effort RET High RET");
    assert_eq!(h.core.message(), Some("claude has no effort High"));
    h.keys("M-x agent-select-effort RET Low RET");
    assert_eq!(ops(&mut h), ["set 0 s effort=low"]);
    configured(&mut h, Err("no".into()));
    assert!(messages(&h).contains("claude could not set effort low: no\n"));
    configured(&mut h, Ok(models("opus", false)));
    h.keys("M-x agent-select-effort RET");
    assert_eq!(h.core.message(), Some("claude has no effort choice"));
    assert!(h.core.minibuffer().is_none());
    assert!(ops(&mut h).is_empty());
}

fn working(h: &mut Harness) -> String {
    h.core.frame();
    let text = h.core.text();
    let mut lines = text.lines().filter(|l| l.starts_with("Work"));
    lines.next_back().unwrap_or_default().to_owned()
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn messages_show_their_clock_times_and_the_turn_its_length() {
    let mut h = session();
    h.clock("14:01:05");
    h.keys("h i RET");
    h.clock("14:03:00");
    h.advance(Duration::from_millis(22_600));
    agent_chunk(&mut h, "m", "Hi\nthere");
    h.clock("14:04:00");
    h.advance(secs(120));
    // Stamped when it arrived; the line goes in when the block ends.
    agent_chunk(&mut h, "m", "!");
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert_eq!(
        h.core.text(),
        "> hi\n14:01:05\nHi\nthere!\n14:03:22\nWorked 2m 22s 14:06:00\n\n> "
    );
    // Only the summary rule's time stays at the right; the others are small, under their block.
    assert_eq!(
        props(&h)
            .iter()
            .filter(|(_, p)| *p == Prop::AlignRight)
            .count(),
        1
    );
    let small = (props(&h).into_iter())
        .filter(|(_, p)| *p == Prop::Line(Decoration::Small))
        .count();
    assert_eq!(small, 2);
    let annotation = Prop::Named(vec![FaceRef::Face(FaceName::Annotation)]);
    assert!(props(&h).contains(&("14:03:22".into(), annotation)));
    // The message's lines cover its block, not the time under it.
    assert_eq!(message_list(&h)[1].1, "Hi\nthere!");
}

#[test]
fn the_clock_counts_on_from_the_frontends_and_wraps_at_midnight() {
    let mut h = session();
    assert_eq!(h.core.clock(), None);
    h.clock("23:59:58");
    h.advance(Duration::from_millis(1_600));
    assert_eq!(h.core.clock().as_deref(), Some("23:59:59"));
    h.advance(Duration::from_millis(1_400));
    assert_eq!(h.core.clock().as_deref(), Some("00:00:01"));
}

#[test]
fn a_thought_gets_a_header_when_it_ends() {
    let mut h = session();
    h.keys("a RET");
    thought(&mut h, "hmm");
    h.advance(Duration::from_millis(4200));
    chunk(&mut h, "So.");
    assert_eq!(
        h.core.text(),
        "> a\n\nThought for 4s\nhmm\n\nSo.\n\nWorking for 0s\n\n> "
    );
    let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
    assert!(props(&h).contains(&("Thought for 4s".into(), dim)));
}

#[test]
fn a_codex_thought_drops_the_blank_lines_around_its_summary_parts() {
    let mut h = session();
    h.keys("a RET");
    for part in ["\n\n", "**One**", "\n\n", "**Two**", "\n\n"] {
        thought(&mut h, part);
    }
    chunk(&mut h, "So.");
    assert_eq!(
        h.core.text(),
        "> a\n\nThought for 0s\n**One**\n\n**Two**\n\nSo.\n\nWorking for 0s\n\n> "
    );
}

#[test]
fn a_tool_call_shows_how_long_it_ran_without_its_permission_wait() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    tool(&mut h, ls());
    h.advance(secs(1));
    ask(&mut h, 1, "t", None);
    h.advance(secs(12));
    h.core.frame();
    assert!(h.core.text().contains("```\nwaiting 0:12\nPermission"));
    h.keys("C-c C-y");
    assert!(h.core.text().contains("Running ls\n"));
    h.advance(Duration::from_millis(1300));
    let done = ToolCallUpdateFields::new().status(ToolCallStatus::Completed);
    tool_update(&mut h, "t", done);
    assert!(h.core.text().contains("```\n2.3s\n"));
    // Later updates keep it, and a long one reads as minutes.
    h.advance(secs(9));
    tool_update(
        &mut h,
        "t",
        ToolCallUpdateFields::new().title("ls".to_string()),
    );
    assert!(h.core.text().contains("```\n2.3s\n"));
    assert_eq!(short_duration(secs(125)), "2m 05s");
}

#[test]
fn the_waiting_counter_keeps_folds_and_an_unshown_call_has_none() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    tool(&mut h, ls());
    ask(&mut h, 1, "t", None);
    ask(&mut h, 2, "unknown", Some("U"));
    for n in 1..=3 {
        h.advance(secs(1));
        h.core.frame();
        let want = format!("```\nwaiting 0:0{n}\n");
        assert!(h.core.text().contains(&want));
    }
    h.keys("M-<");
    assert_eq!(
        shown(&mut h, 20)[2..4],
        ["▸ ●   Running ls", "waiting 0:03"]
    );
    h.keys("C-c C-c");
    assert!(h.core.text().contains("Running ls\n"));
}

#[test]
fn the_working_line_ticks_below_output_and_keeps_the_prompt() {
    let mut h = session();
    h.keys("a RET d r a f t");
    let point = h.core.point();
    h.advance(secs(1));
    assert_eq!(working(&mut h), "Working for 1s");
    assert_eq!(h.core.point(), point, "same length");
    h.advance(secs(135));
    assert_eq!(working(&mut h), "Working for 2m 16s");
    assert!(h.core.text().ends_with("\n\n> draft"));
    assert_eq!(h.core.point(), h.core.text().chars().count());
    chunk(&mut h, "out");
    h.keys("X");
    assert!(
        h.core
            .text()
            .ends_with("out\n\nWorking for 2m 16s\n\n> draftX")
    );
    // Ticks leave the prompt area's undo alone.
    h.advance(secs(1));
    h.core.frame();
    h.keys("C-/");
    assert!(h.core.text().ends_with("Working for 2m 17s\n\n> "));
    assert_eq!(long_duration(secs(3725)), "1h 02m 05s");
}

#[test]
fn the_line_settles_however_the_turn_ends() {
    for (end, line) in [
        (Some(Ok(StopReason::EndTurn)), ""),
        (Some(Ok(StopReason::Cancelled)), "Turn ended: cancelled\n\n"),
        (Some(Err("boom")), "Error: boom\n\n"),
        (None, ""),
    ] {
        let mut h = session();
        h.keys("a RET");
        h.advance(secs(3));
        match end {
            Some(result) => prompted(&mut h, result),
            None => exited(&mut h, 0, 1),
        }
        assert_eq!(h.core.text(), format!("> a\n\n{line}Worked 3s\n\n> "));
        assert_eq!(h.core.next_deadline(), None);
    }
}

#[test]
fn a_hidden_session_wakes_nothing_and_is_current_when_shown() {
    let mut h = session();
    h.keys("a RET");
    assert!(h.core.next_deadline().is_some());
    h.keys("C-x b * M e s s a g e s * RET");
    assert_eq!(h.core.next_deadline(), None);
    h.advance(secs(60));
    let session = h.core.agents[&AgentId(0)].buffer;
    let text = |h: &Harness| h.core.buffers.get(session).unwrap().text().to_string();
    assert!(
        text(&h).contains("Working for 0s"),
        "untouched while hidden"
    );
    h.keys("C-x b RET");
    assert!(h.core.next_deadline().is_some());
    assert_eq!(working(&mut h), "Working for 1m 00s");
}

#[test]
fn a_hidden_turn_end_settles_at_once() {
    let mut h = session();
    h.keys("a RET C-x b * M e s s a g e s * RET");
    h.advance(secs(7));
    prompted(&mut h, Ok(StopReason::EndTurn));
    let session = h.core.agents[&AgentId(0)].buffer;
    let text = h.core.buffers.get(session).unwrap().text().to_string();
    assert_eq!(text, "> a\n\nWorked 7s\n\n> ");
}

/// A finished `read` call `id` of `path`.
fn read(id: &'static str, path: &'static str) -> ToolCall {
    ToolCall::new(id, format!("Read {path}"))
        .kind(ToolKind::Read)
        .status(ToolCallStatus::Completed)
        .raw_output(serde_json::json!("text"))
        .locations(vec![ToolCallLocation::new(path)])
}

/// An unfinished call `id` of `kind` without content.
fn call(id: &'static str, kind: ToolKind) -> ToolCall {
    ToolCall::new(id, id.to_string()).kind(kind)
}

fn visible(h: &mut Harness) -> Vec<String> {
    let lines = shown(h, 30);
    (lines.into_iter()).filter(|l| !l.is_empty()).collect()
}

/// A session with a turn running and `read` calls `a` and `b` folded into a group.
fn grouped() -> Harness {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    tool(&mut h, read("a", "a.rs"));
    tool(&mut h, read("b", "b.rs"));
    h
}

/// Moves point to the line starting with `line` after any indent, from the top.
fn on_line(h: &mut Harness, line: &str) {
    let text = h.core.text();
    let at = (text.char_indices())
        .scan(0, |chars, (byte, c)| {
            let at = *chars;
            *chars += 1;
            Some((byte, at, c))
        })
        .find(|&(byte, _, c)| {
            let start = byte == 0 || text[..byte].ends_with('\n');
            start && c != '\n' && text[byte..].trim_start_matches(' ').starts_with(line)
        })
        .map(|(_, at, _)| at)
        .expect("the line");
    let core = &mut h.core;
    let window = &mut core.windows.window;
    window.set_point(&mut core.buffers, at).unwrap();
}

#[test]
fn two_calls_fold_into_one_line() {
    let mut h = grouped();
    assert_eq!(
        h.core.text(),
        "> a\n\n▸    Read files\n  ▸ ✓    Read a.rs 0.0s\nOutput:\n```\ntext\n```\n  \
         ▸ ✓    Read b.rs 0.0s\nOutput:\n```\ntext\n```\n0.0s\nWorking for 0s\n\n> "
    );
    assert_eq!(
        visible(&mut h),
        ["❯ a", "▸    Read files", "0.0s", "Working for 0s", "❯ "]
    );
    let rule = |(_, p): &(String, Prop)| matches!(p, Prop::Line(Decoration::Rule(_)));
    assert!(!props(&h).iter().any(rule), "the group line has no rule");
    // A third call and updates keep it folded.
    tool(&mut h, read("c", "c.rs"));
    tool_update(
        &mut h,
        "a",
        ToolCallUpdateFields::new().title("x".to_string()),
    );
    assert_eq!(visible(&mut h)[1..3], ["▸    Read files", "0.0s"]);
    assert_eq!(visible(&mut h).len(), 5);
}

#[test]
fn times_under_every_block_start_in_one_column() {
    let mut h = session();
    h.clock("14:01:00");
    h.keys("a RET");
    tool(&mut h, read("a", "a.rs"));
    chunk(&mut h, "one");
    tool(&mut h, read("b", "b.rs"));
    tool(&mut h, read("c", "c.rs"));
    prompted(&mut h, Ok(StopReason::EndTurn));
    let indents: Vec<_> = (props(&h).into_iter())
        .filter_map(|(text, p)| match p {
            Prop::Line(Decoration::Indent(n)) => Some((text, n)),
            _ => None,
        })
        .collect();
    // One column under every block; the summary rule has its own indent.
    let want = [("1", 2), ("0", 2), ("1", 2), ("0", 2), ("W", 2)];
    assert_eq!(indents, want.map(|(t, n)| (t.to_string(), n)));
}

#[test]
fn a_single_call_has_no_group_line_and_messages_split_groups() {
    let mut h = session();
    h.keys("a RET");
    tool(&mut h, read("a", "a.rs"));
    chunk(&mut h, "one");
    assert_eq!(visible(&mut h)[1..3], ["▸ ✓   Read a.rs", "0.0s"]);
    tool(&mut h, read("b", "b.rs"));
    tool(&mut h, read("c", "c.rs"));
    chunk(&mut h, "two");
    tool(&mut h, read("d", "d.rs"));
    tool(&mut h, read("e", "e.rs"));
    let lines = visible(&mut h);
    let groups = lines.iter().filter(|l| l.contains("Read files")).count();
    assert_eq!(groups, 2, "{lines:?}");
    assert!(lines.contains(&"two".to_string()));
}

#[test]
fn a_prompt_and_a_session_line_end_a_run() {
    let groups = |h: &mut Harness| {
        let lines = visible(h);
        lines.iter().filter(|l| l.contains("Read files")).count()
    };
    let mut h = grouped();
    prompted(&mut h, Ok(StopReason::EndTurn));
    h.keys("b RET");
    tool(&mut h, read("c", "c.rs"));
    tool(&mut h, read("d", "d.rs"));
    assert_eq!(groups(&mut h), 2);
    let mut h = grouped();
    prompted(&mut h, Ok(StopReason::MaxTokens));
    tool(&mut h, read("c", "c.rs"));
    tool(&mut h, read("d", "d.rs"));
    assert_eq!(groups(&mut h), 2);
}

#[test]
fn a_shown_plan_doesnt_split_a_group_and_a_first_plan_does() {
    let mut h = session();
    h.keys("a RET");
    let entry = [("x", PlanEntryStatus::Pending, PlanEntryPriority::Low)];
    tool(&mut h, read("a", "a.rs"));
    plan(&mut h, &entry);
    tool(&mut h, read("b", "b.rs"));
    assert!(
        !h.core.text().contains("Read files"),
        "the first plan ends the run"
    );
    tool(&mut h, read("c", "c.rs"));
    plan(&mut h, &entry);
    tool(&mut h, read("d", "d.rs"));
    assert!(h.core.text().contains("Read files"));
    assert_eq!(
        visible(&mut h)
            .iter()
            .filter(|l| l.contains("Read"))
            .count(),
        2
    );
}

#[test]
fn verbs_follow_first_appearance_and_the_running_one_is_present() {
    let line = |h: &mut Harness| visible(h)[1].clone();
    let mut h = session();
    h.keys("a RET");
    tool(&mut h, read("a", "a.rs"));
    tool(&mut h, call("b", ToolKind::Execute));
    tool(&mut h, read("c", "c.rs"));
    assert_eq!(line(&mut h), "▸    Read files, running commands");
    let mut h = session();
    h.keys("a RET");
    tool(
        &mut h,
        call("a", ToolKind::Edit).status(ToolCallStatus::Completed),
    );
    tool(
        &mut h,
        call("b", ToolKind::Search).status(ToolCallStatus::Completed),
    );
    tool(
        &mut h,
        call("c", ToolKind::Delete).status(ToolCallStatus::Completed),
    );
    tool(
        &mut h,
        call("d", ToolKind::Fetch).status(ToolCallStatus::Completed),
    );
    tool(
        &mut h,
        call("e", ToolKind::Other).status(ToolCallStatus::Completed),
    );
    assert_eq!(
        line(&mut h),
        "▸    Edited files, searched code, fetched pages, used tools"
    );
    let icons: Vec<_> = (props(&h).into_iter())
        .filter_map(|(_, p)| match p {
            Prop::Icon(icon) => Some(icon),
            _ => None,
        })
        .collect();
    assert_eq!(icons, [Icon::Pencil]);
}

#[test]
fn a_failed_call_shows_on_the_folded_line_and_the_duration_spans_the_group() {
    let mut h = session();
    h.keys("a RET");
    tool(&mut h, call("a", ToolKind::Read));
    h.advance(secs(1));
    tool(&mut h, call("b", ToolKind::Execute));
    let finish = |s| ToolCallUpdateFields::new().status(s);
    tool_update(&mut h, "a", finish(ToolCallStatus::Completed));
    h.advance(secs(2));
    assert_eq!(visible(&mut h)[1], "▸    Read files, running commands");
    tool_update(&mut h, "b", finish(ToolCallStatus::Failed));
    assert_eq!(
        visible(&mut h)[1..3],
        ["▸    Read files, ran commands", "1 failed 3.0s"]
    );
    assert!(h.core.text().contains("\n1 failed  3.0s\n"));
    let failed = (props(&h).into_iter()).find(|(t, _)| t == "1 failed");
    let want = Prop::Named(vec![FaceRef::Face(FaceName::StatusFailed)]);
    assert_eq!(failed.map(|(_, p)| p), Some(want));
}

#[test]
fn an_open_body_is_indented_past_its_status_mark() {
    let mut h = grouped();
    on_line(&mut h, "▸    Read");
    h.keys("TAB");
    on_line(&mut h, "▸ ✓    Read a.rs");
    h.keys("TAB");
    let output = drawn_line(&mut h, "Output:");
    assert_eq!(output.indent(), 6, "a grouped row's mark is at column 4");
    let text = drawn_line(&mut h, "text");
    let block = Decoration::Background(FaceName::SessionBlock);
    assert_eq!(
        (text.indent(), text.decorations.contains(&block)),
        (6, true)
    );
    assert_eq!(drawn_line(&mut h, "Read a.rs").indent(), 0);
    let mut h = session();
    tool(&mut h, ls().raw_output(serde_json::json!("a.rs")));
    h.keys("M-< TAB");
    assert_eq!(drawn_line(&mut h, "$ ls").indent(), 4);
}

#[test]
fn tab_opens_a_group_then_a_row_and_folds_back_a_level_at_a_time() {
    let mut h = grouped();
    on_line(&mut h, "▸    Read");
    h.keys("TAB");
    assert_eq!(
        visible(&mut h)[1..4],
        [
            "▾    Read files",
            "  ▸ ✓   Read a.rs 0.0s",
            "  ▸ ✓   Read b.rs 0.0s"
        ]
    );
    on_line(&mut h, "▸ ✓    Read a.rs");
    h.keys("TAB");
    assert_eq!(visible(&mut h)[2], "  ▾ ✓   Read a.rs 0.0s");
    assert!(visible(&mut h).iter().any(|l| l.contains("text")));
    // Inside the call, back to its row, the group staying open.
    h.keys("C-n C-n TAB");
    assert_eq!(visible(&mut h)[2], "  ▸ ✓   Read a.rs 0.0s");
    assert_eq!(visible(&mut h).len(), 7);
    on_line(&mut h, "▾    Read");
    h.keys("TAB");
    assert_eq!(visible(&mut h).len(), 5);
    // What was open stays open across a fold.
    h.keys("TAB");
    on_line(&mut h, "▸ ✓    Read b.rs");
    h.keys("TAB");
    on_line(&mut h, "▾    Read");
    h.keys("TAB TAB");
    assert_eq!(visible(&mut h)[3], "  ▾ ✓   Read b.rs 0.0s");
}

#[test]
fn tab_on_a_row_with_nothing_to_open_folds_the_group() {
    let mut h = session();
    h.keys("a RET");
    tool(&mut h, call("a", ToolKind::Read));
    tool(&mut h, call("b", ToolKind::Read));
    on_line(&mut h, "▸    Reading");
    h.keys("TAB");
    on_line(&mut h, "▸ ●    Reading a");
    h.keys("TAB");
    assert_eq!(visible(&mut h)[1], "▸    Reading files");
    assert_eq!(visible(&mut h).len(), 4);
    let at = h.core.current().text().line_to_char(2);
    assert_eq!(h.core.point(), at);
}

#[test]
fn an_open_call_keeps_its_run_open_as_a_group_forms() {
    let mut h = session();
    h.keys("a RET");
    tool(&mut h, read("a", "a.rs"));
    on_line(&mut h, "▸ ✓    Read a.rs");
    h.keys("TAB");
    tool(&mut h, read("b", "b.rs"));
    let lines = visible(&mut h);
    assert_eq!(lines[1], "▾    Read files");
    assert_eq!(lines[2], "  ▾ ✓   Read a.rs 0.0s");
}

#[test]
fn thoughts_are_rows_of_the_group() {
    let mut h = session();
    h.keys("a RET");
    thought(&mut h, "first\nthought");
    tool(&mut h, read("a", "a.rs"));
    thought(&mut h, "second");
    tool(&mut h, read("b", "b.rs"));
    thought(&mut h, "last");
    tool(&mut h, read("c", "c.rs"));
    assert_eq!(visible(&mut h).len(), 5);
    on_line(&mut h, "▸    Read");
    h.keys("TAB");
    assert_eq!(
        visible(&mut h)[1..8],
        [
            "▾    Read files",
            "  ▸ Thought for 0s",
            "  ▸ ✓   Read a.rs 0.0s",
            "  ▸ Thought for 0s",
            "  ▸ ✓   Read b.rs 0.0s",
            "  ▸ Thought for 0s",
            "  ▸ ✓   Read c.rs 0.0s"
        ]
    );
    on_line(&mut h, "▸ Thought");
    h.keys("TAB");
    assert_eq!(visible(&mut h)[2], "  ▾ Thought for 0s");
    assert_eq!(visible(&mut h)[3..5], ["first", "thought"]);
    h.keys("C-n TAB");
    assert_eq!(visible(&mut h)[2], "  ▸ Thought for 0s");
    assert_eq!(visible(&mut h)[3], "  ▸ ✓   Read a.rs 0.0s");
}

#[test]
fn a_thought_streaming_into_a_folded_group_stays_hidden() {
    let mut h = grouped();
    thought(&mut h, "hmm");
    assert_eq!(visible(&mut h).len(), 4);
    thought(&mut h, " more");
    assert_eq!(visible(&mut h).len(), 4);
    chunk(&mut h, "done");
    assert_eq!(visible(&mut h).len(), 6);
    assert!(
        h.core
            .text()
            .contains("  ▸ Thought for 0s\nhmm more\n0.0s\ndone")
    );
}

#[test]
fn a_waiting_call_shows_under_the_fold_until_answered() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    tool(&mut h, read("a", "a.rs"));
    tool(&mut h, call("t", ToolKind::Execute));
    ask(&mut h, 1, "t", Some("t"));
    h.advance(secs(2));
    let lines = visible(&mut h);
    assert_eq!(
        lines[1..6],
        [
            "▸    Read files, running commands",
            "  ▸ ●   Running t waiting 0:02",
            "Permission: t",
            "1. Yes (allow_once)",
            "2. Always (allow_always)"
        ],
        "{lines:?}"
    );
    h.keys("C-c C-y");
    assert_eq!(visible(&mut h).len(), 4);
    on_line(&mut h, "▸    Read");
    h.keys("TAB");
    assert_eq!(
        visible(&mut h)[2..5],
        [
            "  ▸ ✓   Read a.rs 0.0s",
            "  ▸ ●   Running t",
            "✓ allowed once"
        ]
    );
}

#[test]
fn a_permission_block_before_the_group_joins_it() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    ask(&mut h, 1, "u", Some("U"));
    tool(&mut h, read("a", "a.rs"));
    tool(&mut h, read("b", "b.rs"));
    let lines = visible(&mut h);
    assert_eq!(lines[1], "▸    Read files", "{lines:?}");
    assert_eq!(lines[2], "Permission: U");
    h.keys("C-c C-y");
    assert_eq!(visible(&mut h).len(), 5);
}

#[test]
fn line_motion_passes_a_folded_group_and_search_and_copy_reach_it() {
    let mut h = grouped();
    h.keys("M-< C-n C-n");
    assert_eq!(h.core.current().text().line_to_char(2), h.core.point());
    h.keys("C-n");
    assert!(visible(&mut h)[1].starts_with('▸'));
    let working = h.core.text().find("Working").unwrap();
    let chars = h.core.text()[..working].chars().count();
    h.keys("C-n");
    assert_eq!(
        h.core.point(),
        chars,
        "the working line, past the blank one"
    );
    h.keys("C-p C-p");
    assert_eq!(h.core.current().text().line_to_char(2), h.core.point());
}

#[test]
fn search_lands_in_a_folded_group_and_tab_opens_it() {
    let mut h = grouped();
    h.keys("C-s b . r s RET");
    let line = h.core.current().line_col(h.core.point()).unwrap().0;
    let text = h.core.text();
    assert!(text.lines().nth(line).unwrap().contains("Read b.rs"));
    h.keys("TAB");
    assert!(h.core.text().contains("▾    Read files"));
    h.keys("M-<");
    assert_eq!(visible(&mut h)[3], "  ▸ ✓   Read b.rs 0.0s");
}

#[test]
fn a_region_over_a_folded_group_copies_its_rows() {
    let mut h = grouped();
    h.keys("M-< C-SPC C-n C-n C-n C-n M-w M-> C-y");
    let prompt = h.core.text();
    let prompt = prompt.rsplit("\n> ").next().unwrap();
    assert!(prompt.contains("Read a.rs") && prompt.contains("Read b.rs"));
}

#[test]
fn terminal_output_reaches_a_hidden_call() {
    let mut h = session();
    h.keys("a RET");
    tool(&mut h, read("a", "a.rs"));
    tool(&mut h, command());
    output(&mut h, "terminal_output", "t", "one\n");
    assert_eq!(visible(&mut h).len(), 4);
    output(&mut h, "terminal_output", "t", "two\n");
    assert_eq!(visible(&mut h).len(), 4);
    on_line(&mut h, "▸    Read");
    h.keys("TAB");
    on_line(&mut h, "▸ ●    Running");
    h.keys("TAB");
    let lines = visible(&mut h);
    assert!(lines.contains(&"two".to_string()), "{lines:?}");
}

#[test]
fn ret_on_hidden_text_shows_no_output() {
    let mut h = grouped();
    on_line(&mut h, "▸ ✓    Read a.rs");
    h.keys("RET");
    assert_eq!(h.core.message(), None);
    assert!(ops(&mut h).is_empty());
}

#[test]
fn replayed_calls_group_without_a_duration() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    let loading = h.core.agents.get_mut(&AgentId(0)).unwrap();
    loading.load = Some((Some("old".into()), loading.buffer));
    update(
        &mut h,
        0,
        SessionUpdate::UserMessageChunk(ContentChunk::new("old".into())),
    );
    thought(&mut h, "hmm\nmore");
    tool(&mut h, read("a", "a.rs"));
    tool(&mut h, read("b", "b.rs"));
    assert_eq!(visible(&mut h)[1], "▸    Read files");
    on_line(&mut h, "▸    Read");
    h.keys("TAB");
    assert_eq!(
        visible(&mut h)[2..5],
        ["  ▸ hmm", "  ▸ ✓   Read a.rs", "  ▸ ✓   Read b.rs"]
    );
}

#[test]
fn replayed_tables_are_drawn_and_thoughts_keep_theirs() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    let loading = h.core.agents.get_mut(&AgentId(0)).unwrap();
    loading.load = Some((Some("old".into()), loading.buffer));
    thought(&mut h, "| t |\n|---|");
    chunk(&mut h, "| a |\n|---|\n| b |\n");
    tool(&mut h, ls().status(ToolCallStatus::Completed));
    assert_eq!(
        h.core.text(),
        format!("| t |\n|---|\n\n a \n b \n\n▸ ✓    Ran ls\n{LS_INPUT}\n\n> ")
    );
}

#[test]
fn replayed_history_shows_no_times_and_later_updates_do() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    h.clock("14:01:00");
    let loading = h.core.agents.get_mut(&AgentId(0)).unwrap();
    loading.load = Some((Some("old".into()), loading.buffer));
    let user = ContentChunk::new("old".into());
    update(&mut h, 0, SessionUpdate::UserMessageChunk(user));
    thought(&mut h, "hmm");
    h.advance(secs(5));
    chunk(&mut h, "reply");
    tool(&mut h, ls().status(ToolCallStatus::Completed));
    assert_eq!(
        h.core.text(),
        format!("> old\n\nhmm\n\nreply\n\n▸ ✓    Ran ls\n{LS_INPUT}\n\n> ")
    );
    ready(&mut h, 0);
    h.advance(secs(1));
    let later = ToolCallUpdateFields::new().title("ls".to_string());
    tool_update(&mut h, "t", later);
    assert!(h.core.text().contains("Ran ls\n"), "no arrival, no time");
    chunk(&mut h, "live");
    h.keys("a RET");
    assert!(h.core.text().contains("live\n14:01:06\n"));
}

#[test]
fn ret_sends_only_a_ready_idle_session_with_input() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    h.keys("x RET");
    assert_eq!(h.core.message(), Some("claude is not ready"));
    ready(&mut h, 0);
    h.keys("RET");
    assert_eq!(ops(&mut h), [r#"prompt 0 s "x""#]);
    h.keys("y RET");
    assert_eq!(h.core.message(), Some("claude is busy"));
    assert!(ops(&mut h).is_empty());
    assert_eq!(h.core.text(), "> x\n\nWorking for 0s\n\n> y");
    prompted(&mut h, Ok(StopReason::EndTurn));
    // Only from the prompt area.
    h.keys("M-< RET M->");
    assert!(ops(&mut h).is_empty());
    h.keys("DEL SPC C-j RET");
    assert!(ops(&mut h).is_empty());
    exited(&mut h, 0, 1);
    h.keys("RET");
    assert_eq!(h.core.message(), Some("claude session has ended"));
}

#[test]
fn c_j_and_s_ret_insert_newlines() {
    let mut h = session();
    h.keys("a C-j b S-RET c");
    assert_eq!(h.core.text(), "> a\nb\nc");
    assert!(ops(&mut h).is_empty());
}

#[test]
fn prompt_lines_after_the_first_start_under_its_text() {
    let mut h = session();
    h.keys("s e n t C-j e c h o RET");
    prompted(&mut h, Ok(StopReason::EndTurn));
    let echo = [drawn_line(&mut h, "sent"), drawn_line(&mut h, "echo")];
    assert_eq!(echo.map(|l| l.indent()), [0, 2], "the sent prompt too");
    h.keys("o n e C-j t w o C-j");
    let mut frame = h.joined_frame(800., 600.);
    let last = frame.windows[0]
        .display
        .lines
        .pop()
        .expect("the prompt's last line");
    let indents = [
        drawn_line(&mut h, "one").indent(),
        drawn_line(&mut h, "two").indent(),
    ];
    assert_eq!(
        (indents, last.indent()),
        ([0, 2], 2),
        "an empty last line too"
    );
}

#[test]
fn a_turn_that_ends_otherwise_says_so() {
    let mut h = session();
    h.keys("a RET");
    prompted(&mut h, Ok(StopReason::Refusal));
    assert_eq!(
        h.core.text(),
        "> a\n\nTurn ended: refusal\n\nWorked 0s\n\n> "
    );
    h.keys("b RET");
    chunk(&mut h, "B");
    prompted(&mut h, Err("Internal error"));
    assert!(
        h.core
            .text()
            .ends_with("> b\n\nB\n\nError: Internal error\n\nWorked 0s\n\n> ")
    );
    // A late answer, with no turn running, adds nothing.
    prompted(&mut h, Ok(StopReason::MaxTokens));
    assert!(
        h.core
            .text()
            .ends_with("Error: Internal error\n\nWorked 0s\n\n> ")
    );
}

#[test]
fn c_c_c_c_cancels_a_running_turn() {
    let mut h = session();
    h.keys("C-c C-c");
    assert_eq!(h.core.message(), Some("No turn is running"));
    h.keys("a RET C-c C-c");
    assert_eq!(ops(&mut h), [r#"prompt 0 s "a""#, "cancel 0 s"]);
    prompted(&mut h, Ok(StopReason::Cancelled));
    assert_eq!(
        h.core.text(),
        "> a\n\nTurn ended: cancelled\n\nWorked 0s\n\n> "
    );
}

#[test]
fn point_never_rests_on_the_prompt_marker() {
    let mut h = session();
    chunk(&mut h, "Hi");
    h.keys("a b");
    let start = h.core.current().prompt_start();
    h.keys("C-a");
    assert_eq!(h.point(), start, "C-a stops after `> `");
    h.keys("C-b");
    assert_eq!(h.point(), start, "C-b stops at the prompt area's start");
    h.keys("C-p C-a C-n");
    assert_eq!(
        h.point(),
        start,
        "a line motion onto the marker lands after it"
    );
    h.keys("C-p");
    assert!(
        h.point() < start - PROMPT.len(),
        "C-p still leaves the prompt"
    );
}

#[test]
fn the_transcript_is_read_only_and_the_prompt_area_edits() {
    let mut h = session();
    h.keys("a RET");
    chunk(&mut h, "Hi");
    let transcript = h.core.text();
    h.keys("M-< C-d");
    assert_eq!(h.core.message(), Some("Text is read-only"));
    h.keys("M-> DEL");
    assert_eq!(h.core.message(), Some("Text is read-only"));
    assert_eq!(h.core.text(), transcript);
    h.keys("d r a f t DEL");
    chunk(&mut h, " there");
    assert_eq!(h.core.text(), "> a\n\nHi there\n\nWorking for 0s\n\n> draf");
    assert_eq!(h.core.point(), h.core.text().chars().count());
    h.keys("C-/");
    assert_eq!(
        h.core.text(),
        "> a\n\nHi there\n\nWorking for 0s\n\n> draft"
    );
}

#[test]
fn killing_a_workspace_stops_its_agents_without_a_crash_note() {
    let mut h = work();
    let work = h.core.workspace().unwrap();
    h.new_workspace("fix", "home");
    h.keys("M-x agent-start RET claude RET");
    ready(&mut h, 0);
    ops(&mut h);
    let fix = h.core.workspace().unwrap();
    h.core.switch_workspace(work).unwrap();
    h.core.kill_workspace(fix).unwrap();
    h.send(Msg::Tick);
    assert_eq!(ops(&mut h), ["kill 0"]);
    h.send(Msg::AgentExited {
        id: AgentId(0),
        code: 0,
        signal: Some("Terminated: 15".into()),
    });
    assert!(!messages(&h).contains("Terminated"));
    assert!(!h.core.live_agents());
}

#[test]
fn killing_the_session_buffer_asks_first_and_the_log_does_not() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    fn kill(h: &mut Harness, name: &str) {
        let strings = vec![name.into()];
        h.call(
            "kill-buffer",
            Args {
                strings,
                ..Args::default()
            },
        );
    }
    kill(&mut h, "*claude stderr*<work>");
    assert!(h.core.minibuffer().is_none());
    assert!(h.core.buffers.find("*claude stderr*<work>").is_none());
    assert!(ops(&mut h).is_empty());
    // Stderr without a log goes nowhere.
    h.send(Msg::AgentStderr {
        id: AgentId(0),
        line: "lost".into(),
    });
    kill(&mut h, "*claude*<work>");
    assert_eq!(
        h.core.minibuffer().unwrap().prompt,
        "Buffer *claude*<work> has a running process; kill it? "
    );
    assert_eq!(listed(&mut h).0, ["no", "yes"]);
    h.keys("yes RET");
    assert!(h.core.buffers.find("*claude*<work>").is_none());
    assert_eq!(ops(&mut h), ["kill 0"]);
    // A request the agent sent before it died is cancelled.
    ask(&mut h, 1, "c", Some("C"));
    assert_eq!(ops(&mut h), ["reply 0 1 cancelled"]);
}

#[test]
fn quitting_asks_about_live_agents() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ready(&mut h, 0);
    ops(&mut h);
    h.keys("C-x C-c");
    assert_eq!(
        h.core.minibuffer().unwrap().prompt,
        "Active processes exist; kill them and exit anyway? "
    );
    h.keys("RET");
    assert!(!h.quit && ops(&mut h).is_empty());
    h.keys("C-x C-c yes RET");
    assert!(h.quit);
    // An ended session doesn't ask.
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    exited(&mut h, 0, 1);
    h.keys("C-x C-c");
    assert!(h.quit);
}

/// Sends session 0 permission request `request` for tool call `call` (titled `title`), with
/// options `allow` (`allow_once`), `always` (`allow_always`) and `reject` (`reject_once`).
fn ask(h: &mut Harness, request: i64, call: &str, title: Option<&str>) {
    use PermissionOptionKind::*;
    let fields = ToolCallUpdateFields::new().title(title.map(String::from));
    let options = [
        ("allow", "Yes", AllowOnce),
        ("always", "Always", AllowAlways),
    ]
    .into_iter()
    .chain([("reject", "No", RejectOnce)])
    .map(|(id, name, kind)| PermissionOption::new(id, name, kind))
    .collect();
    let ask =
        RequestPermissionRequest::new("s", ToolCallUpdate::new(call.to_string(), fields), options);
    h.send(Msg::AgentRequest {
        id: AgentId(0),
        request: RequestId::Number(request),
        body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
    });
}

/// The block `ask` shows for a request titled `title`.
fn asked(title: &str) -> String {
    format!(
        "Permission: {title}\n1. Yes (allow_once)\n2. Always (allow_always)\n3. No (reject_once)"
    )
}

#[test]
fn a_permission_request_shows_inline_and_waits() {
    let mut h = session();
    h.keys("a RET");
    chunk(&mut h, "Editing.");
    ops(&mut h);
    ask(&mut h, 4, "c", Some("Edit a.txt"));
    assert!(ops(&mut h).is_empty(), "nothing answered yet");
    let block = asked("Edit a.txt");
    assert_eq!(
        h.core.text(),
        format!("> a\n\nEditing.\n\n{block}\n\nWorking for 0s\n\n> ")
    );
    let rule = Prop::Line(Decoration::Rule(FaceName::SessionThought));
    let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
    let props: Vec<_> = (props(&h).into_iter())
        .filter(|(_, p)| *p == rule || *p == dim)
        .map(|(t, _)| t)
        .collect();
    assert_eq!(
        props,
        [
            "P",
            "1",
            " (allow_once)",
            "2",
            " (allow_always)",
            "3",
            " (reject_once)",
            "Working for 0s"
        ]
    );
}

#[test]
fn a_title_comes_from_the_request_the_shown_call_or_the_id() {
    let mut h = session();
    let call = ToolCall::new("c", "Read\nmore");
    update(&mut h, 0, SessionUpdate::ToolCall(call));
    ask(&mut h, 1, "c", None);
    ask(&mut h, 2, "d", None);
    let text = h.core.text();
    assert!(text.contains("Permission: Read …\n"), "{text}");
    assert!(text.contains("Permission: d\n"), "{text}");
}

#[test]
fn the_keys_answer_the_oldest_request_first() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    ask(&mut h, 1, "a", Some("A"));
    ask(&mut h, 2, "b", Some("B"));
    ask(&mut h, 3, "c", Some("C"));
    h.keys("C-c C-y");
    assert_eq!(ops(&mut h), ["reply 0 1 allow"]);
    h.keys("C-c C-k");
    assert_eq!(ops(&mut h), ["reply 0 2 reject"]);
    let c = asked("C");
    assert_eq!(
        h.core.text(),
        format!("> a\n\n✓ allowed once\n\n✗ rejected\n\n{c}\n\nWorking for 0s\n\n> ")
    );
    h.keys("C-c C-y");
    assert_eq!(ops(&mut h), ["reply 0 3 allow"]);
    h.keys("C-c C-y");
    assert_eq!(h.core.message(), Some("No permission request is pending"));
    assert!(ops(&mut h).is_empty());
}

#[test]
fn ret_on_an_option_line_picks_it() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    ask(&mut h, 1, "a", Some("A"));
    ask(&mut h, 2, "b", Some("B"));
    // The newer request's second option, then the older one's header, which sends nothing.
    h.keys("C-p C-p C-p C-p C-p RET");
    assert_eq!(ops(&mut h), ["reply 0 2 always"]);
    assert!(h.core.text().contains("✓ allowed always\n"));
    h.keys("M-< C-n C-n RET");
    assert_eq!(h.core.message(), None);
    assert!(ops(&mut h).is_empty());
    h.keys("C-n C-n C-n RET");
    assert_eq!(ops(&mut h), ["reply 0 1 reject"]);
    assert_eq!(
        h.core.text(),
        "> a\n\n✗ rejected\n\n✓ allowed always\n\nWorking for 0s\n\n> "
    );
}

#[test]
fn an_answered_permission_line_is_dim_and_has_no_rule() {
    use PermissionOptionKind::*;
    for (kind, line) in [
        (AllowOnce, "✓ allowed once"),
        (AllowAlways, "✓ allowed always"),
        (RejectOnce, "✗ rejected"),
        (RejectAlways, "✗ rejected always"),
    ] {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        let ask = RequestPermissionRequest::new(
            "s",
            ToolCallUpdate::new("c", ToolCallUpdateFields::new()),
            vec![PermissionOption::new("o", "Option", kind)],
        );
        h.send(Msg::AgentRequest {
            id: AgentId(0),
            request: RequestId::Number(1),
            body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
        });
        h.keys("C-p C-p C-p C-p RET");
        assert_eq!(ops(&mut h), ["reply 0 1 o"]);
        assert_eq!(
            h.core.text(),
            format!("> a\n\n{line}\n\nWorking for 0s\n\n> ")
        );
        let rule = Prop::Line(Decoration::Rule(FaceName::SessionThought));
        let dim = Prop::Named(vec![FaceRef::Face(FaceName::SessionThought)]);
        let props = props(&h);
        assert!(props.contains(&(line.to_string(), dim)), "{props:?}");
        assert!(!props.iter().any(|(_, p)| *p == rule), "{props:?}");
    }
}

#[test]
fn a_turn_ending_cancels_its_pending_requests() {
    let ends = [
        (Ok(StopReason::EndTurn), ""),
        (Ok(StopReason::Refusal), "Turn ended: refusal\n\n"),
        (Err("boom"), "Error: boom\n\n"),
    ];
    for (result, end) in ends {
        let mut h = session();
        h.keys("a RET");
        ops(&mut h);
        ask(&mut h, 1, "a", Some("A"));
        ask(&mut h, 2, "b", Some("B"));
        prompted(&mut h, result);
        assert_eq!(ops(&mut h), ["reply 0 1 cancelled", "reply 0 2 cancelled"]);
        assert_eq!(
            h.core.text(),
            format!("> a\n\n✗ cancelled\n\n✗ cancelled\n\n{end}Worked 0s\n\n> ")
        );
    }
}

#[test]
fn a_missing_option_kind_answers_nothing() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    let options = vec![PermissionOption::new(
        "always",
        "Always",
        PermissionOptionKind::AllowAlways,
    )];
    let ask = RequestPermissionRequest::new(
        "s",
        ToolCallUpdate::new("c", ToolCallUpdateFields::new().title("Run".to_string())),
        options,
    );
    h.send(Msg::AgentRequest {
        id: AgentId(0),
        request: RequestId::Number(1),
        body: Request(Box::new(AgentRequest::RequestPermissionRequest(ask))),
    });
    h.keys("C-c C-y");
    assert_eq!(h.core.message(), Some("No allow_once option for Run"));
    h.keys("C-c C-k");
    assert_eq!(h.core.message(), Some("No reject_once option for Run"));
    assert!(ops(&mut h).is_empty());
}

#[test]
fn cancelling_answers_pending_requests_and_ends_the_turn_and_an_exit_drops_them() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    ask(&mut h, 1, "a", Some("A"));
    ask(&mut h, 2, "b", Some("B"));
    h.keys("C-c C-c");
    assert_eq!(
        ops(&mut h),
        ["cancel 0 s", "reply 0 1 cancelled", "reply 0 2 cancelled"]
    );
    let cancelled = "✗ cancelled\n\n✗ cancelled";
    assert_eq!(
        h.core.text(),
        format!("> a\n\n{cancelled}\n\nWorking for 0s\n\n> ")
    );
    prompted(&mut h, Ok(StopReason::Cancelled));
    let ended = format!("> a\n\n{cancelled}\n\nTurn ended: cancelled\n\nWorked 0s\n\n> ");
    assert_eq!(h.core.text(), ended);
    h.keys("b RET");
    ops(&mut h);
    ask(&mut h, 3, "c", Some("C"));
    exited(&mut h, 0, 1);
    assert_eq!(ops(&mut h), ["kill 0"]);
    assert!(h.core.text().ends_with("✗ cancelled\n\nWorked 0s\n\n> "));
    h.keys("C-c C-y");
    assert_eq!(h.core.message(), Some("No permission request is pending"));
    // A request after the exit is cancelled and not shown.
    let text = h.core.text();
    ask(&mut h, 4, "d", Some("D"));
    assert_eq!(ops(&mut h), ["reply 0 4 cancelled"]);
    assert_eq!(h.core.text(), text);
}

#[test]
fn other_requests_and_unready_sessions_are_refused_and_ended_ones_cancelled() {
    use agent_client_protocol_schema::v1::CreateTerminalRequest;
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    ask(&mut h, 1, "c", Some("Early"));
    assert_eq!(ops(&mut h), ["reply 0 1 Method not found"]);
    ready(&mut h, 0);
    ops(&mut h);
    let terminal = CreateTerminalRequest::new("s", "ls");
    h.send(Msg::AgentRequest {
        id: AgentId(0),
        request: RequestId::Number(2),
        body: Request(Box::new(AgentRequest::CreateTerminalRequest(terminal))),
    });
    assert_eq!(ops(&mut h), ["reply 0 2 Method not found"]);
    exited(&mut h, 0, 0);
    ops(&mut h);
    ask(&mut h, 3, "c", Some("Late"));
    assert_eq!(ops(&mut h), ["reply 0 3 cancelled"]);
    assert_eq!(h.core.text(), "> ");
}

/// Sends session 0 question `request`, a Claude-shaped one with options A (`First`) and B
/// and an own-answer field, on tool call `t` with `fields` replacing its properties.
fn question(h: &mut Harness, request: i64, fields: Option<Value>) {
    let fields = fields.unwrap_or_else(|| {
        serde_json::json!({
            "question_0": {"type": "string", "oneOf": [
                {"const": "a", "title": "A", "description": "First"},
                {"const": "b", "title": "B"}]},
            "question_0_custom": {"type": "string", "title": "Other"},
        })
    });
    let ask = serde_json::from_value(serde_json::json!({
        "sessionId": "s", "toolCallId": "t", "mode": "form", "message": "Which?",
        "requestedSchema": {"type": "object", "properties": fields},
    }))
    .unwrap();
    h.send(Msg::AgentRequest {
        id: AgentId(0),
        request: RequestId::Number(request),
        body: Request(Box::new(AgentRequest::CreateElicitationRequest(ask))),
    });
}

const QUESTION: &str = " Question  Which?\n   1   A  First\n   2   B\n   o   Own answer\n  \
    1-2 answer   RET answer with the row at point   C-c C-k skip";

/// Point's offset at the column before the chip of the line holding `text`.
fn on_chip(h: &Harness, text: &str) -> usize {
    let at = h.core.text().find(text).unwrap();
    h.core.text()[..at].chars().count() + 1
}

#[test]
fn a_question_arriving_at_the_bottom_shows_whole() {
    let mut h = session();
    h.keys("a RET");
    chunk(&mut h, &"line\n\n".repeat(20));
    shown(&mut h, 10);
    question(&mut h, 1, None);
    let lines = shown(&mut h, 10);
    assert!(
        lines.iter().any(|l| l.contains("Question  Which?")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("C-c C-k skip")),
        "{lines:?}"
    );
    // From the prompt, C-c C-q shows the header too.
    h.keys("C-c C-k x");
    question(&mut h, 2, None);
    let before = shown(&mut h, 6);
    assert!(
        !before.iter().any(|l| l.contains("Question  Which?")),
        "{before:?}"
    );
    h.keys("C-c C-q");
    let lines = shown(&mut h, 6);
    assert!(
        lines.iter().any(|l| l.contains("Question  Which?")),
        "{lines:?}"
    );
}

fn accepted(request: i64, content: &str) -> String {
    format!(r#"reply 0 {request} {{"action":"accept","content":{content}}}"#)
}

#[test]
fn a_question_shows_and_a_digit_answers_it() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    question(&mut h, 1, None);
    assert!(ops(&mut h).is_empty(), "nothing answered yet");
    assert_eq!(
        h.core.text(),
        format!("> a\n\n{QUESTION}\n\nWorking for 0s\n\n> ")
    );
    assert_eq!(h.point(), on_chip(&h, "   1   A"), "on its first option");
    // Frame 6d: a box one column in, and the row at point highlighted.
    assert_eq!(
        drawn_line(&mut h, "1   A").decorations,
        [
            Decoration::Indent(1),
            Decoration::Background(FaceName::Permission),
            Decoration::Background(FaceName::Region)
        ]
    );
    assert_eq!(
        drawn_line(&mut h, "Question").decorations,
        [
            Decoration::Indent(1),
            Decoration::Background(FaceName::Permission)
        ]
    );
    assert_eq!(
        h.display(5).mode_line.right[0],
        vec![("● awaiting answer".into(), FaceName::StatusWaiting)]
    );
    h.keys("3");
    assert!(ops(&mut h).is_empty(), "past the options");
    h.keys("2");
    assert_eq!(ops(&mut h), [accepted(1, r#"{"question_0":"b"}"#)]);
    assert_eq!(
        h.core.text(),
        "> a\n\n▸ ✓ Which?  B\n question_0  B\n\nWorking for 0s\n\n> "
    );
    assert!(
        !visible(&mut h).contains(&" question_0  B".into()),
        "folded"
    );
    assert_eq!(
        h.point(),
        h.core.text().chars().count(),
        "back in the prompt"
    );
    h.keys("2");
    assert!(h.core.text().ends_with("> 2"), "digits type again");
}

#[test]
fn ret_and_an_own_answer_answer_a_question() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    question(&mut h, 1, None);
    h.keys("RET");
    assert_eq!(ops(&mut h), [accepted(1, r#"{"question_0":"a"}"#)]);
    question(&mut h, 2, None);
    h.keys("o");
    assert_eq!(h.core.minibuffer().unwrap().prompt, "Own answer: ");
    h.keys("RET");
    assert_eq!(h.core.message(), Some("Answer is empty"));
    h.keys("o Redis RET");
    assert_eq!(
        ops(&mut h),
        [accepted(2, r#"{"question_0_custom":"Redis"}"#)]
    );
    assert!(
        h.core.text().contains("▸ ✓ Which?  Redis\n"),
        "{}",
        h.core.text()
    );
    // RET on the own-answer row asks too.
    let codex = serde_json::json!({
        "env": {"type": "string", "title": "Where?", "oneOf": [
            {"const": "staging", "title": "staging"},
            {"const": "None of the above", "title": "None of the above"}]},
        "env_note": {"type": "string", "_meta": {"codex": {"role": "user_note"}}},
    });
    question(&mut h, 3, Some(codex));
    assert!(
        h.core
            .text()
            .contains(" Question  Which?\n Where?\n   1   staging\n   o   Own answer\n")
    );
    h.keys("C-n RET qa RET");
    assert_eq!(
        ops(&mut h),
        [accepted(
            3,
            r#"{"env":"None of the above","env_note":"qa"}"#
        )]
    );
}

#[test]
fn a_question_while_typing_waits_and_c_c_c_q_goes_to_it() {
    let mut h = session();
    h.keys("draft");
    question(&mut h, 1, None);
    assert_eq!(
        h.core.message(),
        Some("Question waiting: C-c C-q goes to it")
    );
    h.keys("1 o");
    assert!(ops(&mut h).is_empty());
    assert!(h.core.text().ends_with("> draft1o"));
    h.keys("C-c C-q");
    assert_eq!(h.point(), on_chip(&h, "   1   A"));
    h.keys("1");
    assert_eq!(ops(&mut h), [accepted(1, r#"{"question_0":"a"}"#)]);
    h.keys("C-c C-q");
    assert_eq!(h.core.message(), Some("No question is pending"));
}

/// codex-acp's async question, `request`, which doesn't stop the turn.
fn async_question(h: &mut Harness, request: i64) {
    let ask = serde_json::from_value(serde_json::json!({
        "sessionId": "s", "mode": "form", "message": "Which env?",
        "requestedSchema": {"type": "object", "properties": {
            "env": {"type": "string", "oneOf": [{"const": "s", "title": "staging"}]}}},
        "_meta": {"codex": {"delivery": "async"}},
    }))
    .unwrap();
    h.send(Msg::AgentRequest {
        id: AgentId(0),
        request: RequestId::Number(request),
        body: Request(Box::new(AgentRequest::CreateElicitationRequest(ask))),
    });
}

#[test]
fn a_nonblocking_question_lets_the_turn_go_on() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    async_question(&mut h, 1);
    let end = h.core.text().chars().count();
    assert_eq!(h.point(), end, "point stays in the prompt");
    assert_eq!(
        h.core.message(),
        Some("Question waiting: C-c C-q goes to it")
    );
    let right = |h: &mut Harness| h.display(5).mode_line.right;
    assert_eq!(
        right(&mut h)[..2],
        [
            vec![("● working".into(), FaceName::StatusRunning)],
            vec![("1 question".into(), FaceName::StatusWaiting)]
        ]
    );
    assert_eq!(
        h.core.session_state("claude", &"s".into()),
        Some(("● working", FaceName::StatusRunning))
    );
    for t in ["t1", "t2", "t3"] {
        tool(&mut h, ToolCall::new(t, "Read"));
    }
    let lines = shown(&mut h, 20);
    let at = |t: &str| lines.iter().position(|l| l.contains(t)).unwrap();
    assert!(
        at("go to it") < at("Using tools"),
        "calls group below: {lines:?}"
    );
    chunk(&mut h, "Later");
    let text = h.core.text();
    let (block, later) = (
        text.find("C-c C-q go to it").unwrap(),
        text.find("Later").unwrap(),
    );
    assert!(block < later, "output goes in below the block: {text}");
    async_question(&mut h, 2);
    assert_eq!(
        right(&mut h)[1],
        vec![("2 questions".into(), FaceName::StatusWaiting)]
    );
    h.keys("C-c C-q 1");
    assert_eq!(ops(&mut h), [accepted(1, r#"{"env":"s"}"#)]);
    async_question(&mut h, 3);
    h.send(Msg::AgentWithdrawn {
        id: AgentId(0),
        request: RequestId::Number(3),
    });
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert_eq!(ops(&mut h), [r#"reply 0 2 {"action":"cancel"}"#]);
    let text = h.core.text();
    assert!(
        text.contains("▸ ○ Which env?  Closed, the turn ended first\n"),
        "{text}"
    );
    assert!(text.contains("▸ ○ Which env?  Closed\n"), "{text}");
    assert_eq!(right(&mut h).len(), 0, "{:?}", right(&mut h));
}

#[test]
fn c_c_c_k_answers_the_request_at_point_first() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    question(&mut h, 1, None);
    ask(&mut h, 2, "c", Some("Edit A"));
    point_in(&mut h, "Edit A", 0);
    h.keys("C-c C-k");
    assert_eq!(
        ops(&mut h),
        ["reply 0 2 reject"],
        "the question stays pending"
    );

    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    question(&mut h, 1, None);
    question(&mut h, 2, None);
    let all = h.core.text();
    let second = all[..all.rfind("Which?").unwrap()].chars().count();
    h.ctx().goto(second).unwrap();
    h.keys("C-c C-k");
    assert_eq!(ops(&mut h), [r#"reply 0 2 {"action":"decline"}"#]);
    h.keys("C-c C-k");
    assert_eq!(ops(&mut h), [r#"reply 0 1 {"action":"decline"}"#]);
}

#[test]
fn skipping_cancelling_and_withdrawing_questions() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    question(&mut h, 1, None);
    h.keys("C-c C-k");
    assert_eq!(ops(&mut h), [r#"reply 0 1 {"action":"decline"}"#]);
    question(&mut h, 2, None);
    ask(&mut h, 3, "c", Some("Edit"));
    h.keys("C-c C-y");
    assert_eq!(
        ops(&mut h),
        ["reply 0 3 allow"],
        "C-c C-y passes the question by"
    );
    question(&mut h, 4, None);
    h.send(Msg::AgentWithdrawn {
        id: AgentId(0),
        request: RequestId::Number(4),
    });
    assert!(ops(&mut h).is_empty(), "the service answered it");
    h.keys("C-c C-c");
    assert_eq!(
        ops(&mut h),
        ["cancel 0 s", r#"reply 0 2 {"action":"cancel"}"#]
    );
    assert_eq!(
        h.core.text(),
        "> a\n\n▸ – Which?  Skipped\n\n▸ ✗ Which?  Cancelled\n\n✓ allowed once\n\n▸ ○ Which?  Closed\n\n\
         Working for 0s\n\n> "
    );
    prompted(&mut h, Ok(StopReason::Cancelled));
    h.keys("b RET");
    ops(&mut h);
    question(&mut h, 5, None);
    prompted(&mut h, Ok(StopReason::EndTurn));
    assert_eq!(ops(&mut h), [r#"reply 0 5 {"action":"cancel"}"#]);
}

#[test]
fn killing_the_buffer_cancels_pending_requests_before_the_agent_stops() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    question(&mut h, 1, None);
    ask(&mut h, 2, "c", Some("Edit"));
    let strings = vec!["*claude*<work>".into()];
    h.call(
        "kill-buffer",
        Args {
            strings,
            ..Args::default()
        },
    );
    h.keys("yes RET");
    assert_eq!(
        ops(&mut h),
        [
            r#"reply 0 1 {"action":"cancel"}"#,
            "reply 0 2 cancelled",
            "kill 0"
        ]
    );
}

#[test]
fn forms_it_cant_draw_are_declined_and_ended_sessions_cancel() {
    let mut h = session();
    let form = serde_json::json!({"ok": {"type": "boolean"}});
    question(&mut h, 1, Some(form));
    assert_eq!(ops(&mut h), [r#"reply 0 1 {"action":"decline"}"#]);
    assert_eq!(
        h.core.message(),
        Some("Can't show claude's question yet; declined")
    );
    assert_eq!(h.core.text(), "> ");
    exited(&mut h, 0, 0);
    ops(&mut h);
    question(&mut h, 2, None);
    assert_eq!(ops(&mut h), [r#"reply 0 2 {"action":"cancel"}"#]);
}

#[test]
fn a_question_stands_in_for_its_calls_row_under_the_fold() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    tool(&mut h, read("a", "a.rs"));
    tool(&mut h, call("t", ToolKind::Other));
    visible(&mut h);
    question(&mut h, 1, None);
    let lines = visible(&mut h);
    assert_eq!(
        lines[1..4],
        [
            "▸    Read files, using tools",
            " Question  Which?",
            "   1   A  First"
        ],
        "{lines:?}"
    );
    h.keys("1");
    assert_eq!(visible(&mut h).len(), 4, "{:?}", visible(&mut h));
}

/// The faces on the text `part` of the line starting `line`.
fn faces_on(h: &Harness, line: &str, part: &str) -> Vec<FaceName> {
    let text = h.core.text();
    let at = text.find(line).unwrap() + line.find(part).unwrap();
    let start = text[..at].chars().count();
    let range = start..start + part.chars().count();
    (h.core.current().properties())
        .filter(|(r, _)| *r == range)
        .filter_map(|(_, p)| match p {
            Prop::Named(faces) => match faces[..] {
                [FaceRef::Face(f)] => Some(f),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn an_answered_line_s_time_goes_under_it_and_into_its_row_in_a_group() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    h.clock("14:03:00");
    tool(&mut h, read("a", "a.rs"));
    question(&mut h, 1, None);
    h.keys("1");
    ops(&mut h);
    assert_eq!(visible(&mut h)[3..5], ["▸ ✓ Which?  A", "14:03:00"]);
    tool(&mut h, read("b", "b.rs"));
    on_line(&mut h, "▸    Read");
    h.keys("TAB");
    assert_eq!(visible(&mut h)[3], "▸ ✓ Which?  A 14:03:00");
    assert!(!h.core.text().contains("\n14:03:00\n"));
    let line = "▸ ✓ Which?  A 14:03:00";
    assert_eq!(faces_on(&h, line, "14:03:00"), [FaceName::Annotation]);
    on_line(&mut h, "▸ ✓ Which?");
    h.keys("TAB");
    assert_eq!(
        visible(&mut h)[3..5],
        ["▾ ✓ Which?  A 14:03:00", " question_0  A"]
    );
}

#[test]
fn each_outcome_shrinks_to_its_line_and_tab_opens_what_was_sent() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    h.clock("14:03:00");
    let choice = |title: &str, option: &str| serde_json::json!({"type": "string", "title": title, "oneOf": [{"const": option, "title": option}, {"const": "x", "title": "X"}]});
    let fields =
        serde_json::json!({"q_0": choice("Status", "Both"), "q_1": choice("Commit", "One")});
    question(&mut h, 1, Some(fields));
    h.keys("1 1 C-c C-y");
    ops(&mut h);
    let line = "▸ ✓ 2 questions  Both · One";
    assert!(
        h.core
            .text()
            .contains(&format!("{line}\n Status  Both\n Commit  One\n14:03:00\n"))
    );
    assert_eq!(faces_on(&h, line, "✓"), [FaceName::StatusRunning]);
    assert_eq!(faces_on(&h, line, "Both · One"), [FaceName::SessionThought]);
    assert_eq!(faces_on(&h, "14:03:00", "14:03:00"), [FaceName::Annotation]);
    assert_eq!(faces_on(&h, line, "▸"), [FaceName::SessionThought]);
    assert!(
        faces_on(&h, line, "2 questions").is_empty(),
        "the label is plain"
    );
    assert_eq!(visible(&mut h)[1], line);
    on_line(&mut h, "▸ ✓ 2 questions");
    h.keys("TAB");
    assert_eq!(
        visible(&mut h)[1..4],
        [
            "▾ ✓ 2 questions  Both · One",
            " Status  Both",
            " Commit  One"
        ]
    );
    assert_eq!(
        drawn_line(&mut h, "Commit  One").decorations,
        [
            Decoration::Indent(3),
            Decoration::Background(FaceName::SessionBlock)
        ]
    );
    assert_eq!(
        faces_on(&h, " Status  Both", "Status"),
        [FaceName::SessionThought]
    );
    h.keys("C-n C-e TAB");
    assert_eq!(visible(&mut h)[1], line);
    assert_eq!(visible(&mut h)[2..4], ["14:03:00", "Working for 0s"]);
    assert_eq!(h.point(), on_chip(&h, line) - 1, "back on the line");
    question(&mut h, 2, None);
    h.keys("C-c C-k");
    question(&mut h, 3, None);
    h.keys("C-c C-c");
    let skipped = "▸ – Which?  Skipped";
    assert_eq!(faces_on(&h, skipped, "–"), [FaceName::SessionThought]);
    assert_eq!(
        faces_on(&h, "▸ ✗ Which?  Cancelled", "✗"),
        [FaceName::StatusFailed]
    );
    on_line(&mut h, "▸ – Which?");
    h.keys("TAB");
    assert_eq!(
        h.core.message(),
        Some("No tool call here"),
        "nothing was sent"
    );
}

#[test]
fn an_answered_question_takes_its_calls_row() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    tool(&mut h, call("t", ToolKind::Other));
    question(&mut h, 1, None);
    h.keys("1");
    tool_update(
        &mut h,
        "t",
        ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
    );
    assert_eq!(
        shown(&mut h, 30)[..5],
        ["❯ a", "", "▸ ✓ Which?  A", "", "Working for 0s"]
    );
    tool(&mut h, read("a", "a.rs"));
    on_line(&mut h, "▸    ");
    h.keys("TAB");
    let lines = visible(&mut h);
    assert_eq!(
        lines[2..4],
        ["▸ ✓ Which?  A", "  ▸ ✓   Read a.rs 0.0s"],
        "{lines:?}"
    );
    assert_eq!(
        drawn_line(&mut h, "Which?  A").decorations,
        [Decoration::Indent(2)]
    );
    on_line(&mut h, "▸ ✓ Which?");
    h.keys("TAB");
    assert_eq!(visible(&mut h)[2..4], ["▾ ✓ Which?  A", " question_0  A"]);
    on_line(&mut h, "▾    ");
    h.keys("TAB");
    let lines = visible(&mut h);
    assert!(
        !lines.iter().any(|l| l.contains("Which?")),
        "folds away: {lines:?}"
    );
}

#[test]
fn a_replayed_question_call_shows_no_question() {
    let mut h = work();
    h.keys("M-x agent-start RET claude RET");
    ops(&mut h);
    let loading = h.core.agents.get_mut(&AgentId(0)).unwrap();
    loading.load = Some((Some("old".into()), loading.buffer));
    tool(
        &mut h,
        call("t", ToolKind::Other).status(ToolCallStatus::Completed),
    );
    assert_eq!(visible(&mut h)[0], "▸ ✓   Used t");
    assert!(!h.core.text().contains("Question"));
    let status = h.display(5).mode_line.right;
    assert!(!format!("{status:?}").contains("awaiting answer"));
}

/// Three Claude-shaped questions: `Status` (Prompt, Both, an own answer), `Tests` (a
/// multi-select of A and B) and `Commit` (One).
fn several(h: &mut Harness, request: i64) {
    let choice = |title: &str, d: &str, options: Value| serde_json::json!({"type": "string", "title": title, "description": d, "oneOf": options});
    let fields = serde_json::json!({
        "question_0": choice("Status", "Where?", serde_json::json!([
            {"const": "p", "title": "Prompt"}, {"const": "b", "title": "Both"}])),
        "question_0_custom": {"type": "string"},
        "question_1": {"type": "array", "title": "Tests", "description": "Which cases?", "maxItems": 1,
            "items": {"anyOf": [{"const": "a", "title": "A"}, {"const": "b", "title": "B"}]}},
        "question_2": choice("Commit", "One or many?", serde_json::json!([
            {"const": "1", "title": "One"}])),
    });
    question(h, request, Some(fields));
}

#[test]
fn several_questions_open_one_at_a_time_and_send_together() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    tool(&mut h, read("a", "a.rs"));
    tool(&mut h, call("t", ToolKind::Other));
    visible(&mut h);
    several(&mut h, 1);
    let lines = visible(&mut h);
    assert_eq!(
        lines[1..5],
        [
            "▸    Read files, using tools",
            " Questions  1 of 3",
            " ▾ ● Status  Where?",
            "      1   Prompt"
        ],
        "{lines:?}"
    );
    // Point before the open question's first chip.
    let before = |h: &Harness, row: &str| {
        h.core
            .text()
            .find(row)
            .map(|at| h.core.text()[..at].chars().count() + 1)
    };
    assert_eq!(Some(h.point()), before(&h, "      1   Prompt"));
    h.keys("2");
    assert!(
        ops(&mut h).is_empty(),
        "a single choice records, sending nothing"
    );
    let text = h.core.text();
    assert!(
        text.contains(
            " Questions  2 of 3\n ▸ ✓ Status  Both\n ▾ ● Tests   Which cases?\n      1   [ ] A\n"
        ),
        "{text}"
    );
    assert_eq!(Some(h.point()), before(&h, "      1   [ ] A"));
    h.keys("1 TAB");
    let text = h.core.text();
    assert!(
        text.contains(" ▸ ✓ Tests   A\n ▾ ● Commit  One or many?\n      1   One\n"),
        "{text}"
    );
    // Answering the last folds them all, sending nothing.
    h.keys("1");
    let text = h.core.text();
    assert!(
        text.contains(
            " Questions  3 of 3 answered\n ▸ ✓ Status  Both\n ▸ ✓ Tests   A\n ▸ ✓ Commit  One\n \
             TAB open  C-c C-y send all  C-c C-k skip all\n"
        ),
        "{text}"
    );
    assert_eq!(Some(h.point()), before(&h, " ▸ ✓ Commit"));
    h.keys("1");
    assert!(
        ops(&mut h).is_empty(),
        "a digit with none open does nothing"
    );
    h.keys("TAB");
    assert!(
        h.core
            .text()
            .contains(" Questions  1 of 3\n ▾ ● Status  Where?\n"),
        "TAB wraps"
    );
    let shown = visible(&mut h);
    assert_eq!(
        shown[1..3],
        ["▸    Read files, using tools", " Questions  1 of 3"],
        "TAB leaves the group folded: {shown:?}"
    );
    h.keys("o Redis RET");
    assert!(h.core.text().contains(" ▸ ✓ Status  Redis\n ▾ ● Tests"));
    h.keys("TAB TAB");
    assert!(
        h.core.text().contains("      o   Own answer  Redis\n"),
        "{}",
        h.core.text()
    );
    h.keys("o");
    assert_eq!(h.core.text(), "Redis");
    h.keys("RET");
    h.keys("2 C-c C-y");
    assert_eq!(h.core.message(), Some("Tests: Pick at most 1"));
    assert!(ops(&mut h).is_empty(), "nothing sent out of range");
    h.keys("2 C-c C-y");
    assert_eq!(
        ops(&mut h),
        [accepted(
            1,
            r#"{"question_0_custom":"Redis","question_1":["a"],"question_2":"1"}"#
        )]
    );
    let text = h.core.text();
    assert!(
        text.contains("▸ ✓ 3 questions  Redis · A · One\n"),
        "{text}"
    );
    assert_eq!(visible(&mut h).len(), 4, "the answer folds with its group");
    // TAB outside a block still opens the group.
    h.keys("M-< C-n C-n TAB");
    assert!(visible(&mut h).len() > 4, "{:?}", visible(&mut h));
}

#[test]
fn several_questions_skip_together() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    several(&mut h, 1);
    h.keys("1 C-c C-k");
    assert_eq!(ops(&mut h), [r#"reply 0 1 {"action":"decline"}"#]);
    assert!(
        h.core.text().contains("▸ – 3 questions  Skipped\n"),
        "{}",
        h.core.text()
    );
}

#[test]
fn permissions_outrank_questions_and_exits_and_withdrawals_close_both() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    question(&mut h, 1, None);
    ask(&mut h, 2, "c", Some("Edit"));
    assert_eq!(
        h.display(5).mode_line.right[0],
        vec![("● awaiting permission".into(), FaceName::StatusWaiting)]
    );
    h.send(Msg::AgentWithdrawn {
        id: AgentId(0),
        request: RequestId::Number(2),
    });
    assert_eq!(
        h.display(5).mode_line.right[0],
        vec![("● awaiting answer".into(), FaceName::StatusWaiting)]
    );
    exited(&mut h, 0, 1);
    assert!(!ops(&mut h).iter().any(|o| o.starts_with("reply")));
    assert!(
        h.core
            .text()
            .starts_with("> a\n\n▸ ✗ Which?  Cancelled\n\n○ withdrawn\n\n"),
        "{}",
        h.core.text()
    );
}

const MULTI: &str = " Question  Which?\n   1   [ ] A\n   2   [ ] B\n   3   [ ] C\n   o   [ ] \
    Own answer\n  1-3 tick   RET tick at point   C-c C-y send   C-c C-k skip\n";

/// A Claude-shaped multi-select with options A, B and C and an own answer, taking `bounds`.
fn multi_select(h: &mut Harness, request: i64, bounds: Value) {
    let mut field = serde_json::json!({"type": "array", "items": {"anyOf": [
        {"const": "a", "title": "A"}, {"const": "b", "title": "B"},
        {"const": "c", "title": "C"}]}});
    field
        .as_object_mut()
        .unwrap()
        .extend(bounds.as_object().unwrap().clone());
    let fields = serde_json::json!({
        "question_0": field,
        "question_0_custom": {"type": "string", "title": "Other"},
    });
    question(h, request, Some(fields));
}

#[test]
fn digits_and_ret_tick_a_multi_select_and_c_c_c_y_sends_it() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    ask(&mut h, 1, "c", Some("Edit"));
    multi_select(&mut h, 2, serde_json::json!({}));
    assert!(h.core.text().contains(MULTI), "{}", h.core.text());
    let at = h.point();
    h.keys("1 3");
    assert!(ops(&mut h).is_empty(), "ticking sends nothing");
    assert!(
        h.core
            .text()
            .contains("   1   [x] A\n   2   [ ] B\n   3   [x] C\n")
    );
    assert_eq!(h.point(), at, "point stays");
    h.keys("RET");
    assert!(h.core.text().contains("   1   [ ] A\n"), "RET unticks");
    let at = h.point();
    h.keys("RET o Redis RET");
    assert!(
        h.core.text().contains("   o   [x] Own answer  Redis\n"),
        "the row shows it"
    );
    assert_eq!(h.point(), at, "point keeps its place as the row grows");
    h.keys("o");
    assert_eq!(h.core.text(), "Redis", "o starts from the answer it has");
    h.keys("C-a C-k RET");
    assert!(
        h.core.text().contains("   o   [ ] Own answer\n"),
        "an empty one clears it"
    );
    h.keys("o Redis RET");
    h.keys("C-c C-y");
    assert_eq!(
        ops(&mut h),
        [accepted(
            2,
            r#"{"question_0":["a","c"],"question_0_custom":"Redis"}"#
        )]
    );
    assert!(
        h.core.text().contains("▸ ✓ Which?  A, C, Redis\n"),
        "{}",
        h.core.text()
    );
    // The permission request stayed pending, and C-c C-y in the prompt answers it.
    h.keys("C-c C-y");
    assert_eq!(ops(&mut h), ["reply 0 1 allow"]);
}

#[test]
fn min_and_max_items_refuse_a_send_without_a_count() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    multi_select(&mut h, 1, serde_json::json!({"minItems": 2, "maxItems": 2}));
    assert!(h.core.text().contains(MULTI), "no count drawn");
    h.keys("1 C-c C-y");
    assert_eq!(h.core.message(), Some("Pick at least 2"));
    h.keys("2 o x RET C-c C-y");
    assert_eq!(h.core.message(), Some("Pick at most 2"));
    assert!(ops(&mut h).is_empty(), "nothing sent");
    h.keys("o C-a C-k RET C-c C-y");
    assert_eq!(ops(&mut h), [accepted(1, r#"{"question_0":["a","b"]}"#)]);
}

#[test]
fn a_tenth_option_is_picked_with_ret() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    let options: Vec<_> = (1..=12)
        .map(|n| serde_json::json!({"const": format!("v{n}"), "title": format!("T{n}")}))
        .collect();
    let fields = serde_json::json!({"c": {"type": "string", "oneOf": options}});
    question(&mut h, 1, Some(fields));
    let text = h.core.text();
    assert!(text.contains("   9   T9\n       T10\n"), "{text}");
    assert!(text.contains("  1-9 answer"), "{text}");
    h.keys("C-n C-n C-n C-n C-n C-n C-n C-n C-n RET");
    assert_eq!(ops(&mut h), [accepted(1, r#"{"c":"v10"}"#)]);
}

#[test]
fn the_preview_follows_point_and_folds_long_text() {
    let mut h = session();
    h.keys("a RET");
    ops(&mut h);
    let long: Vec<_> = (1..=12).map(|n| format!("l{n}")).collect();
    let preview = |p: &str| serde_json::json!({"_claude/askUserQuestionOption": {"preview": p}});
    let fields = serde_json::json!({"c": {"type": "string", "oneOf": [
        {"const": "a", "title": "A", "_meta": preview("one\ntwo")},
        {"const": "b", "title": "B", "_meta": preview(&long.join("\n"))},
        {"const": "c", "title": "C"}]}});
    question(&mut h, 1, Some(fields));
    assert!(
        h.core
            .text()
            .contains("  1-3 answer   C-n C-p preview another   C-c C-k skip"),
        "{}",
        h.core.text()
    );
    let lines = shown(&mut h, 30);
    let at = lines.iter().position(|l| l.contains("1   A")).unwrap();
    assert_eq!(
        lines[at + 1..at + 4],
        ["        one ", "        two ", "   2   B"]
    );
    let line = drawn_line(&mut h, "one");
    assert_eq!(
        line.spans[1].face.bg,
        h.core.settings().theme.face(FaceName::SessionBlock).bg
    );
    assert!(!h.core.text().contains("one"), "not in the buffer");
    h.keys("C-n");
    let lines = shown(&mut h, 30);
    assert!(!lines.iter().any(|l| l.contains("one")), "{lines:?}");
    let at = lines.iter().position(|l| l.contains("2   B")).unwrap();
    assert_eq!(lines[at + 1].trim(), "l1");
    assert_eq!(lines[at + 7].trim(), "l7");
    assert_eq!(lines[at + 8].trim(), "… 5 more lines");
    assert!(lines[at + 9].contains("3   C"), "{lines:?}");
    h.keys("C-n");
    let lines = shown(&mut h, 30);
    assert!(!lines.iter().any(|l| l.contains("l1")), "C has none");
}
