//! Package management commands.
//!
//! This module provides CLI commands for managing package dependencies:
//! - `ato add <package>` - Add a dependency
//! - `ato install` - Install all dependencies
//! - `ato update` - Update dependencies to latest versions
//! - `ato remove <package>` - Remove a dependency

use crate::error::{CliError, CliResult};
use ato_sema::{PackageManager, DependencySpec};
use std::path::Path;

/// Add a package dependency.
pub fn add(package: &str, project_path: Option<&Path>) -> CliResult<()> {
    let project_root = find_project_root(project_path)?;

    let mut manager = PackageManager::new(project_root)
        .map_err(|e| CliError::io(format!("failed to initialize package manager: {}", e)))?;

    manager.add_dependency(package)
        .map_err(|e| CliError::io(format!("failed to add dependency: {}", e)))?;

    println!("{} Added {}", console::style("\u{2713}").green(), package);
    Ok(())
}

/// Install all dependencies from ato.yaml.
pub fn install(project_path: Option<&Path>) -> CliResult<()> {
    let project_root = find_project_root(project_path)?;

    let mut manager = PackageManager::new(project_root)
        .map_err(|e| CliError::io(format!("failed to initialize package manager: {}", e)))?;

    let deps = manager.config().dependencies.clone();
    if deps.is_empty() {
        println!("{} No dependencies to install.",
            console::style("\u{2713}").green());
        return Ok(());
    }

    manager.install()
        .map_err(|e| CliError::io(format!("failed to install dependencies: {}", e)))?;

    Ok(())
}

/// Update all dependencies to latest versions.
pub fn update(project_path: Option<&Path>) -> CliResult<()> {
    let project_root = find_project_root(project_path)?;

    let mut manager = PackageManager::new(project_root)
        .map_err(|e| CliError::io(format!("failed to initialize package manager: {}", e)))?;

    manager.update()
        .map_err(|e| CliError::io(format!("failed to update dependencies: {}", e)))?;

    println!("{} Dependencies updated", console::style("\u{2713}").green());
    Ok(())
}

/// Remove a package dependency.
pub fn remove(package: &str, project_path: Option<&Path>) -> CliResult<()> {
    let project_root = find_project_root(project_path)?;

    let mut manager = PackageManager::new(project_root)
        .map_err(|e| CliError::io(format!("failed to initialize package manager: {}", e)))?;

    manager.remove_dependency(package)
        .map_err(|e| CliError::io(format!("failed to remove dependency: {}", e)))?;

    println!("{} Removed {}", console::style("\u{2713}").green(), package);
    Ok(())
}

/// List all installed dependencies.
pub fn list(project_path: Option<&Path>) -> CliResult<()> {
    let project_root = find_project_root(project_path)?;

    let manager = PackageManager::new(project_root)
        .map_err(|e| CliError::io(format!("failed to initialize package manager: {}", e)))?;

    let deps = manager.list_dependencies();

    if deps.is_empty() {
        println!("No dependencies installed.");
        return Ok(());
    }

    println!("Installed dependencies:\n");

    for (dep, locked) in deps {
        let identifier = dep.identifier();
        let source = match dep {
            DependencySpec::Registry { release, .. } => {
                format!("registry{}", release.as_ref().map(|r| format!("@{}", r)).unwrap_or_default())
            }
            DependencySpec::Git { url, git_ref, .. } => {
                format!("git:{}{}", url, git_ref.as_ref().map(|r| format!("#{}", r)).unwrap_or_default())
            }
            DependencySpec::File { path, .. } => {
                format!("file:{}", path.display())
            }
        };

        let resolved = locked.map(|l| l.resolved.as_str()).unwrap_or("not installed");

        println!("  {} ({})", identifier, source);
        println!("    resolved: {}", resolved);
    }

    Ok(())
}

/// Ensure all packages are installed before building.
///
/// This is the "auto-install" entry point called by `ato build`.
/// It is a no-op when:
/// - There are no dependencies in ato.yaml
/// - All declared dependencies already exist in .ato/modules/
///
/// If packages are missing, runs the full install process.
/// Errors are reported with a suggestion to run `ato install` manually.
pub fn ensure_packages_installed(project_root: &Path) -> CliResult<()> {
    let config = ato_sema::AtoConfig::load(project_root)
        .map_err(|e| CliError::io(format!("failed to load ato.yaml: {}", e)))?;

    if config.dependencies.is_empty() {
        return Ok(());
    }

    // Fast path: check if all dependencies are already present in .ato/modules/
    let modules_dir = project_root.join(".ato/modules");
    let all_present = modules_dir.exists() && config.dependencies.iter().all(|dep| {
        let id = dep.identifier();
        // The identifier is like "atopile/generics"; the symlink in .ato/modules/
        // is created as .ato/modules/<identifier> (with the slash becoming a dir).
        modules_dir.join(&id).exists()
    });

    if all_present {
        return Ok(());
    }

    // Some packages are missing — run install
    eprintln!(
        "{} Some dependencies are missing, installing...",
        console::style("!").yellow()
    );

    let mut manager = PackageManager::new(project_root.to_path_buf())
        .map_err(|e| CliError::io(format!("failed to initialize package manager: {}", e)))?;

    manager.install().map_err(|e| {
        CliError::io(format!(
            "auto-install failed: {}\n\nTry running `ato install` manually.",
            e
        ))
    })?;

    Ok(())
}

/// Find the project root directory.
fn find_project_root(project_path: Option<&Path>) -> CliResult<std::path::PathBuf> {
    if let Some(path) = project_path {
        if path.join("ato.yaml").exists() {
            return Ok(path.to_path_buf());
        }
        return Err(CliError::io(format!(
            "no ato.yaml found in {}",
            path.display()
        )));
    }

    // Search upward from current directory
    let mut current = std::env::current_dir().map_err(|e| {
        CliError::io(format!("failed to get current directory: {}", e))
    })?;

    loop {
        if current.join("ato.yaml").exists() {
            return Ok(current);
        }

        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    Err(CliError::io(
        "no ato.yaml found in current directory or any parent directory".to_string()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create ato.yaml
        fs::write(project_dir.join("ato.yaml"), "requires-atopile: ^0.9.0\n").unwrap();

        // Find from project root
        let result = find_project_root(Some(project_dir));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), project_dir);
    }

    #[test]
    fn test_find_project_root_no_config() {
        let temp = TempDir::new().unwrap();
        let result = find_project_root(Some(temp.path()));
        assert!(result.is_err());
    }

    #[test]
    fn test_ensure_packages_installed_no_deps() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create ato.yaml with no dependencies
        fs::write(
            project_dir.join("ato.yaml"),
            "requires-atopile: '^0.9.0'\n",
        )
        .unwrap();

        // Should succeed immediately (no-op)
        let result = ensure_packages_installed(project_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_packages_installed_all_present() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create ato.yaml with a registry dependency
        fs::write(
            project_dir.join("ato.yaml"),
            "requires-atopile: '^0.9.0'\n\
             dependencies:\n\
             - type: registry\n  \
               identifier: atopile/generics\n",
        )
        .unwrap();

        // Create .ato/modules/atopile/generics to simulate installed package
        let modules_dir = project_dir.join(".ato/modules/atopile/generics");
        fs::create_dir_all(&modules_dir).unwrap();

        // Should succeed (fast path: all present)
        let result = ensure_packages_installed(project_dir);
        assert!(result.is_ok());
    }
}
