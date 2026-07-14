use criterion::{Criterion, black_box, criterion_group, criterion_main};
use openscad_rs::parse;
use std::path::Path;

const SAMPLE: &str = r"
// Parametric rounded box
module rounded_box(size = [10, 10, 10], r = 1, center = false) {
    if (r > 0) {
        minkowski() {
            cube(size - [2*r, 2*r, 2*r], center = center);
            sphere(r = r, $fn = 20);
        }
    } else {
        cube(size, center = center);
    }
}

function area(w, h) = w * h;
function volume(w, h, d) = w * h * d;
function clamp(val, lo, hi) = max(lo, min(hi, val));

sizes = [for (i = [1:10]) [i*10, i*5, i*3]];

for (i = [0:len(sizes)-1]) {
    translate([i * 50, 0, 0])
        rounded_box(size = sizes[i], r = clamp(i, 1, 5));
}

difference() {
    union() {
        cube([100, 100, 10], center = true);
        translate([0, 0, 5])
            cylinder(h = 20, r1 = 30, r2 = 20, $fn = 64);
    }
    translate([0, 0, -1])
        cylinder(h = 40, r = 15, $fn = 64);
}
";

/// Load every `.scad` file from the vendored `OpenSCAD` test suite.
#[allow(clippy::cast_precision_loss)]
fn load_test_suite() -> Vec<(String, String)> {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("vendor")
        .join("openscad")
        .join("tests")
        .join("data")
        .join("scad");

    if !test_dir.exists() {
        return vec![];
    }

    glob::glob(&format!("{}/**/*.scad", test_dir.display()))
        .unwrap()
        .filter_map(|entry| {
            let path = entry.ok()?;
            let name = path
                .strip_prefix(&test_dir)
                .unwrap_or(&path)
                .display()
                .to_string();
            let source = String::from_utf8_lossy(&std::fs::read(&path).ok()?).into_owned();
            Some((name, source))
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn bench_parse(c: &mut Criterion) {
    // ── Single-file benchmarks ──────────────────────────────────
    c.bench_function("parse_sample", |b| {
        b.iter(|| parse(black_box(SAMPLE)).unwrap());
    });

    let large = SAMPLE.repeat(100);
    c.bench_function("parse_large_37kb", |b| {
        b.iter(|| parse(black_box(&large)).unwrap());
    });

    // ── Full test suite benchmark ───────────────────────────────
    let files = load_test_suite();
    if files.is_empty() {
        return;
    }

    let total_bytes: usize = files.iter().map(|(_, s)| s.len()).sum();
    let total_kb = total_bytes as f64 / 1024.0;

    c.bench_function(
        &format!("parse_test_suite_{}files_{total_kb:.0}kb", files.len()),
        |b| {
            b.iter(|| {
                let mut ok = 0u32;
                for (_, source) in &files {
                    if parse(black_box(source)).is_ok() {
                        ok += 1;
                    }
                }
                ok
            });
        },
    );

    // ── Find largest files for individual benchmarks ────────────
    let mut by_size: Vec<_> = files.iter().collect();
    by_size.sort_by_key(|entry| std::cmp::Reverse(entry.1.len()));
    for (name, source) in by_size.iter().take(3) {
        c.bench_function(&format!("parse_largest_{name}_{}b", source.len()), |b| {
            b.iter(|| parse(black_box(source)));
        });
    }
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
