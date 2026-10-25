//! Async I/O services. They run on the Tokio runtime and message the core.

pub mod agents;
pub mod configuration;
pub mod environment;
pub mod files;
pub mod grep;
pub mod projects;
pub mod recovery;
pub mod terminal;
pub mod watcher;

use aiditor_core::Msg;
use tokio::sync::mpsc;

/// The shared service channel; terminals use their own bounded channels.
pub fn channel() -> (mpsc::Sender<Msg>, mpsc::Receiver<Msg>) {
    mpsc::channel(256)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use aiditor_core::Core;
    use aiditor_core::key::{Key, KeyCode};

    use super::*;

    #[tokio::test]
    async fn service_results_reach_the_core_in_order() {
        let (tx, mut rx) = channel();
        tokio::spawn(async move {
            for c in ['x', 'y'] {
                tx.send(Msg::Key(Key::plain(KeyCode::Char(c))))
                    .await
                    .unwrap();
            }
        });
        let mut core = Core::default();
        while let Some(msg) = rx.recv().await {
            core.handle(Instant::now(), msg);
        }
        assert_eq!(core.text(), "xy");
    }
}
