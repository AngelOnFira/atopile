//! Token types for the Ato language lexer.

use logos::Logos;
use serde::{Deserialize, Serialize};

/// A token with its kind, span, and optional text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub text: String,
}

/// Source location span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset of the start of the token.
    pub start: usize,
    /// Byte offset of the end of the token (exclusive).
    pub end: usize,
    /// Line number (1-indexed).
    pub line: usize,
    /// Column number (1-indexed).
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }

    /// Create a span covering from self to other.
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            column: if self.line <= other.line {
                self.column
            } else {
                other.column
            },
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }
    }
}

/// All token kinds in the Ato language.
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenKind {
    // === Keywords ===
    #[token("component")]
    Component,
    #[token("module")]
    Module,
    #[token("interface")]
    Interface,
    #[token("pin")]
    Pin,
    #[token("signal")]
    Signal,
    #[token("new")]
    New,
    #[token("from")]
    From,
    #[token("import")]
    Import,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("assert")]
    Assert,
    #[token("to")]
    To,
    #[token("True")]
    True,
    #[token("False")]
    False,
    #[token("within")]
    Within,
    #[token("is")]
    Is,
    #[token("pass")]
    Pass,
    #[token("trait")]
    Trait,

    // === Reserved keywords (unused but reserved) ===
    #[token("int")]
    Int,
    #[token("float")]
    Float,
    #[token("string")]
    StringKw,
    #[token("str")]
    Str,
    #[token("bytes")]
    Bytes,
    #[token("if")]
    If,
    #[token("parameter")]
    Parameter,
    #[token("param")]
    Param,
    #[token("test")]
    Test,
    #[token("require")]
    Require,
    #[token("requires")]
    Requires,
    #[token("check")]
    Check,
    #[token("report")]
    Report,
    #[token("ensure")]
    Ensure,

    // === Connection operators ===
    #[token("~>")]
    Sperm,
    #[token("<~")]
    LSperm,
    #[token("~")]
    Wire,

    // === Comparison operators ===
    #[token("==")]
    Equals,
    #[token("!=")]
    NotEq,
    #[token("<>")]
    NotEqAlt,
    #[token(">=")]
    GreaterEq,
    #[token("<=")]
    LessEq,
    #[token(">")]
    GreaterThan,
    #[token("<")]
    LessThan,

    // === Assignment operators ===
    #[token("=")]
    Assign,
    #[token("+=")]
    AddAssign,
    #[token("-=")]
    SubAssign,
    #[token("*=")]
    MultAssign,
    #[token("/=")]
    DivAssign,
    #[token("//=")]
    IDivAssign,
    #[token("**=")]
    PowerAssign,
    #[token("@=")]
    AtAssign,
    #[token("&=")]
    AndAssign,
    #[token("|=")]
    OrAssign,
    #[token("^=")]
    XorAssign,
    #[token("<<=")]
    LeftShiftAssign,
    #[token(">>=")]
    RightShiftAssign,

    // === Arithmetic operators ===
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("**")]
    Power,
    #[token("*")]
    Star,
    #[token("//")]
    IDiv,
    #[token("/")]
    Div,
    #[token("%")]
    Percent,

    // === Bitwise operators ===
    #[token("|")]
    OrOp,
    #[token("&")]
    AndOp,
    #[token("^")]
    Xor,
    #[token("<<")]
    LeftShift,
    #[token(">>")]
    RightShift,

    // === Special operators ===
    #[token("->")]
    Arrow,
    #[token("::")]
    DoubleColon,
    #[regex(r"\+/-|\u00B1")]
    PlusOrMinus,

    // === Delimiters ===
    #[token("(")]
    OpenParen,
    #[token(")")]
    CloseParen,
    #[token("[")]
    OpenBracket,
    #[token("]")]
    CloseBracket,
    #[token("{")]
    OpenBrace,
    #[token("}")]
    CloseBrace,
    #[token(":")]
    Colon,
    #[token(";")]
    Semicolon,
    #[token(",")]
    Comma,
    #[token("...")]
    Ellipsis,
    #[token(".")]
    Dot,
    #[token("@")]
    At,

    // === Literals ===
    /// A string literal (single or double quoted, with optional prefix).
    #[regex(r#"[rRuUfF]?[rRfF]?'([^'\\]|\\.)*'"#)]
    #[regex(r#"[rRuUfF]?[rRfF]?"([^"\\]|\\.)*""#)]
    #[regex(r#"[rRuUfF]?[rRfF]?'''([^'\\]|\\.|'[^']|''[^'])*'''"#)]
    #[regex(r#"[rRuUfF]?[rRfF]?"""([^"\\]|\\.|"[^"]|""[^"])*""""#)]
    String,

    /// Bytes literal.
    #[regex(r#"[bB][rR]?'([^'\\]|\\.)*'"#)]
    #[regex(r#"[bB][rR]?"([^"\\]|\\.)*""#)]
    #[regex(r#"[rR][bB]'([^'\\]|\\.)*'"#)]
    #[regex(r#"[rR][bB]"([^"\\]|\\.)*""#)]
    BytesLiteral,

    /// A number (integer or float, with optional unit suffix).
    /// Matches: 123, 3.14, 0xFF, 0b101, 0o77, 1e10, 10kohm, 5V, 100nF
    #[regex(r"[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?[a-zA-Z_]*")]
    #[regex(r"0[xX][0-9a-fA-F]+")]
    #[regex(r"0[bB][01]+")]
    #[regex(r"0[oO][0-7]+")]
    Number,

    /// Imaginary number (e.g., 3j, 1.5J).
    #[regex(r"[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?[jJ]")]
    ImagNumber,

    /// An identifier (name).
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", priority = 1)]
    Name,

    // === Whitespace and structure ===
    /// A newline character.
    #[regex(r"\r?\n")]
    Newline,

    /// Pragma directive (e.g., #pragma experiment("FOR_LOOP")).
    #[regex(r"#pragma[^\r\n]*")]
    Pragma,

    /// A comment (# to end of line) - hidden from token stream.
    #[regex(r"#[^\r\n]*")]
    Comment,

    /// Explicit line joining (backslash followed by newline).
    #[regex(r"\\[ \t]*\r?\n")]
    ExplicitLineJoining,

    /// Whitespace (for internal tracking, not emitted normally).
    #[regex(r"[ \t\f]+")]
    Whitespace,

    // === Synthetic tokens (generated by lexer, not matched) ===
    /// Increase in indentation level.
    Indent,
    /// Decrease in indentation level.
    Dedent,

    /// End of file.
    Eof,

    /// Error token - unrecognized character.
    Error,
}

impl TokenKind {
    /// Returns true if this token is a keyword.
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Component
                | TokenKind::Module
                | TokenKind::Interface
                | TokenKind::Pin
                | TokenKind::Signal
                | TokenKind::New
                | TokenKind::From
                | TokenKind::Import
                | TokenKind::For
                | TokenKind::In
                | TokenKind::Assert
                | TokenKind::To
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Within
                | TokenKind::Is
                | TokenKind::Pass
                | TokenKind::Trait
        )
    }

    /// Returns true if this token is a reserved keyword.
    pub fn is_reserved(&self) -> bool {
        matches!(
            self,
            TokenKind::Int
                | TokenKind::Float
                | TokenKind::StringKw
                | TokenKind::Str
                | TokenKind::Bytes
                | TokenKind::If
                | TokenKind::Parameter
                | TokenKind::Param
                | TokenKind::Test
                | TokenKind::Require
                | TokenKind::Requires
                | TokenKind::Check
                | TokenKind::Report
                | TokenKind::Ensure
        )
    }

    /// Returns true if this is an opening bracket.
    pub fn is_open_bracket(&self) -> bool {
        matches!(
            self,
            TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::OpenBrace
        )
    }

    /// Returns true if this is a closing bracket.
    pub fn is_close_bracket(&self) -> bool {
        matches!(
            self,
            TokenKind::CloseParen | TokenKind::CloseBracket | TokenKind::CloseBrace
        )
    }

    /// Returns true if this token should be hidden (not emitted to token stream).
    pub fn is_hidden(&self) -> bool {
        matches!(
            self,
            TokenKind::Comment | TokenKind::ExplicitLineJoining | TokenKind::Whitespace
        )
    }

    /// Returns the name of this token kind.
    pub fn name(&self) -> &'static str {
        match self {
            TokenKind::Component => "component",
            TokenKind::Module => "module",
            TokenKind::Interface => "interface",
            TokenKind::Pin => "pin",
            TokenKind::Signal => "signal",
            TokenKind::New => "new",
            TokenKind::From => "from",
            TokenKind::Import => "import",
            TokenKind::For => "for",
            TokenKind::In => "in",
            TokenKind::Assert => "assert",
            TokenKind::To => "to",
            TokenKind::True => "True",
            TokenKind::False => "False",
            TokenKind::Within => "within",
            TokenKind::Is => "is",
            TokenKind::Pass => "pass",
            TokenKind::Trait => "trait",
            TokenKind::Int => "int",
            TokenKind::Float => "float",
            TokenKind::StringKw => "string",
            TokenKind::Str => "str",
            TokenKind::Bytes => "bytes",
            TokenKind::If => "if",
            TokenKind::Parameter => "parameter",
            TokenKind::Param => "param",
            TokenKind::Test => "test",
            TokenKind::Require => "require",
            TokenKind::Requires => "requires",
            TokenKind::Check => "check",
            TokenKind::Report => "report",
            TokenKind::Ensure => "ensure",
            TokenKind::Sperm => "~>",
            TokenKind::LSperm => "<~",
            TokenKind::Wire => "~",
            TokenKind::Equals => "==",
            TokenKind::NotEq => "!=",
            TokenKind::NotEqAlt => "<>",
            TokenKind::GreaterEq => ">=",
            TokenKind::LessEq => "<=",
            TokenKind::GreaterThan => ">",
            TokenKind::LessThan => "<",
            TokenKind::Assign => "=",
            TokenKind::AddAssign => "+=",
            TokenKind::SubAssign => "-=",
            TokenKind::MultAssign => "*=",
            TokenKind::DivAssign => "/=",
            TokenKind::IDivAssign => "//=",
            TokenKind::PowerAssign => "**=",
            TokenKind::AtAssign => "@=",
            TokenKind::AndAssign => "&=",
            TokenKind::OrAssign => "|=",
            TokenKind::XorAssign => "^=",
            TokenKind::LeftShiftAssign => "<<=",
            TokenKind::RightShiftAssign => ">>=",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Power => "**",
            TokenKind::Star => "*",
            TokenKind::IDiv => "//",
            TokenKind::Div => "/",
            TokenKind::Percent => "%",
            TokenKind::OrOp => "|",
            TokenKind::AndOp => "&",
            TokenKind::Xor => "^",
            TokenKind::LeftShift => "<<",
            TokenKind::RightShift => ">>",
            TokenKind::Arrow => "->",
            TokenKind::DoubleColon => "::",
            TokenKind::PlusOrMinus => "+/-",
            TokenKind::OpenParen => "(",
            TokenKind::CloseParen => ")",
            TokenKind::OpenBracket => "[",
            TokenKind::CloseBracket => "]",
            TokenKind::OpenBrace => "{",
            TokenKind::CloseBrace => "}",
            TokenKind::Colon => ":",
            TokenKind::Semicolon => ";",
            TokenKind::Comma => ",",
            TokenKind::Ellipsis => "...",
            TokenKind::Dot => ".",
            TokenKind::At => "@",
            TokenKind::String => "STRING",
            TokenKind::BytesLiteral => "BYTES",
            TokenKind::Number => "NUMBER",
            TokenKind::ImagNumber => "IMAG_NUMBER",
            TokenKind::Name => "NAME",
            TokenKind::Newline => "NEWLINE",
            TokenKind::Pragma => "PRAGMA",
            TokenKind::Comment => "COMMENT",
            TokenKind::ExplicitLineJoining => "LINE_JOINING",
            TokenKind::Whitespace => "WS",
            TokenKind::Indent => "INDENT",
            TokenKind::Dedent => "DEDENT",
            TokenKind::Eof => "EOF",
            TokenKind::Error => "ERROR",
        }
    }
}
