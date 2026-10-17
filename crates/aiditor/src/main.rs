use std::time::Instant;

use aiditor_core::{Core, Msg};
use gpui::{App, Application, Context, Task, Window, WindowOptions, div, prelude::*};
use tokio::sync::mpsc;
use tracing_subscriber::{EnvFilter, filter::LevelFilter};

struct Editor {
    core: Core,
    #[expect(
        dead_code,
        reason = "keeps the channel open; effects will clone it into services"
    )]
    tx: mpsc::Sender<Msg>,
    /// Wakes the core at its next deadline; replacing it cancels the old one.
    timer: Option<Task<()>>,
}

impl Editor {
    fn new(cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = aiditor_services::channel();
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.recv().await {
                if this
                    .update(cx, |editor, cx| editor.handle(msg, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Editor {
            core: Core::default(),
            tx,
            timer: None,
        }
    }

    /// Every message reaches the core here, so the timer is always re-armed.
    fn handle(&mut self, msg: Msg, cx: &mut Context<Self>) {
        #[expect(clippy::never_loop, reason = "Effect has no variants yet")]
        for effect in self.core.handle(Instant::now(), msg) {
            match effect {}
        }
        self.timer = self.core.next_deadline().map(|deadline| {
            let wait = cx
                .background_executor()
                .timer(deadline.saturating_duration_since(Instant::now()));
            cx.spawn(async move |this, cx| {
                wait.await;
                this.update(cx, |editor, cx| editor.handle(Msg::Tick, cx))
                    .ok();
            })
        });
        cx.notify();
    }
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.core.text())
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    // Services run here, on background threads; GPUI owns the main thread.
    let runtime = tokio::runtime::Runtime::new().expect("failed to start the Tokio runtime");
    let _guard = runtime.enter();
    tracing::info!("starting");

    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(Editor::new))
            .expect("failed to open a window");
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
