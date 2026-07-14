#![allow(clippy::cast_precision_loss)]

use std::time::Instant;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: bench_file <path.scad>");
    let bytes = std::fs::read(&path).unwrap();
    let source = String::from_utf8_lossy(&bytes);
    let size = source.len();

    // Warm the parser and allocator before timing.
    for _ in 0..3 {
        let _ = openscad_rs::parse(&source);
    }

    let iters: i32 = if size > 1_000_000 { 10 } else { 100 };
    let start = Instant::now();
    for _ in 0..iters {
        let _ = openscad_rs::parse(&source);
    }
    let elapsed = start.elapsed();
    let per_iter_us = elapsed.as_secs_f64() * 1_000_000.0 / f64::from(iters);
    let mbps = size as f64 / (per_iter_us / 1_000_000.0) / 1_048_576.0;

    println!("{per_iter_us:.1}us {mbps:.1}MB/s {size}B");
}
