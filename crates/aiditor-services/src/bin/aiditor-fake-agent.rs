//! A scripted ACP agent for tests. It reports what the client sent on stderr and opens a session
//! with two modes, then runs the JSON script `AIDITOR_FAKE_AGENT` names: its `session` steps after
//! `session/new` and the n-th list of `prompts` on the n-th `session/prompt`. The format is in
//! docs/editor.md under Testing.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    CancelNotification, CurrentModeUpdate, InitializeRequest, InitializeResponse,
    NewSessionRequest, NewSessionResponse, PermissionOption, PromptRequest, PromptResponse,
    RequestPermissionRequest, SessionMode, SessionModeState, SessionNotification, SessionUpdate,
    SetSessionModeRequest, SetSessionModeResponse, StopReason, ToolCallUpdate,
};
use agent_client_protocol::{Agent, ByteStreams, Client, ConnectionTo};
use serde::Deserialize;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::sync::CancellationToken;

const SESSION: &str = "fake-session";
const MODES: [&str; 2] = ["auto", "plan"];

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Script {
    #[serde(default)]
    session: Vec<Step>,
    #[serde(default)]
    prompts: Vec<Vec<Step>>,
}

#[derive(Deserialize)]
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
}

#[derive(Deserialize)]
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
    for step in steps {
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
        }
    }
    Ok(StopReason::EndTurn)
}

#[tokio::main]
async fn main() -> agent_client_protocol::Result<()> {
    let script = match load() {
        Ok(script) => Arc::new(script),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };
    let state = Arc::new(Mutex::new(State::default()));
    let (session_state, prompt_state, cancel_state) = (state.clone(), state.clone(), state);
    let prompt_script = script.clone();
    let transport = ByteStreams::new(
        tokio::io::stdout().compat_write(),
        tokio::io::stdin().compat(),
    );
    Agent
        .builder()
        .on_receive_request(
            async |request: InitializeRequest, responder, _| {
                let caps = &request.client_capabilities;
                eprintln!("pid={}", std::process::id());
                eprintln!(
                    "fs.readTextFile={} fs.writeTextFile={} terminal={}",
                    caps.fs.read_text_file, caps.fs.write_text_file, caps.terminal
                );
                responder.respond(InitializeResponse::new(ProtocolVersion::V1))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: NewSessionRequest, responder, client: ConnectionTo<Client>| {
                eprintln!("cwd={}", request.cwd.display());
                eprintln!("A={}", std::env::var("A").unwrap_or_default());
                let modes = MODES.map(|id| SessionMode::new(id, id)).to_vec();
                responder.respond(
                    NewSessionResponse::new(SESSION).modes(SessionModeState::new(MODES[0], modes)),
                )?;
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
            async |request: SetSessionModeRequest, responder, client: ConnectionTo<Client>| {
                if !MODES.contains(&&*request.mode_id.0) {
                    return responder
                        .respond_with_error(agent_client_protocol::Error::invalid_params());
                }
                responder.respond(SetSessionModeResponse::new())?;
                let update =
                    SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(request.mode_id));
                client.send_notification(SessionNotification::new(SESSION, update))
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
        .await
}
