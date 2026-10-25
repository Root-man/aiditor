//! A minimal ACP agent for the agent service's tests. It reports what the client sent on stderr
//! and opens a session with two modes.
//!
//! `AIDITOR_TEST_AGENT` picks a behavior: `request` asks for a permission once the session is
//! open and reports the answer; `update` sends one agent message chunk once the session is open;
//! `exit3` writes two stderr lines and exits with code 3; `trap`
//! ignores SIGTERM.

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    ContentChunk, InitializeRequest, InitializeResponse, NewSessionRequest, NewSessionResponse,
    PermissionOption, PermissionOptionKind, RequestPermissionRequest, SessionMode,
    SessionModeState, SessionNotification, SessionUpdate, ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol::{Agent, ByteStreams, Client, ConnectionTo};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[tokio::main]
async fn main() -> agent_client_protocol::Result<()> {
    let behavior = std::env::var("AIDITOR_TEST_AGENT").unwrap_or_default();
    match behavior.as_str() {
        "exit3" => {
            eprintln!("first");
            eprintln!("second");
            std::process::exit(3);
        }
        "trap" => _ = unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN) },
        _ => {}
    }
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
                let modes = vec![
                    SessionMode::new("auto", "Auto"),
                    SessionMode::new("plan", "Plan"),
                ];
                responder.respond(
                    NewSessionResponse::new("test-session")
                        .modes(SessionModeState::new("auto", modes)),
                )?;
                if behavior == "update" {
                    let chunk = ContentChunk::new("hi".into());
                    let update = SessionUpdate::AgentMessageChunk(chunk);
                    client.send_notification(SessionNotification::new("test-session", update))?;
                }
                if behavior == "request" {
                    let ask = RequestPermissionRequest::new(
                        "test-session",
                        ToolCallUpdate::new("call-1", ToolCallUpdateFields::new()),
                        vec![PermissionOption::new(
                            "allow",
                            "Allow",
                            PermissionOptionKind::AllowOnce,
                        )],
                    );
                    let request = client.send_request(ask);
                    // Answered in its own task, so the dispatch loop can route the reply.
                    client.spawn(async move {
                        match request.block_task().await {
                            Ok(response) => eprintln!("permission answered: {response:?}"),
                            Err(err) => eprintln!("permission error: {}", err.message),
                        }
                        Ok(())
                    })?;
                }
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_to(transport)
        .await
}
