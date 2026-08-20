#![allow(dead_code)]

use std::borrow::Cow;

use openscad_rs::{
    Expr, ExprKind, ParseError, SourceFile, Span, Statement, Visitor, walk_expr, walk_statement,
};

pub fn bounded_source(data: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(&data[..data.len().min(32 * 1024)])
}

pub fn assert_span(source: &str, span: Span) {
    assert!(span.start <= span.end);
    assert!(span.end <= source.len());
    assert!(source.is_char_boundary(span.start));
    assert!(source.is_char_boundary(span.end));
}

pub fn validate_error(source: &str, error: &ParseError) {
    let span = match error {
        ParseError::UnexpectedToken { span, .. }
        | ParseError::UnexpectedEof { span, .. }
        | ParseError::Custom { span, .. }
        | ParseError::InvalidToken { span } => span,
    };
    let start = span.offset();
    let end = start.saturating_add(span.len());
    assert!(end <= source.len());
    assert!(source.is_char_boundary(start));
    assert!(source.is_char_boundary(end));
}

pub fn validate_ast(source: &str, file: &SourceFile) {
    assert_span(source, file.span);
    assert_ordered_children(file.span, file.statements.iter().map(Statement::span));
    let mut validator = SpanValidator { source, nodes: 0 };
    validator.visit_file(file);
    assert!(validator.nodes <= source.len().saturating_mul(4).saturating_add(1));
}

fn assert_ordered_children(parent: Span, children: impl IntoIterator<Item = Span>) {
    let mut previous_end = parent.start;
    for child in children {
        assert!(parent.start <= child.start);
        assert!(child.end <= parent.end);
        assert!(previous_end <= child.start);
        previous_end = child.end;
    }
}

struct SpanValidator<'a> {
    source: &'a str,
    nodes: usize,
}

impl SpanValidator<'_> {
    fn check(&mut self, span: Span) {
        assert_span(self.source, span);
        self.nodes += 1;
    }
}

impl Visitor for SpanValidator<'_> {
    fn visit_statement(&mut self, statement: &Statement) {
        let parent = statement.span();
        self.check(parent);
        match statement {
            Statement::Assignment { expr, .. } => {
                assert_ordered_children(parent, [expr.span]);
            }
            Statement::ModuleDefinition { params, body, .. } => {
                for parameter in params {
                    self.check(parameter.span);
                    if let Some(default) = &parameter.default {
                        assert_ordered_children(parameter.span, [default.span]);
                    }
                }
                assert_ordered_children(parent, params.iter().map(|parameter| parameter.span));
                assert_ordered_children(parent, body.iter().map(Statement::span));
            }
            Statement::FunctionDefinition { params, body, .. } => {
                for parameter in params {
                    self.check(parameter.span);
                    if let Some(default) = &parameter.default {
                        assert_ordered_children(parameter.span, [default.span]);
                    }
                }
                assert_ordered_children(parent, params.iter().map(|parameter| parameter.span));
                assert_ordered_children(parent, [body.span]);
            }
            Statement::ModuleInstantiation { args, children, .. } => {
                for argument in args {
                    self.check(argument.span);
                    assert_ordered_children(argument.span, [argument.value.span]);
                }
                assert_ordered_children(parent, args.iter().map(|argument| argument.span));
                assert_ordered_children(parent, children.iter().map(Statement::span));
            }
            Statement::IfElse {
                condition,
                then_body,
                else_body,
                ..
            } => {
                assert_ordered_children(parent, [condition.span]);
                assert_ordered_children(parent, then_body.iter().map(Statement::span));
                if let Some(else_body) = else_body {
                    assert_ordered_children(parent, else_body.iter().map(Statement::span));
                }
            }
            Statement::Block { body, .. } => {
                assert_ordered_children(parent, body.iter().map(Statement::span));
            }
            Statement::Include { .. } | Statement::Use { .. } | Statement::Empty { .. } => {}
        }
        walk_statement(self, statement);
    }

    fn visit_expr(&mut self, expression: &Expr) {
        let parent = expression.span;
        self.check(parent);
        match &expression.kind {
            ExprKind::UnaryOp { operand, .. } => {
                assert_ordered_children(parent, [operand.span]);
            }
            ExprKind::BinaryOp { left, right, .. } => {
                assert_ordered_children(parent, [left.span, right.span]);
            }
            ExprKind::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                assert_ordered_children(parent, [condition.span, then_expr.span, else_expr.span]);
            }
            ExprKind::FunctionCall { callee, args } => {
                assert_ordered_children(parent, [callee.span]);
                for argument in args {
                    self.check(argument.span);
                    assert_ordered_children(argument.span, [argument.value.span]);
                }
                assert_ordered_children(parent, args.iter().map(|argument| argument.span));
            }
            ExprKind::Index { object, index } => {
                assert_ordered_children(parent, [object.span, index.span]);
            }
            ExprKind::MemberAccess { object, .. } => {
                assert_ordered_children(parent, [object.span]);
            }
            ExprKind::Vector(elements) => {
                assert_ordered_children(parent, elements.iter().map(|element| element.span));
            }
            ExprKind::Range { start, step, end } => {
                assert_ordered_children(
                    parent,
                    [
                        Some(start.span),
                        step.as_ref().map(|step| step.span),
                        Some(end.span),
                    ]
                    .into_iter()
                    .flatten(),
                );
            }
            ExprKind::Let { assignments, body }
            | ExprKind::LcLet { assignments, body }
            | ExprKind::LcFor { assignments, body } => {
                for assignment in assignments {
                    self.check(assignment.span);
                    assert_ordered_children(assignment.span, [assignment.value.span]);
                }
                assert_ordered_children(parent, assignments.iter().map(|argument| argument.span));
                assert_ordered_children(parent, [body.span]);
            }
            ExprKind::Assert { args, body } | ExprKind::Echo { args, body } => {
                for argument in args {
                    self.check(argument.span);
                    assert_ordered_children(argument.span, [argument.value.span]);
                }
                assert_ordered_children(parent, args.iter().map(|argument| argument.span));
                if let Some(body) = body {
                    assert_ordered_children(parent, [body.span]);
                }
            }
            ExprKind::AnonymousFunction { params, body } => {
                for parameter in params {
                    self.check(parameter.span);
                    if let Some(default) = &parameter.default {
                        assert_ordered_children(parameter.span, [default.span]);
                    }
                }
                assert_ordered_children(parent, params.iter().map(|parameter| parameter.span));
                assert_ordered_children(parent, [body.span]);
            }
            ExprKind::LcForC {
                init,
                condition,
                update,
                body,
            } => {
                for assignment in init.iter().chain(update) {
                    self.check(assignment.span);
                    assert_ordered_children(assignment.span, [assignment.value.span]);
                }
                assert_ordered_children(parent, init.iter().map(|argument| argument.span));
                assert_ordered_children(parent, [condition.span]);
                assert_ordered_children(parent, update.iter().map(|argument| argument.span));
                assert_ordered_children(parent, [body.span]);
            }
            ExprKind::LcIf {
                condition,
                then_expr,
                else_expr,
            } => {
                assert_ordered_children(parent, [condition.span, then_expr.span]);
                if let Some(else_expr) = else_expr {
                    assert_ordered_children(parent, [else_expr.span]);
                }
            }
            ExprKind::LcEach { body } => {
                assert_ordered_children(parent, [body.span]);
            }
            ExprKind::Number(_)
            | ExprKind::String(_)
            | ExprKind::BoolTrue
            | ExprKind::BoolFalse
            | ExprKind::Undef
            | ExprKind::Identifier(_) => {}
        }
        walk_expr(self, expression);
    }
}

struct ByteCursor<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn byte(&mut self) -> u8 {
        let value = self.data[self.offset % self.data.len()];
        self.offset += 1;
        value
    }

    fn small_int(&mut self) -> i16 {
        i16::from(self.byte() % 17) - 8
    }
}

fn expression(cursor: &mut ByteCursor<'_>, depth: usize) -> String {
    if depth == 0 {
        return match cursor.byte() % 7 {
            0 => cursor.small_int().to_string(),
            1 => format!("{}/{}", cursor.small_int(), cursor.byte() % 7 + 1),
            2 => "true".into(),
            3 => "false".into(),
            4 => "undef".into(),
            5 => "x".into(),
            _ => r#""text\n\u03a9""#.into(),
        };
    }

    match cursor.byte() % 12 {
        0 => {
            let operators = [
                "+", "-", "*", "/", "%", "^", "==", "!=", "<", ">=", "&&", "||",
            ];
            let operator = operators[usize::from(cursor.byte()) % operators.len()];
            format!(
                "({} {operator} {})",
                expression(cursor, depth - 1),
                expression(cursor, depth - 1)
            )
        }
        1 => format!("!({})", expression(cursor, depth - 1)),
        2 => format!(
            "({} ? {} : {})",
            expression(cursor, depth - 1),
            expression(cursor, depth - 1),
            expression(cursor, depth - 1)
        ),
        3 => format!(
            "[{}, {}, {}]",
            expression(cursor, depth - 1),
            expression(cursor, depth - 1),
            expression(cursor, depth - 1)
        ),
        4 => format!(
            "[{} : {} : {}]",
            cursor.small_int(),
            cursor.byte() % 4 + 1,
            cursor.small_int()
        ),
        5 => format!(
            "(let (x = {}) {})",
            expression(cursor, depth - 1),
            expression(cursor, depth - 1)
        ),
        6 => format!(
            "max({}, {})",
            expression(cursor, depth - 1),
            expression(cursor, depth - 1)
        ),
        7 => format!("[1, 2, {}][0]", expression(cursor, depth - 1)),
        8 => format!("[1, 2, {}].x", expression(cursor, depth - 1)),
        9 => format!("[for (i = [0:2]) i + ({})]", expression(cursor, depth - 1)),
        10 => format!(
            "(assert(true, {}) {})",
            expression(cursor, depth - 1),
            expression(cursor, depth - 1)
        ),
        _ => format!(
            "(echo({}) {})",
            expression(cursor, depth - 1),
            expression(cursor, depth - 1)
        ),
    }
}

fn statement(cursor: &mut ByteCursor<'_>, index: usize) -> String {
    let depth = usize::from(cursor.byte() % 4);
    let expr = expression(cursor, depth);
    match cursor.byte() % 8 {
        0 => format!("v{index} = {expr};"),
        1 => format!("cube(size = {expr});"),
        2 => format!("if ({expr}) sphere(r = 1); else cube(1);"),
        3 => format!(
            "translate([{}, {}, {}]) cube(1);",
            cursor.small_int(),
            cursor.small_int(),
            cursor.small_int()
        ),
        4 => format!("m({expr}) {{ sphere(1); }}"),
        5 => "for (i = [0:2]) translate([i, 0, 0]) cube(1);".into(),
        6 => format!("#rotate([0, 0, {}]) square(2);", cursor.small_int()),
        _ => format!("echo({expr}) cube(1);"),
    }
}

pub fn structured_program(data: &[u8]) -> Option<String> {
    if data.is_empty() {
        return None;
    }
    let mut cursor = ByteCursor::new(data);
    let mut source = String::from(
        "function f(x = 1) = x * x + 1;\n\
         module m(a = 1) { translate([a, 0, 0]) children(); }\n",
    );
    let count = usize::from(cursor.byte() % 12) + 1;
    for index in 0..count {
        source.push_str(&statement(&mut cursor, index));
        source.push('\n');
    }
    Some(source)
}
