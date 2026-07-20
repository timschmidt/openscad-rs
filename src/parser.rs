/// Recursive-descent parser for `OpenSCAD`.
///
/// Consumes a token stream from the lexer and produces an AST.
/// Uses precedence climbing for expression parsing.
use crate::ast::{
    Argument, BinaryOp, Expr, ExprKind, Modifiers, Parameter, SourceFile, Statement, UnaryOp,
};
use crate::error::{ParseError, ParseResult};
use crate::lexer::{self, SpannedToken};
use crate::span::Span;
use crate::token::Token;

/// Parse an `OpenSCAD` source string into an AST.
///
/// # Errors
/// Returns a `ParseError` if the source contains syntax errors.
pub fn parse(source: &str) -> ParseResult<SourceFile> {
    let tokens = lexer::lex(source);
    let mut parser = Parser::new(source, tokens);
    parser.parse_file()
}

struct Parser<'src> {
    source: &'src str,
    tokens: Vec<SpannedToken>,
    pos: usize,
    depth: usize,
}

const MAX_DEPTH: usize = 256;

impl<'src> Parser<'src> {
    const fn new(source: &'src str, tokens: Vec<SpannedToken>) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
            depth: 0,
        }
    }

    fn enter_recursion(&mut self) -> ParseResult<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(ParseError::custom(
                "maximum nesting depth exceeded",
                self.peek_span(),
            ))
        } else {
            Ok(())
        }
    }

    const fn leave_recursion(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // ── Helpers ──────────────────────────────────────────────

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn peek_at(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset).map(|(t, _)| t)
    }

    fn peek_span(&self) -> Span {
        self.tokens.get(self.pos).map_or_else(
            || Span::new(self.source.len(), self.source.len()),
            |(_, s)| *s,
        )
    }

    fn advance(&mut self) -> Option<SpannedToken> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    const fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn slice(&self, span: Span) -> &'src str {
        &self.source[span.start..span.end]
    }

    fn expect(&mut self, expected: &Token) -> ParseResult<Span> {
        match self.peek() {
            Some(tok) if std::mem::discriminant(tok) == std::mem::discriminant(expected) => {
                let (_, span) = self.advance().unwrap();
                Ok(span)
            }
            Some(_) => {
                let (tok, span) = self.tokens[self.pos].clone();
                Err(ParseError::unexpected_token(
                    &tok.to_string(),
                    &expected.to_string(),
                    span,
                ))
            }
            None => Err(ParseError::unexpected_eof(
                &expected.to_string(),
                self.source.len(),
            )),
        }
    }

    fn expect_identifier(&mut self) -> ParseResult<(String, Span)> {
        match self.peek() {
            Some(Token::Identifier) => {
                let (_, span) = self.advance().unwrap();
                Ok((self.slice(span).to_string(), span))
            }
            // `for`, `let`, `each`, `assert`, `echo` are valid identifiers in module_id context
            Some(Token::For | Token::Let | Token::Each | Token::Assert | Token::Echo) => {
                let (tok, span) = self.advance().unwrap();
                Ok((tok.to_string(), span))
            }
            Some(_) => {
                let (tok, span) = self.tokens[self.pos].clone();
                Err(ParseError::unexpected_token(
                    &tok.to_string(),
                    "identifier",
                    span,
                ))
            }
            None => Err(ParseError::unexpected_eof("identifier", self.source.len())),
        }
    }

    fn eat(&mut self, expected: &Token) -> Option<Span> {
        if self
            .peek()
            .is_some_and(|t| std::mem::discriminant(t) == std::mem::discriminant(expected))
        {
            Some(self.advance().unwrap().1)
        } else {
            None
        }
    }

    // ── File-level ───────────────────────────────────────────

    fn parse_file(&mut self) -> ParseResult<SourceFile> {
        let mut statements = Vec::new();
        while !self.at_end() {
            statements.push(self.parse_statement()?);
        }
        let span = if statements.is_empty() {
            Span::new(0, 0)
        } else {
            statements[0]
                .span()
                .merge(statements.last().unwrap().span())
        };
        Ok(SourceFile { statements, span })
    }

    // ── Statements ───────────────────────────────────────────

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        self.enter_recursion()?;
        let result = self.parse_statement_inner();
        self.leave_recursion();
        result
    }

    fn parse_statement_inner(&mut self) -> ParseResult<Statement> {
        match self.peek() {
            Some(Token::Semicolon) => {
                let span = self.advance().unwrap().1;
                Ok(Statement::Empty { span })
            }
            Some(Token::LBrace) => self.parse_block(),
            Some(Token::Include) => self.parse_include(),
            Some(Token::Use) => self.parse_use(),
            Some(Token::Module) => self.parse_module_def(),
            Some(Token::Function) => self.parse_function_def(),
            Some(Token::If) => self.parse_if_else(),
            // Check for assignment: identifier followed by `=` (but not `==`)
            Some(Token::Identifier) if self.is_assignment_ahead() => self.parse_assignment(),
            // Module instantiation (possibly with modifiers)
            Some(
                Token::Identifier
                | Token::For
                | Token::Let
                | Token::Assert
                | Token::Echo
                | Token::Each
                | Token::Bang
                | Token::Hash
                | Token::Percent
                | Token::Star,
            ) => self.parse_module_instantiation(),
            Some(_) => {
                let (tok, span) = self.tokens[self.pos].clone();
                Err(ParseError::unexpected_token(
                    &tok.to_string(),
                    "statement",
                    span,
                ))
            }
            None => Err(ParseError::unexpected_eof("statement", self.source.len())),
        }
    }

    fn is_assignment_ahead(&self) -> bool {
        // Look for `identifier =` where `=` is not `==`
        if self.pos + 1 < self.tokens.len() {
            matches!(self.tokens[self.pos + 1].0, Token::Assign)
                && (self.pos + 2 >= self.tokens.len()
                    || !matches!(self.tokens[self.pos + 2].0, Token::Assign))
        } else {
            false
        }
    }

    fn parse_block(&mut self) -> ParseResult<Statement> {
        let start = self.expect(&Token::LBrace)?;
        let mut body = Vec::new();
        while self.peek() != Some(&Token::RBrace) && !self.at_end() {
            body.push(self.parse_statement()?);
        }
        let end = self.expect(&Token::RBrace)?;
        Ok(Statement::Block {
            body,
            span: start.merge(end),
        })
    }

    #[allow(clippy::unnecessary_wraps)]
    fn parse_include(&mut self) -> ParseResult<Statement> {
        let (_, span) = self.advance().unwrap(); // consume Include token
        let path = lexer::extract_include_path(self.slice(span)).to_string();
        Ok(Statement::Include { path, span })
    }

    #[allow(clippy::unnecessary_wraps)]
    fn parse_use(&mut self) -> ParseResult<Statement> {
        let (_, span) = self.advance().unwrap(); // consume Use token
        let path = lexer::extract_include_path(self.slice(span)).to_string();
        Ok(Statement::Use { path, span })
    }

    fn parse_assignment(&mut self) -> ParseResult<Statement> {
        let (name, start) = self.expect_identifier()?;
        self.expect(&Token::Assign)?;
        let expr = self.parse_expr()?;
        let end = self.expect(&Token::Semicolon)?;
        Ok(Statement::Assignment {
            name,
            expr,
            span: start.merge(end),
        })
    }

    fn parse_module_def(&mut self) -> ParseResult<Statement> {
        let start = self.expect(&Token::Module)?;
        let (name, _) = self.expect_identifier()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_parameters()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_child_body()?;
        let span = start.merge(
            body.last()
                .map_or_else(|| self.peek_span(), super::ast::Statement::span),
        );
        Ok(Statement::ModuleDefinition {
            name,
            params,
            body,
            span: start.merge(span),
        })
    }

    fn parse_function_def(&mut self) -> ParseResult<Statement> {
        let start = self.expect(&Token::Function)?;
        let (name, _) = self.expect_identifier()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_parameters()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::Assign)?;
        let body = self.parse_expr()?;
        let end = self.expect(&Token::Semicolon)?;
        Ok(Statement::FunctionDefinition {
            name,
            params,
            body,
            span: start.merge(end),
        })
    }

    fn parse_if_else(&mut self) -> ParseResult<Statement> {
        let start = self.expect(&Token::If)?;
        self.expect(&Token::LParen)?;
        let condition = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        let then_body = self.parse_child_body()?;
        let else_body = if self.eat(&Token::Else).is_some() {
            Some(self.parse_child_body()?)
        } else {
            None
        };
        let end_span = else_body
            .as_ref()
            .and_then(|b| b.last().map(super::ast::Statement::span))
            .or_else(|| then_body.last().map(super::ast::Statement::span))
            .unwrap_or(start);
        Ok(Statement::IfElse {
            condition,
            then_body,
            else_body,
            span: start.merge(end_span),
        })
    }

    fn parse_module_instantiation(&mut self) -> ParseResult<Statement> {
        let start_span = self.peek_span();
        let mut modifiers = Modifiers::default();

        // Parse modifier prefixes
        loop {
            match self.peek() {
                Some(Token::Bang) => {
                    self.advance();
                    modifiers.root = true;
                }
                Some(Token::Hash) => {
                    self.advance();
                    modifiers.highlight = true;
                }
                Some(Token::Percent) => {
                    self.advance();
                    modifiers.background = true;
                }
                Some(Token::Star) if self.is_modifier_star() => {
                    self.advance();
                    modifiers.disable = true;
                }
                _ => break,
            }
        }

        // After modifiers, we might have if/else
        if self.peek() == Some(&Token::If) {
            let stmt = self.parse_if_else()?;
            // Wrap with modifiers if any were set
            if modifiers != Modifiers::default()
                && let Statement::IfElse {
                    condition,
                    then_body,
                    else_body,
                    span,
                } = stmt
            {
                return Ok(Statement::IfElse {
                    condition,
                    then_body,
                    else_body,
                    span: start_span.merge(span),
                });
            }
            return Ok(stmt);
        }

        let (name, _) = self.expect_identifier()?;
        self.expect(&Token::LParen)?;
        let args = self.parse_arguments()?;
        self.expect(&Token::RParen)?;

        let children = self.parse_child_body()?;
        let end_span = children
            .last()
            .map_or_else(|| self.peek_span(), super::ast::Statement::span);

        Ok(Statement::ModuleInstantiation {
            name,
            args,
            children,
            modifiers,
            span: start_span.merge(end_span),
        })
    }

    /// Check whether `*` starts a modifier-prefixed statement.
    fn is_modifier_star(&self) -> bool {
        if self.pos + 1 < self.tokens.len() {
            matches!(
                self.tokens[self.pos + 1].0,
                Token::Identifier
                    | Token::For
                    | Token::Let
                    | Token::Assert
                    | Token::Echo
                    | Token::Each
                    | Token::If
                    | Token::Bang
                    | Token::Hash
                    | Token::Percent
                    | Token::Star
            )
        } else {
            false
        }
    }

    /// Parse `;`, a single child statement, or a braced child block.
    fn parse_child_body(&mut self) -> ParseResult<Vec<Statement>> {
        match self.peek() {
            Some(Token::Semicolon) => {
                self.advance();
                Ok(vec![])
            }
            Some(Token::LBrace) => {
                self.advance(); // consume `{`
                let mut body = Vec::new();
                while self.peek() != Some(&Token::RBrace) && !self.at_end() {
                    body.push(self.parse_statement()?);
                }
                self.expect(&Token::RBrace)?;
                Ok(body)
            }
            _ => {
                let stmt = self.parse_statement()?;
                Ok(vec![stmt])
            }
        }
    }

    // ── Parameters & Arguments ───────────────────────────────

    fn parse_parameters(&mut self) -> ParseResult<Vec<Parameter>> {
        let mut params = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            return Ok(params);
        }
        loop {
            params.push(self.parse_parameter()?);
            if self.eat(&Token::Comma).is_none() {
                break;
            }
            // Allow trailing comma
            if self.peek() == Some(&Token::RParen) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_parameter(&mut self) -> ParseResult<Parameter> {
        let (name, span) = self.expect_identifier()?;
        let default = if self.eat(&Token::Assign).is_some() {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let end = default.as_ref().map_or(span, |e| e.span);
        Ok(Parameter {
            name,
            default,
            span: span.merge(end),
        })
    }

    fn parse_arguments(&mut self) -> ParseResult<Vec<Argument>> {
        let mut args = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_argument()?);
            if self.eat(&Token::Comma).is_none() {
                break;
            }
            // Allow trailing comma
            if self.peek() == Some(&Token::RParen) {
                break;
            }
        }
        Ok(args)
    }

    fn parse_argument(&mut self) -> ParseResult<Argument> {
        // Try named argument: `name = expr`
        if matches!(self.peek(), Some(Token::Identifier)) && self.is_named_arg_ahead() {
            let (name, start) = self.expect_identifier()?;
            self.expect(&Token::Assign)?;
            let value = self.parse_expr()?;
            let span = start.merge(value.span);
            return Ok(Argument {
                name: Some(name),
                value,
                span,
            });
        }
        let value = self.parse_expr()?;
        let span = value.span;
        Ok(Argument {
            name: None,
            value,
            span,
        })
    }

    fn is_named_arg_ahead(&self) -> bool {
        if self.pos + 1 < self.tokens.len() {
            matches!(self.tokens[self.pos + 1].0, Token::Assign)
                && (self.pos + 2 >= self.tokens.len()
                    || !matches!(self.tokens[self.pos + 2].0, Token::Assign))
        } else {
            false
        }
    }

    // ── Expressions (precedence climbing) ────────────────────

    fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.enter_recursion()?;
        let result = self.parse_expr_inner();
        self.leave_recursion();
        result
    }

    fn parse_expr_inner(&mut self) -> ParseResult<Expr> {
        // Check for special expression forms
        match self.peek() {
            Some(Token::Function) => return self.parse_anonymous_function(),
            Some(Token::Let) => return self.parse_let_expr(),
            Some(Token::Assert) => return self.parse_assert_expr(),
            Some(Token::Echo) => return self.parse_echo_expr(),
            _ => {}
        }

        let mut expr = self.parse_ternary()?;

        // Check for ternary
        if self.peek() == Some(&Token::Question) {
            self.advance();
            let then_expr = self.parse_expr()?;
            self.expect(&Token::Colon)?;
            let else_expr = self.parse_expr()?;
            let span = expr.span.merge(else_expr.span);
            expr = Expr::new(
                ExprKind::Ternary {
                    condition: Box::new(expr),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                },
                span,
            );
        }

        Ok(expr)
    }

    fn parse_anonymous_function(&mut self) -> ParseResult<Expr> {
        let start = self.expect(&Token::Function)?;
        self.expect(&Token::LParen)?;
        let params = self.parse_parameters()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_expr()?;
        let span = start.merge(body.span);
        Ok(Expr::new(
            ExprKind::AnonymousFunction {
                params,
                body: Box::new(body),
            },
            span,
        ))
    }

    fn parse_let_expr(&mut self) -> ParseResult<Expr> {
        let start = self.expect(&Token::Let)?;
        self.expect(&Token::LParen)?;
        let assignments = self.parse_arguments()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_expr()?;
        let span = start.merge(body.span);
        Ok(Expr::new(
            ExprKind::Let {
                assignments,
                body: Box::new(body),
            },
            span,
        ))
    }

    fn parse_assert_expr(&mut self) -> ParseResult<Expr> {
        let start = self.expect(&Token::Assert)?;
        self.expect(&Token::LParen)?;
        let args = self.parse_arguments()?;
        self.expect(&Token::RParen)?;
        let body = if !self.at_end()
            && !matches!(
                self.peek(),
                Some(Token::Semicolon | Token::RParen | Token::RBracket | Token::Comma)
            ) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let end = body.as_ref().map_or(start, |b| b.span);
        Ok(Expr::new(ExprKind::Assert { args, body }, start.merge(end)))
    }

    fn parse_echo_expr(&mut self) -> ParseResult<Expr> {
        let start = self.expect(&Token::Echo)?;
        self.expect(&Token::LParen)?;
        let args = self.parse_arguments()?;
        self.expect(&Token::RParen)?;
        let body = if !self.at_end()
            && !matches!(
                self.peek(),
                Some(Token::Semicolon | Token::RParen | Token::RBracket | Token::Comma)
            ) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let end = body.as_ref().map_or(start, |b| b.span);
        Ok(Expr::new(ExprKind::Echo { args, body }, start.merge(end)))
    }

    // Precedence levels (lowest to highest):
    // 1. ternary (handled in parse_expr)
    // 2. logical or
    // 3. logical and
    // 4. equality
    // 5. comparison
    // 6. bitwise or
    // 7. bitwise and
    // 8. shift
    // 9. addition
    // 10. multiplication
    // 11. exponent
    // 12. unary
    // 13. postfix (call, index, member)
    // 14. primary

    fn parse_ternary(&mut self) -> ParseResult<Expr> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_logical_and()?;
        while self.peek() == Some(&Token::Or) {
            self.advance();
            let right = self.parse_logical_and()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op: BinaryOp::LogicalOr,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_equality()?;
        while self.peek() == Some(&Token::And) {
            self.advance();
            let right = self.parse_equality()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op: BinaryOp::LogicalAnd,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Some(Token::EqualEqual) => BinaryOp::Equal,
                Some(Token::NotEqual) => BinaryOp::NotEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_bitwise_or()?;
        loop {
            let op = match self.peek() {
                Some(Token::Less) => BinaryOp::Less,
                Some(Token::LessEqual) => BinaryOp::LessEqual,
                Some(Token::Greater) => BinaryOp::Greater,
                Some(Token::GreaterEqual) => BinaryOp::GreaterEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_bitwise_or()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_bitwise_and()?;
        while self.peek() == Some(&Token::Pipe) {
            self.advance();
            let right = self.parse_bitwise_and()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op: BinaryOp::BitwiseOr,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_shift()?;
        while self.peek() == Some(&Token::Ampersand) {
            self.advance();
            let right = self.parse_shift()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op: BinaryOp::BitwiseAnd,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_addition()?;
        loop {
            let op = match self.peek() {
                Some(Token::ShiftLeft) => BinaryOp::ShiftLeft,
                Some(Token::ShiftRight) => BinaryOp::ShiftRight,
                _ => break,
            };
            self.advance();
            let right = self.parse_addition()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_addition(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_multiplication()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinaryOp::Add,
                Some(Token::Minus) => BinaryOp::Subtract,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinaryOp::Multiply,
                Some(Token::Slash) => BinaryOp::Divide,
                Some(Token::Percent) => BinaryOp::Modulo,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        match self.peek() {
            Some(Token::Minus) => {
                let start = self.advance().unwrap().1;
                let operand = self.parse_unary()?;
                // Optimize: fold negative number literals
                if let ExprKind::Number(n) = operand.kind {
                    return Ok(Expr::new(ExprKind::Number(-n), start.merge(operand.span)));
                }
                let span = start.merge(operand.span);
                Ok(Expr::new(
                    ExprKind::UnaryOp {
                        op: UnaryOp::Negate,
                        operand: Box::new(operand),
                    },
                    span,
                ))
            }
            Some(Token::Plus) => {
                self.advance();
                self.parse_unary()
            }
            Some(Token::Bang) => {
                let start = self.advance().unwrap().1;
                let operand = self.parse_unary()?;
                let span = start.merge(operand.span);
                Ok(Expr::new(
                    ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        operand: Box::new(operand),
                    },
                    span,
                ))
            }
            Some(Token::Tilde) => {
                let start = self.advance().unwrap().1;
                let operand = self.parse_unary()?;
                let span = start.merge(operand.span);
                Ok(Expr::new(
                    ExprKind::UnaryOp {
                        op: UnaryOp::BinaryNot,
                        operand: Box::new(operand),
                    },
                    span,
                ))
            }
            _ => self.parse_exponent(),
        }
    }

    fn parse_exponent(&mut self) -> ParseResult<Expr> {
        let left = self.parse_postfix()?;
        if self.peek() == Some(&Token::Caret) {
            self.advance();
            // Right-associative: recurse into unary (not exponent)
            let right = self.parse_unary()?;
            let span = left.span.merge(right.span);
            Ok(Expr::new(
                ExprKind::BinaryOp {
                    op: BinaryOp::Exponent,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            ))
        } else {
            Ok(left)
        }
    }

    fn parse_postfix(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Token::LParen) => {
                    self.advance();
                    let args = self.parse_arguments()?;
                    let end = self.expect(&Token::RParen)?;
                    let span = expr.span.merge(end);
                    expr = Expr::new(
                        ExprKind::FunctionCall {
                            callee: Box::new(expr),
                            args,
                        },
                        span,
                    );
                }
                Some(Token::LBracket) => {
                    self.advance();
                    let index = self.parse_expr()?;
                    let end = self.expect(&Token::RBracket)?;
                    let span = expr.span.merge(end);
                    expr = Expr::new(
                        ExprKind::Index {
                            object: Box::new(expr),
                            index: Box::new(index),
                        },
                        span,
                    );
                }
                Some(Token::Dot) => {
                    self.advance();
                    let (member, end) = self.expect_identifier()?;
                    let span = expr.span.merge(end);
                    expr = Expr::new(
                        ExprKind::MemberAccess {
                            object: Box::new(expr),
                            member,
                        },
                        span,
                    );
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        match self.peek() {
            Some(Token::Number(_)) => {
                let (tok, span) = self.advance().unwrap();
                if let Token::Number(n) = tok {
                    Ok(Expr::new(ExprKind::Number(n), span))
                } else {
                    unreachable!()
                }
            }
            Some(Token::String(_)) => {
                let (tok, span) = self.advance().unwrap();
                if let Token::String(s) = tok {
                    Ok(Expr::new(ExprKind::String(s), span))
                } else {
                    unreachable!()
                }
            }
            Some(Token::True) => {
                let span = self.advance().unwrap().1;
                Ok(Expr::new(ExprKind::BoolTrue, span))
            }
            Some(Token::False) => {
                let span = self.advance().unwrap().1;
                Ok(Expr::new(ExprKind::BoolFalse, span))
            }
            Some(Token::Undef) => {
                let span = self.advance().unwrap().1;
                Ok(Expr::new(ExprKind::Undef, span))
            }
            Some(Token::Identifier) => {
                let (_, span) = self.advance().unwrap();
                let name = self.slice(span).to_string();
                Ok(Expr::new(ExprKind::Identifier(name), span))
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(Token::LBracket) => self.parse_vector_or_range(),
            Some(_) => {
                let (tok, span) = self.tokens[self.pos].clone();
                Err(ParseError::unexpected_token(
                    &tok.to_string(),
                    "expression",
                    span,
                ))
            }
            None => Err(ParseError::unexpected_eof("expression", self.source.len())),
        }
    }

    fn parse_vector_or_range(&mut self) -> ParseResult<Expr> {
        let start = self.expect(&Token::LBracket)?;

        // Empty vector
        if self.peek() == Some(&Token::RBracket) {
            let end = self.advance().unwrap().1;
            return Ok(Expr::new(ExprKind::Vector(vec![]), start.merge(end)));
        }

        // Check for list comprehension elements at start of vector
        if matches!(
            self.peek(),
            Some(Token::For | Token::Let | Token::If | Token::Each)
        ) {
            let lc = self.parse_list_comprehension_element()?;
            let mut elements = vec![lc];
            while self.eat(&Token::Comma).is_some() {
                if self.peek() == Some(&Token::RBracket) {
                    break;
                }
                if matches!(
                    self.peek(),
                    Some(Token::For | Token::Let | Token::If | Token::Each)
                ) {
                    elements.push(self.parse_list_comprehension_element()?);
                } else {
                    elements.push(self.parse_expr()?);
                }
            }
            let end = self.expect(&Token::RBracket)?;
            return Ok(Expr::new(ExprKind::Vector(elements), start.merge(end)));
        }

        // Parse first expression
        let first = self.parse_expr()?;

        // Check for range syntax: [start : end] or [start : step : end]
        if self.peek() == Some(&Token::Colon) {
            self.advance();
            let second = self.parse_expr()?;
            if self.peek() == Some(&Token::Colon) {
                // [start : step : end]
                self.advance();
                let third = self.parse_expr()?;
                let end = self.expect(&Token::RBracket)?;
                return Ok(Expr::new(
                    ExprKind::Range {
                        start: Box::new(first),
                        step: Some(Box::new(second)),
                        end: Box::new(third),
                    },
                    start.merge(end),
                ));
            }
            // [start : end]
            let end = self.expect(&Token::RBracket)?;
            return Ok(Expr::new(
                ExprKind::Range {
                    start: Box::new(first),
                    step: None,
                    end: Box::new(second),
                },
                start.merge(end),
            ));
        }

        // Vector: collect remaining elements
        let mut elements = vec![first];
        while self.eat(&Token::Comma).is_some() {
            // Trailing comma
            if self.peek() == Some(&Token::RBracket) {
                break;
            }
            // Check for list comprehension inside vector
            if matches!(
                self.peek(),
                Some(Token::For | Token::Let | Token::If | Token::Each)
            ) {
                elements.push(self.parse_list_comprehension_element()?);
            } else {
                elements.push(self.parse_expr()?);
            }
        }
        let end = self.expect(&Token::RBracket)?;
        Ok(Expr::new(ExprKind::Vector(elements), start.merge(end)))
    }

    fn parse_list_comprehension_element(&mut self) -> ParseResult<Expr> {
        match self.peek() {
            Some(Token::For) => {
                let start = self.advance().unwrap().1;
                self.expect(&Token::LParen)?;
                let args = self.parse_arguments()?;

                // Check for C-style for: `for (init ; cond ; update)`
                if self.peek() == Some(&Token::Semicolon) {
                    self.advance();
                    let condition = self.parse_expr()?;
                    self.expect(&Token::Semicolon)?;
                    let update = self.parse_arguments()?;
                    self.expect(&Token::RParen)?;
                    let body = self.parse_lc_body()?;
                    let span = start.merge(body.span);
                    return Ok(Expr::new(
                        ExprKind::LcForC {
                            init: args,
                            condition: Box::new(condition),
                            update,
                            body: Box::new(body),
                        },
                        span,
                    ));
                }

                self.expect(&Token::RParen)?;
                let body = self.parse_lc_body()?;
                let span = start.merge(body.span);
                Ok(Expr::new(
                    ExprKind::LcFor {
                        assignments: args,
                        body: Box::new(body),
                    },
                    span,
                ))
            }
            Some(Token::Let) => {
                let start = self.advance().unwrap().1;
                self.expect(&Token::LParen)?;
                let assignments = self.parse_arguments()?;
                self.expect(&Token::RParen)?;
                let body = self.parse_lc_body()?;
                let span = start.merge(body.span);
                Ok(Expr::new(
                    ExprKind::LcLet {
                        assignments,
                        body: Box::new(body),
                    },
                    span,
                ))
            }
            Some(Token::If) => {
                let start = self.advance().unwrap().1;
                self.expect(&Token::LParen)?;
                let condition = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                let then_expr = self.parse_lc_body()?;
                let else_expr = if self.eat(&Token::Else).is_some() {
                    Some(Box::new(self.parse_lc_body()?))
                } else {
                    None
                };
                let end = else_expr.as_ref().map_or(then_expr.span, |e| e.span);
                Ok(Expr::new(
                    ExprKind::LcIf {
                        condition: Box::new(condition),
                        then_expr: Box::new(then_expr),
                        else_expr,
                    },
                    start.merge(end),
                ))
            }
            Some(Token::Each) => {
                let start = self.advance().unwrap().1;
                let body = self.parse_lc_body()?;
                let span = start.merge(body.span);
                Ok(Expr::new(
                    ExprKind::LcEach {
                        body: Box::new(body),
                    },
                    span,
                ))
            }
            _ => self.parse_expr(),
        }
    }

    /// Parse a nested list-comprehension element or its result expression.
    fn parse_lc_body(&mut self) -> ParseResult<Expr> {
        if matches!(
            self.peek(),
            Some(Token::For | Token::Let | Token::If | Token::Each)
        ) {
            self.parse_list_comprehension_element()
        } else if self.peek() == Some(&Token::LParen)
            && matches!(
                self.peek_at(1),
                Some(Token::For | Token::Let | Token::If | Token::Each)
            )
        {
            // Parenthesized list comprehension: ( for(...) ... )
            self.advance();
            let inner = self.parse_list_comprehension_element()?;
            self.expect(&Token::RParen)?;
            Ok(inner)
        } else {
            self.parse_expr()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(source: &str) -> SourceFile {
        parse(source).unwrap_or_else(|e| panic!("parse error for `{source}`: {e}"))
    }

    fn parse_err(source: &str) -> ParseError {
        parse(source).unwrap_err()
    }

    #[test]
    fn test_empty() {
        let file = parse_ok("");
        assert!(file.statements.is_empty());
    }

    #[test]
    fn test_assignment() {
        let file = parse_ok("x = 42;");
        assert_eq!(file.statements.len(), 1);
        match &file.statements[0] {
            Statement::Assignment { name, expr, .. } => {
                assert_eq!(name, "x");
                assert!(matches!(
                    &expr.kind,
                    ExprKind::Number(n) if n == &hyperreal::Real::from(42_u8)
                ));
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_module_instantiation() {
        let file = parse_ok("cube(10);");
        assert_eq!(file.statements.len(), 1);
        match &file.statements[0] {
            Statement::ModuleInstantiation { name, args, .. } => {
                assert_eq!(name, "cube");
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected module instantiation, got {other:?}"),
        }
    }

    #[test]
    fn test_module_with_children() {
        let file = parse_ok("translate([1,2,3]) { cube(5); sphere(3); }");
        match &file.statements[0] {
            Statement::ModuleInstantiation { name, children, .. } => {
                assert_eq!(name, "translate");
                assert_eq!(children.len(), 2);
            }
            other => panic!("expected module instantiation, got {other:?}"),
        }
    }

    #[test]
    fn test_module_definition() {
        let file = parse_ok("module box(size = 10, h) { cube(size); }");
        match &file.statements[0] {
            Statement::ModuleDefinition {
                name, params, body, ..
            } => {
                assert_eq!(name, "box");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].name, "size");
                assert!(params[0].default.is_some());
                assert_eq!(params[1].name, "h");
                assert!(params[1].default.is_none());
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected module def, got {other:?}"),
        }
    }

    #[test]
    fn test_function_definition() {
        let file = parse_ok("function add(a, b) = a + b;");
        match &file.statements[0] {
            Statement::FunctionDefinition {
                name, params, body, ..
            } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
                assert!(matches!(
                    body.kind,
                    ExprKind::BinaryOp {
                        op: BinaryOp::Add,
                        ..
                    }
                ));
            }
            other => panic!("expected function def, got {other:?}"),
        }
    }

    #[test]
    fn test_if_else() {
        let file = parse_ok("if (x > 0) cube(x); else sphere(1);");
        match &file.statements[0] {
            Statement::IfElse {
                then_body,
                else_body,
                ..
            } => {
                assert_eq!(then_body.len(), 1);
                assert!(else_body.is_some());
                assert_eq!(else_body.as_ref().unwrap().len(), 1);
            }
            other => panic!("expected if/else, got {other:?}"),
        }
    }

    #[test]
    fn test_modifiers() {
        let file = parse_ok("!#cube(10);");
        match &file.statements[0] {
            Statement::ModuleInstantiation { modifiers, .. } => {
                assert!(modifiers.root);
                assert!(modifiers.highlight);
                assert!(!modifiers.background);
                assert!(!modifiers.disable);
            }
            other => panic!("expected module instantiation, got {other:?}"),
        }
    }

    #[test]
    fn test_vector() {
        let file = parse_ok("x = [1, 2, 3];");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                assert!(matches!(expr.kind, ExprKind::Vector(ref v) if v.len() == 3));
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_range() {
        let file = parse_ok("x = [0:10];");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                assert!(matches!(expr.kind, ExprKind::Range { step: None, .. }));
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_range_with_step() {
        let file = parse_ok("x = [0:2:10];");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                assert!(matches!(expr.kind, ExprKind::Range { step: Some(_), .. }));
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_include_use() {
        let file = parse_ok("include <lib/base.scad>\nuse <utils.scad>");
        assert_eq!(file.statements.len(), 2);
        match &file.statements[0] {
            Statement::Include { path, .. } => assert_eq!(path, "lib/base.scad"),
            other => panic!("expected include, got {other:?}"),
        }
        match &file.statements[1] {
            Statement::Use { path, .. } => assert_eq!(path, "utils.scad"),
            other => panic!("expected use, got {other:?}"),
        }
    }

    #[test]
    fn test_operator_precedence() {
        let file = parse_ok("x = 1 + 2 * 3;");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                // Multiplication binds more tightly than addition.
                match &expr.kind {
                    ExprKind::BinaryOp {
                        op: BinaryOp::Add,
                        right,
                        ..
                    } => {
                        assert!(matches!(
                            right.kind,
                            ExprKind::BinaryOp {
                                op: BinaryOp::Multiply,
                                ..
                            }
                        ));
                    }
                    other => panic!("expected Add, got {other:?}"),
                }
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_ternary() {
        let file = parse_ok("x = a ? b : c;");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                assert!(matches!(expr.kind, ExprKind::Ternary { .. }));
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_anonymous_function() {
        let file = parse_ok("f = function(x) x * 2;");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                assert!(matches!(expr.kind, ExprKind::AnonymousFunction { .. }));
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_list_comprehension() {
        let file = parse_ok("x = [for (i = [0:10]) i * 2];");
        assert_eq!(file.statements.len(), 1);
    }

    #[test]
    fn test_nested_modules() {
        let file = parse_ok("rotate([0, 0, 45]) translate([10, 0, 0]) cube(5);");
        assert_eq!(file.statements.len(), 1);
        match &file.statements[0] {
            Statement::ModuleInstantiation { name, children, .. } => {
                assert_eq!(name, "rotate");
                assert_eq!(children.len(), 1);
            }
            other => panic!("expected module instantiation, got {other:?}"),
        }
    }

    #[test]
    fn test_complex_program() {
        let source = r"
            // A parametric box
            module rounded_box(size = [10, 10, 10], r = 1) {
                if (r > 0) {
                    translate([r, r, 0])
                        cube(size - [2*r, 2*r, 0]);
                } else {
                    cube(size);
                }
            }
            
            function area(w, h) = w * h;
            
            x = area(10, 20);
            rounded_box(size = [x, 30, 5], r = 2);
        ";
        let file = parse_ok(source);
        assert_eq!(file.statements.len(), 4);
    }

    #[test]
    fn test_error_missing_semicolon() {
        let err = parse_err("x = 42");
        assert!(matches!(err, ParseError::UnexpectedEof { .. }));
    }

    #[test]
    fn test_named_arguments() {
        let file = parse_ok("cube(size = 10, center = true);");
        match &file.statements[0] {
            Statement::ModuleInstantiation { args, .. } => {
                assert_eq!(args.len(), 2);
                assert_eq!(args[0].name.as_deref(), Some("size"));
                assert_eq!(args[1].name.as_deref(), Some("center"));
            }
            other => panic!("expected module instantiation, got {other:?}"),
        }
    }

    #[test]
    fn test_member_access() {
        let file = parse_ok("x = v.x;");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                assert!(
                    matches!(expr.kind, ExprKind::MemberAccess { ref member, .. } if member == "x")
                );
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_index_access() {
        let file = parse_ok("x = v[0];");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                assert!(matches!(expr.kind, ExprKind::Index { .. }));
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn test_let_expression() {
        let file = parse_ok("x = let(a = 1, b = 2) a + b;");
        match &file.statements[0] {
            Statement::Assignment { expr, .. } => {
                assert!(matches!(expr.kind, ExprKind::Let { .. }));
            }
            other => panic!("expected assignment, got {other:?}"),
        }
    }
}
