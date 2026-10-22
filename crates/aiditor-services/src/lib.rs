//! Async I/O services. They run on the Tokio runtime and message the core.

pub mod files;
pub mod grep;
pub mod projects;
pub mod recovery;

use aiditor_core::Msg;
use tokio::sync::mpsc;

/// The channel every service sends results on. Bounded, so a core that falls behind slows services down.
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
