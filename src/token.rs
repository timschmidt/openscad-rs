use hyperreal::Real;

/// Token types for the `OpenSCAD` lexer.
///
/// Matches the full grammar defined in the official `OpenSCAD` `lexer.l` and `parser.y`.
use logos::Logos;

#[allow(clippy::needless_pass_by_ref_mut)] // Required by logos callback signature
fn parse_string(lex: &mut logos::Lexer<'_, Token>) -> String {
    let slice = lex.slice();
    // Strip surrounding quotes
    let inner = &slice[1..slice.len() - 1];
    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') | None => result.push('\\'),
                Some('"') => result.push('"'),
                Some('x') => {
                    let hex: String = chars.by_ref().take(2).collect();
                    if let Ok(val) = u32::from_str_radix(&hex, 16) {
                        if val == 0 {
                            result.push(' ');
                        } else if let Some(ch) = char::from_u32(val) {
                            result.push(ch);
                        }
                    }
                }
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(val) = u32::from_str_radix(&hex, 16)
                        && let Some(ch) = char::from_u32(val)
                    {
                        result.push(ch);
                    }
                }
                Some('U') => {
                    let hex: String = chars.by_ref().take(6).collect();
                    if let Ok(val) = u32::from_str_radix(&hex, 16)
                        && let Some(ch) = char::from_u32(val)
                    {
                        result.push(ch);
                    }
                }
                // Unknown escape — keep backslash + char (OpenSCAD warns but continues)
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[allow(clippy::needless_pass_by_ref_mut)] // Required by logos callback signature
fn parse_number(lex: &mut logos::Lexer<'_, Token>) -> Real {
    fn parse_mantissa(source: &str) -> Real {
        let normalized;
        let source = if source.starts_with('.') {
            normalized = format!("0{source}");
            &normalized
        } else if source.ends_with('.') {
            normalized = format!("{source}0");
            &normalized
        } else {
            source
        };
        source
            .parse()
            .expect("the numeric token regex accepts exact decimal literals")
    }

    let source = lex.slice();
    let Some((mantissa, exponent)) = source.split_once('e').or_else(|| source.split_once('E'))
    else {
        return parse_mantissa(source);
    };

    let mantissa = parse_mantissa(mantissa);
    let exponent = exponent.strip_prefix('+').unwrap_or(exponent);
    let exponent: Real = exponent
        .parse()
        .expect("the numeric token regex accepts integer exponents");
    let scale = Real::from(10_u8)
        .pow(exponent)
        .expect("ten raised to an integer exponent is a real number");
    mantissa * scale
}

#[allow(clippy::needless_pass_by_ref_mut)] // Required by logos callback signature
fn parse_hex(lex: &mut logos::Lexer<'_, Token>) -> Real {
    lex.slice()[2..].bytes().fold(Real::zero(), |value, digit| {
        let digit = match digit {
            b'0'..=b'9' => digit - b'0',
            b'a'..=b'f' => digit - b'a' + 10,
            b'A'..=b'F' => digit - b'A' + 10,
            _ => unreachable!("the hexadecimal token regex accepts only hexadecimal digits"),
        };
        value * Real::from(16_u8) + Real::from(digit)
    })
}

/// All tokens in the `OpenSCAD` language.
// Keeping `Real` inline makes the public token API mirror the exact-number AST
// and avoids a second allocation for every numeric literal.
#[allow(clippy::large_enum_variant)]
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n\x0c]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
pub enum Token {
    // ── Keywords ──────────────────────────────────────────────
    #[token("module")]
    Module,
    #[token("function")]
    Function,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("for")]
    For,
    #[token("let")]
    Let,
    #[token("assert")]
    Assert,
    #[token("echo")]
    Echo,
    #[token("each")]
    Each,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("undef")]
    Undef,

    // ── Include / Use ────────────────────────────────────────
    #[regex(r"include\s*<[^>]*>")]
    Include,
    #[regex(r"use\s*<[^>]*>")]
    Use,

    // ── Literals ─────────────────────────────────────────────
    #[regex(r"0x[0-9a-fA-F]+", parse_hex)]
    #[regex(r"[0-9]+\.?[0-9]*([eE][+-]?[0-9]+)?", parse_number)]
    #[regex(r"\.[0-9]+([eE][+-]?[0-9]+)?", parse_number)]
    Number(Real),

    #[regex(r#""([^"\\]|\\.)*""#, parse_string)]
    String(String),

    // ── Identifiers ──────────────────────────────────────────
    #[regex(r"[a-zA-Z_$][a-zA-Z0-9_]*")]
    #[regex(r"[0-9]+[a-zA-Z_][a-zA-Z0-9_]*", priority = 1)]
    Identifier,

    // ── Multi-char operators ─────────────────────────────────
    #[token("<=")]
    LessEqual,
    #[token(">=")]
    GreaterEqual,
    #[token("==")]
    EqualEqual,
    #[token("!=")]
    NotEqual,
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("<<")]
    ShiftLeft,
    #[token(">>")]
    ShiftRight,

    // ── Single-char operators & delimiters ────────────────────
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("^")]
    Caret,
    #[token("!")]
    Bang,
    #[token("~")]
    Tilde,
    #[token("<")]
    Less,
    #[token(">")]
    Greater,
    #[token("=")]
    Assign,
    #[token("?")]
    Question,
    #[token(":")]
    Colon,
    #[token(";")]
    Semicolon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("#")]
    Hash,
    #[token("&")]
    Ampersand,
    #[token("|")]
    Pipe,
}

impl Token {
    /// Return whether this token is a reserved `OpenSCAD` keyword.
    #[must_use]
    pub const fn is_keyword(&self) -> bool {
        matches!(
            self,
            Self::Module
                | Self::Function
                | Self::If
                | Self::Else
                | Self::For
                | Self::Let
                | Self::Assert
                | Self::Echo
                | Self::Each
                | Self::True
                | Self::False
                | Self::Undef
        )
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Module => write!(f, "module"),
            Self::Function => write!(f, "function"),
            Self::If => write!(f, "if"),
            Self::Else => write!(f, "else"),
            Self::For => write!(f, "for"),
            Self::Let => write!(f, "let"),
            Self::Assert => write!(f, "assert"),
            Self::Echo => write!(f, "echo"),
            Self::Each => write!(f, "each"),
            Self::True => write!(f, "true"),
            Self::False => write!(f, "false"),
            Self::Undef => write!(f, "undef"),
            Self::Include => write!(f, "include"),
            Self::Use => write!(f, "use"),
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Identifier => write!(f, "identifier"),
            Self::LessEqual => write!(f, "<="),
            Self::GreaterEqual => write!(f, ">="),
            Self::EqualEqual => write!(f, "=="),
            Self::NotEqual => write!(f, "!="),
            Self::And => write!(f, "&&"),
            Self::Or => write!(f, "||"),
            Self::ShiftLeft => write!(f, "<<"),
            Self::ShiftRight => write!(f, ">>"),
            Self::Plus => write!(f, "+"),
            Self::Minus => write!(f, "-"),
            Self::Star => write!(f, "*"),
            Self::Slash => write!(f, "/"),
            Self::Percent => write!(f, "%"),
            Self::Caret => write!(f, "^"),
            Self::Bang => write!(f, "!"),
            Self::Tilde => write!(f, "~"),
            Self::Less => write!(f, "<"),
            Self::Greater => write!(f, ">"),
            Self::Assign => write!(f, "="),
            Self::Question => write!(f, "?"),
            Self::Colon => write!(f, ":"),
            Self::Semicolon => write!(f, ";"),
            Self::Comma => write!(f, ","),
            Self::Dot => write!(f, "."),
            Self::LParen => write!(f, "("),
            Self::RParen => write!(f, ")"),
            Self::LBracket => write!(f, "["),
            Self::RBracket => write!(f, "]"),
            Self::LBrace => write!(f, "{{"),
            Self::RBrace => write!(f, "}}"),
            Self::Hash => write!(f, "#"),
            Self::Ampersand => write!(f, "&"),
            Self::Pipe => write!(f, "|"),
        }
    }
}
