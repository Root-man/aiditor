use std::time::{Duration, Instant};

pub fn measure(name: &str, mut operation: impl FnMut(), output: &mut String) {
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

// Setup and teardown stay outside each timed operation for cases that cannot reuse their state.
pub fn prepared<T>(
    name: &str,
    mut setup: impl FnMut() -> T,
    mut operation: impl FnMut(&mut T),
    output: &mut String,
) {
    for _ in 0..3 {
        operation(&mut setup());
    }
    let mut samples = Vec::new();
    for _ in 0..21 {
        let mut state = setup();
        let start = Instant::now();
        operation(&mut state);
        samples.push(start.elapsed().as_secs_f64() * 1e9);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{name}: {:.0} ns/op (median of 21 single-operation batches)",
        samples[10]
    );
    output.push_str(&format!(
        "{name}\t{}\t{}\t{}\n",
        samples[10], samples[0], samples[20]
    ));
}
