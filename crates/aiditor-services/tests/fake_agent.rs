//! The fake agent driven directly as an ACP client, for what the agent service can't send yet:
//! prompts, config options and cancel.

use std::process::Stdio;
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    CancelNotification, InitializeRequest, NewSessionRequest, PromptRequest,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionConfigKind, SessionConfigOption, SessionNotification,
    SessionUpdate, SetSessionConfigOptionRequest, StopReason,
};
use agent_client_protocol::{Agent, ByteStreams, Client, ConnectionTo};
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const AGENT: &str = env!("CARGO_BIN_EXE_aiditor-fake-agent");
const SESSION: &str = "fake-session";

#[derive(Debug)]
enum Event {
    Update(SessionUpdate),
    Asked(RequestPermissionRequest),
}

/// A fake with an open session in its temporary directory, writing its stderr to `stderr` there;
/// `fake("")` runs with no script; permissions get the first option, and one without options is
/// never answered.
struct Fake {
    agent: ConnectionTo<Agent>,
    events: mpsc::Receiver<Event>,
    dir: TempDir,
    _child: Child,
}

async fn fake(script: &str) -> Fake {
    let dir = tempfile::tempdir().unwrap();
    let mut command = Command::new(AGENT);
    if !script.is_empty() {
        std::fs::write(dir.path().join("script.json"), script).unwrap();
        command.env("AIDITOR_FAKE_AGENT", "script.json");
    }
    let mut child = command
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create(dir.path().join("stderr")).unwrap())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let transport = ByteStreams::new(
        child.stdin.take().unwrap().compat_write(),
        child.stdout.take().unwrap().compat(),
    );
    let (tx, events) = mpsc::channel(16);
    let (asks, (ready, connected)) = (tx.clone(), oneshot::channel());
    let cwd = dir.path().to_owned();
    tokio::spawn(
        Client
            .builder()
            .on_receive_request(
                async move |ask: RequestPermissionRequest,
                            responder,
                            agent: ConnectionTo<Agent>| {
                    let option = ask.options.first().map(|o| o.option_id.clone());
                    _ = asks.send(Event::Asked(ask)).await;
                    let Some(option) = option else {
                        // Held open for as long as the connection runs.
                        return agent.spawn(async move {
                            std::future::pending::<()>().await;
                            drop(responder);
                            Ok(())
                        });
                    };
                    let outcome = SelectedPermissionOutcome::new(option);
                    responder.respond(RequestPermissionResponse::new(
                        RequestPermissionOutcome::Selected(outcome),
                    ))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |notification: SessionNotification, _| {
                    _ = tx.send(Event::Update(notification.update)).await;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(transport, async |agent: ConnectionTo<Agent>| {
                let initialize = InitializeRequest::new(ProtocolVersion::V1);
                agent.send_request(initialize).block_task().await?;
                let session = agent.send_request(NewSessionRequest::new(cwd));
                session.block_task().await?;
                _ = ready.send(agent.clone());
                agent.incoming_closed().await;
                Ok(())
            }),
    );
    let agent = (timeout(Duration::from_secs(10), connected).await)
        .expect("a session within 10 s")
        .expect("the session opens");
    Fake {
        agent,
        events,
        dir,
        _child: child,
    }
}

impl Fake {
    async fn prompt(&self) -> StopReason {
        let prompt = (self.agent).send_request(PromptRequest::new(SESSION, Vec::new()));
        (timeout(Duration::from_secs(10), prompt.block_task()).await)
            .expect("a prompt answer within 10 s")
            .expect("the prompt succeeds")
            .stop_reason
    }

    async fn event(&mut self) -> Event {
        (timeout(Duration::from_secs(10), self.events.recv()).await)
            .expect("an event within 10 s")
            .expect("the connection is open")
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_streams_asks_writes_and_ends() {
    let mut fake = fake(
        r#"{"prompts": [[
            {"update": {"sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": "hi"}}},
            {"permission": {"toolCall": {"toolCallId": "call-1"},
                            "options": [{"optionId": "allow", "name": "Allow",
                                         "kind": "allow_once"}]}},
            {"write": {"path": "out/a.txt", "text": "written"}},
            {"stop": "max_tokens"},
            {"write": {"path": "b.txt", "text": "skipped"}}
        ]]}"#,
    )
    .await;
    assert_eq!(fake.prompt().await, StopReason::MaxTokens);
    let update = fake.event().await;
    assert!(
        matches!(&update, Event::Update(SessionUpdate::AgentMessageChunk(_))),
        "{update:?}"
    );
    let ask = fake.event().await;
    assert!(
        matches!(&ask, Event::Asked(ask) if ask.tool_call.tool_call_id.0.as_ref() == "call-1"),
        "{ask:?}"
    );
    let a = std::fs::read_to_string(fake.dir.path().join("out/a.txt")).unwrap();
    assert_eq!(a, "written");
    assert!(!fake.dir.path().join("b.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn prompts_past_the_script_end_the_turn() {
    let fake = fake(r#"{"prompts": [[{"stop": "refusal"}]]}"#).await;
    assert_eq!(fake.prompt().await, StopReason::Refusal);
    assert_eq!(fake.prompt().await, StopReason::EndTurn);
    assert_eq!(fake.prompt().await, StopReason::EndTurn);
}

#[tokio::test(flavor = "multi_thread")]
async fn setting_the_mode_reports_it() {
    /// The `mode` option's current value.
    fn mode(options: &[SessionConfigOption]) -> Option<&str> {
        let option = options.iter().find(|o| o.id.0.as_ref() == "mode")?;
        match &option.kind {
            SessionConfigKind::Select(select) => Some(&select.current_value.0),
            _ => None,
        }
    }
    let mut fake = fake("").await;
    let set = SetSessionConfigOptionRequest::new(SESSION, "mode", "plan");
    let answer = fake.agent.send_request(set).block_task().await.unwrap();
    assert_eq!(mode(&answer.config_options), Some("plan"));
    let update = fake.event().await;
    assert!(
        matches!(&update, Event::Update(SessionUpdate::ConfigOptionUpdate(update))
            if mode(&update.config_options) == Some("plan")),
        "{update:?}"
    );
    let set = SetSessionConfigOptionRequest::new(SESSION, "mode", "nope");
    fake.agent.send_request(set).block_task().await.unwrap_err();
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_ends_a_sleeping_turn() {
    let fake = fake(r#"{"prompts": [[{"sleep": 60000}]]}"#).await;
    let agent = fake.agent.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        agent.send_notification(CancelNotification::new(SESSION))
    });
    assert_eq!(fake.prompt().await, StopReason::Cancelled);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_ends_a_turn_waiting_for_a_permission() {
    let mut fake = fake(
        r#"{"prompts": [[{"permission": {"toolCall": {"toolCallId": "call-1"}, "options": []}},
                         {"write": {"path": "a.txt", "text": "skipped"}}]]}"#,
    )
    .await;
    let agent = fake.agent.clone();
    let prompt = tokio::spawn(async move {
        agent
            .send_request(PromptRequest::new(SESSION, Vec::new()))
            .block_task()
            .await
    });
    assert!(matches!(fake.event().await, Event::Asked(_)));
    fake.agent
        .send_notification(CancelNotification::new(SESSION))
        .unwrap();
    let answer = timeout(Duration::from_secs(10), prompt)
        .await
        .expect("an answer within 10 s");
    assert_eq!(answer.unwrap().unwrap().stop_reason, StopReason::Cancelled);
    assert!(!fake.dir.path().join("a.txt").exists());
}

#[test]
fn a_bad_script_exits_with_2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("script.json");
    std::fs::write(&path, r#"{"session": [{"dance": 1}]}"#).unwrap();
    let output = std::process::Command::new(AGENT)
        .env("AIDITOR_FAKE_AGENT", &path)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("script.json"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_stdout_keeps_the_process_alive() {
    let mut fake = fake(
        r#"{"session": [{"sleep": 100}, "closeStdout", {"sleep": 200}, {"stderr": "still here"}]}"#,
    )
    .await;
    let closed = timeout(Duration::from_secs(10), fake.events.recv()).await;
    assert!(closed.expect("the connection closes within 10 s").is_none());
    let stderr = fake.dir.path().join("stderr");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !std::fs::read_to_string(&stderr)
        .unwrap()
        .contains("still here")
    {
        assert!(
            std::time::Instant::now() < deadline,
            "no stderr after the close"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
