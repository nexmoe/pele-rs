//! Run with `cargo run -p pele-rs --release --example reuse_bench`.
use pele_rs::{ParseOptions, RenderOptions, parse, render};
use std::{hint::black_box, time::Instant};

fn median(mut run: impl FnMut(), iterations: usize) -> f64 {
    for _ in 0..10 {
        run();
    }
    let mut times = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        for _ in 0..iterations {
            run();
        }
        times.push(start.elapsed().as_secs_f64() * 1e6 / iterations as f64);
    }
    times.sort_by(f64::total_cmp);
    times[3]
}

fn main() {
    let mut source = String::from("flowchart LR\n");
    for i in 0..99 {
        source.push_str(&format!("N{i}[Node {i}] --> N{}[Node {}]\n", i + 1, i + 1));
    }
    let options = RenderOptions::default();
    let parsed = parse(&source, &ParseOptions::default()).unwrap();
    assert_eq!(
        parsed.render(&options).unwrap(),
        render(&source, &options).unwrap()
    );
    let one_shot = median(
        || {
            black_box(render(black_box(&source), &options).unwrap());
        },
        100,
    );
    let reused = median(
        || {
            black_box(parsed.render(&options).unwrap());
        },
        100,
    );
    println!("100-node chain, complete SVG, median of 7 batches x 100 renders");
    println!("parse + render: {one_shot:.2} us; reused model: {reused:.2} us");
}
