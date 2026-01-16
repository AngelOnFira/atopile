//! Expression parsing for the Ato language.

use ato_lexer::TokenKind;
use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use super::Parser;

impl Parser<'_> {
    /// Parse an arithmetic expression.
    /// arithmetic_expression: arithmetic_expression (OR_OP | AND_OP) sum | sum
    pub fn parse_arithmetic_expression(&mut self) -> ParseResult<Expression> {
        self.parse_bitwise()
    }

    /// Parse bitwise or/and operations.
    fn parse_bitwise(&mut self) -> ParseResult<Expression> {
        let start_span = self.current_span();
        let mut left = self.parse_sum()?;

        while self.check_any(&[TokenKind::OrOp, TokenKind::AndOp]) {
            let op = if self.match_token(TokenKind::OrOp) {
                BinaryOp::BitOr
            } else {
                self.advance();
                BinaryOp::BitAnd
            };

            let right = self.parse_sum()?;
            let end_span = self.previous_span();

            left = Expression::Binary(Box::new(BinaryExpr {
                left,
                operator: op,
                right,
                span: start_span.merge(&end_span),
            }));
        }

        Ok(left)
    }

    /// Parse sum (addition/subtraction).
    /// sum: sum (PLUS | MINUS) term | term
    fn parse_sum(&mut self) -> ParseResult<Expression> {
        let start_span = self.current_span();
        let mut left = self.parse_term()?;

        while self.check_any(&[TokenKind::Plus, TokenKind::Minus]) {
            let op = if self.match_token(TokenKind::Plus) {
                BinaryOp::Add
            } else {
                self.advance();
                BinaryOp::Sub
            };

            let right = self.parse_term()?;
            let end_span = self.previous_span();

            left = Expression::Binary(Box::new(BinaryExpr {
                left,
                operator: op,
                right,
                span: start_span.merge(&end_span),
            }));
        }

        Ok(left)
    }

    /// Parse term (multiplication/division).
    /// term: term (STAR | DIV) power | power
    fn parse_term(&mut self) -> ParseResult<Expression> {
        let start_span = self.current_span();
        let mut left = self.parse_power()?;

        while self.check_any(&[TokenKind::Star, TokenKind::Div]) {
            let op = if self.match_token(TokenKind::Star) {
                BinaryOp::Mul
            } else {
                self.advance();
                BinaryOp::Div
            };

            let right = self.parse_power()?;
            let end_span = self.previous_span();

            left = Expression::Binary(Box::new(BinaryExpr {
                left,
                operator: op,
                right,
                span: start_span.merge(&end_span),
            }));
        }

        Ok(left)
    }

    /// Parse power expression.
    /// power: functional (POWER functional)?
    fn parse_power(&mut self) -> ParseResult<Expression> {
        let start_span = self.current_span();
        let left = self.parse_functional()?;

        if self.match_token(TokenKind::Power) {
            let right = self.parse_functional()?;
            let end_span = self.previous_span();

            Ok(Expression::Binary(Box::new(BinaryExpr {
                left,
                operator: BinaryOp::Power,
                right,
                span: start_span.merge(&end_span),
            })))
        } else {
            Ok(left)
        }
    }

    /// Parse functional expression.
    /// functional: bound | name '(' bound+ ')'
    fn parse_functional(&mut self) -> ParseResult<Expression> {
        let start_span = self.current_span();

        // Check if this is a function call: name(args)
        if self.check(TokenKind::Name) {
            if let Some(next) = self.peek() {
                if next.kind == TokenKind::OpenParen {
                    let name = self.parse_identifier()?;
                    self.expect(TokenKind::OpenParen)?;

                    let mut args = Vec::new();
                    if !self.check(TokenKind::CloseParen) {
                        args.push(self.parse_atom()?);
                        while !self.check(TokenKind::CloseParen) && !self.is_at_end() {
                            // Arguments are space-separated in the grammar, not comma-separated
                            args.push(self.parse_atom()?);
                        }
                    }

                    self.expect(TokenKind::CloseParen)?;
                    let end_span = self.previous_span();

                    return Ok(Expression::FunctionCall(FunctionCall {
                        name,
                        args,
                        span: start_span.merge(&end_span),
                    }));
                }
            }
        }

        self.parse_atom()
    }

    /// Parse an atom (primary expression).
    /// atom: field_reference | literal_physical | arithmetic_group
    fn parse_atom(&mut self) -> ParseResult<Expression> {
        let start_span = self.current_span();

        match self.current_kind() {
            TokenKind::OpenParen => {
                self.advance();
                let inner = self.parse_arithmetic_expression()?;
                self.expect(TokenKind::CloseParen)?;
                Ok(Expression::Group(Box::new(inner)))
            }

            TokenKind::Number => {
                let physical = self.parse_physical_literal()?;
                Ok(Expression::Literal(Literal::Physical(physical)))
            }

            TokenKind::Plus | TokenKind::Minus => {
                let op = if self.match_token(TokenKind::Plus) {
                    UnaryOp::Pos
                } else {
                    self.advance();
                    UnaryOp::Neg
                };

                // After sign, we expect a number
                if self.check(TokenKind::Number) {
                    let physical = self.parse_physical_literal()?;
                    let end_span = self.previous_span();

                    Ok(Expression::Unary(Box::new(UnaryExpr {
                        operator: op,
                        operand: Expression::Literal(Literal::Physical(physical)),
                        span: start_span.merge(&end_span),
                    })))
                } else {
                    let operand = self.parse_atom()?;
                    let end_span = self.previous_span();

                    Ok(Expression::Unary(Box::new(UnaryExpr {
                        operator: op,
                        operand,
                        span: start_span.merge(&end_span),
                    })))
                }
            }

            TokenKind::Name => {
                let field_ref = self.parse_field_reference()?;
                Ok(Expression::FieldRef(field_ref))
            }

            TokenKind::String => {
                let s = self.parse_string_literal()?;
                Ok(Expression::Literal(Literal::String(s)))
            }

            TokenKind::True | TokenKind::False => {
                let b = self.parse_bool_literal()?;
                Ok(Expression::Literal(Literal::Bool(b)))
            }

            _ => Err(ParseError::expected("expression", self.current_span())),
        }
    }

    /// Parse a physical literal (quantity, range, or bilateral).
    /// literal_physical: bound_quantity | bilateral_quantity | quantity
    pub fn parse_physical_literal(&mut self) -> ParseResult<PhysicalLiteral> {
        let start_span = self.current_span();
        let quantity = self.parse_quantity()?;

        // Check for range: quantity TO quantity
        if self.match_token(TokenKind::To) {
            let to_quantity = self.parse_quantity()?;
            let end_span = self.previous_span();

            return Ok(PhysicalLiteral::Range(QuantityRange {
                from: quantity,
                to: to_quantity,
                span: start_span.merge(&end_span),
            }));
        }

        // Check for bilateral: quantity +/- tolerance
        if self.match_token(TokenKind::PlusOrMinus) {
            let tolerance = self.parse_tolerance()?;
            let end_span = self.previous_span();

            return Ok(PhysicalLiteral::Bilateral(BilateralQuantity {
                base: quantity,
                tolerance,
                span: start_span.merge(&end_span),
            }));
        }

        Ok(PhysicalLiteral::Quantity(quantity))
    }

    /// Parse a quantity (number with optional unit).
    fn parse_quantity(&mut self) -> ParseResult<Quantity> {
        let start_span = self.current_span();

        // Handle optional sign
        let has_sign = self.check_any(&[TokenKind::Plus, TokenKind::Minus]);
        let sign = if self.match_token(TokenKind::Minus) {
            "-"
        } else if self.match_token(TokenKind::Plus) {
            ""
        } else {
            ""
        };

        let num_token = self.expect(TokenKind::Number)?;
        let number = NumberLiteral {
            value: if has_sign && sign == "-" {
                format!("-{}", num_token.text)
            } else {
                num_token.text.clone()
            },
            span: num_token.span,
        };

        // Check for unit (NAME immediately after number)
        let unit = if self.check(TokenKind::Name) {
            Some(self.parse_identifier()?)
        } else {
            None
        };

        let end_span = self.previous_span();
        Ok(Quantity {
            number,
            unit,
            span: start_span.merge(&end_span),
        })
    }

    /// Parse a tolerance value.
    fn parse_tolerance(&mut self) -> ParseResult<Tolerance> {
        let start_span = self.current_span();

        let num_token = self.expect(TokenKind::Number)?;
        let value = num_token.text.clone();

        let is_percent = self.match_token(TokenKind::Percent);

        let unit = if !is_percent && self.check(TokenKind::Name) {
            Some(self.parse_identifier()?)
        } else {
            None
        };

        let end_span = self.previous_span();
        Ok(Tolerance {
            value,
            is_percent,
            unit,
            span: start_span.merge(&end_span),
        })
    }

    /// Parse a comparison expression.
    /// comparison: arithmetic_expression compare_op_pair+
    pub fn parse_comparison(&mut self) -> ParseResult<Comparison> {
        let start_span = self.current_span();
        let left = self.parse_arithmetic_expression()?;

        let mut operations = Vec::new();
        while self.check_any(&[
            TokenKind::LessThan,
            TokenKind::GreaterThan,
            TokenKind::LessEq,
            TokenKind::GreaterEq,
            TokenKind::Within,
            TokenKind::Is,
        ]) {
            let op_span = self.current_span();
            let kind = match self.current_kind() {
                TokenKind::LessThan => {
                    self.advance();
                    CompareOpKind::LessThan
                }
                TokenKind::GreaterThan => {
                    self.advance();
                    CompareOpKind::GreaterThan
                }
                TokenKind::LessEq => {
                    self.advance();
                    CompareOpKind::LessEq
                }
                TokenKind::GreaterEq => {
                    self.advance();
                    CompareOpKind::GreaterEq
                }
                TokenKind::Within => {
                    self.advance();
                    CompareOpKind::Within
                }
                TokenKind::Is => {
                    self.advance();
                    CompareOpKind::Is
                }
                _ => unreachable!(),
            };

            let right = self.parse_arithmetic_expression()?;
            let end_span = self.previous_span();

            operations.push(CompareOp {
                kind,
                right,
                span: op_span.merge(&end_span),
            });
        }

        if operations.is_empty() {
            return Err(ParseError::expected("comparison operator", self.current_span()));
        }

        let end_span = self.previous_span();
        Ok(Comparison {
            left,
            operations,
            span: start_span.merge(&end_span),
        })
    }

    /// Parse an assignable value.
    /// assignable: string | new_stmt | literal_physical | arithmetic_expression | boolean_
    pub fn parse_assignable(&mut self) -> ParseResult<Assignable> {
        match self.current_kind() {
            TokenKind::String => {
                let s = self.parse_string_literal()?;
                Ok(Assignable::String(s))
            }

            TokenKind::New => {
                let new_expr = self.parse_new_expression()?;
                Ok(Assignable::New(new_expr))
            }

            TokenKind::True | TokenKind::False => {
                let b = self.parse_bool_literal()?;
                Ok(Assignable::Boolean(b))
            }

            TokenKind::Number | TokenKind::Plus | TokenKind::Minus => {
                // Could be physical literal or arithmetic expression
                // Try physical literal first
                let physical = self.parse_physical_literal()?;
                Ok(Assignable::Physical(physical))
            }

            TokenKind::Name | TokenKind::OpenParen => {
                // Arithmetic expression starting with identifier or group
                let expr = self.parse_arithmetic_expression()?;
                Ok(Assignable::Arithmetic(expr))
            }

            _ => Err(ParseError::expected("assignable value", self.current_span())),
        }
    }

    /// Parse a new expression.
    /// new_stmt: NEW type_reference ('[' new_count ']')? template?
    pub fn parse_new_expression(&mut self) -> ParseResult<NewExpr> {
        let start_span = self.current_span();
        self.expect(TokenKind::New)?;

        let type_ref = self.parse_type_reference()?;

        // Optional array count
        let count = if self.match_token(TokenKind::OpenBracket) {
            let num = self.parse_number_literal()?;
            self.expect(TokenKind::CloseBracket)?;
            Some(num)
        } else {
            None
        };

        // Optional template
        let template = if self.check(TokenKind::LessThan) {
            Some(self.parse_template()?)
        } else {
            None
        };

        let end_span = self.previous_span();
        Ok(NewExpr {
            type_ref,
            count,
            template,
            span: start_span.merge(&end_span),
        })
    }
}
