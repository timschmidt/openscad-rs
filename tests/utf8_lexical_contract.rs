use std::collections::BTreeMap;

use hyperreal::Real;
use openscad_rs::{
    Argument, Expr, ExprKind, Parameter, Span, Statement, Visitor, lexer::lex_with_errors, parse,
    token::Token, walk_expr, walk_statement,
};

fn string_value(literal: &str) -> String {
    let source = format!("value = {literal};");
    let mut file = parse(&source).unwrap();
    assert_eq!(file.statements.len(), 1);
    let Statement::Assignment { name, expr, .. } = file.statements.remove(0) else {
        panic!("one complete string assignment");
    };
    assert_eq!(name, "value");
    assert_eq!(&source[expr.span.start..expr.span.end], literal);
    assert_eq!(expr.span, Span::new(8, 8 + literal.len()));
    let ExprKind::String(value) = expr.kind else {
        panic!("a decoded string literal");
    };
    value
}

#[test]
fn line_feed_continuation_retains_value_and_original_byte_span() {
    assert_eq!(string_value("\"A\\\nB\""), "AB");
}

#[test]
fn crlf_continuation_retains_value_and_original_byte_span() {
    assert_eq!(string_value("\"A\\\r\nB\""), "AB");
}

#[test]
fn literal_line_feed_matches_the_pinned_string_value() {
    assert_eq!(string_value("\"A\nB\""), "AB");
}

#[test]
fn unknown_and_incomplete_escapes_keep_the_following_characters() {
    // These complete literal values are independently observed on the pinned
    // engine. Collect every case before comparing, retaining all failures.
    let actual = [r#""A\qB""#, r#""A\xFFB""#, r#""A\u12z4B""#].map(string_value);
    assert_eq!(actual, ["AqB", "AxFFB", "Au12z4B"]);
}

#[test]
fn ordinary_escapes_zero_byte_and_unicode_payloads_stay_exact() {
    assert_eq!(string_value(r#""A\nB\tC\rD\\E\"F""#), "A\nB\tC\rD\\E\"F");
    assert_eq!(string_value(r#""\x00""#), " ");
    assert_eq!(string_value(r#""\u03a9""#), "Ω");
    assert_eq!(
        string_value("\"ho\u{308}he\u{a0}\u{feff}\""),
        "ho\u{308}he\u{a0}\u{feff}"
    );
}

#[test]
fn breaking_unicode_spaces_preserve_numeric_values_and_source_spans() {
    let source = "\u{feff}alpha\u{a0}=\u{feff}0.1;\u{a0}\u{feff}";
    let (tokens, errors) = lex_with_errors(source);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], (Token::Identifier, Span::new(3, 8)));
    let number_start = source.find("0.1").unwrap();
    assert_eq!(tokens[2].1, Span::new(number_start, number_start + 3));
    let mut file = parse(source).unwrap();
    assert_eq!(file.statements.len(), 1);
    let Statement::Assignment { name, expr, .. } = file.statements.remove(0) else {
        panic!("one numeric assignment");
    };
    assert_eq!(name, "alpha");
    assert_eq!(expr.span, tokens[2].1);
    let ExprKind::Number(value) = expr.kind else {
        panic!("an exact decimal number");
    };
    assert_eq!(value, (Real::from(1) / Real::from(10)).unwrap());
}

#[test]
fn canonical_identifier_spellings_share_names_without_normalizing_source() {
    for (raw, expected) in [
        ("ho\u{308}he", "höhe"),
        ("\u{2126}", "Ω"),
        ("\u{212a}", "K"),
        ("\u{212b}", "Å"),
        ("\u{1100}\u{1161}", "가"),
        ("寸法", "寸法"),
    ] {
        let source = format!("{raw} = {raw};");
        let (tokens, errors) = lex_with_errors(&source);
        assert!(errors.is_empty(), "{raw}: {errors:?}");
        assert_eq!(tokens[0].1, Span::new(0, raw.len()));
        let mut file = parse(&source).unwrap();
        let Statement::Assignment { name, expr, .. } = file.statements.remove(0) else {
            panic!("one identifier assignment");
        };
        assert_eq!(name, expected);
        assert_eq!(&source[expr.span.start..expr.span.end], raw);
        assert_eq!(expr.kind, ExprKind::Identifier(expected.to_owned()));
    }
    let file = parse("include <ho\u{308}he.scad>\nuse <ho\u{308}he.scad>").unwrap();
    assert!(
        matches!(&file.statements[0], Statement::Include { path, .. } if path == "ho\u{308}he.scad")
    );
    assert!(
        matches!(&file.statements[1], Statement::Use { path, .. } if path == "ho\u{308}he.scad")
    );
}

#[derive(Default)]
struct Names(BTreeMap<String, usize>);

impl Names {
    fn name(&mut self, name: &str) {
        *self.0.entry(name.to_owned()).or_default() += 1;
    }

    fn parameters(&mut self, parameters: &[Parameter]) {
        for parameter in parameters {
            self.name(&parameter.name);
        }
    }

    fn arguments(&mut self, arguments: &[Argument]) {
        for argument in arguments {
            if let Some(name) = &argument.name {
                self.name(name);
            }
        }
    }
}

impl Visitor for Names {
    fn visit_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Assignment { name, .. } => self.name(name),
            Statement::ModuleDefinition { name, params, .. }
            | Statement::FunctionDefinition { name, params, .. } => {
                self.name(name);
                self.parameters(params);
            }
            Statement::ModuleInstantiation { name, args, .. } => {
                self.name(name);
                self.arguments(args);
            }
            _ => {}
        }
        walk_statement(self, statement);
    }

    fn visit_expr(&mut self, expression: &Expr) {
        match &expression.kind {
            ExprKind::Identifier(name) => self.name(name),
            ExprKind::MemberAccess { member, .. } => self.name(member),
            ExprKind::FunctionCall { args, .. } => self.arguments(args),
            ExprKind::Let { assignments, .. }
            | ExprKind::LcFor { assignments, .. }
            | ExprKind::LcLet { assignments, .. } => self.arguments(assignments),
            ExprKind::AnonymousFunction { params, .. } => self.parameters(params),
            _ => {}
        }
        walk_expr(self, expression);
    }
}

#[test]
fn canonical_names_reach_declarations_arguments_bindings_and_members() {
    let source = "module ho\u{308}he(pa\u{308}ram=1) { echo(pa\u{308}ram); }\n\
                  function fu\u{308}n(pa\u{308}ram)=pa\u{308}ram;\n\
                  ho\u{308}he(pa\u{308}ram=fu\u{308}n(pa\u{308}ram=2));\n\
                  a=let(pa\u{308}ram=1)pa\u{308}ram;\n\
                  b=object.me\u{308}mber;\n\
                  c=function(pa\u{308}ram)pa\u{308}ram;\n\
                  d=[for(pa\u{308}ram=[1:2])pa\u{308}ram];\n\
                  $ho\u{308}he=$ho\u{308}he;";
    let file = parse(source).unwrap();
    let mut names = Names::default();
    names.visit_file(&file);
    let expected: BTreeMap<String, usize> = [
        ("höhe", 2),
        ("$höhe", 2),
        ("päram", 12),
        ("fün", 2),
        ("echo", 1),
        ("a", 1),
        ("b", 1),
        ("c", 1),
        ("d", 1),
        ("object", 1),
        ("mëmber", 1),
    ]
    .into_iter()
    .map(|(name, count)| (name.to_owned(), count))
    .collect();
    assert_eq!(names.0, expected);
}

#[test]
fn pinned_identifier_profile_and_malformed_strings_still_fail_typed() {
    for name in [
        "µ",
        "Ａ",
        "♥",
        "a\u{fe0f}",
        "a\u{200d}",
        "a\u{202e}",
        "\u{308}a",
        "\u{e000}",
        "\u{37e}",
    ] {
        assert!(parse(&format!("{name}=1;")).is_err(), "{name:?}");
    }
    for source in ["x=\"unterminated", "x=\"escaped\\", "/* unterminated"] {
        assert!(parse(source).is_err(), "{source:?}");
    }
    // Existing ASCII and legacy digit-leading identifiers remain admitted.
    assert!(parse("_name=1; $name=2; 3dName=3;").is_ok());
}

#[test]
fn unicode_escape_boundaries_use_the_pinned_space_convention() {
    let actual = [
        r#""before\uD800after""#,
        r#""before\uDFFFafter""#,
        r#""before\U110000after""#,
        r#""before\UFFFFFFafter""#,
        r#""before\u0000after""#,
    ]
    .map(string_value);
    assert_eq!(actual, ["before after"; 5]);
    assert_eq!(string_value(r#""\U10FFFF""#), "\u{10ffff}");
    assert_eq!(string_value(r#""\uFFFF""#), "\u{ffff}");
}

#[test]
fn literal_crlf_keeps_its_carriage_return_and_original_source_span() {
    assert_eq!(string_value("\"before\r\nafter\""), "before\rafter");
}

#[test]
fn unicode_breaking_spaces_end_tokens_at_valid_source_boundaries() {
    let sources = [
        "\u{a0}",
        "\u{feff}",
        "a\u{a0}=1;",
        "a\u{feff}=1;",
        "α\u{a0}β",
        "a\u{a0}b",
        "a\u{feff}b",
    ];
    let observations: Vec<_> = sources
        .into_iter()
        .map(|source| {
            let (tokens, errors) = lex_with_errors(source);
            println!("{source:?}: tokens={tokens:?}, errors={errors:?}");
            (source, tokens, errors)
        })
        .collect();
    for (source, tokens, errors) in observations {
        assert!(errors.is_empty(), "{source:?}: {errors:?}");
        for (_, span) in tokens {
            assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
        }
    }
    for source in ["α\u{a0}β", "a\u{a0}b", "a\u{feff}b"] {
        let (tokens, _) = lex_with_errors(source);
        assert!(matches!(
            tokens.as_slice(),
            [(Token::Identifier, _), (Token::Identifier, _)]
        ));
        assert!(
            parse(&format!("{source}=1;")).is_err(),
            "whitespace must not join names"
        );
    }
}
