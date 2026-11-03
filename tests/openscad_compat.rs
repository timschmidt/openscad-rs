//! Compatibility ratchet: parse every `.scad` file in the pinned `OpenSCAD`
//! test corpus and compare the set that parses with a recorded list.
//!
//! The corpus is the `vendor/openscad` submodule. It includes experimental and
//! deliberately invalid syntax, so not every file is expected to parse.
//! Instead, `tests/openscad_compat_passing.txt` records exactly which files
//! parse. The test fails when:
//! - a recorded file stops parsing (a regression);
//! - a file that is not recorded starts parsing, so the fix is locked in;
//! - a recorded file no longer exists, because the corpus pin changed.
//!
//! To accept an intentional change, rerun with `OPENSCAD_COMPAT_BLESS=1`. That
//! rewrites the list; commit it together with the change that caused it.
//!
//! Without the submodule the test is skipped, unless `OPENSCAD_COMPAT_REQUIRED`
//! is set, as it is in CI. Then a missing corpus is a failure, not a pass.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const PASSING_LIST: &str = "tests/openscad_compat_passing.txt";

#[test]
fn parse_openscad_test_files() {
    // Debug builds need more stack for deeply nested fixture files.
    let builder = std::thread::Builder::new().stack_size(8 * 1024 * 1024);
    let handler = builder
        .spawn(run_compat_tests)
        .expect("failed to spawn test thread");
    handler.join().expect("test thread panicked");
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn corpus_dir() -> PathBuf {
    manifest_dir().join("vendor/openscad/tests/data/scad")
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn recorded_passing() -> BTreeSet<String> {
    let text = std::fs::read_to_string(manifest_dir().join(PASSING_LIST))
        .unwrap_or_else(|error| panic!("cannot read {PASSING_LIST}: {error}"));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn write_passing(passing: &BTreeSet<String>) {
    let mut text = String::from(
        "# Files in vendor/openscad/tests/data/scad that openscad-rs parses.\n\
         # Maintained by tests/openscad_compat.rs; regenerate with\n\
         # OPENSCAD_COMPAT_BLESS=1 cargo test --test openscad_compat\n",
    );
    for path in passing {
        text.push_str(path);
        text.push('\n');
    }
    std::fs::write(manifest_dir().join(PASSING_LIST), text)
        .unwrap_or_else(|error| panic!("cannot write {PASSING_LIST}: {error}"));
}

fn run_compat_tests() {
    let test_dir = corpus_dir();
    if !test_dir.exists() {
        assert!(
            std::env::var_os("OPENSCAD_COMPAT_REQUIRED").is_none(),
            "OPENSCAD_COMPAT_REQUIRED is set, but the corpus is missing at {}; \
             run `git submodule update --init`",
            test_dir.display()
        );
        eprintln!(
            "Skipping OpenSCAD compat tests: submodule not checked out at {}",
            test_dir.display()
        );
        return;
    }

    let mut total = 0_u32;
    let mut passing = BTreeSet::new();
    let mut failed = Vec::new();

    for entry in glob::glob(&format!("{}/**/*.scad", test_dir.display())).unwrap() {
        let path = entry.unwrap();
        let name = relative(&path, &test_dir);
        total += 1;

        // A file that cannot be read fails, so it cannot pass silently.
        let source = match std::fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                failed.push(format!("  FAIL (read error): {name} — {e}"));
                continue;
            }
        };

        match openscad_rs::parse(&source) {
            Ok(_) => {
                passing.insert(name);
            }
            Err(e) => failed.push(format!("  FAIL: {name} — {e}")),
        }
    }

    println!("\n=== OpenSCAD Compatibility Results ===");
    println!(
        "Total: {total}, Passed: {}, Failed: {}",
        passing.len(),
        failed.len()
    );
    if !failed.is_empty() {
        println!("\nFailed files:");
        for f in &failed {
            println!("{f}");
        }
    }

    assert!(total > 0, "the corpus at {} is empty", test_dir.display());

    if std::env::var_os("OPENSCAD_COMPAT_BLESS").is_some() {
        write_passing(&passing);
        println!(
            "\nRecorded {} passing files in {PASSING_LIST}.",
            passing.len()
        );
        return;
    }

    let recorded = recorded_passing();
    let regressed: Vec<_> = recorded
        .iter()
        .filter(|name| !passing.contains(*name))
        .collect();
    let newly_passing: Vec<_> = passing.difference(&recorded).collect();

    let mut problems = Vec::new();
    if !regressed.is_empty() {
        problems.push(format!(
            "{} recorded files no longer parse (or no longer exist):\n  {}",
            regressed.len(),
            regressed
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join("\n  ")
        ));
    }
    if !newly_passing.is_empty() {
        problems.push(format!(
            "{} files now parse but are not recorded; lock the fix in with \
             OPENSCAD_COMPAT_BLESS=1:\n  {}",
            newly_passing.len(),
            newly_passing
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join("\n  ")
        ));
    }
    assert!(
        problems.is_empty(),
        "OpenSCAD compatibility ratchet changed ({} recorded, {} now parse):\n{}",
        recorded.len(),
        passing.len(),
        problems.join("\n")
    );
}
