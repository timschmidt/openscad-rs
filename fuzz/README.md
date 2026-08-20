# openscad-rs fuzzing

These `cargo-fuzz` targets cover the raw lexer, arbitrary parser input,
diagnostic formatting, AST traversal/span invariants, and a generated valid
OpenSCAD grammar.

```sh
cargo +nightly fuzz list
cargo +nightly fuzz run fuzz_lexer -- -max_total_time=60
cargo +nightly fuzz run fuzz_parser -- -max_total_time=60
cargo +nightly fuzz run fuzz_structured_parser -- -max_total_time=60
cargo +nightly fuzz run fuzz_numeric_literals -- -max_total_time=60
```

Inputs are capped before parsing and generated syntax has bounded depth. Keep
minimized, license-clean regressions as ordinary unit tests when possible.
