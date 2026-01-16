//! Statement parsing for the Ato language using chumsky 0.9 with error recovery.

use crate::ast::*;
use ato_lexer::{Span, Token, TokenKind};
use chumsky::prelude::*;
use std::ops::Range;

use super::expr::*;

/// Convert a Range<usize> to our Span type.
fn to_span(range: &Range<usize>) -> Span {
    Span::new(range.start, range.end, 1, 1)
}

/// Parse a pragma statement.
fn pragma_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::Pragma)
        .then_ignore(tok(TokenKind::Newline).or_not())
        .map_with_span(|t: Token, span: Range<usize>| {
            Statement::Pragma(PragmaStmt {
                content: t.text.clone(),
                span: to_span(&span),
            })
        })
}

/// Parse an import statement.
fn import_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::Import)
        .ignore_then(type_reference())
        .then(
            choice((
                // Deprecated form: import X from "path"
                tok(TokenKind::From)
                    .ignore_then(string_literal())
                    .map(ImportKind::Deprecated),
                // Standard form with additional imports
                tok(TokenKind::Comma)
                    .ignore_then(type_reference())
                    .repeated()
                    .map(ImportKind::Standard),
            )),
        )
        .map_with_span(|(first, kind), span: Range<usize>| match kind {
            ImportKind::Deprecated(path) => Statement::DepImport(DepImportStmt {
                type_ref: first,
                from_path: path,
                span: to_span(&span),
            }),
            ImportKind::Standard(rest) => {
                let mut imports = vec![first];
                imports.extend(rest);
                Statement::Import(ImportStmt {
                    from_path: None,
                    imports,
                    span: to_span(&span),
                })
            }
        })
}

enum ImportKind {
    Deprecated(StringLiteral),
    Standard(Vec<TypeRef>),
}

/// Parse a from...import statement.
fn from_import_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::From)
        .ignore_then(string_literal())
        .then_ignore(tok(TokenKind::Import))
        .then(
            type_reference()
                .separated_by(tok(TokenKind::Comma))
                .at_least(1),
        )
        .map_with_span(|(path, imports), span: Range<usize>| {
            Statement::Import(ImportStmt {
                from_path: Some(path),
                imports,
                span: to_span(&span),
            })
        })
}

/// Parse a pin name.
fn pin_name() -> impl Parser<Token, PinName, Error = Simple<Token>> + Clone {
    choice((
        identifier().map(PinName::Identifier),
        number_literal().map(PinName::Number),
        string_literal().map(PinName::String),
    ))
}

/// Parse a pin declaration.
fn pin_declaration() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::Pin)
        .ignore_then(pin_name())
        .map_with_span(|name, span: Range<usize>| {
            Statement::PinDeclaration(PinDeclaration {
                name,
                span: to_span(&span),
            })
        })
}

/// Parse a connectable element.
fn connectable() -> impl Parser<Token, Connectable, Error = Simple<Token>> + Clone {
    choice((
        tok(TokenKind::Signal)
            .ignore_then(identifier())
            .map_with_span(|name, span: Range<usize>| {
                Connectable::SignalDef(SignalDef {
                    name,
                    span: to_span(&span),
                })
            }),
        tok(TokenKind::Pin)
            .ignore_then(pin_name())
            .map_with_span(|name, span: Range<usize>| {
                Connectable::PinDef(PinDeclaration {
                    name,
                    span: to_span(&span),
                })
            }),
        field_reference().map(Connectable::FieldRef),
    ))
}

/// Parse a signal definition (possibly followed by connection).
fn signal_def() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::Signal)
        .ignore_then(identifier())
        .then(
            choice((
                // signal ~ connectable
                tok(TokenKind::Wire)
                    .ignore_then(connectable())
                    .map(ConnectionSuffix::Simple),
                // signal ~> connectable ~> ...
                tok(TokenKind::Sperm)
                    .ignore_then(connectable())
                    .then(
                        tok(TokenKind::Sperm)
                            .ignore_then(connectable())
                            .repeated(),
                    )
                    .map(|(first, rest)| {
                        let mut elements = vec![first];
                        elements.extend(rest);
                        ConnectionSuffix::Forward(elements)
                    }),
                // signal <~ connectable <~ ...
                tok(TokenKind::LSperm)
                    .ignore_then(connectable())
                    .then(
                        tok(TokenKind::LSperm)
                            .ignore_then(connectable())
                            .repeated(),
                    )
                    .map(|(first, rest)| {
                        let mut elements = vec![first];
                        elements.extend(rest);
                        ConnectionSuffix::Backward(elements)
                    }),
            ))
            .or_not(),
        )
        .map_with_span(|(name, suffix), span: Range<usize>| {
            let signal = SignalDef {
                name: name.clone(),
                span: to_span(&span),
            };

            match suffix {
                None => Statement::SignalDef(signal),
                Some(ConnectionSuffix::Simple(right)) => Statement::Connection(Connection {
                    left: Connectable::SignalDef(signal),
                    right,
                    span: to_span(&span),
                }),
                Some(ConnectionSuffix::Forward(rest)) => {
                    let mut elements = vec![Connectable::SignalDef(signal)];
                    elements.extend(rest);
                    Statement::DirectedConnection(DirectedConnection {
                        direction: ConnectionDirection::Forward,
                        elements,
                        span: to_span(&span),
                    })
                }
                Some(ConnectionSuffix::Backward(rest)) => {
                    let mut elements = vec![Connectable::SignalDef(signal)];
                    elements.extend(rest);
                    Statement::DirectedConnection(DirectedConnection {
                        direction: ConnectionDirection::Backward,
                        elements,
                        span: to_span(&span),
                    })
                }
            }
        })
}

enum ConnectionSuffix {
    Simple(Connectable),
    Forward(Vec<Connectable>),
    Backward(Vec<Connectable>),
}

/// Parse a pass statement.
fn pass_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::Pass).map_with_span(|_, span: Range<usize>| {
        Statement::Pass(PassStmt {
            span: to_span(&span),
        })
    })
}

/// Parse a string statement (docstring).
fn string_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    string_literal().map(|s| {
        Statement::StringStmt(StringStmt {
            span: s.span,
            value: s,
        })
    })
}

/// Parse an assert statement.
fn assert_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::Assert)
        .ignore_then(comparison())
        .map_with_span(|comp, span: Range<usize>| {
            Statement::Assert(AssertStmt {
                comparison: comp,
                span: to_span(&span),
            })
        })
}

/// Parse a template argument.
fn template_arg() -> impl Parser<Token, TemplateArg, Error = Simple<Token>> + Clone {
    identifier()
        .then_ignore(tok(TokenKind::Assign))
        .then(literal())
        .map_with_span(|(name, value), span: Range<usize>| TemplateArg {
            name,
            value,
            span: to_span(&span),
        })
}

/// Parse a template (<arg=value, ...>).
fn template() -> impl Parser<Token, Template, Error = Simple<Token>> + Clone {
    template_arg()
        .separated_by(tok(TokenKind::Comma))
        .allow_trailing()
        .delimited_by(tok(TokenKind::LessThan), tok(TokenKind::GreaterThan))
        .map_with_span(|args, span: Range<usize>| Template {
            args,
            span: to_span(&span),
        })
}

/// Parse a trait statement.
fn trait_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::Trait)
        .ignore_then(type_reference())
        .then(
            tok(TokenKind::DoubleColon)
                .ignore_then(identifier())
                .or_not(),
        )
        .then(template().or_not())
        .map_with_span(|((type_ref, constructor), template), span: Range<usize>| {
            Statement::Trait(TraitStmt {
                target: None,
                type_ref,
                constructor,
                template,
                span: to_span(&span),
            })
        })
}

/// Parse a new expression.
fn new_expr() -> impl Parser<Token, NewExpr, Error = Simple<Token>> + Clone {
    tok(TokenKind::New)
        .ignore_then(type_reference())
        .then(
            number_literal()
                .delimited_by(tok(TokenKind::OpenBracket), tok(TokenKind::CloseBracket))
                .or_not(),
        )
        .then(template().or_not())
        .map_with_span(|((type_ref, count), template), span: Range<usize>| NewExpr {
            type_ref,
            count,
            template,
            span: to_span(&span),
        })
}

/// Parse an assignable value.
fn assignable() -> impl Parser<Token, Assignable, Error = Simple<Token>> + Clone {
    choice((
        string_literal().map(Assignable::String),
        new_expr().map(Assignable::New),
        bool_literal().map(Assignable::Boolean),
        physical_literal().map(Assignable::Physical),
        arithmetic_expression().map(Assignable::Arithmetic),
    ))
}

/// Parse a statement starting with a name (assignment, connection, etc.).
fn name_starting_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    field_reference()
        .then(choice((
            // Declaration with optional assignment: field: type (= value)?
            tok(TokenKind::Colon)
                .ignore_then(identifier())
                .then(tok(TokenKind::Assign).ignore_then(assignable()).or_not())
                .map(|(type_info, value)| NameSuffix::Declaration { type_info, value }),
            // Assignment: field = value
            tok(TokenKind::Assign)
                .ignore_then(assignable())
                .map(NameSuffix::Assign),
            // Cumulative assignment: field += value or field -= value
            choice((
                tok(TokenKind::AddAssign).to(CumOperator::Add),
                tok(TokenKind::SubAssign).to(CumOperator::Sub),
            ))
            .then(arithmetic_expression())
            .map(|(op, value)| NameSuffix::CumAssign { op, value }),
            // Set assignment: field |= value or field &= value
            choice((
                tok(TokenKind::OrAssign).to(SetOperator::Or),
                tok(TokenKind::AndAssign).to(SetOperator::And),
            ))
            .then(arithmetic_expression())
            .map(|(op, value)| NameSuffix::SetAssign { op, value }),
            // Connection: field ~ connectable
            tok(TokenKind::Wire)
                .ignore_then(connectable())
                .map(NameSuffix::Connect),
            // Directed connection forward: field ~> connectable ~> ...
            tok(TokenKind::Sperm)
                .ignore_then(connectable())
                .then(
                    tok(TokenKind::Sperm)
                        .ignore_then(connectable())
                        .repeated(),
                )
                .map(|(first, rest)| NameSuffix::ForwardConnect { first, rest }),
            // Directed connection backward: field <~ connectable <~ ...
            tok(TokenKind::LSperm)
                .ignore_then(connectable())
                .then(
                    tok(TokenKind::LSperm)
                        .ignore_then(connectable())
                        .repeated(),
                )
                .map(|(first, rest)| NameSuffix::BackwardConnect { first, rest }),
            // Retype: field -> Type
            tok(TokenKind::Arrow)
                .ignore_then(type_reference())
                .map(NameSuffix::Retype),
        )))
        .map_with_span(|(field_ref, suffix), span: Range<usize>| match suffix {
            NameSuffix::Declaration { type_info, value } => {
                let decl = Declaration {
                    field: field_ref.clone(),
                    type_info,
                    span: to_span(&span),
                };
                match value {
                    Some(v) => Statement::Assignment(Assignment {
                        target: AssignTarget::Declaration(decl),
                        value: v,
                        span: to_span(&span),
                    }),
                    None => Statement::Declaration(decl),
                }
            }
            NameSuffix::Assign(value) => Statement::Assignment(Assignment {
                target: AssignTarget::FieldRef(field_ref),
                value,
                span: to_span(&span),
            }),
            NameSuffix::CumAssign { op, value } => Statement::CumAssignment(CumAssignment {
                target: AssignTarget::FieldRef(field_ref),
                operator: op,
                value,
                span: to_span(&span),
            }),
            NameSuffix::SetAssign { op, value } => Statement::SetAssignment(SetAssignment {
                target: AssignTarget::FieldRef(field_ref),
                operator: op,
                value,
                span: to_span(&span),
            }),
            NameSuffix::Connect(right) => Statement::Connection(Connection {
                left: Connectable::FieldRef(field_ref),
                right,
                span: to_span(&span),
            }),
            NameSuffix::ForwardConnect { first, rest } => {
                let mut elements = vec![Connectable::FieldRef(field_ref), first];
                elements.extend(rest);
                Statement::DirectedConnection(DirectedConnection {
                    direction: ConnectionDirection::Forward,
                    elements,
                    span: to_span(&span),
                })
            }
            NameSuffix::BackwardConnect { first, rest } => {
                let mut elements = vec![Connectable::FieldRef(field_ref), first];
                elements.extend(rest);
                Statement::DirectedConnection(DirectedConnection {
                    direction: ConnectionDirection::Backward,
                    elements,
                    span: to_span(&span),
                })
            }
            NameSuffix::Retype(new_type) => Statement::Retype(Retype {
                field: field_ref,
                new_type,
                span: to_span(&span),
            }),
        })
}

enum NameSuffix {
    Declaration {
        type_info: Identifier,
        value: Option<Assignable>,
    },
    Assign(Assignable),
    CumAssign {
        op: CumOperator,
        value: Expression,
    },
    SetAssign {
        op: SetOperator,
        value: Expression,
    },
    Connect(Connectable),
    ForwardConnect {
        first: Connectable,
        rest: Vec<Connectable>,
    },
    BackwardConnect {
        first: Connectable,
        rest: Vec<Connectable>,
    },
    Retype(TypeRef),
}

/// Parse a slice ([start:stop:step]).
fn slice() -> impl Parser<Token, Slice, Error = Simple<Token>> + Clone {
    choice((
        // [::step]
        tok(TokenKind::DoubleColon)
            .ignore_then(number_literal().or_not())
            .map(|step| (None, None, step)),
        // [start:stop:step] or [start:stop] or [:stop] etc
        number_literal()
            .or_not()
            .then(
                tok(TokenKind::Colon)
                    .ignore_then(number_literal().or_not())
                    .then(
                        tok(TokenKind::Colon)
                            .ignore_then(number_literal().or_not())
                            .or_not(),
                    )
                    .or_not(),
            )
            .map(|(start, rest)| match rest {
                Some((stop, step)) => (start, stop, step.flatten()),
                None => (start, None, None),
            }),
    ))
    .delimited_by(tok(TokenKind::OpenBracket), tok(TokenKind::CloseBracket))
    .map_with_span(|(start, stop, step), span: Range<usize>| Slice {
        start,
        stop,
        step,
        span: to_span(&span),
    })
}

/// Parse an iterable for a for loop.
fn iterable() -> impl Parser<Token, Iterable, Error = Simple<Token>> + Clone {
    choice((
        // List literal: [field1, field2, ...]
        field_reference()
            .separated_by(tok(TokenKind::Comma))
            .allow_trailing()
            .delimited_by(tok(TokenKind::OpenBracket), tok(TokenKind::CloseBracket))
            .map(Iterable::List),
        // Field reference with optional slice
        field_reference()
            .then(slice().or_not())
            .map(|(field, slice)| Iterable::FieldRef { field, slice }),
    ))
}

/// Parse a simple statement.
fn simple_stmt() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    choice((
        import_stmt(),
        from_import_stmt(),
        pin_declaration(),
        signal_def(),
        assert_stmt(),
        pass_stmt(),
        trait_stmt(),
        string_stmt(),
        name_starting_stmt(),
    ))
}

/// Parse semicolon-separated statements on one line.
fn simple_stmts() -> impl Parser<Token, Vec<Statement>, Error = Simple<Token>> + Clone {
    simple_stmt()
        .separated_by(tok(TokenKind::Semicolon))
        .at_least(1)
        .then_ignore(tok(TokenKind::Semicolon).or_not())
        .then_ignore(tok(TokenKind::Newline).or_not())
}

/// Parse a block (indented or single-line).
fn block() -> impl Parser<Token, Vec<Statement>, Error = Simple<Token>> + Clone {
    recursive(|block| {
        choice((
            // Single-line block
            simple_stmts(),
            // Multi-line block
            tok(TokenKind::Newline)
                .ignore_then(tok(TokenKind::Indent))
                .ignore_then(
                    choice((
                        pragma_stmt().map(|s| vec![s]),
                        block_def(block.clone()).map(|s| vec![s]),
                        for_stmt(block.clone()).map(|s| vec![s]),
                        simple_stmts(),
                    ))
                    .then_ignore(tok(TokenKind::Newline).repeated())
                    .repeated()
                    .at_least(1)
                    .flatten(),
                )
                .then_ignore(tok(TokenKind::Dedent)),
        ))
    })
}

/// Parse a block definition (module, component, interface).
fn block_def(
    block: impl Parser<Token, Vec<Statement>, Error = Simple<Token>> + Clone,
) -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    let block_kind = choice((
        tok(TokenKind::Module).to(BlockKind::Module),
        tok(TokenKind::Component).to(BlockKind::Component),
        tok(TokenKind::Interface).to(BlockKind::Interface),
    ));

    block_kind
        .then(identifier())
        .then(tok(TokenKind::From).ignore_then(type_reference()).or_not())
        .then_ignore(tok(TokenKind::Colon))
        .then(block)
        .map_with_span(|(((kind, name), super_type), body), span: Range<usize>| {
            Statement::BlockDef(BlockDef {
                kind,
                name,
                super_type,
                body,
                span: to_span(&span),
            })
        })
}

/// Parse a for statement.
fn for_stmt(
    block: impl Parser<Token, Vec<Statement>, Error = Simple<Token>> + Clone,
) -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    tok(TokenKind::For)
        .ignore_then(identifier())
        .then_ignore(tok(TokenKind::In))
        .then(iterable())
        .then_ignore(tok(TokenKind::Colon))
        .then(block)
        .map_with_span(|((variable, iterable), body), span: Range<usize>| {
            Statement::For(ForStmt {
                variable,
                iterable,
                body,
                span: to_span(&span),
            })
        })
}

/// Parse a top-level statement with error recovery.
fn statement() -> impl Parser<Token, Statement, Error = Simple<Token>> + Clone {
    recursive(|_| {
        let blk = block();
        choice((
            pragma_stmt(),
            block_def(blk.clone()),
            for_stmt(blk),
            simple_stmt(),
        ))
    })
}

/// Parse an entire file.
///
/// This parser parses top-level statements and collects parse errors.
/// For error recovery, use parse_recovery() which will continue after errors.
pub fn file_parser() -> impl Parser<Token, File, Error = Simple<Token>> {
    tok(TokenKind::Newline)
        .repeated()
        .ignore_then(
            statement()
                .then_ignore(tok(TokenKind::Newline).repeated())
                .repeated(),
        )
        .then_ignore(tok(TokenKind::Eof).or_not())
        .then_ignore(end())
        .map_with_span(|statements, span: Range<usize>| File {
            statements,
            span: to_span(&span),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ato_lexer::Lexer;

    fn parse_file(source: &str) -> (Option<File>, Vec<Simple<Token>>) {
        let lexer = Lexer::new(source);
        let tokens: Vec<Token> = lexer.collect();
        let len = source.len();

        file_parser().parse_recovery(chumsky::Stream::from_iter(
            len..len,
            tokens.into_iter().map(|t| (t.clone(), t.span.start..t.span.end)),
        ))
    }

    #[test]
    fn test_parse_empty() {
        let (ast, errors) = parse_file("");
        assert!(errors.is_empty());
        assert!(ast.is_some());
        assert!(ast.unwrap().statements.is_empty());
    }

    #[test]
    fn test_parse_pragma() {
        let (ast, errors) = parse_file("#pragma experiment(\"FOR_LOOP\")\n");
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        assert!(ast.is_some());
    }

    #[test]
    fn test_parse_module() {
        let (ast, errors) = parse_file("module M:\n    pass\n");
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        let file = ast.unwrap();
        assert_eq!(file.statements.len(), 1);
    }

    #[test]
    fn test_parse_nested_block() {
        let (ast, errors) = parse_file("module A:\n    module B:\n        pass\n");
        assert!(errors.is_empty(), "Errors: {:?}", errors);
        let file = ast.unwrap();
        if let Statement::BlockDef(outer) = &file.statements[0] {
            assert_eq!(outer.name.name, "A");
            if let Statement::BlockDef(inner) = &outer.body[0] {
                assert_eq!(inner.name.name, "B");
            } else {
                panic!("Expected inner block");
            }
        } else {
            panic!("Expected outer block");
        }
    }
}
