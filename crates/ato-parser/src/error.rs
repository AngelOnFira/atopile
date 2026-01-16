//! Error types for the Ato parser with miette integration.

use ato_lexer::{Span, TokenKind};
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// A parse error with rich context for display.
#[derive(Debug, Clone, Error, Diagnostic)]
pub enum ParseError {
    #[error("unexpected token")]
    #[diagnostic(code(ato::parse::unexpected_token))]
    UnexpectedToken {
        expected: String,
        found: TokenKind,
        #[label("found {found:?} here")]
        span: SourceSpan,
    },

    #[error("unexpected end of file")]
    #[diagnostic(code(ato::parse::unexpected_eof))]
    UnexpectedEof {
        expected: String,
        #[label("expected {expected}")]
        span: SourceSpan,
    },

    #[error("expected {expected}")]
    #[diagnostic(code(ato::parse::expected))]
    Expected {
        expected: String,
        #[label("expected {expected} here")]
        span: SourceSpan,
    },

    #[error("invalid number literal")]
    #[diagnostic(code(ato::parse::invalid_number))]
    InvalidNumber {
        #[label("invalid number")]
        span: SourceSpan,
    },

    #[error("invalid indentation")]
    #[diagnostic(code(ato::parse::invalid_indentation))]
    InvalidIndentation {
        #[label("expected indented block")]
        span: SourceSpan,
    },

    #[error("mixed connection operators")]
    #[diagnostic(
        code(ato::parse::mixed_connection),
        help("use either ~> or <~ consistently, not both")
    )]
    MixedConnectionOperators {
        #[label("mixed operators in connection chain")]
        span: SourceSpan,
    },

    #[error("empty block")]
    #[diagnostic(
        code(ato::parse::empty_block),
        help("add a 'pass' statement if the block should be empty")
    )]
    EmptyBlock {
        #[label("block has no statements")]
        span: SourceSpan,
    },

    #[error("lexer error: {message}")]
    #[diagnostic(code(ato::lexer::error))]
    LexerError {
        message: String,
        #[label("{message}")]
        span: SourceSpan,
    },
}

impl ParseError {
    /// Create an unexpected token error.
    pub fn unexpected_token(expected: impl Into<String>, found: TokenKind, span: Span) -> Self {
        ParseError::UnexpectedToken {
            expected: expected.into(),
            found,
            span: span_to_source_span(span),
        }
    }

    /// Create an unexpected EOF error.
    pub fn unexpected_eof(expected: impl Into<String>, span: Span) -> Self {
        ParseError::UnexpectedEof {
            expected: expected.into(),
            span: span_to_source_span(span),
        }
    }

    /// Create an expected error.
    pub fn expected(expected: impl Into<String>, span: Span) -> Self {
        ParseError::Expected {
            expected: expected.into(),
            span: span_to_source_span(span),
        }
    }

    /// Create an invalid number error.
    pub fn invalid_number(span: Span) -> Self {
        ParseError::InvalidNumber {
            span: span_to_source_span(span),
        }
    }

    /// Create an invalid indentation error.
    pub fn invalid_indentation(span: Span) -> Self {
        ParseError::InvalidIndentation {
            span: span_to_source_span(span),
        }
    }

    /// Create a mixed connection operators error.
    pub fn mixed_connection_operators(span: Span) -> Self {
        ParseError::MixedConnectionOperators {
            span: span_to_source_span(span),
        }
    }

    /// Create an empty block error.
    pub fn empty_block(span: Span) -> Self {
        ParseError::EmptyBlock {
            span: span_to_source_span(span),
        }
    }

    /// Create a lexer error.
    pub fn lexer_error(message: impl Into<String>, span: Span) -> Self {
        ParseError::LexerError {
            message: message.into(),
            span: span_to_source_span(span),
        }
    }
}

/// Convert our Span to miette's SourceSpan.
fn span_to_source_span(span: Span) -> SourceSpan {
    SourceSpan::new(span.start.into(), (span.end - span.start).into())
}

/// Result type for parsing operations.
pub type ParseResult<T> = Result<T, ParseError>;
