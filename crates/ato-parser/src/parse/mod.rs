//! Parser implementation for the Ato language.

mod expr;
mod stmt;

use ato_lexer::{Lexer, Span, Token, TokenKind};
use crate::ast::*;
use crate::error::{ParseError, ParseResult};


/// The Ato parser.
pub struct Parser<'a> {
    #[allow(dead_code)]
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given source code.
    pub fn new(source: &'a str) -> Self {
        let lexer = Lexer::new(source);
        let tokens: Vec<Token> = lexer.collect();
        Self {
            source,
            tokens,
            pos: 0,
        }
    }

    /// Parse the entire file.
    pub fn parse_file(&mut self) -> ParseResult<File> {
        let start_span = self.current_span();
        let mut statements = Vec::new();

        while !self.is_at_end() {
            // Skip newlines at file level
            if self.check(TokenKind::Newline) {
                self.advance();
                continue;
            }

            if self.check(TokenKind::Eof) {
                break;
            }

            let stmt = self.parse_statement()?;
            statements.push(stmt);
        }

        let end_span = self.previous_span();
        Ok(File {
            statements,
            span: start_span.merge(&end_span),
        })
    }

    // === Token Navigation ===

    /// Get the current token.
    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens.last().expect("token stream should have at least EOF")
        })
    }

    /// Get the current token kind.
    fn current_kind(&self) -> TokenKind {
        self.current().kind
    }

    /// Get the current span.
    fn current_span(&self) -> Span {
        self.current().span
    }

    /// Get the previous token's span.
    fn previous_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            self.current_span()
        }
    }

    /// Get the previous token.
    fn previous(&self) -> &Token {
        if self.pos > 0 {
            &self.tokens[self.pos - 1]
        } else {
            self.current()
        }
    }

    /// Check if we're at the end of the token stream.
    fn is_at_end(&self) -> bool {
        self.current_kind() == TokenKind::Eof
    }

    /// Check if the current token matches the given kind.
    fn check(&self, kind: TokenKind) -> bool {
        self.current_kind() == kind
    }

    /// Check if the current token matches any of the given kinds.
    fn check_any(&self, kinds: &[TokenKind]) -> bool {
        kinds.contains(&self.current_kind())
    }

    /// Advance to the next token and return the previous one.
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        self.previous()
    }

    /// Consume a token of the expected kind, or return an error.
    fn expect(&mut self, kind: TokenKind) -> ParseResult<&Token> {
        if self.check(kind) {
            Ok(self.advance())
        } else if self.is_at_end() {
            Err(ParseError::unexpected_eof(
                format!("{:?}", kind),
                self.current_span(),
            ))
        } else {
            Err(ParseError::unexpected_token(
                format!("{:?}", kind),
                self.current_kind(),
                self.current_span(),
            ))
        }
    }

    /// Try to consume a token of the expected kind, returning true if successful.
    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Look ahead at the next token without consuming.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos + 1)
    }

    /// Look ahead at the next token's kind.
    #[allow(dead_code)]
    fn peek_kind(&self) -> Option<TokenKind> {
        self.peek().map(|t| t.kind)
    }

    // === Helpers ===

    /// Parse an identifier (NAME token or contextual keyword).
    fn parse_identifier(&mut self) -> ParseResult<Identifier> {
        // Many reserved keywords can be used as identifiers in certain contexts
        let is_contextual_keyword = matches!(
            self.current_kind(),
            TokenKind::Int
                | TokenKind::Float
                | TokenKind::StringKw
                | TokenKind::Str
                | TokenKind::Bytes
                | TokenKind::Parameter
                | TokenKind::Param
                | TokenKind::Test
                | TokenKind::Require
                | TokenKind::Requires
                | TokenKind::Check
                | TokenKind::Report
                | TokenKind::Ensure
        );

        if self.check(TokenKind::Name) || is_contextual_keyword {
            let token = self.advance();
            Ok(Identifier {
                name: token.text.clone(),
                span: token.span,
            })
        } else {
            Err(ParseError::expected("identifier", self.current_span()))
        }
    }

    /// Parse a type reference (Name.Name.Name).
    fn parse_type_reference(&mut self) -> ParseResult<TypeRef> {
        let start_span = self.current_span();
        let mut parts = vec![self.parse_identifier()?];

        while self.match_token(TokenKind::Dot) {
            parts.push(self.parse_identifier()?);
        }

        let end_span = self.previous_span();
        Ok(TypeRef {
            parts,
            span: start_span.merge(&end_span),
        })
    }

    /// Parse a field reference (name.name[index].name).
    fn parse_field_reference(&mut self) -> ParseResult<FieldRef> {
        let start_span = self.current_span();
        let mut parts = vec![self.parse_field_reference_part()?];

        while self.check(TokenKind::Dot) {
            // Check if this is a pin reference (number after dot)
            if let Some(next) = self.peek() {
                if next.kind == TokenKind::Number {
                    self.advance(); // consume dot
                    let num_token = self.advance();
                    let pin_ref = Some(NumberLiteral {
                        value: num_token.text.clone(),
                        span: num_token.span,
                    });
                    let end_span = self.previous_span();
                    return Ok(FieldRef {
                        parts,
                        pin_ref,
                        span: start_span.merge(&end_span),
                    });
                }
            }
            self.advance(); // consume dot
            parts.push(self.parse_field_reference_part()?);
        }

        let end_span = self.previous_span();
        Ok(FieldRef {
            parts,
            pin_ref: None,
            span: start_span.merge(&end_span),
        })
    }

    /// Parse a single part of a field reference.
    fn parse_field_reference_part(&mut self) -> ParseResult<FieldRefPart> {
        let start_span = self.current_span();
        let name = self.parse_identifier()?;

        let index = if self.match_token(TokenKind::OpenBracket) {
            let num_token = self.expect(TokenKind::Number)?;
            let num = NumberLiteral {
                value: num_token.text.clone(),
                span: num_token.span,
            };
            self.expect(TokenKind::CloseBracket)?;
            Some(num)
        } else {
            None
        };

        let end_span = self.previous_span();
        Ok(FieldRefPart {
            name,
            index,
            span: start_span.merge(&end_span),
        })
    }

    /// Parse a string literal.
    fn parse_string_literal(&mut self) -> ParseResult<StringLiteral> {
        let token = self.expect(TokenKind::String)?;
        Ok(StringLiteral {
            value: token.text.clone(),
            span: token.span,
        })
    }

    /// Parse a number literal.
    fn parse_number_literal(&mut self) -> ParseResult<NumberLiteral> {
        let token = self.expect(TokenKind::Number)?;
        Ok(NumberLiteral {
            value: token.text.clone(),
            span: token.span,
        })
    }

    /// Parse a boolean literal.
    fn parse_bool_literal(&mut self) -> ParseResult<BoolLiteral> {
        let span = self.current_span();
        let value = if self.match_token(TokenKind::True) {
            true
        } else if self.match_token(TokenKind::False) {
            false
        } else {
            return Err(ParseError::expected("boolean", span));
        };
        Ok(BoolLiteral {
            value,
            span,
        })
    }

    /// Parse a template (<arg=value, ...>).
    fn parse_template(&mut self) -> ParseResult<Template> {
        let start_span = self.current_span();
        self.expect(TokenKind::LessThan)?;

        let mut args = Vec::new();
        if !self.check(TokenKind::GreaterThan) {
            args.push(self.parse_template_arg()?);
            while self.match_token(TokenKind::Comma) {
                if self.check(TokenKind::GreaterThan) {
                    break; // trailing comma
                }
                args.push(self.parse_template_arg()?);
            }
        }

        self.expect(TokenKind::GreaterThan)?;
        let end_span = self.previous_span();

        Ok(Template {
            args,
            span: start_span.merge(&end_span),
        })
    }

    /// Parse a single template argument.
    fn parse_template_arg(&mut self) -> ParseResult<TemplateArg> {
        let start_span = self.current_span();
        let name = self.parse_identifier()?;
        self.expect(TokenKind::Assign)?;
        let value = self.parse_literal()?;
        let end_span = self.previous_span();

        Ok(TemplateArg {
            name,
            value,
            span: start_span.merge(&end_span),
        })
    }

    /// Parse a literal (number, string, or boolean).
    fn parse_literal(&mut self) -> ParseResult<Literal> {
        match self.current_kind() {
            TokenKind::Number => {
                let num = self.parse_number_literal()?;
                Ok(Literal::Number(num))
            }
            TokenKind::String => {
                let s = self.parse_string_literal()?;
                Ok(Literal::String(s))
            }
            TokenKind::True | TokenKind::False => {
                let b = self.parse_bool_literal()?;
                Ok(Literal::Bool(b))
            }
            _ => Err(ParseError::expected("literal", self.current_span())),
        }
    }

    /// Parse a slice ([start:stop:step]).
    fn parse_slice(&mut self) -> ParseResult<Slice> {
        let start_span = self.current_span();
        self.expect(TokenKind::OpenBracket)?;

        let mut start = None;
        let mut stop = None;
        let mut step = None;

        // Handle [::step] case
        if self.match_token(TokenKind::DoubleColon) {
            if self.check(TokenKind::Number) {
                step = Some(self.parse_number_literal()?);
            }
        } else {
            // Handle [start:stop:step] case
            if self.check(TokenKind::Number) {
                start = Some(self.parse_number_literal()?);
            }

            if self.match_token(TokenKind::Colon) {
                if self.check(TokenKind::Number) {
                    stop = Some(self.parse_number_literal()?);
                }

                if self.match_token(TokenKind::Colon) {
                    if self.check(TokenKind::Number) {
                        step = Some(self.parse_number_literal()?);
                    }
                }
            }
        }

        self.expect(TokenKind::CloseBracket)?;
        let end_span = self.previous_span();

        Ok(Slice {
            start,
            stop,
            step,
            span: start_span.merge(&end_span),
        })
    }
}
