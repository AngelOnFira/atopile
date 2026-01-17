//! Import resolution for Ato source files.
//!
//! This module handles finding and loading imported files, building
//! the module dependency graph.

use crate::error::{ErrorCollector, SemaError};
use ato_parser::{File, ImportStmt, DepImportStmt, Statement};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Result of parsing a file for imports.
#[derive(Debug)]
pub struct ParsedFile {
    /// The file path.
    pub path: PathBuf,
    /// The parsed AST.
    pub ast: File,
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

/// Resolves imports and builds a dependency graph.
pub struct ImportResolver {
    /// The root directory for resolving relative imports.
    root_dir: PathBuf,

    /// All parsed files, keyed by canonical path.
    files: HashMap<PathBuf, ParsedFile>,

    /// Files that are currently being processed (for cycle detection).
    processing: HashSet<PathBuf>,

    /// Collected errors.
    errors: ErrorCollector,

    /// File loader (for testing - can be replaced with mock).
    loader: Box<dyn FileLoader>,
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
#[derive(Debug, Default)]
pub struct MockFileLoader {
    files: HashMap<PathBuf, String>,
}

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

impl ImportResolver {
    /// Create a new import resolver.
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
            files: HashMap::new(),
            processing: HashSet::new(),
            errors: ErrorCollector::new(),
            loader: Box::new(FsFileLoader),
        }
    }

    /// Create an import resolver with a custom file loader (for testing).
    pub fn with_loader(root_dir: impl Into<PathBuf>, loader: Box<dyn FileLoader>) -> Self {
        Self {
            root_dir: root_dir.into(),
            files: HashMap::new(),
            processing: HashSet::new(),
            errors: ErrorCollector::new(),
            loader,
        }
    }

    /// Resolve imports starting from a source file.
    pub fn resolve(&mut self, source: &str, source_path: Option<&Path>) -> Result<(), Vec<SemaError>> {
        let path = source_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.root_dir.join("<input>"));

        self.resolve_file(source, &path)?;

        if self.errors.has_errors() {
            Err(self.errors.errors().to_vec())
        } else {
            Ok(())
        }
    }

    /// Get all parsed files.
    pub fn files(&self) -> &HashMap<PathBuf, ParsedFile> {
        &self.files
    }

    /// Get the errors collected during resolution.
    pub fn errors(&self) -> &ErrorCollector {
        &self.errors
    }

    /// Take the errors out of the resolver.
    pub fn take_errors(&mut self) -> Vec<SemaError> {
        std::mem::take(&mut self.errors).into_errors()
    }

    /// Parse and resolve imports for a single file.
    fn resolve_file(&mut self, source: &str, path: &Path) -> Result<(), Vec<SemaError>> {
        let canonical_path = self.loader.canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf());

        // Check for cycles
        if self.processing.contains(&canonical_path) {
            // Already processing this file - skip to avoid infinite loop
            return Ok(());
        }

        // Check if already processed
        if self.files.contains_key(&canonical_path) {
            return Ok(());
        }

        // Mark as processing
        self.processing.insert(canonical_path.clone());

        // Parse the file
        let ast = match ato_parser::parse(source) {
            Ok(ast) => ast,
            Err(errors) => {
                self.errors.push(SemaError::ParseError {
                    file: path.display().to_string(),
                    message: errors.iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join("; "),
                });
                self.processing.remove(&canonical_path);
                return Ok(());
            }
        };

        // Extract imports
        let imports = self.extract_imports(&ast);

        // Store the parsed file
        self.files.insert(canonical_path.clone(), ParsedFile {
            path: canonical_path.clone(),
            ast,
            imports: imports.clone(),
        });

        // Resolve each import
        for import in imports {
            self.resolve_import(&import, &canonical_path);
        }

        // Done processing
        self.processing.remove(&canonical_path);

        Ok(())
    }

    /// Extract import information from an AST.
    fn extract_imports(&self, ast: &File) -> Vec<ImportInfo> {
        let mut imports = Vec::new();

        for stmt in &ast.statements {
            match stmt {
                Statement::Import(import) => {
                    self.extract_import_stmt(import, &mut imports);
                }
                Statement::DepImport(dep_import) => {
                    self.extract_dep_import_stmt(dep_import, &mut imports);
                }
                _ => {}
            }
        }

        imports
    }

    /// Extract imports from an import statement.
    fn extract_import_stmt(&self, import: &ImportStmt, imports: &mut Vec<ImportInfo>) {
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
    fn extract_dep_import_stmt(&self, import: &DepImportStmt, imports: &mut Vec<ImportInfo>) {
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

    /// Resolve a single import.
    fn resolve_import(&mut self, import: &ImportInfo, _from_file: &Path) {
        if let Some(from_path) = &import.from_path {
            // This is a `from "path" import Name` style import
            let import_path = self.resolve_import_path(from_path);

            if !self.loader.exists(&import_path) {
                self.errors.push(SemaError::file_not_found(
                    import_path.display().to_string(),
                    Some(import.span),
                ));
                return;
            }

            // Load and parse the imported file
            match self.loader.read(&import_path) {
                Ok(source) => {
                    let _ = self.resolve_file(&source, &import_path);
                }
                Err(e) => {
                    self.errors.push(SemaError::IoError {
                        message: format!("failed to read '{}': {}", import_path.display(), e),
                    });
                }
            }
        }
        // For simple `import Name` without a path, we rely on the name being
        // defined in the same file or in the stdlib. This is handled during
        // name resolution, not here.
    }

    /// Resolve an import path relative to the root directory.
    fn resolve_import_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root_dir.join(path)
        }
    }
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

        let resolver = ImportResolver::new(".");
        let imports = resolver.extract_imports(&ast);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].name, "Foo");
        assert!(imports[0].from_path.is_none());
    }

    #[test]
    fn test_extract_from_import() {
        let source = r#"from "path/to/file.ato" import Bar"#;
        let source = format!("{}\n", source);
        let ast = ato_parser::parse(&source).unwrap();

        let resolver = ImportResolver::new(".");
        let imports = resolver.extract_imports(&ast);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].name, "Bar");
        assert_eq!(imports[0].from_path, Some("path/to/file.ato".into()));
    }

    #[test]
    fn test_extract_qualified_import() {
        let source = "import Foo.Bar.Baz\n";
        let ast = ato_parser::parse(source).unwrap();

        let resolver = ImportResolver::new(".");
        let imports = resolver.extract_imports(&ast);

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

    #[test]
    fn test_resolve_with_mock() {
        let mut loader = MockFileLoader::new();
        loader.add_file(
            PathBuf::from("/root/main.ato"),
            "from \"lib.ato\" import Foo\nmodule Main:\n    pass\n",
        );
        loader.add_file(
            PathBuf::from("/root/lib.ato"),
            "module Foo:\n    pass\n",
        );

        let mut resolver = ImportResolver::with_loader(
            "/root",
            Box::new(loader),
        );

        let source = "from \"lib.ato\" import Foo\nmodule Main:\n    pass\n";
        let result = resolver.resolve(source, Some(Path::new("/root/main.ato")));

        // Should succeed
        assert!(result.is_ok(), "Resolution should succeed: {:?}", result);

        // Should have parsed both files
        assert_eq!(resolver.files().len(), 2);
    }
}
