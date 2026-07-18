# openscad-rs

[![Crates.io](https://img.shields.io/crates/v/openscad-rs.svg)](https://crates.io/crates/openscad-rs)
[![docs.rs](https://docs.rs/openscad-rs/badge.svg)](https://docs.rs/openscad-rs)
[![CI](https://github.com/ierror/openscad-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ierror/openscad-rs/actions/workflows/ci.yml)

A [OpenSCAD](https://openscad.org) parser library for Rust.

Parses `.scad` source files into a well-typed AST suitable for building compilers, formatters, linters, and language servers.

## Features

- **Fast** — [logos](https://github.com/maciejhirsz/logos)-based zero-copy lexer compiles to jump tables
- **Complete** — Covers the full OpenSCAD language grammar (98.5% pass rate on OpenSCAD's own test suite)
- **Typed AST** — Every node carries source spans for precise error reporting and tooling
- **Compiler-ready** — Designed as a foundation for downstream compiler applications
- **Safe** — `#[forbid(unsafe_code)]`, pedantic clippy, comprehensive tests
- **Zero dependencies at runtime** — Only `logos`, `miette`, and `thiserror`

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
openscad-rs = "0.1.0"
```

Parse a source file:

```rust
use openscad_rs::{parse, Statement, ExprKind};

let source = r#"
    module rounded_box(size = [10, 10, 10], r = 1) {
        if (r > 0) {
            minkowski() {
                cube(size - [2*r, 2*r, 2*r]);
                sphere(r = r, $fn = 20);
            }
        } else {
            cube(size);
        }
    }

    rounded_box(size = [30, 20, 10], r = 2);
"#;

let ast = parse(source).expect("parse error");

for stmt in &ast.statements {
    match stmt {
        Statement::ModuleDefinition { name, params, .. } => {
            println!("module {name}({} params)", params.len());
        }
        Statement::ModuleInstantiation { name, args, .. } => {
            println!("call {name}({} args)", args.len());
        }
        _ => {}
    }
}
// Output:
// module rounded_box(2 params)
// call rounded_box(2 args)
```

## Architecture

```
src/
├── lib.rs       # Public API: parse(), re-exports
├── token.rs     # Token enum (logos-generated)
├── lexer.rs     # Tokenizer: source → tokens with spans
├── ast.rs       # AST node types: Expr, Statement, etc.
├── parser.rs    # Recursive-descent parser
├── span.rs      # Source location tracking
├── error.rs     # ParseError with miette diagnostics
└── visit.rs     # AST visitor trait
```

## Supported Language Features

| Feature                                                            | Status |
| ------------------------------------------------------------------ | ------ |
| Literals (numbers, hex, strings, booleans, undef)                  | ✅     |
| String escape sequences (`\n`, `\t`, `\xHH`, `\uHHHH`, `\UHHHHHH`) | ✅     |
| Variables & assignments                                            | ✅     |
| Full operator precedence (17 levels)                               | ✅     |
| Ternary expressions                                                | ✅     |
| Vectors & ranges                                                   | ✅     |
| Module definitions & instantiation                                 | ✅     |
| Function definitions                                               | ✅     |
| Anonymous functions                                                | ✅     |
| `if`/`else` (statement & expression)                               | ✅     |
| `for`, `let`, `each` list comprehensions                           | ✅     |
| `echo()`, `assert()`                                               | ✅     |
| `include <file>`, `use <file>`                                     | ✅     |
| Modifier prefixes (`!`, `#`, `%`, `*`)                             | ✅     |
| Comments (`//`, `/* */`)                                           | ✅     |
| Member access (`obj.x`) & indexing (`v[i]`)                        | ✅     |
| Bitwise operators (`&`, `\|`, `<<`, `>>`, `~`)                     | ✅     |
| Exponentiation (`^`)                                               | ✅     |
| Named & positional arguments                                       | ✅     |
| Trailing commas                                                    | ✅     |

## AST Visitor

Traverse the AST with the built-in visitor trait:

```rust
use openscad_rs::{parse, Visitor, Expr, ExprKind, Statement};

struct ModuleCounter(usize);

impl Visitor for ModuleCounter {
    fn visit_statement(&mut self, stmt: &Statement) {
        if matches!(stmt, Statement::ModuleInstantiation { .. }) {
            self.0 += 1;
        }
        // Call default implementation to recurse into children
        openscad_rs::visit::Visitor::visit_statement(self, stmt);
    }
}

let ast = parse("union() { cube(5); sphere(3); }").unwrap();
let mut counter = ModuleCounter(0);
counter.visit_file(&ast);
assert_eq!(counter.0, 3); // union, cube, sphere
```

## Compatibility

The parser is validated against the [OpenSCAD test suite](https://github.com/openscad/openscad/tree/master/tests/data/scad) (523 `.scad` files), achieving a **98.5% pass rate**.

To run the compatibility tests:

```bash
git submodule update --init
cargo test --test openscad_compat -- --nocapture
```

## Benchmarking

```bash
cargo bench
```

## Design Decisions

- **Parser only** — No semantic analysis, type checking, or evaluation. This is purely syntactic parsing. Downstream crates handle the rest.
- **`include`/`use` are AST nodes** — We parse the directive but don't resolve or load files. That's the compiler's responsibility.
- **Owned AST** — Uses `String` and `Box<Expr>` for simplicity. Arena allocation can be added later for zero-copy parsing.
- **Lossless source locations** — Every AST node carries a `Span` with byte offsets for precise source mapping.
- **Recursion depth guard** — Prevents stack overflow on adversarial/deeply nested inputs.

## Contact

[@boerni@chaos.social](https://chaos.social/@boerni)

## License

Licensed under either of:

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.
