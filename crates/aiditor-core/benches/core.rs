use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

use aiditor_core::{Core, Msg, buffer::Buffers, display, key::parse_keys, settings::Settings};

fn measure(name: &str, mut operation: impl FnMut(), output: &mut String) {
    let mut iterations = 1;
    loop {
        let start = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        if start.elapsed() >= Duration::from_millis(10) {
            break;
        }
        iterations *= 2;
    }
    let mut samples = Vec::new();
    for _ in 0..21 {
        let start = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        samples.push(start.elapsed().as_secs_f64() * 1e9 / iterations as f64);
    }
    samples.sort_by(f64::total_cmp);
    println!("{name}: {:.0} ns/op (median of 21 batches)", samples[10]);
    output.push_str(&format!(
        "{name}\t{}\t{}\t{}\n",
        samples[10], samples[0], samples[20]
    ));
}

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
    let frame = display::build(buffer, 50_000, 50, middle, None, &settings, 0..120);
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
            format!("1/{matches} ")
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
    assert_eq!(core.frame().minibuffer.unwrap().count, "1/80000 ");
    if let Some(path) = std::env::var_os("BENCH_OUTPUT") {
        std::fs::write(path, output).expect("write benchmark results");
    }
}
