//! # openscad-rs
//!
//! A high-performance `OpenSCAD` parser library for Rust.
//!
//! Parses `.scad` source files into a well-typed AST suitable for
//! building compilers, formatters, linters, and language servers.
//! Numeric literals are represented directly as [`hyperreal::Real`] values.
//!
//! ## Quick Start
//!
//! ```rust
//! use openscad_rs::parse;
//!
//! let source = r#"
//!     module box(size = 10) {
//!         cube(size);
//!     }
//!     box(size = 20);
//! "#;
//!
//! let ast = parse(source).expect("parse error");
//! println!("Parsed {} statements", ast.statements.len());
//! ```

// Derive expansion assigns diagnostic fields that user-written code does not read.
#![allow(unused_assignments)]

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod token;
pub mod visit;

pub use ast::{
    Argument, BinaryOp, Expr, ExprKind, Modifiers, Parameter, SourceFile, Statement, UnaryOp,
};
pub use error::{ParseError, ParseResult};
pub use parser::parse;
pub use span::Span;
pub use visit::{Visitor, walk_expr, walk_file, walk_statement};
