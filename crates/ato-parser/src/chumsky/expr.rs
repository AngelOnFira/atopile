//! Expression parsing for the Ato language using chumsky 0.9.
//!
//! This module implements expression parsing with proper operator precedence
//! using manual precedence climbing (chumsky 0.9 doesn't have pratt()).

use crate::ast::*;
use ato_lexer::{Span, Token, TokenKind};
use chumsky::prelude::*;
use std::ops::Range;

/// Convert a Range<usize> to our Span type.
fn to_span(range: Range<usize>) -> Span {
    Span::new(range.start, range.end, 1, 1)
}

/// Helper to create a token matcher.
pub fn tok(kind: TokenKind) -> impl Parser<Token, Token, Error = Simple<Token>> + Clone {
    filter(move |t: &Token| t.kind == kind)
}

/// Parse an identifier.
/// Some reserved keywords can be used as identifiers in certain contexts (e.g., template arguments).
pub fn identifier() -> impl Parser<Token, Identifier, Error = Simple<Token>> + Clone {
    filter(|t: &Token| {
        matches!(
            t.kind,
            TokenKind::Name
                | TokenKind::Int
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
        )
    })
    .map_with_span(|t: Token, span: Range<usize>| Identifier {
        name: t.text.clone(),
        span: to_span(span),
    })
}

/// Parse a number literal.
pub fn number_literal() -> impl Parser<Token, NumberLiteral, Error = Simple<Token>> + Clone {
    tok(TokenKind::Number).map_with_span(|t: Token, span: Range<usize>| NumberLiteral {
        value: t.text.clone(),
        span: to_span(span),
    })
}

/// Parse a string literal.
pub fn string_literal() -> impl Parser<Token, StringLiteral, Error = Simple<Token>> + Clone {
    tok(TokenKind::String).map_with_span(|t: Token, span: Range<usize>| StringLiteral {
        value: t.text.clone(),
        span: to_span(span),
    })
}

/// Parse a boolean literal.
pub fn bool_literal() -> impl Parser<Token, BoolLiteral, Error = Simple<Token>> + Clone {
    choice((
        tok(TokenKind::True).map_with_span(|_, span: Range<usize>| BoolLiteral {
            value: true,
            span: to_span(span),
        }),
        tok(TokenKind::False).map_with_span(|_, span: Range<usize>| BoolLiteral {
            value: false,
            span: to_span(span),
        }),
    ))
}

/// Parse an array index [n].
fn array_index() -> impl Parser<Token, NumberLiteral, Error = Simple<Token>> + Clone {
    tok(TokenKind::OpenBracket)
        .ignore_then(number_literal())
        .then_ignore(tok(TokenKind::CloseBracket))
}

/// Parse a field reference part (name with optional index).
fn field_ref_part() -> impl Parser<Token, FieldRefPart, Error = Simple<Token>> + Clone {
    identifier()
        .then(array_index().or_not())
        .map_with_span(|(name, index), span: Range<usize>| FieldRefPart {
            name,
            index,
            span: to_span(span),
        })
}

/// Parse a field reference (a.b.c[0].d).
pub fn field_reference() -> impl Parser<Token, FieldRef, Error = Simple<Token>> + Clone {
    field_ref_part()
        .separated_by(tok(TokenKind::Dot))
        .at_least(1)
        .then(
            // Optional trailing pin reference (.1)
            tok(TokenKind::Dot)
                .ignore_then(number_literal())
                .or_not(),
        )
        .map_with_span(|(parts, pin_ref), span: Range<usize>| FieldRef {
            parts,
            pin_ref,
            span: to_span(span),
        })
}

/// Parse a type reference (Name.Name.Name).
pub fn type_reference() -> impl Parser<Token, TypeRef, Error = Simple<Token>> + Clone {
    identifier()
        .separated_by(tok(TokenKind::Dot))
        .at_least(1)
        .map_with_span(|parts, span: Range<usize>| TypeRef {
            parts,
            span: to_span(span),
        })
}

/// Split a number token text into numeric and unit parts.
/// E.g., "10kohm" -> ("10", Some("kohm")), "42" -> ("42", None)
fn split_number_unit(text: &str) -> (&str, Option<&str>) {
    // Find where the unit suffix starts (first alphabetic character after digits/dots/exponent)
    let mut unit_start = text.len();
    let mut seen_e = false;
    let mut in_exponent = false;

    for (i, c) in text.char_indices() {
        if c == 'e' || c == 'E' {
            seen_e = true;
            in_exponent = true;
        } else if in_exponent && (c == '+' || c == '-') {
            // Part of exponent
        } else if in_exponent && c.is_ascii_digit() {
            // Part of exponent
        } else if c.is_alphabetic() || c == '_' {
            // This is the start of the unit suffix (but not 'e'/'E' for exponent)
            if !seen_e || (seen_e && i > 0 && !text[..i].ends_with('e') && !text[..i].ends_with('E')) {
                unit_start = i;
                break;
            }
            // If we just saw 'e'/'E', reset and continue
            if c != 'e' && c != 'E' {
                unit_start = i;
                break;
            }
        } else if c.is_ascii_digit() || c == '.' {
            in_exponent = false;
        }
    }

    if unit_start == text.len() {
        (text, None)
    } else {
        (&text[..unit_start], Some(&text[unit_start..]))
    }
}

/// Parse a quantity (number with optional unit).
/// The lexer tokenizes "10kohm" as a single Number token, so we split it here.
pub fn quantity() -> impl Parser<Token, Quantity, Error = Simple<Token>> + Clone {
    // Optional sign
    let sign = choice((tok(TokenKind::Minus).to("-"), tok(TokenKind::Plus).to("")))
        .or_not()
        .map(|s| s.unwrap_or(""));

    sign.then(number_literal())
        .map_with_span(|(sign, num), span: Range<usize>| {
            let (num_part, unit_part) = split_number_unit(&num.value);

            let number_value = if sign == "-" {
                format!("-{}", num_part)
            } else {
                num_part.to_string()
            };

            let unit = unit_part.map(|u| Identifier {
                name: u.to_string(),
                span: num.span, // Approximate span
            });

            Quantity {
                number: NumberLiteral {
                    value: number_value,
                    span: num.span,
                },
                unit,
                span: to_span(span),
            }
        })
}

/// Parse a tolerance value.
/// Handles cases like "5%", "5", "5mA" where the unit might be in the number token.
fn tolerance() -> impl Parser<Token, Tolerance, Error = Simple<Token>> + Clone {
    number_literal()
        .then(tok(TokenKind::Percent).or_not())
        .map_with_span(|(num, is_percent), span: Range<usize>| {
            let (num_part, unit_part) = split_number_unit(&num.value);

            let unit = unit_part.map(|u| Identifier {
                name: u.to_string(),
                span: num.span,
            });

            Tolerance {
                value: num_part.to_string(),
                is_percent: is_percent.is_some(),
                unit,
                span: to_span(span),
            }
        })
}

/// Parse a physical literal (quantity, range, or bilateral).
pub fn physical_literal() -> impl Parser<Token, PhysicalLiteral, Error = Simple<Token>> + Clone {
    quantity()
        .then(
            choice((
                // Range: quantity TO quantity
                tok(TokenKind::To)
                    .ignore_then(quantity())
                    .map(PhysicalSuffix::Range),
                // Bilateral: quantity +/- tolerance
                tok(TokenKind::PlusOrMinus)
                    .ignore_then(tolerance())
                    .map(PhysicalSuffix::Bilateral),
            ))
            .or_not(),
        )
        .map_with_span(|(base, suffix), span: Range<usize>| match suffix {
            Some(PhysicalSuffix::Range(to)) => PhysicalLiteral::Range(QuantityRange {
                from: base,
                to,
                span: to_span(span),
            }),
            Some(PhysicalSuffix::Bilateral(tol)) => PhysicalLiteral::Bilateral(BilateralQuantity {
                base,
                tolerance: tol,
                span: to_span(span),
            }),
            None => PhysicalLiteral::Quantity(base),
        })
}

/// Helper enum for physical literal parsing.
enum PhysicalSuffix {
    Range(Quantity),
    Bilateral(Tolerance),
}

/// Parse a literal.
pub fn literal() -> impl Parser<Token, Literal, Error = Simple<Token>> + Clone {
    choice((
        physical_literal().map(Literal::Physical),
        string_literal().map(Literal::String),
        bool_literal().map(Literal::Bool),
    ))
}

/// Parse a primary expression (atom).
/// Note: The grouped expression case needs to accept any arithmetic expression,
/// not just atoms. We achieve this by making arithmetic_expression recursive.
fn atom(
    full_expr: impl Parser<Token, Expression, Error = Simple<Token>> + Clone,
) -> impl Parser<Token, Expression, Error = Simple<Token>> + Clone {
    choice((
        // Grouped expression - accepts full arithmetic expressions
        full_expr
            .delimited_by(tok(TokenKind::OpenParen), tok(TokenKind::CloseParen))
            .map(|e| Expression::Group(Box::new(e))),
        // Physical literal (must come before field_reference to handle numbers)
        physical_literal().map(|p| Expression::Literal(Literal::Physical(p))),
        // Field reference
        field_reference().map(Expression::FieldRef),
        // String literal
        string_literal().map(|s| Expression::Literal(Literal::String(s))),
        // Boolean literal
        bool_literal().map(|b| Expression::Literal(Literal::Bool(b))),
    ))
}

/// Parse an arithmetic expression with proper precedence.
/// This is the main entry point for expression parsing with full recursion support.
pub fn arithmetic_expression() -> impl Parser<Token, Expression, Error = Simple<Token>> + Clone {
    recursive(|full_expr| {
        // Power expression (** is right-associative)
        let power = recursive(|power| {
            atom(full_expr.clone())
                .then(tok(TokenKind::Power).ignore_then(power).or_not())
                .map_with_span(|(base, exp), span: Range<usize>| match exp {
                    Some(e) => Expression::Binary(Box::new(BinaryExpr {
                        left: base,
                        operator: BinaryOp::Power,
                        right: e,
                        span: to_span(span),
                    })),
                    None => base,
                })
        });

        // Term (* and /)
        let term = power
            .clone()
            .then(
                choice((
                    tok(TokenKind::Star).to(BinaryOp::Mul),
                    tok(TokenKind::Div).to(BinaryOp::Div),
                ))
                .then(power)
                .repeated(),
            )
            .foldl(|left, (op, right)| {
                let span = left.span().merge(&right.span());
                Expression::Binary(Box::new(BinaryExpr {
                    left,
                    operator: op,
                    right,
                    span,
                }))
            });

        // Sum (+ and -)
        let sum = term
            .clone()
            .then(
                choice((
                    tok(TokenKind::Plus).to(BinaryOp::Add),
                    tok(TokenKind::Minus).to(BinaryOp::Sub),
                ))
                .then(term)
                .repeated(),
            )
            .foldl(|left, (op, right)| {
                let span = left.span().merge(&right.span());
                Expression::Binary(Box::new(BinaryExpr {
                    left,
                    operator: op,
                    right,
                    span,
                }))
            });

        // Bitwise operations (| and &, lowest precedence)
        sum.clone()
            .then(
                choice((
                    tok(TokenKind::OrOp).to(BinaryOp::BitOr),
                    tok(TokenKind::AndOp).to(BinaryOp::BitAnd),
                ))
                .then(sum)
                .repeated(),
            )
            .foldl(|left, (op, right)| {
                let span = left.span().merge(&right.span());
                Expression::Binary(Box::new(BinaryExpr {
                    left,
                    operator: op,
                    right,
                    span,
                }))
            })
    })
}

/// Parse a comparison operator.
fn compare_op() -> impl Parser<Token, CompareOpKind, Error = Simple<Token>> + Clone {
    choice((
        tok(TokenKind::LessEq).to(CompareOpKind::LessEq),
        tok(TokenKind::GreaterEq).to(CompareOpKind::GreaterEq),
        tok(TokenKind::LessThan).to(CompareOpKind::LessThan),
        tok(TokenKind::GreaterThan).to(CompareOpKind::GreaterThan),
        tok(TokenKind::Within).to(CompareOpKind::Within),
        tok(TokenKind::Is).to(CompareOpKind::Is),
    ))
}

/// Parse a comparison expression.
pub fn comparison() -> impl Parser<Token, Comparison, Error = Simple<Token>> + Clone {
    arithmetic_expression()
        .then(
            compare_op()
                .then(arithmetic_expression())
                .map_with_span(|(kind, right), span: Range<usize>| CompareOp {
                    kind,
                    right,
                    span: to_span(span),
                })
                .repeated()
                .at_least(1),
        )
        .map_with_span(|(left, operations), span: Range<usize>| Comparison {
            left,
            operations,
            span: to_span(span),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ato_lexer::Lexer;

    fn parse_expr(source: &str) -> Expression {
        let lexer = Lexer::new(source);
        let tokens: Vec<Token> = lexer.collect();
        let len = source.len();

        arithmetic_expression()
            .parse(chumsky::Stream::from_iter(
                len..len,
                tokens.into_iter().map(|t| (t.clone(), t.span.start..t.span.end)),
            ))
            .unwrap()
    }

    #[test]
    fn test_parse_number() {
        let expr = parse_expr("42");
        assert!(matches!(expr, Expression::Literal(Literal::Physical(_))));
    }

    #[test]
    fn test_parse_quantity() {
        let expr = parse_expr("10kohm");
        if let Expression::Literal(Literal::Physical(PhysicalLiteral::Quantity(q))) = expr {
            assert_eq!(q.number.value, "10");
            assert!(q.unit.is_some());
            assert_eq!(q.unit.as_ref().unwrap().name, "kohm");
        } else {
            panic!("Expected quantity");
        }
    }

    #[test]
    fn test_parse_binary_add() {
        let expr = parse_expr("1 + 2");
        if let Expression::Binary(b) = expr {
            assert_eq!(b.operator, BinaryOp::Add);
        } else {
            panic!("Expected binary expression");
        }
    }

    #[test]
    fn test_parse_precedence() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let expr = parse_expr("1 + 2 * 3");
        if let Expression::Binary(b) = expr {
            assert_eq!(b.operator, BinaryOp::Add);
            assert!(matches!(b.right, Expression::Binary(_)));
        } else {
            panic!("Expected binary expression");
        }
    }
}
