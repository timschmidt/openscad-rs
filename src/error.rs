/// Error types for the `OpenSCAD` parser.
use crate::span::Span;
use miette::Diagnostic;
use thiserror::Error;

/// A parse error with source location.
#[derive(Error, Debug, Diagnostic, Clone)]
pub enum ParseError {
    #[error("unexpected token: found `{found}`, expected {expected}")]
    UnexpectedToken {
        found: String,
        expected: String,
        #[label("here")]
        span: miette::SourceSpan,
    },

    #[error("unexpected end of input, expected {expected}")]
    UnexpectedEof {
        expected: String,
        #[label("here")]
        span: miette::SourceSpan,
    },

    #[error("{message}")]
    Custom {
        message: String,
        #[label("{message}")]
        span: miette::SourceSpan,
    },

    #[error("invalid token")]
    InvalidToken {
        #[label("invalid token")]
        span: miette::SourceSpan,
    },
}

impl ParseError {
    #[must_use]
    pub fn unexpected_token(found: &str, expected: &str, span: Span) -> Self {
        Self::UnexpectedToken {
            found: found.to_string(),
            expected: expected.to_string(),
            span: (span.start, span.len()).into(),
        }
    }

    #[must_use]
    pub fn unexpected_eof(expected: &str, pos: usize) -> Self {
        Self::UnexpectedEof {
            expected: expected.to_string(),
            span: (pos, 0).into(),
        }
    }

    #[must_use]
    pub fn custom(message: &str, span: Span) -> Self {
        Self::Custom {
            message: message.to_string(),
            span: (span.start, span.len()).into(),
        }
    }

    #[must_use]
    pub fn invalid_token(span: Span) -> Self {
        Self::InvalidToken {
            span: (span.start, span.len()).into(),
        }
    }
}

/// Result type alias for parser operations.
pub type ParseResult<T> = Result<T, ParseError>;
