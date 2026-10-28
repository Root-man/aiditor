//! A scripted ACP agent for tests. It reports what the client sent on stderr and opens a session
//! with a two-value mode option, then runs the JSON script `AIDITOR_FAKE_AGENT` names: its
//! `session` steps after `session/new` and the n-th list of `prompts` on the n-th
//! `session/prompt`, answers `session/list` from its `sessions` pages and replays its
//! `history` on `session/load`, taking additional directories with `directories`. With
//! `AIDITOR_FAKE_AGENT_REPLAY` it replays a recorded transcript instead.
//! Both are in docs/editor.md under Testing.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, ConfigOptionUpdate, InitializeRequest,
    InitializeResponse, ListSessionsRequest, ListSessionsResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PermissionOption, PromptRequest,
    PromptResponse, RequestPermissionRequest, SessionAdditionalDirectoriesCapabilities,
    SessionCapabilities, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOption, SessionInfo, SessionListCapabilities, SessionNotification,
    SessionUpdate, SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, StopReason,
    ToolCallUpdate,
};
use agent_client_protocol::{Agent, ByteStreams, Client, ConnectionTo};
use serde::Deserialize;
use serde_json::Value;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::sync::CancellationToken;

const SESSION: &str = "fake-session";
const MODES: [&str; 2] = ["auto", "plan"];

/// The steps after a `closeStdout`, with what they run with; they outlive the connection, so
/// `main` runs them once it ends.
type Rest = (Vec<Step>, PathBuf, ConnectionTo<Client>, CancellationToken);
static REST: Mutex<Option<Rest>> = Mutex::new(None);

/// The session's config options: the `mode` select set to `current`.
fn options(current: &str) -> Vec<SessionConfigOption> {
    let values = MODES
        .map(|id| SessionConfigSelectOption::new(id, id))
        .to_vec();
    let mode = SessionConfigOption::select("mode", "Mode", current.to_owned(), values);
    vec![mode.category(SessionConfigOptionCategory::Mode)]
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Script {
    #[serde(default)]
    session: Vec<Step>,
    #[serde(default)]
    prompts: Vec<Vec<Step>>,
    /// `session/list`'s pages; without them the fake can't list.
    sessions: Option<Vec<Vec<SessionInfo>>>,
    /// The updates `session/load` replays; without them the fake can't load.
    history: Option<Vec<SessionUpdate>>,
    /// Whether it advertises `additionalDirectories`.
    #[serde(default)]
    directories: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Step {
    Update(SessionUpdate),
    Permission(Permission),
    Write { path: PathBuf, text: String },
    Sleep(u64),
    Stderr(String),
    Exit(i32),
    Stop(StopReason),
    IgnoreSigterm,
    CloseStdout,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Permission {
    tool_call: ToolCallUpdate,
    options: Vec<PermissionOption>,
}

/// What the handlers share: the session's `cwd`, the prompts run so far and the running turn.
#[derive(Default)]
struct State {
    cwd: PathBuf,
    turns: usize,
    turn: CancellationToken,
}

fn dirs(paths: &[PathBuf]) -> String {
    let paths: Vec<_> = paths.iter().map(|p| p.display().to_string()).collect();
    paths.join(",")
}

fn load() -> Result<Script, String> {
    let Ok(path) = std::env::var("AIDITOR_FAKE_AGENT") else {
        return Ok(Script::default());
    };
    let text = std::fs::read_to_string(&path).map_err(|err| format!("{path}: {err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("{path}: {err}"))
}

/// Runs `steps` in order until one stops the turn or `cancel` fires; returns the stop reason.
async fn run(
    steps: &[Step],
    cwd: &std::path::Path,
    client: &ConnectionTo<Client>,
    cancel: &CancellationToken,
) -> agent_client_protocol::Result<StopReason> {
    for (i, step) in steps.iter().enumerate() {
        if cancel.is_cancelled() {
            return Ok(StopReason::Cancelled);
        }
        match step {
            Step::Update(update) => {
                client.send_notification(SessionNotification::new(SESSION, update.clone()))?
            }
            Step::Permission(ask) => {
                let ask = RequestPermissionRequest::new(
                    SESSION,
                    ask.tool_call.clone(),
                    ask.options.clone(),
                );
                let answer = tokio::select! {
                    answer = client.send_request(ask).block_task() => answer,
                    _ = cancel.cancelled() => return Ok(StopReason::Cancelled),
                };
                match answer {
                    Ok(response) => eprintln!(
                        "permission answered: {}",
                        serde_json::to_string(&response.outcome).unwrap()
                    ),
                    Err(err) => eprintln!("permission error: {}", err.message),
                }
            }
            Step::Write { path, text } => {
                let path = cwd.join(path);
                let written = (path.parent().map_or(Ok(()), std::fs::create_dir_all))
                    .and_then(|()| std::fs::write(&path, text));
                if let Err(err) = written {
                    eprintln!("write error: {}: {err}", path.display());
                }
            }
            Step::Sleep(ms) => tokio::select! {
                () = tokio::time::sleep(Duration::from_millis(*ms)) => {}
                _ = cancel.cancelled() => return Ok(StopReason::Cancelled),
            },
            Step::Stderr(line) => eprintln!("{line}"),
            Step::Exit(code) => std::process::exit(*code),
            Step::Stop(reason) => return Ok(*reason),
            Step::IgnoreSigterm => _ = unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN) },
            // /dev/null over fd 1 closes the pipe and keeps the fd from being reused by a file.
            Step::CloseStdout => {
                let null = std::fs::OpenOptions::new().write(true).open("/dev/null");
                let null = null.expect("/dev/null opens");
                unsafe { libc::dup2(std::os::fd::AsRawFd::as_raw_fd(&null), libc::STDOUT_FILENO) };
                // The client then closes stdin, ending the connection and its tasks.
                let rest = (steps[i + 1..].to_vec(), cwd.to_owned());
                *REST.lock().unwrap() = Some((rest.0, rest.1, client.clone(), cancel.clone()));
                return Ok(StopReason::EndTurn);
            }
        }
    }
    Ok(StopReason::EndTurn)
}

fn exit(code: i32, message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(code)
}

/// Replays the transcript at `path`: writes the agent messages recorded before the next client
/// message, checks what the client sends against that one, and repeats.
fn replay(path: &str) -> ! {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|err| exit(2, format!("{path}: {err}")));
    let records = text.lines().filter(|line| !line.is_empty()).map(|line| {
        let mut record: Value =
            serde_json::from_str(line).unwrap_or_else(|err| exit(2, format!("{path}: {err}")));
        match record["from"].as_str() {
            Some(from @ ("client" | "agent")) => (from == "client", record["message"].take()),
            _ => exit(2, format!("{path}: not a transcript line: {line}")),
        }
    });
    let records: Vec<_> = records.collect();
    // The recorded client request ids, as JSON, to the live ones.
    let mut ids = HashMap::<String, Value>::new();
    let mut cwd: Option<String> = None;
    let (mut stdin, mut stdout) = (std::io::stdin().lock().lines(), std::io::stdout().lock());
    let mut records = records.into_iter();
    loop {
        let expected = loop {
            match records.next() {
                Some((true, message)) => break Some(message),
                Some((false, mut message)) => {
                    let live = (message.get("method").is_none())
                        .then(|| ids.get(&message["id"].to_string()))
                        .flatten();
                    if let Some(live) = live {
                        message["id"] = live.clone();
                    }
                    let mut line = message.to_string();
                    if let Some(cwd) = &cwd {
                        line = line.replace("{cwd}", cwd);
                    }
                    let written = writeln!(stdout, "{line}").and_then(|()| stdout.flush());
                    written.unwrap_or_else(|err| exit(1, format!("writing to the client: {err}")));
                }
                None => break None,
            }
        };
        let Some(Ok(line)) = stdin.next() else {
            std::process::exit(0)
        };
        let live: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        let matched = expected
            .as_ref()
            .filter(|recorded| match recorded.get("method") {
                Some(method) => live.get("method") == Some(method),
                None => live.get("method").is_none() && live.get("id") == recorded.get("id"),
            });
        let Some(recorded) = matched else {
            let expected = expected.map_or("the end".into(), |e| e.to_string());
            exit(
                1,
                format!("unexpected client message: {line}\nexpected: {expected}"),
            );
        };
        if recorded.get("method").is_some()
            && let (Some(from), Some(to)) = (recorded.get("id"), live.get("id"))
        {
            ids.insert(from.to_string(), to.clone());
        }
        if let Some(path) = live["params"]["cwd"]
            .as_str()
            .filter(|_| live["method"] == "session/new")
        {
            // Escaped as JSON, as `{cwd}` sits inside the line's strings.
            let quoted = Value::from(path).to_string();
            cwd = Some(quoted[1..quoted.len() - 1].to_owned());
        }
    }
}

#[tokio::main]
async fn main() -> agent_client_protocol::Result<()> {
    if let Ok(path) = std::env::var("AIDITOR_FAKE_AGENT_REPLAY") {
        replay(&path);
    }
    let script = match load() {
        Ok(script) => Arc::new(script),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };
    let state = Arc::new(Mutex::new(State::default()));
    let (session_state, prompt_state, cancel_state) = (state.clone(), state.clone(), state);
    let (prompt_script, init_script, list_script, load_script) = (
        script.clone(),
        script.clone(),
        script.clone(),
        script.clone(),
    );
    let load_state = session_state.clone();
    let transport = ByteStreams::new(
        tokio::io::stdout().compat_write(),
        tokio::io::stdin().compat(),
    );
    let result = Agent
        .builder()
        .on_receive_request(
            async move |request: InitializeRequest, responder, _| {
                let caps = &request.client_capabilities;
                eprintln!("pid={}", std::process::id());
                eprintln!(
                    "fs.readTextFile={} fs.writeTextFile={} terminal={} terminal_output={}",
                    caps.fs.read_text_file,
                    caps.fs.write_text_file,
                    caps.terminal,
                    (caps.meta.as_ref()).and_then(|m| m.get("terminal_output"))
                        == Some(&true.into())
                );
                let mut sessions = SessionCapabilities::new();
                if init_script.sessions.is_some() {
                    sessions = sessions.list(SessionListCapabilities::new());
                }
                if init_script.directories {
                    sessions = sessions
                        .additional_directories(SessionAdditionalDirectoriesCapabilities::new());
                }
                let capabilities = (AgentCapabilities::new().session_capabilities(sessions))
                    .load_session(init_script.history.is_some());
                responder.respond(
                    InitializeResponse::new(ProtocolVersion::V1).agent_capabilities(capabilities),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: NewSessionRequest, responder, client: ConnectionTo<Client>| {
                eprintln!("cwd={}", request.cwd.display());
                eprintln!("dirs={}", dirs(&request.additional_directories));
                eprintln!("A={}", std::env::var("A").unwrap_or_default());
                let options = options(MODES[0]);
                responder.respond(NewSessionResponse::new(SESSION).config_options(options))?;
                // shortcut: a cancel after the first prompt can't stop `session` steps still
                // running; give them their own token if a test needs that.
                let cancel = {
                    let mut state = session_state.lock().unwrap();
                    state.cwd = request.cwd.clone();
                    state.turn.clone()
                };
                let (script, cwd) = (script.clone(), request.cwd);
                // In its own task, so the dispatch loop can route replies and cancels.
                client.clone().spawn(async move {
                    run(&script.session, &cwd, &client, &cancel).await.map(drop)
                })
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: LoadSessionRequest, responder, client: ConnectionTo<Client>| {
                eprintln!("load={}", request.session_id);
                eprintln!("cwd={}", request.cwd.display());
                eprintln!("dirs={}", dirs(&request.additional_directories));
                let Some(history) = &load_script.history else {
                    return responder
                        .respond_with_error(agent_client_protocol::Error::method_not_found());
                };
                for update in history {
                    let update =
                        SessionNotification::new(request.session_id.clone(), update.clone());
                    client.send_notification(update)?;
                }
                load_state.lock().unwrap().cwd = request.cwd;
                responder.respond(LoadSessionResponse::new().config_options(options(MODES[0])))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_: PromptRequest, responder, client: ConnectionTo<Client>| {
                let (n, cwd, cancel) = {
                    let mut state = prompt_state.lock().unwrap();
                    state.turns += 1;
                    state.turn = CancellationToken::new();
                    (state.turns - 1, state.cwd.clone(), state.turn.clone())
                };
                let script = prompt_script.clone();
                client.clone().spawn(async move {
                    let steps = script.prompts.get(n).map_or(&[][..], Vec::as_slice);
                    let reason = run(steps, &cwd, &client, &cancel).await?;
                    responder.respond(PromptResponse::new(reason))
                })
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async |request: SetSessionConfigOptionRequest,
                   responder,
                   client: ConnectionTo<Client>| {
                let value = request.value.as_value_id().map(|v| &*v.0);
                let Some(value) =
                    value.filter(|v| MODES.contains(v) && &*request.config_id.0 == "mode")
                else {
                    return responder
                        .respond_with_error(agent_client_protocol::Error::invalid_params());
                };
                responder.respond(SetSessionConfigOptionResponse::new(options(value)))?;
                let update =
                    SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(options(value)));
                client.send_notification(SessionNotification::new(SESSION, update))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: ListSessionsRequest, responder, _| {
                let pages = list_script.sessions.as_deref().unwrap_or_default();
                let n = (request.cursor.as_deref()).map_or(Ok(0), str::parse::<usize>);
                let Some((n, page)) = n.ok().and_then(|n| Some((n, pages.get(n)?))) else {
                    return responder
                        .respond_with_error(agent_client_protocol::Error::invalid_params());
                };
                let sessions = (page.iter())
                    .filter(|s| request.cwd.as_ref().is_none_or(|cwd| &s.cwd == cwd))
                    .cloned()
                    .collect();
                let next = (n + 1 < pages.len()).then(|| (n + 1).to_string());
                responder.respond(ListSessionsResponse::new(sessions).next_cursor(next))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            async move |_: CancelNotification, _| {
                cancel_state.lock().unwrap().turn.cancel();
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_to(transport)
        .await;
    let rest = REST.lock().unwrap().take();
    if let Some((steps, cwd, client, cancel)) = rest {
        _ = run(&steps, &cwd, &client, &cancel).await;
    }
    result
}
