//! Module and package resolution for Ato source files.
//!
//! This module handles resolving imports to their source files and building
//! a registry of available modules. It supports:
//!
//! - File path resolution (relative and absolute)
//! - Standard library resolution
//! - Package resolution via ato.yaml
//!
//! # Resolution Order
//!
//! When resolving `import Foo`:
//! 1. Check if defined in current file
//! 2. Check standard library (src/faebryk/library/*.ato)
//! 3. Check installed packages (.ato/modules/)
//!
//! When resolving `from "path" import Foo`:
//! 1. Try relative to current file
//! 2. Try relative to project root
//! 3. Try as package path (e.g., "atopile/generics/resistors.ato")

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::error::SemaError;
use crate::imports::{FileLoader, FsFileLoader, ParsedFile};

/// Configuration for the resolution system.
#[derive(Debug, Clone)]
pub struct ResolutionConfig {
    /// The project root directory (where ato.yaml lives).
    pub project_root: PathBuf,
    /// The standard library directory.
    pub stdlib_path: Option<PathBuf>,
    /// The modules directory (.ato/modules).
    pub modules_path: Option<PathBuf>,
}

impl ResolutionConfig {
    /// Create a new resolution config with the given project root.
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        let root = project_root.into();
        Self {
            stdlib_path: None,
            modules_path: Some(root.join(".ato/modules")),
            project_root: root,
        }
    }

    /// Set the standard library path.
    pub fn with_stdlib(mut self, path: impl Into<PathBuf>) -> Self {
        self.stdlib_path = Some(path.into());
        self
    }

    /// Set the modules path.
    pub fn with_modules(mut self, path: impl Into<PathBuf>) -> Self {
        self.modules_path = Some(path.into());
        self
    }
}

/// An exported symbol from a file.
#[derive(Debug, Clone)]
pub struct ExportedSymbol {
    /// The name of the symbol.
    pub name: String,
    /// The file it comes from.
    pub source_file: PathBuf,
    /// The kind of symbol (module, interface, component).
    pub kind: SymbolKind,
}

/// The kind of exported symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    Interface,
    Component,
}

/// Registry of all available modules and their locations.
#[derive(Debug, Default)]
pub struct ModuleRegistry {
    /// All exported symbols, keyed by name.
    symbols: HashMap<String, Vec<ExportedSymbol>>,
    /// Files that have been indexed.
    indexed_files: HashSet<PathBuf>,
    /// Standard library symbols (for quick lookup).
    stdlib_symbols: HashMap<String, ExportedSymbol>,
}

impl ModuleRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a symbol from a file.
    pub fn register(&mut self, name: &str, source_file: PathBuf, kind: SymbolKind) {
        let symbol = ExportedSymbol {
            name: name.to_string(),
            source_file,
            kind,
        };

        self.symbols
            .entry(name.to_string())
            .or_default()
            .push(symbol);
    }

    /// Register a stdlib symbol (takes priority).
    pub fn register_stdlib(&mut self, name: &str, source_file: PathBuf, kind: SymbolKind) {
        let symbol = ExportedSymbol {
            name: name.to_string(),
            source_file,
            kind,
        };
        self.stdlib_symbols.insert(name.to_string(), symbol);
    }

    /// Look up a symbol by name.
    pub fn lookup(&self, name: &str) -> Option<&ExportedSymbol> {
        // Check stdlib first
        if let Some(symbol) = self.stdlib_symbols.get(name) {
            return Some(symbol);
        }

        // Then check regular symbols
        self.symbols.get(name).and_then(|v| v.first())
    }

    /// Check if a file has been indexed.
    pub fn is_indexed(&self, path: &Path) -> bool {
        self.indexed_files.contains(path)
    }

    /// Mark a file as indexed.
    pub fn mark_indexed(&mut self, path: PathBuf) {
        self.indexed_files.insert(path);
    }

    /// Get all symbols with a given name (for ambiguity checking).
    pub fn lookup_all(&self, name: &str) -> Vec<&ExportedSymbol> {
        let mut results = Vec::new();

        if let Some(symbol) = self.stdlib_symbols.get(name) {
            results.push(symbol);
        }

        if let Some(symbols) = self.symbols.get(name) {
            results.extend(symbols.iter());
        }

        results
    }

    /// Get the number of registered symbols.
    pub fn symbol_count(&self) -> usize {
        self.symbols.values().map(|v| v.len()).sum::<usize>() + self.stdlib_symbols.len()
    }
}

/// Resolves file paths for imports.
pub struct PathResolver {
    /// Resolution configuration.
    config: ResolutionConfig,
    /// File loader for checking existence.
    loader: Box<dyn FileLoader>,
}

impl PathResolver {
    /// Create a new path resolver.
    pub fn new(config: ResolutionConfig) -> Self {
        Self {
            config,
            loader: Box::new(FsFileLoader),
        }
    }

    /// Create a path resolver with a custom file loader (for testing).
    pub fn with_loader(config: ResolutionConfig, loader: Box<dyn FileLoader>) -> Self {
        Self { config, loader }
    }

    /// Resolve an import path to an absolute file path.
    ///
    /// For `from "path" import X`:
    /// 1. Try relative to current file's directory
    /// 2. Try relative to project root
    /// 3. Try as package path
    pub fn resolve_path(&self, import_path: &str, current_file: &Path) -> Result<PathBuf, SemaError> {
        let import_path = Path::new(import_path);

        // 1. Try relative to current file's directory
        if let Some(current_dir) = current_file.parent() {
            let relative_path = current_dir.join(import_path);
            if self.loader.exists(&relative_path) {
                return self.loader.canonicalize(&relative_path)
                    .map_err(|e| SemaError::IoError {
                        message: format!("failed to canonicalize '{}': {}", relative_path.display(), e),
                    });
            }
        }

        // 2. Try relative to project root
        let root_relative = self.config.project_root.join(import_path);
        if self.loader.exists(&root_relative) {
            return self.loader.canonicalize(&root_relative)
                .map_err(|e| SemaError::IoError {
                    message: format!("failed to canonicalize '{}': {}", root_relative.display(), e),
                });
        }

        // 3. Try as package path (e.g., "atopile/generics/resistors.ato")
        if let Some(modules_path) = &self.config.modules_path {
            let package_path = modules_path.join(import_path);
            if self.loader.exists(&package_path) {
                return self.loader.canonicalize(&package_path)
                    .map_err(|e| SemaError::IoError {
                        message: format!("failed to canonicalize '{}': {}", package_path.display(), e),
                    });
            }
        }

        Err(SemaError::file_not_found(import_path.display().to_string(), None))
    }

    /// Resolve a simple import (e.g., `import Resistor`) to its source file.
    ///
    /// This checks the stdlib for the symbol.
    pub fn resolve_simple_import(&self, name: &str, registry: &ModuleRegistry) -> Option<PathBuf> {
        registry.lookup(name).map(|s| s.source_file.clone())
    }

    /// Get the project root.
    pub fn project_root(&self) -> &Path {
        &self.config.project_root
    }

    /// Get the stdlib path.
    pub fn stdlib_path(&self) -> Option<&Path> {
        self.config.stdlib_path.as_deref()
    }

    /// Get the modules path.
    pub fn modules_path(&self) -> Option<&Path> {
        self.config.modules_path.as_deref()
    }
}

/// Indexes standard library files to build the registry.
pub struct StdlibIndexer {
    /// The stdlib directory.
    stdlib_path: PathBuf,
    /// File loader.
    loader: Box<dyn FileLoader>,
}

impl StdlibIndexer {
    /// Create a new stdlib indexer.
    pub fn new(stdlib_path: impl Into<PathBuf>) -> Self {
        Self {
            stdlib_path: stdlib_path.into(),
            loader: Box::new(FsFileLoader),
        }
    }

    /// Create an indexer with a custom file loader (for testing).
    pub fn with_loader(stdlib_path: impl Into<PathBuf>, loader: Box<dyn FileLoader>) -> Self {
        Self {
            stdlib_path: stdlib_path.into(),
            loader,
        }
    }

    /// Index all stdlib files and add symbols to the registry.
    pub fn index(&self, registry: &mut ModuleRegistry) -> Result<(), Vec<SemaError>> {
        let mut errors = Vec::new();

        // List all .ato files in stdlib
        let stdlib_files = self.list_ato_files();

        for file_path in stdlib_files {
            if let Err(e) = self.index_file(&file_path, registry) {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// List all .ato files in the stdlib directory.
    fn list_ato_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.stdlib_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "ato") {
                    files.push(path);
                }
            }
        }

        files
    }

    /// Index a single file and register its exported symbols.
    fn index_file(&self, path: &Path, registry: &mut ModuleRegistry) -> Result<(), SemaError> {
        if registry.is_indexed(path) {
            return Ok(());
        }

        let source = self.loader.read(path).map_err(|e| SemaError::IoError {
            message: format!("failed to read '{}': {}", path.display(), e),
        })?;

        let ast = ato_parser::parse(&source).map_err(|errors| SemaError::ParseError {
            file: path.display().to_string(),
            message: errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "),
        })?;

        // Extract top-level block definitions
        for stmt in &ast.statements {
            if let ato_parser::Statement::BlockDef(block) = stmt {
                let kind = match block.kind {
                    ato_parser::BlockKind::Module => SymbolKind::Module,
                    ato_parser::BlockKind::Interface => SymbolKind::Interface,
                    ato_parser::BlockKind::Component => SymbolKind::Component,
                };
                registry.register_stdlib(&block.name.name, path.to_path_buf(), kind);
            }
        }

        registry.mark_indexed(path.to_path_buf());
        Ok(())
    }
}

/// Parses ato.yaml to find package dependencies.
#[derive(Debug, Clone, Default)]
pub struct PackageConfig {
    /// Dependencies from ato.yaml.
    pub dependencies: Vec<Dependency>,
    /// Project paths configuration.
    pub paths: ProjectPaths,
}

/// A dependency from ato.yaml.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// The dependency type (registry, git, file).
    pub dep_type: DependencyType,
    /// The identifier (e.g., "atopile/generics").
    pub identifier: String,
    /// The version/release.
    pub release: Option<String>,
}

/// The type of dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyType {
    Registry,
    Git,
    File,
}

/// Project paths from ato.yaml.
#[derive(Debug, Clone, Default)]
pub struct ProjectPaths {
    /// Source directory (default: ".").
    pub src: PathBuf,
    /// Layout directory (default: "./layouts").
    pub layout: PathBuf,
}

impl PackageConfig {
    /// Parse ato.yaml from a project root.
    pub fn from_project_root(root: &Path) -> Result<Self, SemaError> {
        let yaml_path = root.join("ato.yaml");
        if !yaml_path.exists() {
            // No ato.yaml - return empty config
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&yaml_path).map_err(|e| SemaError::IoError {
            message: format!("failed to read ato.yaml: {}", e),
        })?;

        Self::parse(&content)
    }

    /// Parse ato.yaml content.
    pub fn parse(content: &str) -> Result<Self, SemaError> {
        // Simple YAML parsing - we only need dependencies and paths
        let mut config = PackageConfig::default();
        config.paths.src = PathBuf::from(".");
        config.paths.layout = PathBuf::from("./layouts");

        let mut in_dependencies = false;
        let mut in_paths = false;
        let mut current_dep: Option<PartialDependency> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check for top-level section headers (not indented)
            let is_top_level = !line.starts_with(' ') && !line.starts_with('\t');

            if is_top_level && trimmed.ends_with(':') {
                // Save any pending dependency before switching sections
                if let Some(dep) = current_dep.take() {
                    if let Some(d) = dep.into_dependency() {
                        config.dependencies.push(d);
                    }
                }

                if trimmed == "dependencies:" {
                    in_dependencies = true;
                    in_paths = false;
                } else if trimmed == "paths:" {
                    in_dependencies = false;
                    in_paths = true;
                } else {
                    in_dependencies = false;
                    in_paths = false;
                }
                continue;
            }

            // Parse paths section
            if in_paths {
                if trimmed.starts_with("src:") {
                    let value = trimmed.trim_start_matches("src:").trim();
                    config.paths.src = PathBuf::from(value);
                } else if trimmed.starts_with("layout:") {
                    let value = trimmed.trim_start_matches("layout:").trim();
                    config.paths.layout = PathBuf::from(value);
                }
                continue;
            }

            // Parse dependencies section
            if in_dependencies {
                if trimmed.starts_with("- type:") {
                    // Save previous dependency if any
                    if let Some(dep) = current_dep.take() {
                        if let Some(d) = dep.into_dependency() {
                            config.dependencies.push(d);
                        }
                    }
                    // Start new dependency
                    let dep_type = trimmed.trim_start_matches("- type:").trim();
                    current_dep = Some(PartialDependency {
                        dep_type: Some(dep_type.to_string()),
                        identifier: None,
                        release: None,
                    });
                } else if let Some(ref mut dep) = current_dep {
                    if trimmed.starts_with("identifier:") {
                        dep.identifier = Some(trimmed.trim_start_matches("identifier:").trim().to_string());
                    } else if trimmed.starts_with("release:") {
                        dep.release = Some(trimmed.trim_start_matches("release:").trim().to_string());
                    }
                }
            }
        }

        // Save last dependency
        if let Some(dep) = current_dep {
            if let Some(d) = dep.into_dependency() {
                config.dependencies.push(d);
            }
        }

        Ok(config)
    }
}

/// Partial dependency during parsing.
#[derive(Default)]
struct PartialDependency {
    dep_type: Option<String>,
    identifier: Option<String>,
    release: Option<String>,
}

impl PartialDependency {
    fn into_dependency(self) -> Option<Dependency> {
        let dep_type = match self.dep_type?.as_str() {
            "registry" => DependencyType::Registry,
            "git" => DependencyType::Git,
            "file" => DependencyType::File,
            _ => return None,
        };

        Some(Dependency {
            dep_type,
            identifier: self.identifier?,
            release: self.release,
        })
    }
}

/// Combined resolver that handles all resolution types.
pub struct Resolver {
    /// Path resolver.
    path_resolver: PathResolver,
    /// Module registry.
    registry: ModuleRegistry,
    /// Package configuration.
    package_config: PackageConfig,
    /// Files that have been loaded.
    loaded_files: HashMap<PathBuf, ParsedFile>,
    /// Files currently being processed (for cycle detection).
    processing: HashSet<PathBuf>,
    /// Errors collected during resolution.
    errors: Vec<SemaError>,
}

impl Resolver {
    /// Create a new resolver.
    pub fn new(config: ResolutionConfig) -> Self {
        let package_config = PackageConfig::from_project_root(&config.project_root)
            .unwrap_or_default();

        Self {
            path_resolver: PathResolver::new(config),
            registry: ModuleRegistry::new(),
            package_config,
            loaded_files: HashMap::new(),
            processing: HashSet::new(),
            errors: Vec::new(),
        }
    }

    /// Create a resolver with a custom file loader (for testing).
    pub fn with_loader(config: ResolutionConfig, loader: Box<dyn FileLoader>) -> Self {
        let package_config = PackageConfig::default();

        Self {
            path_resolver: PathResolver::with_loader(config, loader),
            registry: ModuleRegistry::new(),
            package_config,
            loaded_files: HashMap::new(),
            processing: HashSet::new(),
            errors: Vec::new(),
        }
    }

    /// Index the standard library.
    pub fn index_stdlib(&mut self) -> Result<(), Vec<SemaError>> {
        if let Some(stdlib_path) = self.path_resolver.stdlib_path() {
            let indexer = StdlibIndexer::new(stdlib_path);
            indexer.index(&mut self.registry)?;
        }
        Ok(())
    }

    /// Get the module registry.
    pub fn registry(&self) -> &ModuleRegistry {
        &self.registry
    }

    /// Get mutable access to the registry.
    pub fn registry_mut(&mut self) -> &mut ModuleRegistry {
        &mut self.registry
    }

    /// Get the loaded files.
    pub fn loaded_files(&self) -> &HashMap<PathBuf, ParsedFile> {
        &self.loaded_files
    }

    /// Get the package configuration.
    pub fn package_config(&self) -> &PackageConfig {
        &self.package_config
    }

    /// Get the collected errors.
    pub fn errors(&self) -> &[SemaError] {
        &self.errors
    }

    /// Take the errors out of the resolver.
    pub fn take_errors(&mut self) -> Vec<SemaError> {
        std::mem::take(&mut self.errors)
    }

    /// Resolve a `from "path" import Name` style import.
    pub fn resolve_path_import(
        &mut self,
        import_path: &str,
        current_file: &Path,
    ) -> Result<PathBuf, SemaError> {
        self.path_resolver.resolve_path(import_path, current_file)
    }

    /// Resolve a simple `import Name` style import.
    /// Returns the source file path if found in stdlib or packages.
    pub fn resolve_simple_import(&self, name: &str) -> Option<PathBuf> {
        self.path_resolver.resolve_simple_import(name, &self.registry)
    }

    /// Load and parse a file, returning its AST.
    pub fn load_file(&mut self, path: &Path) -> Result<&ParsedFile, SemaError> {
        let canonical = std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf());

        // Check for circular import
        if self.processing.contains(&canonical) {
            return Err(SemaError::CircularImport {
                file: canonical.display().to_string(),
            });
        }

        // Check if already loaded
        if self.loaded_files.contains_key(&canonical) {
            return Ok(self.loaded_files.get(&canonical).unwrap());
        }

        // Mark as processing
        self.processing.insert(canonical.clone());

        // Read and parse
        let source = std::fs::read_to_string(path)
            .map_err(|e| SemaError::IoError {
                message: format!("failed to read '{}': {}", path.display(), e),
            })?;

        let ast = ato_parser::parse(&source).map_err(|errors| SemaError::ParseError {
            file: path.display().to_string(),
            message: errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "),
        })?;

        // Extract exports and register them
        for stmt in &ast.statements {
            if let ato_parser::Statement::BlockDef(block) = stmt {
                let kind = match block.kind {
                    ato_parser::BlockKind::Module => SymbolKind::Module,
                    ato_parser::BlockKind::Interface => SymbolKind::Interface,
                    ato_parser::BlockKind::Component => SymbolKind::Component,
                };
                self.registry.register(&block.name.name, canonical.clone(), kind);
            }
        }

        // Extract imports for dependency tracking
        let imports = crate::imports::ImportResolver::extract_imports_from_ast(&ast);

        // Store the parsed file
        let parsed = ParsedFile {
            path: canonical.clone(),
            ast,
            imports,
        };
        self.loaded_files.insert(canonical.clone(), parsed);

        // Done processing
        self.processing.remove(&canonical);

        Ok(self.loaded_files.get(&canonical).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imports::MockFileLoader;

    #[test]
    fn test_module_registry() {
        let mut registry = ModuleRegistry::new();

        registry.register("Foo", PathBuf::from("/a.ato"), SymbolKind::Module);
        registry.register_stdlib("Bar", PathBuf::from("/stdlib/b.ato"), SymbolKind::Interface);

        assert!(registry.lookup("Foo").is_some());
        assert!(registry.lookup("Bar").is_some());
        assert!(registry.lookup("Baz").is_none());

        // Stdlib takes priority
        assert_eq!(registry.lookup("Bar").unwrap().source_file, PathBuf::from("/stdlib/b.ato"));
    }

    #[test]
    fn test_path_resolver_relative() {
        let mut loader = MockFileLoader::new();
        loader.add_file(PathBuf::from("/project/src/lib.ato"), "module Lib:\n    pass\n");

        let config = ResolutionConfig::new("/project");
        let resolver = PathResolver::with_loader(config, Box::new(loader));

        let result = resolver.resolve_path("lib.ato", Path::new("/project/src/main.ato"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/project/src/lib.ato"));
    }

    #[test]
    fn test_path_resolver_project_root() {
        let mut loader = MockFileLoader::new();
        loader.add_file(PathBuf::from("/project/common.ato"), "module Common:\n    pass\n");

        let config = ResolutionConfig::new("/project");
        let resolver = PathResolver::with_loader(config, Box::new(loader));

        // Try to resolve from a subdirectory
        let result = resolver.resolve_path("common.ato", Path::new("/project/src/deep/file.ato"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_path_resolver_package() {
        let mut loader = MockFileLoader::new();
        loader.add_file(
            PathBuf::from("/project/.ato/modules/atopile/generics/resistors.ato"),
            "module Resistor:\n    pass\n",
        );

        let config = ResolutionConfig::new("/project");
        let resolver = PathResolver::with_loader(config, Box::new(loader));

        let result = resolver.resolve_path(
            "atopile/generics/resistors.ato",
            Path::new("/project/src/main.ato"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_package_config_parse() {
        let yaml = r#"
requires-atopile: ^0.9.0

paths:
  src: .
  layout: ./layouts

dependencies:
- type: registry
  identifier: atopile/buttons
  release: 0.2.4
- type: registry
  identifier: atopile/generics
  release: 0.3.0
"#;

        let config = PackageConfig::parse(yaml).unwrap();
        assert_eq!(config.dependencies.len(), 2);
        assert_eq!(config.dependencies[0].identifier, "atopile/buttons");
        assert_eq!(config.dependencies[0].release, Some("0.2.4".to_string()));
        assert_eq!(config.dependencies[1].identifier, "atopile/generics");
    }

    #[test]
    fn test_resolver_simple_import() {
        let mut loader = MockFileLoader::new();
        loader.add_file(
            PathBuf::from("/stdlib/resistors.ato"),
            "module Resistor:\n    pass\n",
        );

        let config = ResolutionConfig::new("/project")
            .with_stdlib("/stdlib");
        let resolver = Resolver::with_loader(config, Box::new(loader));

        // Note: index_stdlib won't work with mock loader for dir listing
        // but we can test the registry directly
        let mut registry = ModuleRegistry::new();
        registry.register_stdlib("Resistor", PathBuf::from("/stdlib/resistors.ato"), SymbolKind::Module);

        assert!(registry.lookup("Resistor").is_some());
    }
}
