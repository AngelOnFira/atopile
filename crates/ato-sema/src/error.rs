//! Error types for semantic analysis.

use ato_lexer::Span;
use thiserror::Error;

/// Errors that can occur during semantic analysis.
#[derive(Debug, Clone, Error)]
pub enum SemaError {
    /// An import could not be resolved.
    #[error("cannot resolve import '{name}'")]
    UnresolvedImport {
        name: String,
        span: Option<Span>,
    },

    /// A file could not be found.
    #[error("file not found: {path}")]
    FileNotFound {
        path: String,
        span: Option<Span>,
    },

    /// A name was used but not defined.
    #[error("undefined name '{name}'")]
    UndefinedName {
        name: String,
        span: Option<Span>,
    },

    /// A name was defined multiple times.
    #[error("duplicate definition of '{name}'")]
    DuplicateDefinition {
        name: String,
        span: Option<Span>,
        first_span: Option<Span>,
    },

    /// Type mismatch in a connection.
    #[error("cannot connect '{left}' to '{right}': incompatible types")]
    TypeMismatch {
        left: String,
        right: String,
        span: Option<Span>,
    },

    /// Invalid connection endpoint.
    #[error("'{name}' is not connectable")]
    NotConnectable {
        name: String,
        span: Option<Span>,
    },

    /// Base type not found for inheritance.
    #[error("base type '{name}' not found")]
    BaseTypeNotFound {
        name: String,
        span: Option<Span>,
    },

    /// Cyclic inheritance detected.
    #[error("cyclic inheritance detected: {chain}")]
    CyclicInheritance {
        chain: String,
        span: Option<Span>,
    },

    /// Invalid field access.
    #[error("'{field}' is not a field of '{parent}'")]
    InvalidFieldAccess {
        field: String,
        parent: String,
        span: Option<Span>,
    },

    /// Invalid array index.
    #[error("array index {index} out of bounds (size is {size})")]
    IndexOutOfBounds {
        index: u32,
        size: u32,
        span: Option<Span>,
    },

    /// Invalid for loop iterable.
    #[error("'{name}' is not iterable")]
    NotIterable {
        name: String,
        span: Option<Span>,
    },

    /// Parse error during import.
    #[error("parse error in '{file}': {message}")]
    ParseError {
        file: String,
        message: String,
    },

    /// IO error.
    #[error("IO error: {message}")]
    IoError {
        message: String,
    },
}

impl SemaError {
    /// Create an unresolved import error.
    pub fn unresolved_import(name: impl Into<String>, span: impl Into<Option<Span>>) -> Self {
        Self::UnresolvedImport {
            name: name.into(),
            span: span.into(),
        }
    }

    /// Create a file not found error.
    pub fn file_not_found(path: impl Into<String>, span: impl Into<Option<Span>>) -> Self {
        Self::FileNotFound {
            path: path.into(),
            span: span.into(),
        }
    }

    /// Create an undefined name error.
    pub fn undefined_name(name: impl Into<String>, span: impl Into<Option<Span>>) -> Self {
        Self::UndefinedName {
            name: name.into(),
            span: span.into(),
        }
    }

    /// Create a duplicate definition error.
    pub fn duplicate_definition(
        name: impl Into<String>,
        span: impl Into<Option<Span>>,
        first_span: impl Into<Option<Span>>,
    ) -> Self {
        Self::DuplicateDefinition {
            name: name.into(),
            span: span.into(),
            first_span: first_span.into(),
        }
    }

    /// Create a type mismatch error.
    pub fn type_mismatch(
        left: impl Into<String>,
        right: impl Into<String>,
        span: impl Into<Option<Span>>,
    ) -> Self {
        Self::TypeMismatch {
            left: left.into(),
            right: right.into(),
            span: span.into(),
        }
    }

    /// Create a not connectable error.
    pub fn not_connectable(name: impl Into<String>, span: impl Into<Option<Span>>) -> Self {
        Self::NotConnectable {
            name: name.into(),
            span: span.into(),
        }
    }

    /// Create a base type not found error.
    pub fn base_not_found(name: impl Into<String>, span: impl Into<Option<Span>>) -> Self {
        Self::BaseTypeNotFound {
            name: name.into(),
            span: span.into(),
        }
    }

    /// Create an invalid field access error.
    pub fn invalid_field_access(
        field: impl Into<String>,
        parent: impl Into<String>,
        span: impl Into<Option<Span>>,
    ) -> Self {
        Self::InvalidFieldAccess {
            field: field.into(),
            parent: parent.into(),
            span: span.into(),
        }
    }

    /// Create an index out of bounds error.
    pub fn index_out_of_bounds(index: u32, size: u32, span: impl Into<Option<Span>>) -> Self {
        Self::IndexOutOfBounds {
            index,
            size,
            span: span.into(),
        }
    }

    /// Create a not iterable error.
    pub fn not_iterable(name: impl Into<String>, span: impl Into<Option<Span>>) -> Self {
        Self::NotIterable {
            name: name.into(),
            span: span.into(),
        }
    }

    /// Get the source span for this error.
    pub fn span(&self) -> Option<Span> {
        match self {
            SemaError::UnresolvedImport { span, .. } => *span,
            SemaError::FileNotFound { span, .. } => *span,
            SemaError::UndefinedName { span, .. } => *span,
            SemaError::DuplicateDefinition { span, .. } => *span,
            SemaError::TypeMismatch { span, .. } => *span,
            SemaError::NotConnectable { span, .. } => *span,
            SemaError::BaseTypeNotFound { span, .. } => *span,
            SemaError::CyclicInheritance { span, .. } => *span,
            SemaError::InvalidFieldAccess { span, .. } => *span,
            SemaError::IndexOutOfBounds { span, .. } => *span,
            SemaError::NotIterable { span, .. } => *span,
            SemaError::ParseError { .. } => None,
            SemaError::IoError { .. } => None,
        }
    }
}

/// Result type for semantic analysis operations.
pub type SemaResult<T> = Result<T, SemaError>;

/// A collection of semantic errors.
#[derive(Debug, Default)]
pub struct ErrorCollector {
    errors: Vec<SemaError>,
}

impl ErrorCollector {
    /// Create a new error collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an error to the collection.
    pub fn push(&mut self, error: SemaError) {
        self.errors.push(error);
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get the number of errors.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Check if the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Consume the collector and return the errors.
    pub fn into_errors(self) -> Vec<SemaError> {
        self.errors
    }

    /// Get a reference to the errors.
    pub fn errors(&self) -> &[SemaError] {
        &self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = SemaError::undefined_name("foo", None);
        assert!(err.to_string().contains("foo"));
    }

    #[test]
    fn test_error_collector() {
        let mut collector = ErrorCollector::new();
        assert!(collector.is_empty());

        collector.push(SemaError::undefined_name("x", None));
        collector.push(SemaError::undefined_name("y", None));

        assert_eq!(collector.len(), 2);
        assert!(collector.has_errors());
    }
}
