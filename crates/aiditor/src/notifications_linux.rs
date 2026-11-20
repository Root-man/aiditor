//! System notifications over the XDG notifications D-Bus interface through `notify-rust`, trimmed
//! from Zed's `gpui_linux/src/linux/system_notifications.rs`: no action buttons, no dismissing, and
//! clicks go to the core as messages on the services channel.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use aiditor_core::Msg;
use notify_rust::{Hint, Notification};
use tokio::sync::mpsc::Sender;

/// The XDG action invoked when the notification's body is activated.
const DEFAULT_ACTION: &str = "default";

/// Set once a failed post has been noted, so a missing server is noted once per run.
static POST_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) struct Notifications {
    /// The server's id for each tag's latest notification, so a newer one replaces it.
    ids: Arc<Mutex<HashMap<String, u32>>>,
}

impl Notifications {
    /// Never `None`: a missing server shows up as a failed post.
    pub(crate) fn new(_tx: Sender<Msg>) -> Option<Self> {
        Some(Self {
            ids: Arc::default(),
        })
    }

    /// Posts at once; a newer notification with the same tag replaces the older one.
    pub(crate) fn show(&self, tag: &str, title: &str, body: &str, tx: Sender<Msg>) {
        self.post(Notification::new(), tag, title, body, tx);
    }

    fn post(&self, mut builder: Notification, tag: &str, title: &str, body: &str, tx: Sender<Msg>) {
        builder
            .appname("aiditor")
            .icon("aiditor")
            .hint(Hint::DesktopEntry("aiditor".into()))
            .summary(title)
            .body(body)
            .action(DEFAULT_ACTION, DEFAULT_ACTION);
        // shortcut: the id is recorded once the post returns, so a second post with the tag before then
        // doesn't replace the first; fine while a buffer's events are seconds apart.
        if let Some(&id) = self.ids.lock().unwrap().get(tag) {
            builder.id(id);
        }
        let (ids, tag) = (self.ids.clone(), tag.to_owned());
        // `show` connects to the session bus and `wait_for_action` blocks until the notification
        // closes, so both run off the UI thread.
        let spawned = std::thread::Builder::new()
            .name("notification".into())
            .spawn(move || match builder.show() {
                Ok(handle) => {
                    ids.lock().unwrap().insert(tag.clone(), handle.id());
                    handle.wait_for_action(|action| {
                        if action == DEFAULT_ACTION {
                            _ = tx.try_send(Msg::NotificationClicked { tag });
                        }
                    });
                }
                Err(error) => {
                    if !POST_FAILED.swap(true, Ordering::Relaxed) {
                        _ = tx.try_send(Msg::Log(format!("Notification not posted: {error}")));
                    }
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("failed to spawn a notification thread: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn a_missing_server_is_noted_once() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let notifications = Notifications::new(tx.clone()).unwrap();
        for _ in 0..2 {
            // No server owns this name, or there is no session bus at all.
            #[allow(deprecated)] // notify-rust marks it test-only, which this is.
            let missing = Notification::at_bus("aiditor_test_missing");
            notifications.post(missing, "1", "title", "body", tx.clone());
        }
        let start = Instant::now();
        let line = loop {
            if let Ok(Msg::Log(line)) = rx.try_recv() {
                break line;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "no failure was noted"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(line.starts_with("Notification not posted: "), "{line}");
        std::thread::sleep(Duration::from_millis(500));
        assert!(rx.try_recv().is_err());
    }
}
