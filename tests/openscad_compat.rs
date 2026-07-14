//! Compatibility tests: parse all .scad files from the `OpenSCAD` test suite.

use std::path::Path;

#[test]
fn parse_openscad_test_files() {
    // Debug builds need more stack for deeply nested fixture files.
    let builder = std::thread::Builder::new().stack_size(8 * 1024 * 1024);
    let handler = builder
        .spawn(run_compat_tests)
        .expect("failed to spawn test thread");
    handler.join().expect("test thread panicked");
}

fn run_compat_tests() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("vendor")
        .join("openscad")
        .join("tests")
        .join("data")
        .join("scad");

    if !test_dir.exists() {
        eprintln!(
            "Skipping OpenSCAD compat tests: submodule not checked out at {}",
            test_dir.display()
        );
        return;
    }

    let mut total = 0;
    let mut passed = 0;
    let mut failed = Vec::new();

    for entry in glob::glob(&format!("{}/**/*.scad", test_dir.display())).unwrap() {
        let path = entry.unwrap();
        total += 1;

        let source = match std::fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                eprintln!("  SKIP (read error): {} — {e}", path.display());
                continue;
            }
        };

        match openscad_rs::parse(&source) {
            Ok(_) => passed += 1,
            Err(e) => {
                let relative = path.strip_prefix(&test_dir).unwrap_or(&path);
                failed.push(format!("  FAIL: {} — {e}", relative.display()));
            }
        }
    }

    println!("\n=== OpenSCAD Compatibility Results ===");
    println!("Total: {total}, Passed: {passed}, Failed: {}", failed.len());
    println!(
        "Pass rate: {:.1}%",
        if total > 0 {
            f64::from(passed) / f64::from(total) * 100.0
        } else {
            0.0
        }
    );

    if !failed.is_empty() {
        println!("\nFailed files:");
        for f in &failed {
            println!("{f}");
        }
    }

    // The upstream corpus includes experimental and intentionally invalid syntax,
    // so this guards against broad regressions rather than requiring every file.
    assert!(
        f64::from(passed) / f64::from(total) > 0.5,
        "Pass rate too low: {passed}/{total}"
    );
}
