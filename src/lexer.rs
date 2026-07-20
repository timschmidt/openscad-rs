/// Lexer for `OpenSCAD` source code.
///
/// Wraps the `logos`-generated tokenizer and provides a convenient iterator
/// that yields `(Token, Span)` pairs.
use crate::span::Span;
use crate::token::Token;
use logos::Logos;

/// A spanned token: the token itself plus its byte range in the source.
pub type SpannedToken = (Token, Span);

/// Tokenize an `OpenSCAD` source string into a vector of spanned tokens.
///
/// Invalid bytes are silently skipped (logos default). If you need error
/// reporting on invalid tokens, use [`lex_with_errors`].
#[must_use]
pub fn lex(source: &str) -> Vec<SpannedToken> {
    let lexer = Token::lexer(source);
    lexer
        .spanned()
        .filter_map(|(result, span)| result.ok().map(|tok| (tok, Span::from(span))))
        .collect()
}

/// Tokenize source code, returning errors for invalid tokens.
#[must_use]
pub fn lex_with_errors(source: &str) -> (Vec<SpannedToken>, Vec<Span>) {
    let lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    for (result, span) in lexer.spanned() {
        match result {
            Ok(tok) => tokens.push((tok, Span::from(span))),
            Err(()) => errors.push(Span::from(span)),
        }
    }
    (tokens, errors)
}

/// Extract the path from an `include<...>` or `use<...>` token slice.
#[must_use]
pub fn extract_include_path(slice: &str) -> &str {
    if let Some(start) = slice.find('<')
        && let Some(end) = slice.rfind('>')
    {
        return &slice[start + 1..end];
    }
    slice
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::Token;
    use hyperreal::Real;

    #[test]
    fn test_basic_tokens() {
        let tokens = lex("cube(10);");
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].0, Token::Identifier);
        assert_eq!(tokens[1].0, Token::LParen);
        assert!(matches!(&tokens[2].0, Token::Number(n) if n == &Real::from(10_u8)));
        assert_eq!(tokens[3].0, Token::RParen);
        assert_eq!(tokens[4].0, Token::Semicolon);
    }

    #[test]
    fn test_keywords() {
        let tokens = lex("module function if else for let assert echo each true false undef");
        let expected = vec![
            Token::Module,
            Token::Function,
            Token::If,
            Token::Else,
            Token::For,
            Token::Let,
            Token::Assert,
            Token::Echo,
            Token::Each,
            Token::True,
            Token::False,
            Token::Undef,
        ];
        let actual: Vec<Token> = tokens.into_iter().map(|(t, _)| t).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_numbers() {
        let tokens = lex("42 7.25 .5 1e10 2.5e-3 0xFF 1.e2 .5E+2 0x10000000000000000");
        assert_eq!(tokens.len(), 9);
        let nums: Vec<Real> = tokens
            .iter()
            .map(|(t, _)| match t {
                Token::Number(n) => n.clone(),
                _ => panic!("expected number"),
            })
            .collect();
        assert_eq!(nums[0], Real::from(42_u8));
        assert_eq!(nums[1], "29/4".parse::<Real>().unwrap());
        assert_eq!(nums[2], "1/2".parse::<Real>().unwrap());
        assert_eq!(nums[3], Real::from(10_000_000_000_u64));
        assert_eq!(nums[4], "1/400".parse::<Real>().unwrap());
        assert_eq!(nums[5], Real::from(255_u8));
        assert_eq!(nums[6], Real::from(100_u8));
        assert_eq!(nums[7], Real::from(50_u8));
        assert_eq!(nums[8], Real::from(18_446_744_073_709_551_616_u128));
    }

    #[test]
    fn test_string_escapes() {
        let tokens = lex(r#""hello\nworld""#);
        assert_eq!(tokens.len(), 1);
        match &tokens[0].0 {
            Token::String(s) => assert_eq!(s, "hello\nworld"),
            other => panic!("expected string, got {other:?}"),
        }
    }

    #[test]
    fn test_string_unicode() {
        let tokens = lex(r#""\u03a9""#);
        assert_eq!(tokens.len(), 1);
        match &tokens[0].0 {
            Token::String(s) => assert_eq!(s, "Ω"),
            other => panic!("expected string, got {other:?}"),
        }
    }

    #[test]
    fn test_comments_skipped() {
        let tokens = lex("x = 1; // comment\ny = /* block */ 2;");
        assert_eq!(tokens.len(), 8);
    }

    #[test]
    fn test_operators() {
        let tokens = lex("<= >= == != && || << >>");
        let expected = vec![
            Token::LessEqual,
            Token::GreaterEqual,
            Token::EqualEqual,
            Token::NotEqual,
            Token::And,
            Token::Or,
            Token::ShiftLeft,
            Token::ShiftRight,
        ];
        let actual: Vec<Token> = tokens.into_iter().map(|(t, _)| t).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_include_use() {
        let tokens = lex("include <file.scad>\nuse <lib.scad>");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].0, Token::Include);
        assert_eq!(tokens[1].0, Token::Use);
    }

    #[test]
    fn test_extract_include_path() {
        assert_eq!(
            extract_include_path("include <foo/bar.scad>"),
            "foo/bar.scad"
        );
        assert_eq!(extract_include_path("use <lib.scad>"), "lib.scad");
    }

    #[test]
    fn test_spans_are_correct() {
        let tokens = lex("ab cd");
        assert_eq!(tokens[0].1, Span::new(0, 2));
        assert_eq!(tokens[1].1, Span::new(3, 5));
    }
}
