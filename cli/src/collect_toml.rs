//! Utilities for collecting `.toml` file paths from a given directory or file.
//!
//! This module provides functions to gather all `.toml` files from a directory, or return a single file path if a file is provided.
//! It is used by the CLI to discover scenario or configuration files for further processing.
//!
//! # Example
//!
//! ```rust
//! use crate::collect_toml_paths;
//! let toml_files = collect_toml_paths("./scenarios").unwrap();
//! ```

use std::path::Path;

use crate::errors::{CLIError, CLIResult};

/// Checks if the given path exists.
///
/// # Errors
/// Returns a `CLIError::FileNotFound` if the path does not exist.
fn check_file_exists<P: AsRef<Path>>(path: P) -> CLIResult<()> {
    if !path.as_ref().exists() {
        Err(CLIError::FileNotFound(format!(
            "Path does not exist: {}",
            path.as_ref().display()
        )))?
    }
    Ok(())
}

/// Collect all `.toml` files if path is a directory, or just return the file path if it's a file.
///
/// # Arguments
/// * `path` - A path to a directory or file.
///
/// # Returns
/// * `Ok(Vec<PathBuf>)` - All `.toml` files in the directory, or the file itself if a file is given.
/// * `Err(CLIError)` - If the path does not exist or directory reading fails.
///
/// # Example
/// ```
/// let files = collect_toml_paths("./scenarios").unwrap();
/// ```
pub fn collect_toml_paths<P: AsRef<Path>>(path: P) -> CLIResult<Vec<std::path::PathBuf>> {
    let path = path.as_ref();
    check_file_exists(path)?;

    if path.is_dir() {
        let mut toml_files = Vec::new();
        let dir = std::fs::read_dir(path).map_err(|e| {
            CLIError::Other(format!(
                "Failed to read directory {}: {}",
                path.display(),
                e
            ))
        })?;

        for entry in dir {
            let entry = entry.map_err(|e| {
                CLIError::TomlRead(format!(
                    "Failed to read entry in directory {}: {}",
                    path.display(),
                    e
                ))
            })?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                toml_files.push(path);
            }
        }
        Ok(toml_files)
    } else {
        Ok(vec![path.to_path_buf()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    /// Test collecting `.toml` files from a directory with mixed file types.
    #[test]
    fn test_collect_toml_paths_with_directory() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("test1.toml");
        let file2 = dir.path().join("test2.toml");
        let file3 = dir.path().join("not_a_toml.txt");
        File::create(&file1).unwrap();
        File::create(&file2).unwrap();
        File::create(&file3).unwrap();

        let mut result = collect_toml_paths(dir.path()).unwrap();
        result.sort();
        let mut expected = vec![file1, file2];
        expected.sort();
        assert_eq!(result, expected);
    }

    /// Test collecting a single `.toml` file by path.
    #[test]
    fn test_collect_toml_paths_with_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("single.toml");
        File::create(&file).unwrap();
        let result = collect_toml_paths(&file).unwrap();
        assert_eq!(result, vec![file]);
    }

    /// Test error handling when the path does not exist.
    #[test]
    fn test_collect_toml_paths_with_nonexistent_path() {
        let path = std::path::PathBuf::from("/nonexistent/path/to/file.toml");
        let result = collect_toml_paths(&path);
        println!("RESULT {:?}", result);
        assert!(result.is_err());
    }
}
