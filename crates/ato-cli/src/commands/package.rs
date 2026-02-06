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
}
