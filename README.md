# openscad-rs

[![Crates.io](https://img.shields.io/crates/v/openscad-rs.svg)](https://crates.io/crates/openscad-rs)
[![docs.rs](https://docs.rs/openscad-rs/badge.svg)](https://docs.rs/openscad-rs)
[![CI](https://github.com/timschmidt/openscad-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/timschmidt/openscad-rs/actions/workflows/ci.yml)

`openscad-rs` parses OpenSCAD source into a typed Rust syntax tree. It is a
parser rather than a geometry evaluator: downstream compilers, formatters,
linters, and language servers can interpret the resulting AST for their own
purposes.

The crate provides a `logos`-based lexer, byte spans on every AST node,
structured parse errors, a recursion-depth guard, and reusable read-only AST
traversal. It forbids unsafe Rust.

## Quick start

Add the crate to a project:

```toml
[dependencies]
openscad-rs = "0.1"
```

Parse source and inspect its statements:

```rust
use openscad_rs::{Statement, parse};

let source = r#"
    module rounded_box(size = [10, 10, 10], r = 1) {
        minkowski() {
            cube(size - [2*r, 2*r, 2*r]);
            sphere(r = r, $fn = 20);
        }
    }

    rounded_box(size = [30, 20, 10], r = 2);
"#;

let file = parse(source)?;

for statement in &file.statements {
    match statement {
        Statement::ModuleDefinition { name, params, .. } => {
            println!("module {name} has {} parameters", params.len());
        }
        Statement::ModuleInstantiation { name, args, .. } => {
            println!("call to {name} has {} arguments", args.len());
        }
        _ => {}
    }
}

# Ok::<(), openscad_rs::ParseError>(())
```

## Core API

- [`parse`](https://docs.rs/openscad-rs/latest/openscad_rs/fn.parse.html)
  lexes and parses one source string into a `SourceFile`.
- `Statement` represents assignments, definitions, module calls, conditionals,
  blocks, and `include`/`use` directives.
- `Expr` and `ExprKind` represent literals, operators, calls, indexing, ranges,
  anonymous functions, and list comprehensions.
- `Span` is a half-open byte range into the original UTF-8 source.
- `ParseError` reports invalid tokens, unexpected syntax, incomplete input, and
  excessive nesting.
- `Visitor` provides read-only recursive traversal. Its `walk_*` helpers let an
  override inspect a node and then continue through that node's children.

For example, count nested module calls while preserving default traversal:

```rust
use openscad_rs::{Statement, Visitor, parse, walk_statement};

struct ModuleCounter(usize);

impl Visitor for ModuleCounter {
    fn visit_statement(&mut self, statement: &Statement) {
        if matches!(statement, Statement::ModuleInstantiation { .. }) {
            self.0 += 1;
        }
        walk_statement(self, statement);
    }
}

let file = parse("union() { cube(5); sphere(3); }")?;
let mut counter = ModuleCounter(0);
counter.visit_file(&file);
assert_eq!(counter.0, 3);

# Ok::<(), openscad_rs::ParseError>(())
```

## Language coverage and limits

The parser recognizes OpenSCAD literals, expressions and precedence, vectors,
ranges, list comprehensions, assignments, user-defined functions and modules,
child statements, modifiers, and `include`/`use` syntax. String escape handling
and source locations are retained in the AST.

Parsing is intentionally syntactic. The crate does not resolve included files,
evaluate expressions, type-check programs, or construct geometry. AST strings
and expression boxes are owned; this favors a straightforward downstream API
over arena allocation or a fully zero-copy tree.

An optional compatibility test runs against the vendored upstream OpenSCAD
fixture corpus. That corpus also contains experimental and deliberately invalid
inputs, so the test enforces a regression floor rather than claiming universal
language acceptance.

## Development

```bash
cargo fmt --all --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo bench
```

To exercise the upstream fixtures and the command-line comparison benchmark:

```bash
git submodule update --init
cargo test --test openscad_compat -- --nocapture
./benches/compare_openscad.sh
```

The comparison script additionally requires the `openscad` executable and
Python 3. Its numbers are local measurements, not a stable performance claim.

## References

- [OpenSCAD documentation and language reference](https://openscad.org/documentation.html)
- [OpenSCAD source grammar and test corpus](https://github.com/openscad/openscad)
- [`logos` lexer documentation](https://docs.rs/logos)
- [`miette` diagnostic documentation](https://docs.rs/miette)
- [`thiserror` derive documentation](https://docs.rs/thiserror)

Related geometry work: [csgrs](https://github.com/timschmidt/csgrs) turns
programmatic inputs into constructive-solid-geometry meshes, while
[synaps-cad](https://github.com/timschmidt/synaps-cad) builds an interactive CAD
application around OpenSCAD-like source and `csgrs`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
