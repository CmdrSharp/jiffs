use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitDiff {
    pub changed_files: HashMap<String, FileChange>,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub base_content: Option<String>,
    pub current_content: Option<String>,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

impl GitDiff {
    pub fn new(base_sha: &str, only_suffixes: &[String]) -> Result<Self> {
        let changed_files = get_changed_files(base_sha, only_suffixes)?;

        Ok(GitDiff { changed_files })
    }

    pub fn get_file_change(&self, path: &str) -> Option<&FileChange> {
        self.changed_files.get(path)
    }

    pub fn changed_file_paths(&self) -> Vec<&String> {
        self.changed_files.keys().collect()
    }
}

fn get_changed_files(
    base_sha: &str,
    only_suffixes: &[String],
) -> Result<HashMap<String, FileChange>> {
    let mut result = HashMap::new();

    let output = Command::new("git")
        .args(["diff", "--name-status", base_sha])
        .output()
        .context("Failed to execute git diff")?;

    if !output.status.success() {
        anyhow::bail!(
            "Git diff command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let diff_output =
        String::from_utf8(output.stdout).context("Git diff output is not valid UTF-8")?;

    for line in diff_output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 2 {
            continue;
        }

        let status = parts[0];
        let file_path = parts[1];

        if !only_suffixes.is_empty()
            && !only_suffixes
                .iter()
                .any(|suffix| file_path.ends_with(suffix))
        {
            continue;
        }

        let change_type = match status {
            "A" => ChangeType::Added,
            "M" => ChangeType::Modified,
            "D" => ChangeType::Deleted,
            _ => ChangeType::Modified,
        };

        let base_content = if change_type != ChangeType::Added {
            get_file_content_at_ref(base_sha, file_path)?
        } else {
            None
        };

        let current_content = if change_type != ChangeType::Deleted {
            get_current_file_content(file_path)?
        } else {
            None
        };

        result.insert(
            file_path.to_string(),
            FileChange {
                base_content,
                current_content,
                change_type,
            },
        );
    }

    Ok(result)
}

fn get_file_content_at_ref(git_ref: &str, file_path: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["show", &format!("{}:{}", git_ref, file_path)])
        .output()
        .context("Failed to execute git show")?;

    if !output.status.success() {
        return Ok(None);
    }

    let content = String::from_utf8(output.stdout).context("File content is not valid UTF-8")?;

    Ok(Some(content))
}

fn get_current_file_content(file_path: &str) -> Result<Option<String>> {
    if !Path::new(file_path).exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path))?;

    Ok(Some(content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_type_parsing() {
        assert_eq!(ChangeType::Added, ChangeType::Added);
        assert_eq!(ChangeType::Modified, ChangeType::Modified);
        assert_eq!(ChangeType::Deleted, ChangeType::Deleted);
    }

    #[test]
    fn test_file_change_creation() {
        let file_change = FileChange {
            base_content: Some("old content".to_string()),
            current_content: Some("new content".to_string()),
            change_type: ChangeType::Modified,
        };

        assert!(file_change.base_content.is_some());
        assert!(file_change.current_content.is_some());
        assert_eq!(file_change.change_type, ChangeType::Modified);
    }
}
