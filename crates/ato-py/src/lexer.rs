//! Python bindings for the Ato lexer.

use ato_lexer::{lex, Token, TokenKind};
use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

/// A token from the Ato lexer, exposed to Python.
#[pyclass(name = "Token")]
#[derive(Clone)]
pub struct PyToken {
    /// The token kind (e.g., "Module", "Name").
    #[pyo3(get)]
    pub kind: String,

    /// The token text.
    #[pyo3(get)]
    pub text: String,

    /// Line number (1-indexed).
    #[pyo3(get)]
    pub line: usize,

    /// Column number (1-indexed).
    #[pyo3(get)]
    pub column: usize,

    /// Start byte offset.
    #[pyo3(get)]
    pub start: usize,

    /// End byte offset.
    #[pyo3(get)]
    pub end: usize,
}

#[pymethods]
impl PyToken {
    fn __repr__(&self) -> String {
        format!(
            "Token(kind={:?}, text={:?}, line={}, column={})",
            self.kind, self.text, self.line, self.column
        )
    }

    fn __str__(&self) -> String {
        format!("{}: {:?}", self.kind, self.text)
    }

    /// Convert to a dictionary.
    fn to_dict(&self) -> pyo3::Py<pyo3::types::PyDict> {
        Python::with_gil(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("kind", &self.kind).unwrap();
            dict.set_item("text", &self.text).unwrap();
            dict.set_item("line", self.line).unwrap();
            dict.set_item("column", self.column).unwrap();
            dict.set_item("start", self.start).unwrap();
            dict.set_item("end", self.end).unwrap();
            dict.into()
        })
    }
}

impl From<Token> for PyToken {
    fn from(token: Token) -> Self {
        Self {
            kind: token_kind_name(token.kind),
            text: token.text,
            line: token.span.line,
            column: token.span.column,
            start: token.span.start,
            end: token.span.end,
        }
    }
}

/// Convert a TokenKind to its string name.
fn token_kind_name(kind: TokenKind) -> String {
    match kind {
        TokenKind::Module => "Module",
        TokenKind::Component => "Component",
        TokenKind::Interface => "Interface",
        TokenKind::Pin => "Pin",
        TokenKind::Signal => "Signal",
        TokenKind::Import => "Import",
        TokenKind::From => "From",
        TokenKind::New => "New",
        TokenKind::For => "For",
        TokenKind::In => "In",
        TokenKind::Assert => "Assert",
        TokenKind::Trait => "Trait",
        TokenKind::Pass => "Pass",
        TokenKind::True => "True",
        TokenKind::False => "False",
        TokenKind::To => "To",
        TokenKind::Within => "Within",
        TokenKind::Is => "Is",
        // Reserved keywords
        TokenKind::Int => "Int",
        TokenKind::Float => "Float",
        TokenKind::StringKw => "StringKw",
        TokenKind::Str => "Str",
        TokenKind::Bytes => "Bytes",
        TokenKind::If => "If",
        TokenKind::Parameter => "Parameter",
        TokenKind::Param => "Param",
        TokenKind::Test => "Test",
        TokenKind::Require => "Require",
        TokenKind::Requires => "Requires",
        TokenKind::Check => "Check",
        TokenKind::Report => "Report",
        TokenKind::Ensure => "Ensure",
        // Literals
        TokenKind::Name => "Name",
        TokenKind::Number => "Number",
        TokenKind::String => "String",
        TokenKind::BytesLiteral => "BytesLiteral",
        TokenKind::ImagNumber => "ImagNumber",
        TokenKind::Pragma => "Pragma",
        // Punctuation
        TokenKind::Colon => "Colon",
        TokenKind::DoubleColon => "DoubleColon",
        TokenKind::Comma => "Comma",
        TokenKind::Dot => "Dot",
        TokenKind::Semicolon => "Semicolon",
        TokenKind::Ellipsis => "Ellipsis",
        TokenKind::At => "At",
        // Operators
        TokenKind::Assign => "Assign",
        TokenKind::Plus => "Plus",
        TokenKind::Minus => "Minus",
        TokenKind::Star => "Star",
        TokenKind::Div => "Div",
        TokenKind::IDiv => "IDiv",
        TokenKind::Power => "Power",
        TokenKind::Percent => "Percent",
        TokenKind::PlusOrMinus => "PlusOrMinus",
        // Comparison
        TokenKind::LessThan => "LessThan",
        TokenKind::GreaterThan => "GreaterThan",
        TokenKind::LessEq => "LessEq",
        TokenKind::GreaterEq => "GreaterEq",
        TokenKind::Equals => "Equals",
        TokenKind::NotEq => "NotEq",
        TokenKind::NotEqAlt => "NotEqAlt",
        // Connection
        TokenKind::Wire => "Wire",
        TokenKind::Sperm => "Sperm",
        TokenKind::LSperm => "LSperm",
        TokenKind::Arrow => "Arrow",
        // Assignment operators
        TokenKind::AddAssign => "AddAssign",
        TokenKind::SubAssign => "SubAssign",
        TokenKind::MultAssign => "MultAssign",
        TokenKind::DivAssign => "DivAssign",
        TokenKind::IDivAssign => "IDivAssign",
        TokenKind::PowerAssign => "PowerAssign",
        TokenKind::AtAssign => "AtAssign",
        TokenKind::OrAssign => "OrAssign",
        TokenKind::AndAssign => "AndAssign",
        TokenKind::XorAssign => "XorAssign",
        TokenKind::LeftShiftAssign => "LeftShiftAssign",
        TokenKind::RightShiftAssign => "RightShiftAssign",
        // Bitwise operators
        TokenKind::OrOp => "OrOp",
        TokenKind::AndOp => "AndOp",
        TokenKind::Xor => "Xor",
        TokenKind::LeftShift => "LeftShift",
        TokenKind::RightShift => "RightShift",
        // Brackets
        TokenKind::OpenParen => "OpenParen",
        TokenKind::CloseParen => "CloseParen",
        TokenKind::OpenBracket => "OpenBracket",
        TokenKind::CloseBracket => "CloseBracket",
        TokenKind::OpenBrace => "OpenBrace",
        TokenKind::CloseBrace => "CloseBrace",
        // Structure
        TokenKind::Newline => "Newline",
        TokenKind::Indent => "Indent",
        TokenKind::Dedent => "Dedent",
        TokenKind::Eof => "Eof",
        TokenKind::Comment => "Comment",
        TokenKind::Whitespace => "Whitespace",
        TokenKind::ExplicitLineJoining => "ExplicitLineJoining",
        TokenKind::Error => "Error",
    }
    .to_string()
}

/// Tokenize Ato source code into a list of tokens.
pub fn tokenize(source: &str) -> PyResult<Vec<PyToken>> {
    let (tokens, errors) = lex(source);

    if !errors.is_empty() {
        let error_msg = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(PyValueError::new_err(format!("Lexer errors: {}", error_msg)));
    }

    Ok(tokens.into_iter().map(PyToken::from).collect())
}
