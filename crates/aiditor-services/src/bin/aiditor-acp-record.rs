//! Runs an ACP agent with its stdio passed through unchanged, appending every JSON-RPC line to OUT
//! as `{"from": "client" | "agent", "message": ...}`, one JSON line each. How the committed
//! transcripts were recorded is in crates/aiditor-services/tests/transcripts/README.md.
//!
//! usage: aiditor-acp-record OUT -- COMMAND [ARGS...]

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio, exit};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{Value, json};

const USAGE: &str = "usage: aiditor-acp-record OUT -- COMMAND [ARGS...]";

fn fail(reason: impl std::fmt::Display) -> ! {
    eprintln!("aiditor-acp-record: {reason}");
    exit(2)
}

/// Copies `from` to `to` line by line, flushing each, and logs each line as sent by `who`.
fn pipe(from: impl Read, mut to: impl Write, who: &str, out: &Mutex<File>) {
    let mut from = BufReader::new(from);
    let mut line = Vec::new();
    while from.read_until(b'\n', &mut line).unwrap_or(0) > 0 {
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\r', '\n']);
        let message = serde_json::from_str(text).unwrap_or_else(|_| Value::from(text));
        let record = format!("{}\n", json!({"from": who, "message": message}));
        // Logged in one write before it passes, so the kill a reply prompts can't cut it short.
        if let Err(err) = out.lock().unwrap().write_all(record.as_bytes()) {
            eprintln!("aiditor-acp-record: writing the transcript: {err}");
        }
        if to.write_all(&line).and_then(|()| to.flush()).is_err() {
            break;
        }
        line.clear();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, dashes, command, args @ ..] = &args[..] else {
        fail(USAGE)
    };
    if dashes != "--" {
        fail(USAGE)
    }
    let out = OpenOptions::new().create(true).append(true).open(out);
    let out = Arc::new(Mutex::new(out.unwrap_or_else(|err| fail(err))));
    // Stays in our process group, so the editor's kill of the group reaches the agent too.
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| fail(format!("{command}: {err}")));
    let (stdin, stdout) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
    let log = out.clone();
    // Dropping the child's stdin at our stdin's end closes it.
    thread::spawn(move || pipe(std::io::stdin(), stdin, "client", &log));
    pipe(stdout, std::io::stdout(), "agent", &out);
    let status = child.wait().unwrap_or_else(|err| fail(err));
    exit((status.code()).unwrap_or_else(|| 128 + status.signal().unwrap_or(0)))
}
