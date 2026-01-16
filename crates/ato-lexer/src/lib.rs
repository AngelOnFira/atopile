//! Lexer for the Ato hardware description language.
//!
//! This lexer tokenizes Ato source code, handling Python-style indentation
//! with INDENT and DEDENT tokens.
//!
//! # Example
//!
//! ```
//! use ato_lexer::Lexer;
//!
//! let source = r#"
//! module MyModule:
//!     pin p1
//!     signal sig
//! "#;
//!
//! let lexer = Lexer::new(source);
//! for token in lexer {
//!     println!("{:?}", token);
//! }
//! ```

mod token;

pub use token::{Span, Token, TokenKind};

use logos::Logos;
use thiserror::Error;

/// Errors that can occur during lexing.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum LexerError {
    #[error("unexpected character at line {line}, column {column}: '{char}'")]
    UnexpectedCharacter { char: char, line: usize, column: usize },

    #[error("inconsistent use of tabs and spaces in indentation at line {line}")]
    InconsistentIndentation { line: usize },

    #[error("inconsistent dedent at line {line}")]
    InconsistentDedent { line: usize },

    #[error("first statement indented at line {line}")]
    FirstStatementIndented { line: usize },

    #[error("unterminated string literal starting at line {line}")]
    UnterminatedString { line: usize },
}

/// The Ato lexer.
///
/// Handles tokenization including Python-style INDENT/DEDENT generation.
pub struct Lexer<'a> {
    source: &'a str,
    inner: logos::Lexer<'a, TokenKind>,

    /// Stack of indentation levels (in spaces).
    indent_stack: Vec<usize>,

    /// Pending tokens to emit.
    pending: Vec<Token>,

    /// Number of opened brackets (for implicit line joining).
    opened_brackets: usize,

    /// Current line number (1-indexed).
    line: usize,

    /// Column at start of current line.
    line_start_offset: usize,

    /// Whether we've seen the first statement yet.
    seen_first_statement: bool,

    /// Whether we've finished lexing.
    finished: bool,

    /// Track indentation style.
    was_space_indent: bool,
    was_tab_indent: bool,

    /// Accumulated errors.
    errors: Vec<LexerError>,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source code.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            inner: TokenKind::lexer(source),
            indent_stack: vec![0],
            pending: Vec::new(),
            opened_brackets: 0,
            line: 1,
            line_start_offset: 0,
            seen_first_statement: false,
            finished: false,
            was_space_indent: false,
            was_tab_indent: false,
            errors: Vec::new(),
        }
    }

    /// Get any accumulated errors.
    pub fn errors(&self) -> &[LexerError] {
        &self.errors
    }

    /// Take the errors, leaving an empty list.
    pub fn take_errors(&mut self) -> Vec<LexerError> {
        std::mem::take(&mut self.errors)
    }

    /// Calculate column from byte offset.
    fn column(&self, offset: usize) -> usize {
        offset - self.line_start_offset + 1
    }

    /// Create a span for the given byte range.
    fn make_span(&self, start: usize, end: usize) -> Span {
        Span::new(start, end, self.line, self.column(start))
    }

    /// Create a token.
    fn make_token(&self, kind: TokenKind, start: usize, end: usize, text: &str) -> Token {
        Token {
            kind,
            span: self.make_span(start, end),
            text: text.to_string(),
        }
    }

    /// Calculate indentation length, handling tabs.
    fn indentation_length(&mut self, text: &str) -> Option<usize> {
        const TAB_LENGTH: usize = 8;
        let mut length = 0;

        for ch in text.chars() {
            match ch {
                ' ' => {
                    self.was_space_indent = true;
                    length += 1;
                }
                '\t' => {
                    self.was_tab_indent = true;
                    length += TAB_LENGTH - (length % TAB_LENGTH);
                }
                '\x0C' => {
                    // Form feed resets indentation
                    length = 0;
                }
                _ => break,
            }
        }

        // Check for mixed tabs and spaces
        if self.was_tab_indent && self.was_space_indent {
            None
        } else {
            Some(length)
        }
    }

    /// Insert INDENT or DEDENT tokens based on indentation change.
    fn insert_indent_or_dedent(&mut self, indent_length: usize, next_span: Span) {
        let prev_indent = *self.indent_stack.last().unwrap();

        if indent_length > prev_indent {
            // INDENT
            self.indent_stack.push(indent_length);
            self.pending.push(Token {
                kind: TokenKind::Indent,
                span: next_span,
                text: "<INDENT>".to_string(),
            });
        } else {
            // DEDENT(s)
            while indent_length < *self.indent_stack.last().unwrap() {
                self.indent_stack.pop();
                let current_indent = *self.indent_stack.last().unwrap();

                if indent_length <= current_indent {
                    self.pending.push(Token {
                        kind: TokenKind::Dedent,
                        span: next_span,
                        text: "<DEDENT>".to_string(),
                    });
                } else {
                    self.errors
                        .push(LexerError::InconsistentDedent { line: self.line });
                    break;
                }
            }
        }
    }

    /// Process a NEWLINE token, potentially generating INDENT/DEDENT.
    fn handle_newline(&mut self, newline_token: Token) {
        if self.opened_brackets > 0 {
            // Inside brackets, newlines are hidden (implicit line joining)
            return;
        }

        // Look ahead for whitespace and next token
        let after_newline_offset = newline_token.span.end;

        // Count leading whitespace after newline
        let remaining = &self.source[after_newline_offset..];
        let ws_len = remaining
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t' || *c == '\x0C')
            .count();

        let ws_text = &remaining[..ws_len];

        // Peek at what comes after whitespace
        let after_ws = &remaining[ws_len..];
        let next_char = after_ws.chars().next();

        // Check if this is a blank line or comment line
        let is_blank_or_comment = match next_char {
            None => true,
            Some('\n') | Some('\r') => true,
            Some('#') => {
                // Check if it's a pragma or regular comment
                !after_ws.starts_with("#pragma")
            }
            _ => false,
        };

        if is_blank_or_comment {
            // Hide the newline before blank lines and comments
            return;
        }

        // Emit the newline
        self.pending.push(newline_token);

        // Calculate indentation
        let indent_length = if let Some(len) = self.indentation_length(ws_text) {
            len
        } else {
            self.errors.push(LexerError::InconsistentIndentation {
                line: self.line + 1,
            });
            return;
        };

        // Generate INDENT/DEDENT based on indentation change
        let next_span = self.make_span(after_newline_offset + ws_len, after_newline_offset + ws_len);
        self.insert_indent_or_dedent(indent_length, next_span);
    }

    /// Get the next raw token from logos.
    fn next_raw(&mut self) -> Option<(TokenKind, std::ops::Range<usize>)> {
        let result = self.inner.next()?;
        let span = self.inner.span();
        match result {
            Ok(kind) => Some((kind, span)),
            Err(()) => Some((TokenKind::Error, span)),
        }
    }

    /// Advance to the next token, handling INDENT/DEDENT generation.
    fn advance(&mut self) -> Option<Token> {
        // Return pending tokens first
        if !self.pending.is_empty() {
            return Some(self.pending.remove(0));
        }

        if self.finished {
            return None;
        }

        loop {
            let (kind, span) = match self.next_raw() {
                Some(t) => t,
                None => {
                    // EOF - emit trailing NEWLINE and DEDENTs
                    self.finished = true;

                    if self.seen_first_statement {
                        // Add trailing newline if needed
                        let eof_span = Span::new(self.source.len(), self.source.len(), self.line, 1);

                        // Emit trailing newline
                        self.pending.push(Token {
                            kind: TokenKind::Newline,
                            span: eof_span,
                            text: "".to_string(),
                        });

                        // Emit remaining DEDENTs
                        while self.indent_stack.len() > 1 {
                            self.indent_stack.pop();
                            self.pending.push(Token {
                                kind: TokenKind::Dedent,
                                span: eof_span,
                                text: "<DEDENT>".to_string(),
                            });
                        }
                    }

                    // Finally emit EOF
                    self.pending.push(Token {
                        kind: TokenKind::Eof,
                        span: Span::new(self.source.len(), self.source.len(), self.line, 1),
                        text: "".to_string(),
                    });

                    return Some(self.pending.remove(0));
                }
            };

            let text = &self.source[span.clone()];
            let token = self.make_token(kind, span.start, span.end, text);

            match kind {
                TokenKind::Newline => {
                    // Update line tracking
                    self.line += 1;
                    self.line_start_offset = span.end;

                    if !self.seen_first_statement {
                        // Hide leading newlines
                        continue;
                    }

                    self.handle_newline(token);

                    // Return first pending token if any
                    if !self.pending.is_empty() {
                        return Some(self.pending.remove(0));
                    }
                    continue;
                }

                TokenKind::Comment | TokenKind::ExplicitLineJoining => {
                    // Skip comments and line continuations
                    if kind == TokenKind::ExplicitLineJoining {
                        self.line += 1;
                        self.line_start_offset = span.end;
                    }
                    continue;
                }

                TokenKind::Whitespace => {
                    // Skip whitespace (we handle it specially for indentation)
                    continue;
                }

                TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::OpenBrace => {
                    self.opened_brackets += 1;
                    self.seen_first_statement = true;
                    return Some(token);
                }

                TokenKind::CloseParen | TokenKind::CloseBracket | TokenKind::CloseBrace => {
                    if self.opened_brackets > 0 {
                        self.opened_brackets -= 1;
                    }
                    self.seen_first_statement = true;
                    return Some(token);
                }

                TokenKind::Error => {
                    let ch = text.chars().next().unwrap_or('?');
                    self.errors.push(LexerError::UnexpectedCharacter {
                        char: ch,
                        line: self.line,
                        column: self.column(span.start),
                    });
                    return Some(token);
                }

                _ => {
                    // Check for first statement indentation
                    if !self.seen_first_statement {
                        self.seen_first_statement = true;

                        // Check if there was whitespace before first statement
                        if span.start > 0 {
                            let leading = &self.source[..span.start];
                            // Find last newline
                            if let Some(nl_pos) = leading.rfind('\n') {
                                let after_nl = &leading[nl_pos + 1..];
                                if !after_nl.is_empty()
                                    && after_nl.chars().all(|c| c == ' ' || c == '\t')
                                {
                                    if let Some(indent_len) = self.indentation_length(after_nl) {
                                        if indent_len > 0 {
                                            self.errors.push(LexerError::FirstStatementIndented {
                                                line: self.line,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }

                    return Some(token);
                }
            }
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.advance()
    }
}

/// Convenience function to lex source code into a vector of tokens.
pub fn lex(source: &str) -> (Vec<Token>, Vec<LexerError>) {
    let mut lexer = Lexer::new(source);
    let tokens: Vec<Token> = lexer.by_ref().collect();
    let errors = lexer.take_errors();
    (tokens, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokens() {
        let source = "module MyModule:";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty());
        assert_eq!(tokens.len(), 5); // module, MyModule, :, NEWLINE, EOF

        assert_eq!(tokens[0].kind, TokenKind::Module);
        assert_eq!(tokens[1].kind, TokenKind::Name);
        assert_eq!(tokens[1].text, "MyModule");
        assert_eq!(tokens[2].kind, TokenKind::Colon);
    }

    #[test]
    fn test_indentation() {
        let source = "module M:\n    pin p1\n";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty(), "Errors: {:?}", errors);

        // Find INDENT token
        let has_indent = tokens.iter().any(|t| t.kind == TokenKind::Indent);
        assert!(has_indent, "Should have INDENT token. Tokens: {:?}", tokens);
    }

    #[test]
    fn test_dedent() {
        let source = "module M:\n    pin p1\nmodule N:";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty(), "Errors: {:?}", errors);

        // Find DEDENT token
        let has_dedent = tokens.iter().any(|t| t.kind == TokenKind::Dedent);
        assert!(has_dedent, "Should have DEDENT token. Tokens: {:?}", tokens);
    }

    #[test]
    fn test_numbers() {
        let source = "x = 10kohm";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty());

        let num_token = tokens.iter().find(|t| t.kind == TokenKind::Number);
        assert!(num_token.is_some());
        assert_eq!(num_token.unwrap().text, "10kohm");
    }

    #[test]
    fn test_operators() {
        let source = "a ~ b ~> c <~ d";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty());

        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert!(kinds.contains(&TokenKind::Wire));
        assert!(kinds.contains(&TokenKind::Sperm));
        assert!(kinds.contains(&TokenKind::LSperm));
    }

    #[test]
    fn test_string_literal() {
        let source = r#"name = "hello""#;
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty());

        let str_token = tokens.iter().find(|t| t.kind == TokenKind::String);
        assert!(str_token.is_some());
        assert_eq!(str_token.unwrap().text, r#""hello""#);
    }

    #[test]
    fn test_pragma() {
        let source = "#pragma experiment(\"FOR_LOOP\")\nmodule M:";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty());

        let pragma = tokens.iter().find(|t| t.kind == TokenKind::Pragma);
        assert!(pragma.is_some());
    }

    #[test]
    fn test_implicit_line_joining() {
        let source = "x = (\n    1 +\n    2\n)";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty());

        // Should NOT have INDENT/DEDENT inside parens
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        let indent_count = kinds.iter().filter(|k| **k == TokenKind::Indent).count();
        assert_eq!(indent_count, 0, "Should not have INDENT inside parens");
    }

    #[test]
    fn test_plus_or_minus() {
        let source = "x = 5V +/- 10%";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty());

        let has_plus_minus = tokens.iter().any(|t| t.kind == TokenKind::PlusOrMinus);
        assert!(has_plus_minus);
    }

    #[test]
    fn test_arrow() {
        let source = "a -> B";
        let (tokens, errors) = lex(source);

        assert!(errors.is_empty());

        let has_arrow = tokens.iter().any(|t| t.kind == TokenKind::Arrow);
        assert!(has_arrow);
    }

    #[test]
    fn test_line_numbers() {
        let source = "a\nb\nc";
        let (tokens, _) = lex(source);

        assert_eq!(tokens[0].span.line, 1); // a
        assert_eq!(tokens[2].span.line, 2); // b (after newline)
        assert_eq!(tokens[4].span.line, 3); // c
    }
}
