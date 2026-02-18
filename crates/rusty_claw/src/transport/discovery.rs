//! CLI discovery and version validation
//!
//! This module provides utilities to locate the Claude Code CLI binary
//! and validate that it meets the minimum version requirements.
//!
//! # Search Strategy
//!
//! The [`CliDiscovery::find`] method searches for the CLI in the following order:
//!
//! 1. **Explicit path** - If provided as an argument
//! 2. **CLAUDE_CLI_PATH** - Environment variable
//! 3. **PATH** - Standard PATH search
//! 4. **Common locations** - Platform-specific install directories
//!
//! # Version Requirements
//!
//! The SDK requires Claude Code CLI version >= 2.0.0. The [`CliDiscovery::validate_version`]
//! method ensures the installed CLI meets this requirement.
//!
//! # Example
//!
//! ```rust,no_run
//! use rusty_claw::transport::CliDiscovery;
//! use std::path::Path;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Auto-discover CLI (searches PATH and common locations)
//! let cli_path = CliDiscovery::find(None).await?;
//! println!("Found CLI at: {}", cli_path.display());
//!
//! // Validate version >= 2.0.0
//! let version = CliDiscovery::validate_version(&cli_path).await?;
//! println!("CLI version: {}", version);
//! # Ok(())
//! # }
//! ```

use crate::error::ClawError;
use std::path::{Path, PathBuf};

/// CLI discovery utility
///
/// This struct provides static methods for locating and validating the
/// Claude Code CLI binary. All methods are async and return Results.
pub struct CliDiscovery;

impl CliDiscovery {
    /// Find the Claude Code CLI binary
    ///
    /// Searches for the CLI in the following order:
    /// 1. Explicit `cli_path` argument (if provided)
    /// 2. `CLAUDE_CLI_PATH` environment variable
    /// 3. System PATH search
    /// 4. Common installation locations
    ///
    /// # Arguments
    ///
    /// * `cli_path` - Optional explicit path to the CLI binary
    ///
    /// # Returns
    ///
    /// - `Ok(PathBuf)` - Path to the discovered CLI binary
    /// - `Err(ClawError::CliNotFound)` - CLI not found in any location
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rusty_claw::transport::CliDiscovery;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Auto-discover from PATH
    /// let cli = CliDiscovery::find(None).await?;
    ///
    /// // Use explicit path
    /// let cli = CliDiscovery::find(Some(&PathBuf::from("/opt/homebrew/bin/claude"))).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find(cli_path: Option<&Path>) -> Result<PathBuf, ClawError> {
        // 1. Check explicit cli_path argument
        if let Some(path) = cli_path {
            if path.exists() {
                return Ok(path.to_path_buf());
            }
        }

        // 2. Check CLAUDE_CLI_PATH environment variable
        if let Ok(env_path) = std::env::var("CLAUDE_CLI_PATH") {
            let path = PathBuf::from(env_path);
            if path.exists() {
                return Ok(path);
            }
        }

        // 3. Search PATH
        if let Ok(path) = Self::search_path().await {
            return Ok(path);
        }

        // 4. Check common installation locations
        let common_locations = Self::common_locations();
        for location in common_locations {
            if location.exists() {
                return Ok(location);
            }
        }

        Err(ClawError::CliNotFound)
    }

    /// Validate that the CLI version is >= 2.0.0
    ///
    /// Executes `claude --version` and parses the semantic version string.
    /// Returns an error if the version is older than 2.0.0.
    ///
    /// # Arguments
    ///
    /// * `cli` - Path to the CLI binary
    ///
    /// # Returns
    ///
    /// - `Ok(String)` - The version string (e.g., "2.1.39")
    /// - `Err(ClawError::InvalidCliVersion)` - Version < 2.0.0 or parse error
    /// - `Err(ClawError::Io)` - Failed to execute CLI
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rusty_claw::transport::CliDiscovery;
    /// use std::path::Path;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let cli_path = Path::new("/opt/homebrew/bin/claude");
    /// let version = CliDiscovery::validate_version(cli_path).await?;
    /// println!("CLI version: {}", version);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn validate_version(cli: &Path) -> Result<String, ClawError> {
        // Run `claude --version`
        let output = tokio::process::Command::new(cli)
            .arg("--version")
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ClawError::InvalidCliVersion {
                version: format!("Failed to run --version: {}", stderr),
            });
        }

        // Parse version from stdout (format: "X.Y.Z (Claude Code)")
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version_str =
            stdout
                .split_whitespace()
                .next()
                .ok_or_else(|| ClawError::InvalidCliVersion {
                    version: "empty output".to_string(),
                })?;

        // Parse semantic version
        let version =
            semver::Version::parse(version_str).map_err(|_| ClawError::InvalidCliVersion {
                version: version_str.to_string(),
            })?;

        // Ensure version >= 2.0.0
        let min_version = semver::Version::new(2, 0, 0);
        if version < min_version {
            return Err(ClawError::InvalidCliVersion {
                version: version.to_string(),
            });
        }

        Ok(version.to_string())
    }

    /// Search for `claude` in PATH
    async fn search_path() -> Result<PathBuf, ClawError> {
        // Get PATH environment variable
        let path_env = std::env::var("PATH").map_err(|_| ClawError::CliNotFound)?;

        // Split by : on Unix, ; on Windows
        let separator = if cfg!(windows) { ';' } else { ':' };

        // On Windows, search for both "claude" and "claude.exe"
        let names: &[&str] = if cfg!(windows) {
            &["claude.exe", "claude.cmd", "claude"]
        } else {
            &["claude"]
        };

        for dir in path_env.split(separator) {
            for name in names {
                let candidate = PathBuf::from(dir).join(name);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }

        Err(ClawError::CliNotFound)
    }

    /// Get common installation locations for the Claude Code CLI
    fn common_locations() -> Vec<PathBuf> {
        let mut locations = Vec::new();

        // Get home directory
        let home = std::env::var("HOME").ok();

        // macOS Homebrew
        locations.push(PathBuf::from("/opt/homebrew/bin/claude"));

        // Standard Unix locations
        locations.push(PathBuf::from("/usr/local/bin/claude"));
        locations.push(PathBuf::from("/usr/bin/claude"));

        // User-specific locations (with home expansion)
        if let Some(home_dir) = home {
            let home_path = PathBuf::from(home_dir);
            locations.push(home_path.join(".local/bin/claude"));
            locations.push(home_path.join(".npm/bin/claude"));
            locations.push(home_path.join(".claude/local/claude"));
        }

        locations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_find_with_explicit_path() {
        // Test that explicit path takes precedence
        // Use a path we know exists (current executable)
        let exe = std::env::current_exe().unwrap();
        let result = CliDiscovery::find(Some(&exe)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), exe);
    }

    #[tokio::test]
    async fn test_find_with_nonexistent_explicit_path() {
        // Test that nonexistent explicit path falls back to discovery
        let fake_path = PathBuf::from("/nonexistent/fake/claude");
        let result = CliDiscovery::find(Some(&fake_path)).await;

        // This should either find claude in PATH or fail with CliNotFound
        // We can't guarantee claude is in PATH in test environment
        match result {
            Ok(_) => {}                       // Found in PATH/common locations
            Err(ClawError::CliNotFound) => {} // Expected if no CLI installed
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_find_in_path() {
        // Test PATH search (if claude is installed)
        let result = CliDiscovery::find(None).await;

        // This test is environment-dependent
        match result {
            Ok(path) => {
                // Verify it's an absolute path
                assert!(path.is_absolute());
                // Verify the file exists
                assert!(path.exists());
            }
            Err(ClawError::CliNotFound) => {
                // Expected if claude is not installed in test environment
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_validate_version_invalid_path() {
        // Test with a path that doesn't exist
        let fake_path = Path::new("/nonexistent/fake/claude");
        let result = CliDiscovery::validate_version(fake_path).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::Io(_)));
    }

    #[tokio::test]
    async fn test_search_path_separator() {
        // Test that search_path handles PATH correctly
        // This is a basic sanity check
        let result = CliDiscovery::search_path().await;

        // Either finds claude or returns CliNotFound
        match result {
            Ok(path) => assert!(path.is_absolute()),
            Err(ClawError::CliNotFound) => {} // Expected if not in PATH
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_common_locations_returns_paths() {
        // Verify common_locations returns non-empty list
        let locations = CliDiscovery::common_locations();
        assert!(!locations.is_empty());

        // Verify all paths are absolute
        for location in locations {
            assert!(location.is_absolute());
        }
    }

    #[tokio::test]
    async fn test_validate_version_with_valid_cli() {
        // Only run if claude is actually installed
        if let Ok(cli_path) = CliDiscovery::find(None).await {
            let result = CliDiscovery::validate_version(&cli_path).await;

            match result {
                Ok(version) => {
                    // Verify version string is valid semver format
                    assert!(semver::Version::parse(&version).is_ok());

                    // Verify version >= 2.0.0 (since validation passed)
                    let ver = semver::Version::parse(&version).unwrap();
                    assert!(ver >= semver::Version::new(2, 0, 0));
                }
                Err(e) => {
                    // If validation fails, it should be InvalidCliVersion
                    assert!(matches!(e, ClawError::InvalidCliVersion { .. }));
                }
            }
        }
    }
}
