use aiditor_core::Core;
use gpui::{App, Application, Context, Window, WindowOptions, div, prelude::*};
use tracing_subscriber::{EnvFilter, filter::LevelFilter};

struct Editor {
    core: Core,
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
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| Editor {
                core: Core::default(),
            })
        })
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
