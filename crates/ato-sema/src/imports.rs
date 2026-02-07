//! Import resolution for Ato source files.
//!
//! This module handles finding and loading imported files, building
//! the module dependency graph.

use ato_parser::{File, ImportStmt, DepImportStmt, Statement};
use std::path::{Path, PathBuf};

// Re-export public types
pub use self::types::*;

mod types {
    use std::path::PathBuf;

    /// Result of parsing a file for imports.
    #[derive(Debug)]
    pub struct ParsedFile {
        /// The file path.
        pub path: PathBuf,
        /// The parsed AST.
        pub ast: ato_parser::File,
        /// Imports from this file.
        pub imports: Vec<ImportInfo>,
    }

    /// Information about a single import.
    #[derive(Debug, Clone)]
    pub struct ImportInfo {
        /// The qualified name being imported (e.g., "Module" or "path.to.Module").
        pub name: String,
        /// The source file path (if `from "path"` was used).
        pub from_path: Option<String>,
        /// The source span of the import statement.
        pub span: ato_lexer::Span,
    }
}

/// Trait for loading files (allows mocking in tests).
pub trait FileLoader: std::fmt::Debug {
    /// Read the contents of a file.
    fn read(&self, path: &Path) -> Result<String, std::io::Error>;

    /// Check if a file exists.
    fn exists(&self, path: &Path) -> bool;

    /// Canonicalize a path.
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, std::io::Error>;
}

/// Default file loader that uses the filesystem.
#[derive(Debug)]
pub struct FsFileLoader;

impl FileLoader for FsFileLoader {
    fn read(&self, path: &Path) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, std::io::Error> {
        path.canonicalize()
    }
}

/// Mock file loader for testing.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct MockFileLoader {
    files: std::collections::HashMap<PathBuf, String>,
}

#[cfg(test)]
impl MockFileLoader {
    /// Create a new mock file loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file to the mock filesystem.
    pub fn add_file(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.files.insert(path.into(), content.into());
    }
}

#[cfg(test)]
impl FileLoader for MockFileLoader {
    fn read(&self, path: &Path) -> Result<String, std::io::Error> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"))
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, std::io::Error> {
        // For mock, just return the path as-is
        Ok(path.to_path_buf())
    }
}

/// Extract import information from an AST.
pub fn extract_imports_from_ast(ast: &File) -> Vec<ImportInfo> {
    let mut imports = Vec::new();

    for stmt in &ast.statements {
        match stmt {
            Statement::Import(import) => {
                extract_import_stmt(import, &mut imports);
            }
            Statement::DepImport(dep_import) => {
                extract_dep_import_stmt(dep_import, &mut imports);
            }
            _ => {}
        }
    }

    imports
}

/// Extract imports from an import statement.
fn extract_import_stmt(import: &ImportStmt, imports: &mut Vec<ImportInfo>) {
    let from_path = import.from_path.as_ref().map(|s| strip_string_quotes(&s.value));

    for type_ref in &import.imports {
        let name = type_ref.parts
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
            .join(".");

        imports.push(ImportInfo {
            name,
            from_path: from_path.clone(),
            span: import.span,
        });
    }
}

/// Extract imports from a deprecated import statement.
fn extract_dep_import_stmt(import: &DepImportStmt, imports: &mut Vec<ImportInfo>) {
    let name = import.type_ref.parts
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(".");

    imports.push(ImportInfo {
        name,
        from_path: Some(strip_string_quotes(&import.from_path.value)),
        span: import.span,
    });
}

/// Strip surrounding quotes from a string value.
/// The parser preserves the quotes in string literals, so we need to remove them.
fn strip_string_quotes(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len()-1].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_import() {
        let source = "import Foo\n";
        let ast = ato_parser::parse(source).unwrap();

        let imports = extract_imports_from_ast(&ast);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].name, "Foo");
        assert!(imports[0].from_path.is_none());
    }

    #[test]
    fn test_extract_from_import() {
        let source = r#"from "path/to/file.ato" import Bar"#;
        let source = format!("{}\n", source);
        let ast = ato_parser::parse(&source).unwrap();

        let imports = extract_imports_from_ast(&ast);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].name, "Bar");
        assert_eq!(imports[0].from_path, Some("path/to/file.ato".into()));
    }

    #[test]
    fn test_extract_qualified_import() {
        let source = "import Foo.Bar.Baz\n";
        let ast = ato_parser::parse(source).unwrap();

        let imports = extract_imports_from_ast(&ast);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].name, "Foo.Bar.Baz");
    }

    #[test]
    fn test_mock_file_loader() {
        let mut loader = MockFileLoader::new();
        loader.add_file(PathBuf::from("test.ato"), "module M:\n    pass\n");

        assert!(loader.exists(Path::new("test.ato")));
        assert!(!loader.exists(Path::new("nonexistent.ato")));

        let content = loader.read(Path::new("test.ato")).unwrap();
        assert!(content.contains("module M"));
    }
}
