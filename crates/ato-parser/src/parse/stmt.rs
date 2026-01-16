//! Statement parsing for the Ato language.

use ato_lexer::TokenKind;
use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use super::Parser;

impl Parser<'_> {
    /// Parse a statement.
    pub fn parse_statement(&mut self) -> ParseResult<Statement> {
        match self.current_kind() {
            TokenKind::Pragma => self.parse_pragma(),
            TokenKind::Module | TokenKind::Component | TokenKind::Interface => self.parse_block_def(),
            TokenKind::For => self.parse_for_statement(),
            _ => self.parse_simple_statement(),
        }
    }

    /// Parse a pragma statement.
    fn parse_pragma(&mut self) -> ParseResult<Statement> {
        let token = self.expect(TokenKind::Pragma)?;
        let stmt = PragmaStmt {
            content: token.text.clone(),
            span: token.span,
        };

        // Consume newline after pragma
        self.match_token(TokenKind::Newline);

        Ok(Statement::Pragma(stmt))
    }

    /// Parse a block definition (module, component, interface).
    fn parse_block_def(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();

        let kind = match self.current_kind() {
            TokenKind::Module => {
                self.advance();
                BlockKind::Module
            }
            TokenKind::Component => {
                self.advance();
                BlockKind::Component
            }
            TokenKind::Interface => {
                self.advance();
                BlockKind::Interface
            }
            _ => return Err(ParseError::expected("module, component, or interface", start_span)),
        };

        let name = self.parse_identifier()?;

        // Optional super type
        let super_type = if self.match_token(TokenKind::From) {
            Some(self.parse_type_reference()?)
        } else {
            None
        };

        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let end_span = self.previous_span();

        Ok(Statement::BlockDef(BlockDef {
            kind,
            name,
            super_type,
            body,
            span: start_span.merge(&end_span),
        }))
    }

    /// Parse a block (indented statements or single line).
    fn parse_block(&mut self) -> ParseResult<Vec<Statement>> {
        // Single-line block: simple_stmts
        if !self.check(TokenKind::Newline) {
            return self.parse_simple_statements();
        }

        // Multi-line block: NEWLINE INDENT stmt+ DEDENT
        self.expect(TokenKind::Newline)?;
        self.expect(TokenKind::Indent)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.is_at_end() {
            // Skip extra newlines
            if self.match_token(TokenKind::Newline) {
                continue;
            }

            // Parse a statement line (which handles semicolon-separated statements)
            let stmts = self.parse_statement_line()?;
            statements.extend(stmts);
        }

        self.expect(TokenKind::Dedent)?;

        Ok(statements)
    }

    /// Parse a line of statements (handles semicolon separation and compound statements).
    fn parse_statement_line(&mut self) -> ParseResult<Vec<Statement>> {
        match self.current_kind() {
            TokenKind::Pragma => {
                let stmt = self.parse_pragma()?;
                Ok(vec![stmt])
            }
            TokenKind::Module | TokenKind::Component | TokenKind::Interface => {
                let stmt = self.parse_block_def()?;
                Ok(vec![stmt])
            }
            TokenKind::For => {
                let stmt = self.parse_for_statement()?;
                Ok(vec![stmt])
            }
            _ => self.parse_simple_statements(),
        }
    }

    /// Parse simple statements (semicolon-separated on one line).
    fn parse_simple_statements(&mut self) -> ParseResult<Vec<Statement>> {
        let mut statements = vec![self.parse_simple_statement()?];

        while self.match_token(TokenKind::Semicolon) {
            if self.check(TokenKind::Newline) || self.is_at_end() {
                break;
            }
            statements.push(self.parse_simple_statement()?);
        }

        self.match_token(TokenKind::Newline);

        Ok(statements)
    }

    /// Parse a simple (non-compound) statement.
    fn parse_simple_statement(&mut self) -> ParseResult<Statement> {
        match self.current_kind() {
            TokenKind::Import => self.parse_import(),
            TokenKind::From => self.parse_from_import(),
            TokenKind::Pin => self.parse_pin_declaration(),
            TokenKind::Signal => self.parse_signal_def(),
            TokenKind::Assert => self.parse_assert(),
            TokenKind::Pass => self.parse_pass(),
            TokenKind::Trait => self.parse_trait(),
            TokenKind::String => self.parse_string_statement(),
            TokenKind::Name => self.parse_name_starting_statement(),
            _ => Err(ParseError::expected("statement", self.current_span())),
        }
    }

    /// Parse an import statement.
    fn parse_import(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();
        self.expect(TokenKind::Import)?;

        let type_ref = self.parse_type_reference()?;

        // Check for deprecated form: import X from "path"
        if self.match_token(TokenKind::From) {
            let path = self.parse_string_literal()?;
            let end_span = self.previous_span();

            return Ok(Statement::DepImport(DepImportStmt {
                type_ref,
                from_path: path,
                span: start_span.merge(&end_span),
            }));
        }

        // Standard form: import X, Y, Z
        let mut imports = vec![type_ref];
        while self.match_token(TokenKind::Comma) {
            imports.push(self.parse_type_reference()?);
        }

        let end_span = self.previous_span();
        Ok(Statement::Import(ImportStmt {
            from_path: None,
            imports,
            span: start_span.merge(&end_span),
        }))
    }

    /// Parse a from...import statement.
    fn parse_from_import(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();
        self.expect(TokenKind::From)?;

        let path = self.parse_string_literal()?;
        self.expect(TokenKind::Import)?;

        let mut imports = vec![self.parse_type_reference()?];
        while self.match_token(TokenKind::Comma) {
            imports.push(self.parse_type_reference()?);
        }

        let end_span = self.previous_span();
        Ok(Statement::Import(ImportStmt {
            from_path: Some(path),
            imports,
            span: start_span.merge(&end_span),
        }))
    }

    /// Parse a pin declaration.
    fn parse_pin_declaration(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();
        self.expect(TokenKind::Pin)?;

        let name = match self.current_kind() {
            TokenKind::Name => PinName::Identifier(self.parse_identifier()?),
            TokenKind::Number => PinName::Number(self.parse_number_literal()?),
            TokenKind::String => PinName::String(self.parse_string_literal()?),
            _ => return Err(ParseError::expected("pin name", self.current_span())),
        };

        let end_span = self.previous_span();
        Ok(Statement::PinDeclaration(PinDeclaration {
            name,
            span: start_span.merge(&end_span),
        }))
    }

    /// Parse a signal definition or signal-starting connection.
    fn parse_signal_def(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();
        self.expect(TokenKind::Signal)?;

        let name = self.parse_identifier()?;
        let signal_span = start_span.merge(&self.previous_span());

        let signal_def = SignalDef {
            name,
            span: signal_span,
        };

        // Check if this is followed by a connection operator
        match self.current_kind() {
            TokenKind::Wire => {
                self.advance();
                let right = self.parse_connectable()?;
                let end_span = self.previous_span();

                Ok(Statement::Connection(Connection {
                    left: Connectable::SignalDef(signal_def),
                    right,
                    span: start_span.merge(&end_span),
                }))
            }

            TokenKind::Sperm | TokenKind::LSperm => {
                let direction = if self.current_kind() == TokenKind::Sperm {
                    ConnectionDirection::Forward
                } else {
                    ConnectionDirection::Backward
                };

                let mut elements = vec![Connectable::SignalDef(signal_def)];

                while self.check_any(&[TokenKind::Sperm, TokenKind::LSperm]) {
                    let current_dir = if self.current_kind() == TokenKind::Sperm {
                        ConnectionDirection::Forward
                    } else {
                        ConnectionDirection::Backward
                    };

                    if current_dir != direction {
                        return Err(ParseError::mixed_connection_operators(self.current_span()));
                    }

                    self.advance();
                    elements.push(self.parse_connectable()?);
                }

                let end_span = self.previous_span();
                Ok(Statement::DirectedConnection(DirectedConnection {
                    direction,
                    elements,
                    span: start_span.merge(&end_span),
                }))
            }

            _ => Ok(Statement::SignalDef(signal_def)),
        }
    }

    /// Parse an assert statement.
    fn parse_assert(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();
        self.expect(TokenKind::Assert)?;

        let comparison = self.parse_comparison()?;
        let end_span = self.previous_span();

        Ok(Statement::Assert(AssertStmt {
            comparison,
            span: start_span.merge(&end_span),
        }))
    }

    /// Parse a pass statement.
    fn parse_pass(&mut self) -> ParseResult<Statement> {
        let span = self.current_span();
        self.expect(TokenKind::Pass)?;

        Ok(Statement::Pass(PassStmt { span }))
    }

    /// Parse a trait statement.
    fn parse_trait(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();
        self.expect(TokenKind::Trait)?;

        // Optional target field (before the type reference)
        // Look ahead to see if we have "field.path type_ref" or just "type_ref"
        let mut target = None;

        // Parse what could be either the target or the type_ref
        let first_ref = self.parse_field_reference()?;

        // If there's another name following (not ::, <, or end of statement),
        // then first_ref is the target and we need to parse the type_ref
        let type_ref = if self.check(TokenKind::Name) {
            // First ref was the target, parse the actual type ref
            target = Some(first_ref);
            self.parse_type_reference()?
        } else {
            // First ref is the type ref, convert it
            TypeRef {
                parts: first_ref.parts.into_iter().map(|p| p.name).collect(),
                span: first_ref.span,
            }
        };

        // Optional constructor: ::name
        let constructor = if self.match_token(TokenKind::DoubleColon) {
            Some(self.parse_identifier()?)
        } else {
            None
        };

        // Optional template: <args>
        let template = if self.check(TokenKind::LessThan) {
            Some(self.parse_template()?)
        } else {
            None
        };

        let end_span = self.previous_span();
        Ok(Statement::Trait(TraitStmt {
            target,
            type_ref,
            constructor,
            template,
            span: start_span.merge(&end_span),
        }))
    }

    /// Parse a string statement (docstring).
    fn parse_string_statement(&mut self) -> ParseResult<Statement> {
        let s = self.parse_string_literal()?;
        Ok(Statement::StringStmt(StringStmt {
            span: s.span,
            value: s,
        }))
    }

    /// Parse a statement starting with a name (assignment, connection, retype, declaration).
    fn parse_name_starting_statement(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();

        // Parse the initial field reference or declaration
        let field_ref = self.parse_field_reference()?;

        // Check what follows
        match self.current_kind() {
            // Declaration: field: type
            TokenKind::Colon if !self.check_any(&[TokenKind::Newline, TokenKind::Semicolon]) => {
                self.advance();
                let type_name = self.parse_identifier()?;
                let end_span = self.previous_span();

                let decl = Declaration {
                    field: field_ref.clone(),
                    type_info: type_name,
                    span: start_span.merge(&end_span),
                };

                // Check if this is followed by assignment
                if self.match_token(TokenKind::Assign) {
                    let value = self.parse_assignable()?;
                    let end_span = self.previous_span();

                    Ok(Statement::Assignment(Assignment {
                        target: AssignTarget::Declaration(decl),
                        value,
                        span: start_span.merge(&end_span),
                    }))
                } else {
                    Ok(Statement::Declaration(decl))
                }
            }

            // Assignment: field = value
            TokenKind::Assign => {
                self.advance();
                let value = self.parse_assignable()?;
                let end_span = self.previous_span();

                Ok(Statement::Assignment(Assignment {
                    target: AssignTarget::FieldRef(field_ref),
                    value,
                    span: start_span.merge(&end_span),
                }))
            }

            // Cumulative assignment: field += value or field -= value
            TokenKind::AddAssign | TokenKind::SubAssign => {
                let op = if self.match_token(TokenKind::AddAssign) {
                    CumOperator::Add
                } else {
                    self.advance();
                    CumOperator::Sub
                };

                let value = self.parse_arithmetic_expression()?;
                let end_span = self.previous_span();

                Ok(Statement::CumAssignment(CumAssignment {
                    target: AssignTarget::FieldRef(field_ref),
                    operator: op,
                    value,
                    span: start_span.merge(&end_span),
                }))
            }

            // Set assignment: field |= value or field &= value
            TokenKind::OrAssign | TokenKind::AndAssign => {
                let op = if self.match_token(TokenKind::OrAssign) {
                    SetOperator::Or
                } else {
                    self.advance();
                    SetOperator::And
                };

                let value = self.parse_arithmetic_expression()?;
                let end_span = self.previous_span();

                Ok(Statement::SetAssignment(SetAssignment {
                    target: AssignTarget::FieldRef(field_ref),
                    operator: op,
                    value,
                    span: start_span.merge(&end_span),
                }))
            }

            // Connection: field ~ field
            TokenKind::Wire => {
                self.advance();
                let right = self.parse_connectable()?;
                let end_span = self.previous_span();

                Ok(Statement::Connection(Connection {
                    left: Connectable::FieldRef(field_ref),
                    right,
                    span: start_span.merge(&end_span),
                }))
            }

            // Directed connection: field ~> field ~> field or field <~ field <~ field
            TokenKind::Sperm | TokenKind::LSperm => {
                let direction = if self.current_kind() == TokenKind::Sperm {
                    ConnectionDirection::Forward
                } else {
                    ConnectionDirection::Backward
                };

                let mut elements = vec![Connectable::FieldRef(field_ref)];

                while self.check_any(&[TokenKind::Sperm, TokenKind::LSperm]) {
                    // Check for mixed operators
                    let current_dir = if self.current_kind() == TokenKind::Sperm {
                        ConnectionDirection::Forward
                    } else {
                        ConnectionDirection::Backward
                    };

                    if current_dir != direction {
                        return Err(ParseError::mixed_connection_operators(self.current_span()));
                    }

                    self.advance();
                    elements.push(self.parse_connectable()?);
                }

                let end_span = self.previous_span();
                Ok(Statement::DirectedConnection(DirectedConnection {
                    direction,
                    elements,
                    span: start_span.merge(&end_span),
                }))
            }

            // Retype: field -> Type
            TokenKind::Arrow => {
                self.advance();
                let new_type = self.parse_type_reference()?;
                let end_span = self.previous_span();

                Ok(Statement::Retype(Retype {
                    field: field_ref,
                    new_type,
                    span: start_span.merge(&end_span),
                }))
            }

            _ => Err(ParseError::expected(
                "assignment, connection, or declaration",
                self.current_span(),
            )),
        }
    }

    /// Parse a connectable element.
    fn parse_connectable(&mut self) -> ParseResult<Connectable> {
        match self.current_kind() {
            TokenKind::Signal => {
                let start_span = self.current_span();
                self.advance();
                let name = self.parse_identifier()?;
                let end_span = self.previous_span();

                Ok(Connectable::SignalDef(SignalDef {
                    name,
                    span: start_span.merge(&end_span),
                }))
            }

            TokenKind::Pin => {
                let start_span = self.current_span();
                self.advance();

                let name = match self.current_kind() {
                    TokenKind::Name => PinName::Identifier(self.parse_identifier()?),
                    TokenKind::Number => PinName::Number(self.parse_number_literal()?),
                    TokenKind::String => PinName::String(self.parse_string_literal()?),
                    _ => return Err(ParseError::expected("pin name", self.current_span())),
                };

                let end_span = self.previous_span();
                Ok(Connectable::PinDef(PinDeclaration {
                    name,
                    span: start_span.merge(&end_span),
                }))
            }

            TokenKind::Name => {
                let field_ref = self.parse_field_reference()?;
                Ok(Connectable::FieldRef(field_ref))
            }

            _ => Err(ParseError::expected("connectable", self.current_span())),
        }
    }

    /// Parse a for statement.
    fn parse_for_statement(&mut self) -> ParseResult<Statement> {
        let start_span = self.current_span();
        self.expect(TokenKind::For)?;

        let variable = self.parse_identifier()?;
        self.expect(TokenKind::In)?;

        let iterable = self.parse_iterable()?;
        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let end_span = self.previous_span();

        Ok(Statement::For(ForStmt {
            variable,
            iterable,
            body,
            span: start_span.merge(&end_span),
        }))
    }

    /// Parse an iterable (field reference with optional slice, or list literal).
    fn parse_iterable(&mut self) -> ParseResult<Iterable> {
        if self.match_token(TokenKind::OpenBracket) {
            // List literal: [field1, field2, ...]
            let mut fields = Vec::new();

            if !self.check(TokenKind::CloseBracket) {
                fields.push(self.parse_field_reference()?);
                while self.match_token(TokenKind::Comma) {
                    if self.check(TokenKind::CloseBracket) {
                        break;
                    }
                    fields.push(self.parse_field_reference()?);
                }
            }

            self.expect(TokenKind::CloseBracket)?;
            Ok(Iterable::List(fields))
        } else {
            // Field reference with optional slice
            let field = self.parse_field_reference()?;

            let slice = if self.check(TokenKind::OpenBracket) {
                // Check if this is a slice or just an index
                // A slice has a colon inside
                Some(self.parse_slice()?)
            } else {
                None
            };

            Ok(Iterable::FieldRef { field, slice })
        }
    }
}
