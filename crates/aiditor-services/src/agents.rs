//! ACP agent processes, each speaking JSON-RPC over its stdio and sending on its own bounded
//! channel.

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentRequest, CancelNotification, ClientCapabilities, Implementation, InitializeRequest,
    ListSessionsRequest, NewSessionRequest, RequestId, SessionInfo, SessionNotification,
};
use agent_client_protocol::{Agent, ByteStreams, Client, ConnectionTo, Responder};
use aiditor_core::{AgentId, AgentOp, Msg, Request};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

/// The agent's requests the core hasn't answered yet; ACP's request enum takes its answer as
/// JSON, so replies are serialized.
type Pending = Arc<Mutex<HashMap<RequestId, Responder>>>;

/// The spawned agents by id, and the processes still listing sessions by request.
#[derive(Default)]
pub struct Agents {
    handles: HashMap<AgentId, Handle>,
    lists: Lists,
}

/// Each running list's process group and whether it exited; its task removes it to terminate it.
type Lists = Arc<Mutex<HashMap<u64, (libc::pid_t, Arc<AtomicBool>)>>>;

/// The connection to the agent and its sender, while its session is open; dropped with the
/// connection so the receiver closes after the exit.
type Connection = Arc<Mutex<Option<(ConnectionTo<Agent>, Sender<Msg>)>>>;

struct Handle {
    /// The agent leads its own process group, so this is the group's id too.
    pid: libc::pid_t,
    exited: Arc<AtomicBool>,
    pending: Pending,
    connection: Connection,
}

/// How long a terminated agent has to exit before SIGKILL.
const GRACE: Duration = Duration::from_millis(100);

/// shortcut: sessions past this many `session/list` pages are left out; page on demand if a
/// real history ever gets that long.
const LIST_PAGES: usize = 50;

impl Handle {
    /// SIGTERM to the agent's group unless it has exited; returns the group for the SIGKILL.
    fn terminate(self) -> Option<libc::pid_t> {
        self.pending.lock().unwrap().clear();
        terminate(self.pid, &self.exited)
    }
}

/// SIGTERM to group `pid` unless the agent has exited; returns the group for the SIGKILL.
fn terminate(pid: libc::pid_t, exited: &AtomicBool) -> Option<libc::pid_t> {
    // shortcut: the group can empty between this check and the SIGKILL, and its id be reused
    // within the grace, as with terminals; track every member if that ever shows up.
    if exited.load(Ordering::SeqCst) {
        return None;
    }
    unsafe { libc::killpg(pid, libc::SIGTERM) };
    Some(pid)
}

/// SIGKILL to `group` after the grace, without blocking.
fn kill_after_grace(group: libc::pid_t) {
    thread::spawn(move || {
        thread::sleep(GRACE);
        unsafe { libc::killpg(group, libc::SIGKILL) };
    });
}

impl Agents {
    /// Run `op` without blocking, on the Tokio runtime; a spawn returns its agent's receiver.
    pub fn run(&mut self, op: AgentOp) -> Option<Receiver<Msg>> {
        match op {
            AgentOp::Spawn {
                id,
                command,
                args,
                cwd,
                env,
            } => Some(self.spawn(id, command, args, cwd, env)),
            AgentOp::Reply {
                id,
                request,
                result,
            } => {
                let responder = (self.handles.get(&id))
                    .and_then(|agent| agent.pending.lock().unwrap().remove(&request));
                let result = result.map(|response| {
                    serde_json::to_value(response).expect("a ClientResponse serializes")
                });
                if let Some(responder) = responder
                    && let Err(err) = responder.respond_with_result(result)
                {
                    tracing::warn!("replying to the agent: {err}");
                }
                None
            }
            AgentOp::Prompt { id, request } => {
                let (connection, tx) = self.connection(id)?;
                // The answer is handled before the next message, so it follows the turn's updates.
                let sent = connection.send_request(request).on_receiving_result(
                    move |result| async move {
                        let result = result.map(|r| r.stop_reason).map_err(|err| err.message);
                        _ = tx.send(Msg::AgentPrompted { id, result }).await;
                        Ok(())
                    },
                );
                if let Err(err) = sent {
                    tracing::warn!("prompting the agent: {err}");
                }
                None
            }
            AgentOp::SetConfigOption { id, request } => {
                let (connection, tx) = self.connection(id)?;
                let sent = connection.send_request(request).on_receiving_result(
                    move |result| async move {
                        let result = result.map(|r| r.config_options).map_err(|err| err.message);
                        _ = tx.send(Msg::AgentConfigured { id, result }).await;
                        Ok(())
                    },
                );
                if let Err(err) = sent {
                    tracing::warn!("setting the agent's config option: {err}");
                }
                None
            }
            AgentOp::Cancel { id, session } => {
                let sent = (self.connection(id)).map(|(connection, _)| {
                    connection.send_notification(CancelNotification::new(session))
                });
                if let Some(Err(err)) = sent {
                    tracing::warn!("cancelling the agent's turn: {err}");
                }
                None
            }
            AgentOp::Kill { id } => {
                if let Some(group) = self.handles.remove(&id).and_then(Handle::terminate) {
                    kill_after_grace(group);
                }
                None
            }
            AgentOp::List {
                request,
                command,
                args,
                cwd,
                env,
            } => Some(self.list(request, command, args, cwd, env)),
        }
    }

    /// The connection of agent `id` and its sender, once its session is open.
    fn connection(&self, id: AgentId) -> Option<(ConnectionTo<Agent>, Sender<Msg>)> {
        self.handles.get(&id)?.connection.lock().unwrap().clone()
    }

    /// Kill every agent, blocking for one shared grace, so no agent outlives the app.
    pub fn kill_all(&mut self) {
        let lists: Vec<_> = self.lists.lock().unwrap().drain().collect();
        let groups: Vec<_> = (self.handles.drain())
            .filter_map(|(_, agent)| agent.terminate())
            .chain(
                lists
                    .iter()
                    .filter_map(|(_, (pid, exited))| terminate(*pid, exited)),
            )
            .collect();
        if groups.is_empty() {
            return;
        }
        thread::sleep(GRACE);
        for group in groups {
            unsafe { libc::killpg(group, libc::SIGKILL) };
        }
    }

    /// Starts the process here, so a kill right after has a pid; the rest runs in its tasks.
    fn spawn(
        &mut self,
        id: AgentId,
        command: String,
        args: Vec<String>,
        cwd: PathBuf,
        env: Vec<(String, String)>,
    ) -> Receiver<Msg> {
        let (tx, rx) = channel(4);
        let fail = |err: String| {
            _ = tx.try_send(Msg::AgentStarted {
                id,
                result: Err(err),
            });
        };
        if !cwd.is_dir() {
            fail(format!("{}: not a directory", cwd.display()));
            return rx;
        }
        let mut child = match command_in(&command, args, &cwd, env, Stdio::piped()) {
            Ok(child) => child,
            Err(err) => {
                fail(format!("{command}: {err}"));
                return rx;
            }
        };
        let pid = child.id().expect("a spawned child has a pid") as libc::pid_t;
        let (stdin, stdout, stderr) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (stdin, stdout, stderr) = (stdin.unwrap(), stdout.unwrap(), stderr.unwrap());
        let exited = Arc::new(AtomicBool::new(false));
        let pending = Pending::default();
        let open = Connection::default();
        self.handles.insert(
            id,
            Handle {
                pid,
                exited: exited.clone(),
                pending: pending.clone(),
                connection: open.clone(),
            },
        );
        let streams = (stdin, stdout);
        let process = (pid, exited.clone());
        let connection = tokio::spawn(connect(
            id,
            cwd,
            streams,
            process,
            pending,
            open,
            tx.clone(),
        ));
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(Msg::AgentStderr { id, line }).await.is_err() {
                    break;
                }
            }
            let status = child.wait().await;
            // A failed start is reported before the exit. shortcut: a process the agent left
            // holding its stdout delays the exit until it closes or the agent is killed; time
            // this out if wrappers ever leave such processes.
            _ = connection.await;
            exited.store(true, Ordering::SeqCst);
            match status {
                Ok(status) => _ = tx.send(exit(id, status)).await,
                Err(err) => tracing::warn!("waiting for the agent: {err}"),
            }
        });
        rx
    }

    /// Starts the process here, so a quit right after can kill it; the rest runs in a task.
    fn list(
        &mut self,
        request: u64,
        command: String,
        args: Vec<String>,
        cwd: PathBuf,
        env: Vec<(String, String)>,
    ) -> Receiver<Msg> {
        let (tx, rx) = channel(1);
        let fail = |err: String| {
            _ = tx.try_send(Msg::AgentListed {
                request,
                result: Err(err),
            });
        };
        if !cwd.is_dir() {
            fail(format!("{}: not a directory", cwd.display()));
            return rx;
        }
        // No log buffer for a throwaway process, so its stderr goes nowhere.
        let mut child = match command_in(&command, args, &cwd, env, Stdio::null()) {
            Ok(child) => child,
            Err(err) => {
                fail(format!("{command}: {err}"));
                return rx;
            }
        };
        let pid = child.id().expect("a spawned child has a pid") as libc::pid_t;
        let (stdin, stdout) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let exited = Arc::new(AtomicBool::new(false));
        (self.lists.lock().unwrap()).insert(request, (pid, exited.clone()));
        let lists = self.lists.clone();
        tokio::spawn(async move {
            // shortcut: an agent that never answers keeps its process until quit; time this out
            // if one ever hangs.
            let result = list_sessions(cwd, stdin, stdout).await;
            _ = tx.send(Msg::AgentListed { request, result }).await;
            // A quit that drained it terminates it instead.
            if lists.lock().unwrap().remove(&request).is_some()
                && let Some(group) = terminate(pid, &exited)
            {
                kill_after_grace(group);
            }
            _ = child.wait().await;
            exited.store(true, Ordering::SeqCst);
        });
        rx
    }
}

/// Initializes the agent and asks for its sessions in `cwd`, following `nextCursor`.
async fn list_sessions(
    cwd: PathBuf,
    stdin: ChildStdin,
    stdout: ChildStdout,
) -> Result<Vec<SessionInfo>, String> {
    let transport = ByteStreams::new(stdin.compat_write(), stdout.compat());
    let listed = Client
        .builder()
        .connect_with(transport, async |agent: ConnectionTo<Agent>| {
            let initialize = InitializeRequest::new(ProtocolVersion::V1)
                .client_info(Implementation::new("aiditor", env!("CARGO_PKG_VERSION")));
            let initialized = agent.send_request(initialize).block_task().await?;
            let capabilities = initialized.agent_capabilities.session_capabilities;
            if capabilities.list.is_none() {
                return Ok(Err("the agent can't list sessions".to_owned()));
            }
            let (mut sessions, mut cursor) = (Vec::new(), None);
            for _ in 0..LIST_PAGES {
                let page = ListSessionsRequest::new().cwd(cwd.clone()).cursor(cursor);
                let page = agent.send_request(page).block_task().await?;
                sessions.extend(page.sessions);
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            Ok(Ok(sessions))
        })
        .await;
    listed.map_err(|err| err.to_string())?
}

/// Starts `command` in `cwd` leading its own process group, with piped stdin and stdout.
fn command_in(
    command: &str,
    args: Vec<String>,
    cwd: &Path,
    env: Vec<(String, String)>,
    stderr: Stdio,
) -> std::io::Result<Child> {
    // With `PATH` in `env`, the program is looked up on that `PATH`.
    Command::new(command)
        .args(args)
        .current_dir(cwd)
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
}

/// Initializes the agent and opens a session in `cwd`, then forwards the agent's requests until
/// it closes its stdout, and then terminates it, so a session that stops speaking still exits.
async fn connect(
    id: AgentId,
    cwd: PathBuf,
    (stdin, stdout): (ChildStdin, ChildStdout),
    (pid, exited): (libc::pid_t, Arc<AtomicBool>),
    pending: Pending,
    open: Connection,
    tx: Sender<Msg>,
) {
    let (requests, updates) = (tx.clone(), tx.clone());
    let transport = ByteStreams::new(stdin.compat_write(), stdout.compat());
    let mut started = false;
    let result = Client
        .builder()
        .on_receive_request(
            async move |body: AgentRequest, responder: Responder, _| {
                let request = responder.id().clone();
                pending.lock().unwrap().insert(request.clone(), responder);
                _ = (requests.send(Msg::AgentRequest {
                    id,
                    request,
                    body: Request(Box::new(body)),
                }))
                .await;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            async move |update: SessionNotification, _| {
                let update = Box::new(update);
                _ = updates.send(Msg::AgentUpdate { id, update }).await;
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_with(transport, async |agent: ConnectionTo<Agent>| {
            // The default capabilities offer neither fs nor terminals: agents use the disk. The
            // `_meta` extension asks for command output in tool calls (#100).
            let meta = serde_json::json!({"terminal_output": true});
            let initialize = InitializeRequest::new(ProtocolVersion::V1)
                .client_capabilities(ClientCapabilities::new().meta(meta.as_object().cloned()))
                .client_info(Implementation::new("aiditor", env!("CARGO_PKG_VERSION")));
            agent.send_request(initialize).block_task().await?;
            let session = (agent.send_request(NewSessionRequest::new(cwd)))
                .block_task()
                .await?;
            started = true;
            // Before the core hears of it, so its first prompt finds the connection.
            *open.lock().unwrap() = Some((agent.clone(), tx.clone()));
            _ = (tx.send(Msg::AgentStarted {
                id,
                result: Ok(session),
            }))
            .await;
            agent.incoming_closed().await;
            Ok(())
        })
        .await;
    open.lock().unwrap().take();
    if started && let Some(group) = terminate(pid, &exited) {
        kill_after_grace(group);
    }
    if let Err(err) = result {
        let err = err.to_string();
        if started {
            tracing::warn!("agent connection: {err}");
        } else {
            _ = (tx.send(Msg::AgentStarted {
                id,
                result: Err(err),
            }))
            .await;
        }
    }
}

fn exit(id: AgentId, status: ExitStatus) -> Msg {
    Msg::AgentExited {
        id,
        code: status.code().map_or(1, |code| code as u32),
        signal: status.signal().map(|signal| {
            // `strsignal` returns a static or thread-local string, valid until the next call.
            unsafe { CStr::from_ptr(libc::strsignal(signal)) }
                .to_string_lossy()
                .into_owned()
        }),
    }
}
