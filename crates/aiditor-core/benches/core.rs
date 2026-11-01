use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use aiditor_core::{
    Core, Msg, buffer::Buffers, display, key::parse_keys, settings::Settings, terminal::Terminal,
};

mod scenarios;
#[path = "../../../benches/support.rs"]
mod support;
use support::measure;

fn main() {
    let fixture: String = (0..100_000)
        .map(|i| {
            let kind = if i % 5 == 0 { "skip" } else { "keep" };
            format!("{i:06} alpha needle value {kind} été\n")
        })
        .collect();
    let path = Path::new("/aiditor-benchmark.txt");
    let mut output = String::new();
    measure(
        "buffer-load/100k-lines",
        || {
            let mut buffers = Buffers::default();
            let id = buffers.create_for_file(path, black_box(&fixture));
            let buffer = buffers.get(id).unwrap();
            black_box(buffer.text());
        },
        &mut output,
    );

    let mut buffers = Buffers::default();
    let id = buffers.create_for_file(path, &fixture);
    let buffer = buffers.get_mut(id).unwrap();
    let original_len = buffer.len_chars();
    assert_eq!(buffer.text().len_lines(), 100_001);
    let middle = buffer.text().line_to_char(50_000);
    measure(
        "edit-undo/100k-lines",
        || {
            buffer.undo_boundary(middle);
            buffer.insert(middle, black_box("🙂")).unwrap();
            black_box(buffer.undo().unwrap());
        },
        &mut output,
    );
    assert_eq!(buffer.len_chars(), original_len);
    assert_eq!(buffer.text().to_string(), fixture);

    let settings = Settings::default();
    let frame = display::build(
        buffer,
        50_000,
        50,
        middle,
        None,
        &settings,
        0..120,
        None,
        &[],
        None,
    );
    assert_eq!(frame.lines.len(), 50);
    assert_eq!(
        frame.lines[0].spans[0].text,
        "050000 alpha needle value skip été"
    );
    measure(
        "display/50-rows-in-100k-lines",
        || {
            black_box(display::build(
                buffer,
                50_000,
                50,
                middle,
                None,
                &settings,
                0..120,
                None,
                &[],
                None,
            ));
        },
        &mut output,
    );

    let mut core = Core::default();
    core.open(path, Some((fixture.as_bytes().to_vec(), 0)));
    core.handle(Instant::now(), Msg::Key(parse_keys("C-s").unwrap()[0]));
    let mut previous = 0;
    let queries = [("needle", 100_000), ("needle !skip", 80_000)];
    for (query, matches) in queries {
        core.handle(
            Instant::now(),
            Msg::Insert {
                text: query.into(),
                replace: Some(0..previous),
            },
        );
        previous = query.encode_utf16().count();
        assert_eq!(
            core.frame().minibuffer.unwrap().count,
            format!("{:>8} ", format!("1/{matches}"))
        );
    }
    measure(
        "swiper/two-queries-in-100k-lines",
        || {
            for (query, _) in queries {
                black_box(core.handle(
                    Instant::now(),
                    Msg::Insert {
                        text: query.into(),
                        replace: Some(0..previous),
                    },
                ));
                previous = query.encode_utf16().count();
            }
        },
        &mut output,
    );
    assert_eq!(core.text(), "needle !skip");
    assert_eq!(core.frame().minibuffer.unwrap().count, " 1/80000 ");

    // One PTY batch is at most 64 KB; the core parses each on the UI thread.
    let (terminal, _) = core.create_terminal("vterm", 50, 120);
    let yes = "y\r\n".repeat(64 * 1024 / 3).into_bytes();
    let cat = fixture.replace('\n', "\r\n").as_bytes()[..64 * 1024].to_vec();
    for (name, batch) in [
        ("terminal-output/64k-batch-of-yes", yes),
        ("terminal-output/64k-batch-of-cat", cat.clone()),
    ] {
        measure(
            name,
            || {
                black_box(core.handle(
                    Instant::now(),
                    Msg::TerminalOutput {
                        id: terminal,
                        bytes: batch.clone(),
                    },
                ));
            },
            &mut output,
        );
    }
    let mut terminal = Terminal::new(50, 120);
    terminal.feed(&cat);
    assert!(terminal.screen().contains("alpha needle value"));
    let theme = Settings::default().theme;
    measure(
        "terminal-display/50x120-screen",
        || {
            black_box(terminal.display(&theme));
        },
        &mut output,
    );
    scenarios::run(&mut output);
    if let Some(path) = std::env::var_os("BENCH_OUTPUT") {
        std::fs::write(path, output).expect("write benchmark results");
    }
}
