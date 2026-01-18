//! Package management for Ato projects.
//!
//! This module handles:
//! - Parsing ato.yaml configuration files
//! - Downloading and caching packages from various sources
//! - Resolving package dependencies
//! - Generating lock files for reproducibility

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::SemaError;

// ============================================================================
// Configuration Types
// ============================================================================

/// The main ato.yaml configuration file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AtoConfig {
    /// Required atopile version (e.g., "^0.9.0").
    #[serde(default)]
    pub requires_atopile: Option<String>,

    /// Legacy version field (deprecated).
    #[serde(default)]
    pub ato_version: Option<String>,

    /// Package metadata (for publishing).
    #[serde(default)]
    pub package: Option<PackageMetadata>,

    /// Project paths configuration.
    #[serde(default)]
    pub paths: PathsConfig,

    /// Build targets.
    #[serde(default)]
    pub builds: HashMap<String, BuildConfig>,

    /// Project dependencies.
    #[serde(default)]
    pub dependencies: Vec<DependencySpec>,
}

/// Package metadata for publishing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    /// Package identifier (e.g., "atopile/generics").
    pub identifier: String,

    /// Package version.
    #[serde(default = "default_version")]
    pub version: String,

    /// Repository URL.
    #[serde(default)]
    pub repository: Option<String>,

    /// Package license.
    #[serde(default)]
    pub license: Option<String>,

    /// Short description.
    #[serde(default)]
    pub summary: Option<String>,

    /// README file path.
    #[serde(default)]
    pub readme: Option<String>,
}

fn default_version() -> String {
    "0.0.0".to_string()
}

/// Project paths configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// Source directory (default: ".").
    #[serde(default = "default_src")]
    pub src: PathBuf,

    /// Layout directory (default: "./layouts").
    #[serde(default = "default_layout")]
    pub layout: PathBuf,

    /// Build output directory (default: "./build").
    #[serde(default = "default_build")]
    pub build: PathBuf,
}

fn default_src() -> PathBuf {
    PathBuf::from(".")
}

fn default_layout() -> PathBuf {
    PathBuf::from("./layouts")
}

fn default_build() -> PathBuf {
    PathBuf::from("./build")
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            src: default_src(),
            layout: default_layout(),
            build: default_build(),
        }
    }
}

/// Build target configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Entry point (e.g., "main.ato:Module").
    #[serde(default)]
    pub entry: Option<String>,

    /// Excluded checks.
    #[serde(default)]
    pub exclude_checks: Vec<String>,

    /// Hide designators in output.
    #[serde(default)]
    pub hide_designators: bool,
}

/// A dependency specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DependencySpec {
    /// Registry dependency (packages.atopile.io).
    Registry {
        /// Package identifier (e.g., "atopile/generics").
        identifier: String,
        /// Version/release constraint.
        #[serde(default)]
        release: Option<String>,
    },

    /// Git repository dependency.
    Git {
        /// Git repository URL.
        #[serde(alias = "repo_url")]
        url: String,
        /// Git ref (branch, tag, or commit).
        #[serde(alias = "ref")]
        git_ref: Option<String>,
        /// Path within the repository.
        #[serde(default)]
        path: Option<PathBuf>,
        /// Package identifier.
        #[serde(default)]
        identifier: Option<String>,
    },

    /// Local file dependency.
    File {
        /// Path to the local package.
        path: PathBuf,
        /// Package identifier.
        #[serde(default)]
        identifier: Option<String>,
    },
}

impl DependencySpec {
    /// Get the package identifier.
    pub fn identifier(&self) -> String {
        match self {
            DependencySpec::Registry { identifier, .. } => identifier.clone(),
            DependencySpec::Git { identifier, url, .. } => {
                identifier.clone().unwrap_or_else(|| {
                    // Extract identifier from URL
                    url.trim_end_matches(".git")
                        .rsplit('/')
                        .take(2)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join("/")
                })
            }
            DependencySpec::File { identifier, path } => {
                identifier.clone().unwrap_or_else(|| {
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                })
            }
        }
    }

    /// Parse a dependency from a string specification.
    ///
    /// Formats:
    /// - `package-name` or `owner/package` -> Registry
    /// - `package-name@version` -> Registry with version
    /// - `git://url.git` or `git://url.git#ref` -> Git
    /// - `file://path` -> File
    pub fn from_str(spec: &str) -> Result<Self, SemaError> {
        if spec.starts_with("git://") {
            let rest = spec.trim_start_matches("git://");
            let (url, git_ref) = if let Some((u, r)) = rest.split_once('#') {
                (u.to_string(), Some(r.to_string()))
            } else {
                (rest.to_string(), None)
            };
            Ok(DependencySpec::Git {
                url,
                git_ref,
                path: None,
                identifier: None,
            })
        } else if spec.starts_with("file://") {
            let path = spec.trim_start_matches("file://");
            Ok(DependencySpec::File {
                path: PathBuf::from(path),
                identifier: None,
            })
        } else {
            // Registry dependency
            let (identifier, release) = if let Some((id, ver)) = spec.split_once('@') {
                (id.to_string(), Some(ver.to_string()))
            } else {
                (spec.to_string(), None)
            };
            Ok(DependencySpec::Registry {
                identifier,
                release,
            })
        }
    }
}

// ============================================================================
// Lock File
// ============================================================================

/// Lock file for reproducible builds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockFile {
    /// Lock file version.
    pub version: u32,

    /// Locked dependencies.
    pub packages: Vec<LockedPackage>,
}

/// A locked package with exact version/commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    /// Package identifier.
    pub identifier: String,

    /// Package source type.
    pub source: String,

    /// Exact version or commit hash.
    pub resolved: String,

    /// Content hash for verification.
    #[serde(default)]
    pub checksum: Option<String>,

    /// Transitive dependencies.
    #[serde(default)]
    pub dependencies: Vec<String>,
}

impl LockFile {
    /// Create a new empty lock file.
    pub fn new() -> Self {
        Self {
            version: 1,
            packages: Vec::new(),
        }
    }

    /// Load a lock file from disk.
    pub fn load(path: &Path) -> Result<Self, SemaError> {
        let content = fs::read_to_string(path).map_err(|e| SemaError::IoError {
            message: format!("failed to read lock file: {}", e),
        })?;

        toml::from_str(&content).map_err(|e| SemaError::IoError {
            message: format!("failed to parse lock file: {}", e),
        })
    }

    /// Save the lock file to disk.
    pub fn save(&self, path: &Path) -> Result<(), SemaError> {
        let content = toml::to_string_pretty(self).map_err(|e| SemaError::IoError {
            message: format!("failed to serialize lock file: {}", e),
        })?;

        fs::write(path, content).map_err(|e| SemaError::IoError {
            message: format!("failed to write lock file: {}", e),
        })
    }

    /// Find a locked package by identifier.
    pub fn find(&self, identifier: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|p| p.identifier == identifier)
    }

    /// Add or update a locked package.
    pub fn upsert(&mut self, package: LockedPackage) {
        if let Some(existing) = self
            .packages
            .iter_mut()
            .find(|p| p.identifier == package.identifier)
        {
            *existing = package;
        } else {
            self.packages.push(package);
        }
    }
}

// ============================================================================
// Package Cache
// ============================================================================

/// Package cache for downloaded packages.
pub struct PackageCache {
    /// Cache directory (e.g., ~/.ato/packages/).
    cache_dir: PathBuf,
}

impl PackageCache {
    /// Create a new package cache with the default directory.
    pub fn default_cache() -> Result<Self, SemaError> {
        let cache_dir = directories::ProjectDirs::from("io", "atopile", "ato")
            .map(|d| d.cache_dir().join("packages"))
            .unwrap_or_else(|| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".ato/packages"))
                    .unwrap_or_else(|_| PathBuf::from(".ato/packages"))
            });

        fs::create_dir_all(&cache_dir).map_err(|e| SemaError::IoError {
            message: format!("failed to create package cache directory: {}", e),
        })?;

        Ok(Self { cache_dir })
    }

    /// Create a package cache with a custom directory.
    pub fn new(cache_dir: PathBuf) -> Result<Self, SemaError> {
        fs::create_dir_all(&cache_dir).map_err(|e| SemaError::IoError {
            message: format!("failed to create package cache directory: {}", e),
        })?;

        Ok(Self { cache_dir })
    }

    /// Get the cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the path for a cached package.
    pub fn package_path(&self, identifier: &str, version: &str) -> PathBuf {
        let safe_id = identifier.replace('/', "-");
        self.cache_dir.join(format!("{}-{}", safe_id, version))
    }

    /// Check if a package is cached.
    pub fn is_cached(&self, identifier: &str, version: &str) -> bool {
        self.package_path(identifier, version).exists()
    }

    /// List all cached packages.
    pub fn list_cached(&self) -> Result<Vec<(String, String)>, SemaError> {
        let mut packages = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Parse "owner-package-version" format
                    if let Some((id, version)) = name.rsplit_once('-') {
                        packages.push((id.replace('-', "/"), version.to_string()));
                    }
                }
            }
        }

        Ok(packages)
    }

    /// Clear all cached packages.
    pub fn clear_all(&self) -> Result<(), SemaError> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).map_err(|e| SemaError::IoError {
                message: format!("failed to clear package cache: {}", e),
            })?;
            fs::create_dir_all(&self.cache_dir).map_err(|e| SemaError::IoError {
                message: format!("failed to recreate package cache directory: {}", e),
            })?;
        }
        Ok(())
    }
}

// ============================================================================
// Package Manager
// ============================================================================

/// Manages package installation and resolution.
pub struct PackageManager {
    /// Project root directory.
    project_root: PathBuf,

    /// Package cache.
    cache: PackageCache,

    /// Project configuration.
    config: AtoConfig,

    /// Lock file.
    lock_file: LockFile,

    /// Modules directory (project-local).
    modules_dir: PathBuf,
}

impl PackageManager {
    /// Create a new package manager for a project.
    pub fn new(project_root: PathBuf) -> Result<Self, SemaError> {
        let config = AtoConfig::load(&project_root)?;
        let cache = PackageCache::default_cache()?;
        let modules_dir = project_root.join(".ato/modules");

        let lock_path = project_root.join("ato.lock");
        let lock_file = if lock_path.exists() {
            LockFile::load(&lock_path)?
        } else {
            LockFile::new()
        };

        Ok(Self {
            project_root,
            cache,
            config,
            lock_file,
            modules_dir,
        })
    }

    /// Get the project configuration.
    pub fn config(&self) -> &AtoConfig {
        &self.config
    }

    /// Get the modules directory.
    pub fn modules_dir(&self) -> &Path {
        &self.modules_dir
    }

    /// Install all dependencies.
    pub fn install(&mut self) -> Result<(), SemaError> {
        fs::create_dir_all(&self.modules_dir).map_err(|e| SemaError::IoError {
            message: format!("failed to create modules directory: {}", e),
        })?;

        for dep in &self.config.dependencies.clone() {
            self.install_dependency(dep)?;
        }

        // Save lock file
        let lock_path = self.project_root.join("ato.lock");
        self.lock_file.save(&lock_path)?;

        Ok(())
    }

    /// Install a single dependency.
    fn install_dependency(&mut self, dep: &DependencySpec) -> Result<PathBuf, SemaError> {
        let identifier = dep.identifier();

        match dep {
            DependencySpec::Registry { release, .. } => {
                self.install_registry_package(&identifier, release.as_deref())
            }
            DependencySpec::Git { url, git_ref, path, .. } => {
                self.install_git_package(&identifier, url, git_ref.as_deref(), path.as_deref())
            }
            DependencySpec::File { path, .. } => {
                self.install_file_package(&identifier, path)
            }
        }
    }

    /// Install a package from the registry.
    fn install_registry_package(
        &mut self,
        identifier: &str,
        release: Option<&str>,
    ) -> Result<PathBuf, SemaError> {
        let version = release.unwrap_or("latest");

        // Check if already in lock file
        if let Some(locked) = self.lock_file.find(identifier) {
            let cached_path = self.cache.package_path(identifier, &locked.resolved);
            if cached_path.exists() {
                // Link to modules directory
                let target = self.modules_dir.join(identifier);
                self.link_package(&cached_path, &target)?;
                return Ok(target);
            }
        }

        // Download from registry (placeholder - would use actual API)
        // For now, we'll try to clone from GitHub as a fallback
        let github_url = format!("https://github.com/{}.git", identifier);
        self.install_git_package(identifier, &github_url, None, None)
    }

    /// Install a package from a git repository.
    fn install_git_package(
        &mut self,
        identifier: &str,
        url: &str,
        git_ref: Option<&str>,
        subpath: Option<&Path>,
    ) -> Result<PathBuf, SemaError> {
        use sha2::{Digest, Sha256};

        // Generate a unique cache key based on URL and ref
        let cache_key = format!("{}@{}", url, git_ref.unwrap_or("HEAD"));
        let mut hasher = Sha256::new();
        hasher.update(cache_key.as_bytes());
        let hash = hex::encode(&hasher.finalize()[..8]);
        let version = git_ref.unwrap_or(&hash);

        let cache_path = self.cache.package_path(identifier, version);

        // Clone if not cached
        if !cache_path.exists() {
            self.clone_repository(url, &cache_path, git_ref)?;
        }

        // Determine source path (may be a subpath within the repo)
        let source_path = if let Some(sub) = subpath {
            cache_path.join(sub)
        } else {
            cache_path.clone()
        };

        // Link to modules directory
        let target = self.modules_dir.join(identifier);
        self.link_package(&source_path, &target)?;

        // Update lock file
        let resolved = self.get_git_commit(&cache_path).unwrap_or_else(|_| version.to_string());
        self.lock_file.upsert(LockedPackage {
            identifier: identifier.to_string(),
            source: "git".to_string(),
            resolved,
            checksum: None,
            dependencies: Vec::new(),
        });

        Ok(target)
    }

    /// Install a package from a local file path.
    fn install_file_package(
        &mut self,
        identifier: &str,
        path: &Path,
    ) -> Result<PathBuf, SemaError> {
        let source_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
        };

        if !source_path.exists() {
            return Err(SemaError::file_not_found(
                source_path.display().to_string(),
                None,
            ));
        }

        // Link to modules directory
        let target = self.modules_dir.join(identifier);
        self.link_package(&source_path, &target)?;

        // Update lock file
        self.lock_file.upsert(LockedPackage {
            identifier: identifier.to_string(),
            source: "file".to_string(),
            resolved: source_path.display().to_string(),
            checksum: None,
            dependencies: Vec::new(),
        });

        Ok(target)
    }

    /// Clone a git repository.
    fn clone_repository(
        &self,
        url: &str,
        target: &Path,
        git_ref: Option<&str>,
    ) -> Result<(), SemaError> {
        // First try using system git command (handles credentials better)
        if let Ok(status) = std::process::Command::new("git")
            .args(["clone", "--depth", "1"])
            .args(git_ref.map(|r| vec!["--branch", r]).unwrap_or_default())
            .arg(url)
            .arg(target)
            .status()
        {
            if status.success() {
                return Ok(());
            }
        }

        // Fall back to git2 with proper credential handling
        let mut callbacks = git2::RemoteCallbacks::new();

        // Try to use credential helper from git config
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            // First try SSH agent
            if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                if let Some(username) = username_from_url {
                    return git2::Cred::ssh_key_from_agent(username);
                }
            }

            // Try default credentials (for public repos)
            if allowed_types.contains(git2::CredentialType::DEFAULT) {
                return git2::Cred::default();
            }

            // Try git credential helper
            if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                if let Some(username) = username_from_url {
                    // For public repos, use empty password
                    return git2::Cred::userpass_plaintext(username, "");
                }
            }

            Err(git2::Error::from_str("no credentials available"))
        });

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);

        match builder.clone(url, target) {
            Ok(repo) => {
                // Checkout specific ref if provided
                if let Some(ref_name) = git_ref {
                    let obj = repo.revparse_single(ref_name).map_err(|e| SemaError::IoError {
                        message: format!("failed to find ref '{}': {}", ref_name, e),
                    })?;
                    repo.checkout_tree(&obj, None).map_err(|e| SemaError::IoError {
                        message: format!("failed to checkout ref '{}': {}", ref_name, e),
                    })?;
                    repo.set_head_detached(obj.id()).map_err(|e| SemaError::IoError {
                        message: format!("failed to set HEAD: {}", e),
                    })?;
                }
                Ok(())
            }
            Err(e) => Err(SemaError::IoError {
                message: format!("failed to clone repository '{}': {}", url, e),
            }),
        }
    }

    /// Get the current commit hash of a git repository.
    fn get_git_commit(&self, repo_path: &Path) -> Result<String, SemaError> {
        let repo = git2::Repository::open(repo_path).map_err(|e| SemaError::IoError {
            message: format!("failed to open repository: {}", e),
        })?;

        let head = repo.head().map_err(|e| SemaError::IoError {
            message: format!("failed to get HEAD: {}", e),
        })?;

        let commit = head.peel_to_commit().map_err(|e| SemaError::IoError {
            message: format!("failed to get commit: {}", e),
        })?;

        Ok(commit.id().to_string())
    }

    /// Link a package to the modules directory.
    fn link_package(&self, source: &Path, target: &Path) -> Result<(), SemaError> {
        // Create parent directories
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| SemaError::IoError {
                message: format!("failed to create parent directory: {}", e),
            })?;
        }

        // Remove existing link/directory
        if target.exists() || target.is_symlink() {
            if target.is_dir() && !target.is_symlink() {
                fs::remove_dir_all(target).map_err(|e| SemaError::IoError {
                    message: format!("failed to remove existing directory: {}", e),
                })?;
            } else {
                fs::remove_file(target).map_err(|e| SemaError::IoError {
                    message: format!("failed to remove existing file/link: {}", e),
                })?;
            }
        }

        // Create symlink (Unix) or copy directory (Windows fallback)
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source, target).map_err(|e| SemaError::IoError {
                message: format!("failed to create symlink: {}", e),
            })?;
        }

        #[cfg(windows)]
        {
            // On Windows, copy the directory instead
            self.copy_dir_recursive(source, target)?;
        }

        Ok(())
    }

    /// Copy a directory recursively (for Windows fallback).
    #[cfg(windows)]
    fn copy_dir_recursive(&self, source: &Path, target: &Path) -> Result<(), SemaError> {
        fs::create_dir_all(target).map_err(|e| SemaError::IoError {
            message: format!("failed to create directory: {}", e),
        })?;

        for entry in fs::read_dir(source).map_err(|e| SemaError::IoError {
            message: format!("failed to read directory: {}", e),
        })? {
            let entry = entry.map_err(|e| SemaError::IoError {
                message: format!("failed to read entry: {}", e),
            })?;
            let source_path = entry.path();
            let target_path = target.join(entry.file_name());

            if source_path.is_dir() {
                self.copy_dir_recursive(&source_path, &target_path)?;
            } else {
                fs::copy(&source_path, &target_path).map_err(|e| SemaError::IoError {
                    message: format!("failed to copy file: {}", e),
                })?;
            }
        }

        Ok(())
    }

    /// Add a dependency to the project.
    pub fn add_dependency(&mut self, spec: &str) -> Result<(), SemaError> {
        let dep = DependencySpec::from_str(spec)?;

        // Check if already exists
        let identifier = dep.identifier();
        if self.config.dependencies.iter().any(|d| d.identifier() == identifier) {
            // Update existing dependency
            self.config.dependencies.retain(|d| d.identifier() != identifier);
        }

        // Add new dependency
        self.config.dependencies.push(dep.clone());

        // Install the dependency
        self.install_dependency(&dep)?;

        // Save updated config
        self.save_config()?;

        // Save lock file
        let lock_path = self.project_root.join("ato.lock");
        self.lock_file.save(&lock_path)?;

        Ok(())
    }

    /// Remove a dependency from the project.
    pub fn remove_dependency(&mut self, identifier: &str) -> Result<(), SemaError> {
        // Remove from config
        self.config.dependencies.retain(|d| d.identifier() != identifier);

        // Remove from modules directory
        let target = self.modules_dir.join(identifier);
        if target.exists() || target.is_symlink() {
            if target.is_dir() && !target.is_symlink() {
                fs::remove_dir_all(&target).map_err(|e| SemaError::IoError {
                    message: format!("failed to remove package directory: {}", e),
                })?;
            } else {
                fs::remove_file(&target).map_err(|e| SemaError::IoError {
                    message: format!("failed to remove package link: {}", e),
                })?;
            }
        }

        // Remove from lock file
        self.lock_file.packages.retain(|p| p.identifier != identifier);

        // Save updated config
        self.save_config()?;

        // Save lock file
        let lock_path = self.project_root.join("ato.lock");
        self.lock_file.save(&lock_path)?;

        Ok(())
    }

    /// Save the configuration back to ato.yaml.
    fn save_config(&self) -> Result<(), SemaError> {
        let yaml_path = self.project_root.join("ato.yaml");
        let content = serde_yaml::to_string(&self.config).map_err(|e| SemaError::IoError {
            message: format!("failed to serialize config: {}", e),
        })?;

        fs::write(&yaml_path, content).map_err(|e| SemaError::IoError {
            message: format!("failed to write ato.yaml: {}", e),
        })
    }

    /// Update all dependencies to their latest versions.
    pub fn update(&mut self) -> Result<(), SemaError> {
        // Clear lock file to force re-resolution
        self.lock_file = LockFile::new();

        // Re-install all dependencies
        self.install()
    }

    /// List all installed dependencies.
    pub fn list_dependencies(&self) -> Vec<(&DependencySpec, Option<&LockedPackage>)> {
        self.config
            .dependencies
            .iter()
            .map(|dep| {
                let locked = self.lock_file.find(&dep.identifier());
                (dep, locked)
            })
            .collect()
    }
}

// ============================================================================
// AtoConfig Implementation
// ============================================================================

impl AtoConfig {
    /// Load configuration from a project directory.
    pub fn load(project_root: &Path) -> Result<Self, SemaError> {
        let yaml_path = project_root.join("ato.yaml");

        if !yaml_path.exists() {
            // Return default config if no ato.yaml exists
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&yaml_path).map_err(|e| SemaError::IoError {
            message: format!("failed to read ato.yaml: {}", e),
        })?;

        serde_yaml::from_str(&content).map_err(|e| SemaError::IoError {
            message: format!("failed to parse ato.yaml: {}", e),
        })
    }

    /// Get the entry point for a build target.
    pub fn get_entry(&self, build_name: &str) -> Option<&str> {
        self.builds
            .get(build_name)
            .and_then(|b| b.entry.as_deref())
    }

    /// Get the default build entry point.
    pub fn default_entry(&self) -> Option<&str> {
        self.get_entry("default")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_spec_from_str() {
        // Registry dependency
        let dep = DependencySpec::from_str("atopile/generics").unwrap();
        assert_eq!(dep.identifier(), "atopile/generics");

        // Registry with version
        let dep = DependencySpec::from_str("atopile/generics@1.0.0").unwrap();
        match dep {
            DependencySpec::Registry { identifier, release } => {
                assert_eq!(identifier, "atopile/generics");
                assert_eq!(release, Some("1.0.0".to_string()));
            }
            _ => panic!("Expected registry dependency"),
        }

        // Git dependency
        let dep = DependencySpec::from_str("git://github.com/atopile/generics.git").unwrap();
        match dep {
            DependencySpec::Git { url, git_ref, .. } => {
                assert_eq!(url, "github.com/atopile/generics.git");
                assert!(git_ref.is_none());
            }
            _ => panic!("Expected git dependency"),
        }

        // Git with ref
        let dep = DependencySpec::from_str("git://github.com/atopile/generics.git#v1.0.0").unwrap();
        match dep {
            DependencySpec::Git { url, git_ref, .. } => {
                assert_eq!(url, "github.com/atopile/generics.git");
                assert_eq!(git_ref, Some("v1.0.0".to_string()));
            }
            _ => panic!("Expected git dependency"),
        }

        // File dependency
        let dep = DependencySpec::from_str("file://./local/package").unwrap();
        match dep {
            DependencySpec::File { path, .. } => {
                assert_eq!(path, PathBuf::from("./local/package"));
            }
            _ => panic!("Expected file dependency"),
        }
    }

    #[test]
    fn test_ato_config_parse() {
        let yaml = r#"
requires-atopile: ^0.9.0

paths:
  src: .
  layout: ./layouts

builds:
  default:
    entry: main.ato:MyModule

dependencies:
  - type: registry
    identifier: atopile/generics
    release: "1.0.0"
  - type: git
    url: https://github.com/atopile/buttons.git
    git_ref: main
"#;

        let config: AtoConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.requires_atopile, Some("^0.9.0".to_string()));
        assert_eq!(config.paths.src, PathBuf::from("."));
        assert_eq!(config.dependencies.len(), 2);

        // Check registry dependency
        match &config.dependencies[0] {
            DependencySpec::Registry { identifier, release } => {
                assert_eq!(identifier, "atopile/generics");
                assert_eq!(release, &Some("1.0.0".to_string()));
            }
            _ => panic!("Expected registry dependency"),
        }

        // Check git dependency
        match &config.dependencies[1] {
            DependencySpec::Git { url, git_ref, .. } => {
                assert_eq!(url, "https://github.com/atopile/buttons.git");
                assert_eq!(git_ref, &Some("main".to_string()));
            }
            _ => panic!("Expected git dependency"),
        }
    }

    #[test]
    fn test_lock_file_roundtrip() {
        let mut lock = LockFile::new();
        lock.upsert(LockedPackage {
            identifier: "atopile/generics".to_string(),
            source: "registry".to_string(),
            resolved: "1.0.0".to_string(),
            checksum: Some("abc123".to_string()),
            dependencies: vec!["atopile/base".to_string()],
        });

        let serialized = toml::to_string_pretty(&lock).unwrap();
        let deserialized: LockFile = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.packages.len(), 1);
        assert_eq!(deserialized.packages[0].identifier, "atopile/generics");
        assert_eq!(deserialized.packages[0].resolved, "1.0.0");
    }
}
