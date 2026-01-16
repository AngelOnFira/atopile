//! Error types and ariadne-based error formatting for the chumsky parser.

use ato_lexer::{Span, Token};
use ariadne::{Color, Label, Report, ReportKind, Source};
use chumsky::error::Simple;
use std::fmt;
use std::ops::Range;

/// A parse error with context for display.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// The span where the error occurred.
    pub span: Span,
    /// The error message.
    pub message: String,
    /// What was expected at this position.
    pub expected: Vec<String>,
    /// What was found instead.
    pub found: Option<String>,
    /// Optional help text.
    pub help: Option<String>,
}

impl ParseError {
    /// Create a ParseError from a chumsky Simple error.
    pub fn from_simple(error: Simple<Token>, _source: &str) -> Self {
        let error_span = error.span();
        let span = Span::new(error_span.start, error_span.end, 1, 1);

        let expected: Vec<String> = error
            .expected()
            .filter_map(|e| e.as_ref().map(|t| format!("{:?}", t.kind)))
            .collect();

        let found = error.found().map(|t| format!("{:?}", t.kind));

        let message = match error.reason() {
            chumsky::error::SimpleReason::Unexpected => {
                if expected.is_empty() {
                    "unexpected token".to_string()
                } else if expected.len() == 1 {
                    format!("expected {}", expected[0])
                } else {
                    format!("expected one of: {}", expected.join(", "))
                }
            }
            chumsky::error::SimpleReason::Unclosed { span: _, delimiter } => {
                format!("unclosed delimiter {:?}", delimiter.kind)
            }
            chumsky::error::SimpleReason::Custom(msg) => msg.to_string(),
        };

        ParseError {
            span,
            message,
            expected,
            found,
            help: None,
        }
    }

    /// Add help text to this error.
    #[allow(dead_code)]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(found) = &self.found {
            write!(f, ", found {}", found)?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}

/// Format a list of parse errors using ariadne for beautiful output.
pub fn format_errors(errors: &[ParseError], source: &str, filename: &str) -> String {
    let mut output = Vec::new();

    for error in errors {
        let report = Report::<(&str, Range<usize>)>::build(
            ReportKind::Error,
            filename,
            error.span.start,
        )
        .with_message(&error.message)
        .with_label(
            Label::new((filename, error.span.start..error.span.end))
                .with_message(if let Some(found) = &error.found {
                    format!("found {} here", found)
                } else {
                    error.message.clone()
                })
                .with_color(Color::Red),
        );

        let report = if let Some(help) = &error.help {
            report.with_help(help)
        } else if !error.expected.is_empty() {
            report.with_help(format!("expected: {}", error.expected.join(", ")))
        } else {
            report
        };

        let report = report.finish();

        // Write to the output buffer
        let mut buf = Vec::new();
        report
            .write((filename, Source::from(source)), &mut buf)
            .unwrap();

        output.extend(buf);
    }

    String::from_utf8(output).unwrap_or_else(|_| "Error formatting failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = ParseError {
            span: Span::new(0, 5, 1, 1),
            message: "expected identifier".to_string(),
            expected: vec!["NAME".to_string()],
            found: Some("Number".to_string()),
            help: None,
        };

        let display = format!("{}", error);
        assert!(display.contains("expected identifier"));
        assert!(display.contains("Number"));
    }

    #[test]
    fn test_format_errors() {
        let errors = vec![ParseError {
            span: Span::new(0, 5, 1, 1),
            message: "unexpected token".to_string(),
            expected: vec!["module".to_string()],
            found: Some("Number".to_string()),
            help: None,
        }];

        let output = format_errors(&errors, "12345\n", "test.ato");
        assert!(output.contains("unexpected token"));
    }
}
