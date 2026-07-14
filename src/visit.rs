//! Read-only traversal of `OpenSCAD` syntax trees.

use crate::ast::{Expr, ExprKind, SourceFile, Statement};

/// A visitor that traverses an AST by reference.
///
/// Each default method visits the node's children. An override can inspect a
/// node and then call the corresponding [`walk_file`], [`walk_statement`], or
/// [`walk_expr`] function to continue recursively.
pub trait Visitor {
    /// Visit a source file and its statements.
    fn visit_file(&mut self, file: &SourceFile) {
        walk_file(self, file);
    }

    /// Visit a statement and its descendant expressions and statements.
    fn visit_statement(&mut self, statement: &Statement) {
        walk_statement(self, statement);
    }

    /// Visit an expression and its descendant expressions.
    fn visit_expr(&mut self, expression: &Expr) {
        walk_expr(self, expression);
    }
}

/// Visit every statement in a source file with `visitor`.
pub fn walk_file<V: Visitor + ?Sized>(visitor: &mut V, file: &SourceFile) {
    for statement in &file.statements {
        visitor.visit_statement(statement);
    }
}

/// Visit the immediate children of `statement` with `visitor`.
pub fn walk_statement<V: Visitor + ?Sized>(visitor: &mut V, statement: &Statement) {
    match statement {
        Statement::Include { .. } | Statement::Use { .. } | Statement::Empty { .. } => {}
        Statement::Assignment { expr, .. } => visitor.visit_expr(expr),
        Statement::ModuleDefinition { params, body, .. } => {
            for parameter in params {
                if let Some(default) = &parameter.default {
                    visitor.visit_expr(default);
                }
            }
            for statement in body {
                visitor.visit_statement(statement);
            }
        }
        Statement::FunctionDefinition { params, body, .. } => {
            for parameter in params {
                if let Some(default) = &parameter.default {
                    visitor.visit_expr(default);
                }
            }
            visitor.visit_expr(body);
        }
        Statement::ModuleInstantiation { args, children, .. } => {
            for argument in args {
                visitor.visit_expr(&argument.value);
            }
            for child in children {
                visitor.visit_statement(child);
            }
        }
        Statement::IfElse {
            condition,
            then_body,
            else_body,
            ..
        } => {
            visitor.visit_expr(condition);
            for statement in then_body {
                visitor.visit_statement(statement);
            }
            if let Some(else_statements) = else_body {
                for statement in else_statements {
                    visitor.visit_statement(statement);
                }
            }
        }
        Statement::Block { body, .. } => {
            for statement in body {
                visitor.visit_statement(statement);
            }
        }
    }
}

/// Visit the immediate children of `expression` with `visitor`.
pub fn walk_expr<V: Visitor + ?Sized>(visitor: &mut V, expression: &Expr) {
    match &expression.kind {
        ExprKind::Number(_)
        | ExprKind::String(_)
        | ExprKind::BoolTrue
        | ExprKind::BoolFalse
        | ExprKind::Undef
        | ExprKind::Identifier(_) => {}
        ExprKind::UnaryOp { operand, .. } => visitor.visit_expr(operand),
        ExprKind::BinaryOp { left, right, .. } => {
            visitor.visit_expr(left);
            visitor.visit_expr(right);
        }
        ExprKind::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            visitor.visit_expr(condition);
            visitor.visit_expr(then_expr);
            visitor.visit_expr(else_expr);
        }
        ExprKind::FunctionCall { callee, args } => {
            visitor.visit_expr(callee);
            for argument in args {
                visitor.visit_expr(&argument.value);
            }
        }
        ExprKind::Index { object, index } => {
            visitor.visit_expr(object);
            visitor.visit_expr(index);
        }
        ExprKind::MemberAccess { object, .. } => visitor.visit_expr(object),
        ExprKind::Vector(elements) => {
            for element in elements {
                visitor.visit_expr(element);
            }
        }
        ExprKind::Range { start, step, end } => {
            visitor.visit_expr(start);
            if let Some(step) = step {
                visitor.visit_expr(step);
            }
            visitor.visit_expr(end);
        }
        ExprKind::Let { assignments, body }
        | ExprKind::LcLet { assignments, body }
        | ExprKind::LcFor {
            assignments, body, ..
        } => {
            for assignment in assignments {
                visitor.visit_expr(&assignment.value);
            }
            visitor.visit_expr(body);
        }
        ExprKind::Assert { args, body } | ExprKind::Echo { args, body } => {
            for argument in args {
                visitor.visit_expr(&argument.value);
            }
            if let Some(body) = body {
                visitor.visit_expr(body);
            }
        }
        ExprKind::AnonymousFunction { params, body } => {
            for parameter in params {
                if let Some(default) = &parameter.default {
                    visitor.visit_expr(default);
                }
            }
            visitor.visit_expr(body);
        }
        ExprKind::LcForC {
            init,
            condition,
            update,
            body,
        } => {
            for assignment in init {
                visitor.visit_expr(&assignment.value);
            }
            visitor.visit_expr(condition);
            for assignment in update {
                visitor.visit_expr(&assignment.value);
            }
            visitor.visit_expr(body);
        }
        ExprKind::LcIf {
            condition,
            then_expr,
            else_expr,
        } => {
            visitor.visit_expr(condition);
            visitor.visit_expr(then_expr);
            if let Some(else_expr) = else_expr {
                visitor.visit_expr(else_expr);
            }
        }
        ExprKind::LcEach { body } => visitor.visit_expr(body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    struct ModuleCounter(usize);

    impl Visitor for ModuleCounter {
        fn visit_statement(&mut self, statement: &Statement) {
            if matches!(statement, Statement::ModuleInstantiation { .. }) {
                self.0 += 1;
            }
            walk_statement(self, statement);
        }
    }

    #[test]
    fn an_override_can_continue_recursive_traversal() {
        let file = parse("union() { cube(5); sphere(3); }").unwrap();
        let mut counter = ModuleCounter(0);

        counter.visit_file(&file);

        assert_eq!(counter.0, 3);
    }
}
